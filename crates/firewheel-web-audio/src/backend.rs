use crate::{
    auto_resume::setup_autoresume, error::JsContext, instant::Instant,
    wasm_processor::ProcessorHost,
};
use firewheel::{
    StreamInfo,
    backend::{AudioBackend, DeviceInfoSimple},
    collector::ArcGc,
    processor::FirewheelProcessor,
};
use std::{
    cell::RefCell,
    num::NonZeroU32,
    rc::Rc,
    sync::{atomic::AtomicBool, mpsc},
    time::Duration,
};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use web_sys::{AudioContext, AudioContextOptions, AudioWorkletNode};

/// The main-thread host for the Web Audio API backend.
///
/// This backend relies on Wasm multi-threading. The Firewheel
/// audio nodes are processed within a Web Audio `AudioWorkletNode`
/// that shares its memory with the initializing Wasm module.
///
/// When the audio context is created, `firewheel-web-audio` will begin listening for
/// a number of user input events that will permit the context to be resumed. If
/// [`WebAudioConfig::request_input`] is `true`, it will also prompt the user for
/// input and connect the input in the Web Audio graph.
///
/// When dropped, the underlying `AudioContext` is closed and all
/// resources are released.
pub struct WebAudioBackend {
    processor: mpsc::Sender<FirewheelProcessor<Self>>,
    is_dropped: Rc<AtomicBool>,
    alive: ArcGc<AtomicBool>,
    web_context: AudioContext,
    processor_node: Rc<RefCell<Option<AudioWorkletNode>>>,
}

impl Drop for WebAudioBackend {
    fn drop(&mut self) {
        self.alive
            .store(false, std::sync::atomic::Ordering::Relaxed);

        if let Some(node) = self.processor_node.borrow().as_ref() {
            if let Err(e) = node.disconnect() {
                log::error!("Failed to disconnect `AudioWorkletNode`: {e:?}");
            }
        }

        if let Err(e) = self.web_context.close() {
            log::error!("Failed to close `AudioContext`: {e:?}");
        }
    }
}

impl core::fmt::Debug for WebAudioBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmBackend")
            .field("is_dropped", &self.is_dropped)
            .field("alive", &self.alive)
            .field("web_context", &self.web_context)
            .finish_non_exhaustive()
    }
}

/// Errors related to initializing the web audio stream.
#[derive(Debug)]
pub enum WebAudioStartError {
    /// An error occurred during Web Audio context initialization.
    Initialization(String),
    /// An error occurred when constructing the `AudioWorkletNode`.
    WorkletCreation(String),
}

impl core::fmt::Display for WebAudioStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initialization(e) => {
                write!(f, "Failed to initialize Web Audio API object: {e}")
            }
            Self::WorkletCreation(e) => {
                write!(f, "Failed to create the backend audio worklet: {e}")
            }
        }
    }
}

impl std::error::Error for WebAudioStartError {}

/// Errors encountered while the web audio stream is running.
#[derive(Debug)]
pub enum WebAudioStreamError {
    /// The `AudioWorkletNode` was unexpectedly dropped.
    UnexpectedDrop,
}

impl core::fmt::Display for WebAudioStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedDrop => {
                write!(f, "The `AudioWorkletNode` was unexpectedly dropped")
            }
        }
    }
}

impl std::error::Error for WebAudioStreamError {}

/// The Web Audio backend's configuration.
#[derive(Debug, Default, Clone)]
pub struct WebAudioConfig {
    /// The desired sample rate.
    ///
    /// If no sample rate is requested, it will be selected automatically
    /// by the Web Audio API.
    pub sample_rate: Option<NonZeroU32>,

    /// Ask the browser to request an input device,
    /// allowing the user to supply a microphone or other input.
    ///
    /// When set to `true`, the
    /// [`FirewheelConfig::num_graph_inputs`][firewheel::FirewheelConfig::num_graph_inputs]
    /// field must be set to [`ChannelCount::STEREO`][firewheel::core::channel_config::ChannelCount::STEREO].
    ///
    /// If input is not requested, the Firewheel graph inputs will be silent.
    pub request_input: bool,
}

/// Manual javascript bindings to access the audio context's timing information.
///
/// https://developer.mozilla.org/en-US/docs/Web/API/AudioContext/getOutputTimestamp
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = AudioTimestamp)]
    pub type AudioTimestamp;

    #[wasm_bindgen(method, getter, js_name = contextTime)]
    pub fn context_time(this: &AudioTimestamp) -> f64;

    #[wasm_bindgen(method, getter, js_name = performanceTime)]
    pub fn performance_time(this: &AudioTimestamp) -> f64;
}

#[wasm_bindgen]
extern "C" {
    type AudioContextExt;

    #[wasm_bindgen(method, js_name = getOutputTimestamp)]
    fn get_output_timestamp(this: &AudioContextExt) -> AudioTimestamp;
}

fn get_output_timestamp(ctx: &AudioContext) -> AudioTimestamp {
    let ext: AudioContextExt = ctx.clone().unchecked_into();
    ext.get_output_timestamp()
}

impl AudioBackend for WebAudioBackend {
    type Enumerator = ();
    type Instant = Instant;
    type Config = WebAudioConfig;
    type StartStreamError = WebAudioStartError;
    type StreamError = WebAudioStreamError;

    fn delay_from_last_process(&self, _: Self::Instant) -> Option<Duration> {
        let timestamp = get_output_timestamp(&self.web_context);
        let performance_time = timestamp.performance_time();
        let now = web_sys::window()?.performance()?.now();

        Some(Duration::from_secs_f64(
            (now - performance_time).max(0.0) / 1000.0,
        ))
    }

    fn enumerator() -> Self::Enumerator {}

    fn input_devices_simple(&mut self) -> Vec<firewheel::backend::DeviceInfoSimple> {
        vec![DeviceInfoSimple {
            name: "default input".into(),
            id: "default input".into(),
        }]
    }

    fn output_devices_simple(&mut self) -> Vec<DeviceInfoSimple> {
        vec![DeviceInfoSimple {
            name: "default input".into(),
            id: "default input".into(),
        }]
    }

    fn start_stream(config: Self::Config) -> Result<(Self, StreamInfo), Self::StartStreamError> {
        let (sender, receiver) = mpsc::channel();

        let context = match config.sample_rate {
            Some(sample_rate) => {
                let options = AudioContextOptions::new();
                options.set_sample_rate(sample_rate.get() as f32);
                web_sys::AudioContext::new_with_context_options(&options)
                    .map_err(|e| WebAudioStartError::Initialization(format!("{e:?}")))?
            }
            None => web_sys::AudioContext::new()
                .map_err(|e| WebAudioStartError::Initialization(format!("{e:?}")))?,
        };

        let _ = context.suspend();

        let sample_rate = context.sample_rate();
        let inputs = if config.request_input { 2 } else { 0 };
        let outputs = 2;

        let alive = ArcGc::new(AtomicBool::new(true));
        let processor_node = Rc::new(RefCell::new(None));
        let is_dropped = Rc::new(AtomicBool::new(false));

        wasm_bindgen_futures::spawn_local({
            let context = context.clone();
            let processor_node = processor_node.clone();
            let alive = alive.clone();
            let is_dropped = is_dropped.clone();
            async move {
                let result = prepare_context(
                    context.clone(),
                    inputs,
                    outputs,
                    receiver,
                    alive,
                    is_dropped,
                    processor_node,
                )
                .await;

                match result {
                    Ok(firewheel_worklet) if inputs > 0 => {
                        let result = crate::auto_resume::setup_autoresume(
                            context.clone(),
                            move || {
                                // Request microphone access
                                let window = web_sys::window().expect("Window should be available");
                                let navigator = window.navigator();
                                let media_devices = navigator
                                    .media_devices()
                                    .expect("`mediaDevices` should be available");

                                let constraints = web_sys::MediaStreamConstraints::new();
                                constraints.set_audio(&JsValue::TRUE);

                                let get_user_media_promise = media_devices
                                    .get_user_media_with_constraints(&constraints)
                                    .expect("Failed to call getUserMedia");

                                let context = context.clone();
                                let firewheel_worklet = firewheel_worklet.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    let future = wasm_bindgen_futures::JsFuture::from(
                                        get_user_media_promise,
                                    );
                                    match future.await {
                                        Ok(media_stream_jsvalue) => {
                                            let media_stream: web_sys::MediaStream =
                                                media_stream_jsvalue
                                                    .dyn_into()
                                                    .expect("Failed to cast to MediaStream");

                                            // Create MediaStreamAudioSourceNode
                                            let options =
                                                web_sys::MediaStreamAudioSourceOptions::new(
                                                    &media_stream,
                                                );
                                            let audio_source_node =
                                                web_sys::MediaStreamAudioSourceNode::new(
                                                    &context, &options,
                                                )
                                                .expect(
                                                    "Failed to create MediaStreamAudioSourceNode",
                                                );

                                            if let Err(e) = audio_source_node
                                                .connect_with_audio_node(&firewheel_worklet)
                                            {
                                                log::error!(
                                                    "Failed to connect media stream to Firewheel worklet: {e:?}"
                                                );
                                            }
                                        }
                                        Err(err) => {
                                            // Handle the error (e.g., user denied microphone access)
                                            log::error!("Failed to acquire audio input: {err:?}");
                                        }
                                    }
                                });
                            },
                        );

                        if let Err(e) = result {
                            log::error!("Failed to set up autoresume: {e:?}");
                        };
                    }
                    Ok(_) => {
                        if let Err(e) = setup_autoresume(context.clone(), || ()) {
                            log::error!("Failed to set up autoresume: {e:?}");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to initialize Web Audio backend: {e:?}");
                        log::warn!(
                            "Audio initialization failed. \
                            Ensure the document is served with appropriate cross origin isolation headers \
                            (https://developer.mozilla.org/en-US/docs/Web/API/Window/crossOriginIsolated) \
                            and compile your wasm with the `+atomics` target feature."
                        );
                    }
                }
            }
        });

        Ok((
            Self {
                web_context: context,
                is_dropped,
                processor: sender,
                processor_node,
                alive,
            },
            StreamInfo {
                sample_rate: NonZeroU32::new(sample_rate as u32)
                    .expect("Web Audio API sample rate should be non-zero"),
                max_block_frames: NonZeroU32::new(crate::BLOCK_FRAMES as u32).unwrap(),
                num_stream_in_channels: inputs as u32,
                num_stream_out_channels: outputs as u32,
                input_device_id: Some("default input".into()),
                output_device_id: "default output".into(),
                ..Default::default()
            },
        ))
    }

    fn set_processor(&mut self, processor: FirewheelProcessor<Self>) {
        if self.processor.send(processor).is_err() {
            self.is_dropped
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn poll_status(&mut self) -> Result<(), Self::StreamError> {
        if self.is_dropped.load(std::sync::atomic::Ordering::Relaxed) {
            Err(WebAudioStreamError::UnexpectedDrop)
        } else {
            Ok(())
        }
    }
}

/// Since it's a reasonable expectation that creating contexts
/// will be infrequent and the buffer sizes small, leaking the
/// buffers is totally fine.
fn create_buffer(len: usize) -> &'static mut [f32] {
    let mut vec = Vec::new();
    vec.reserve_exact(len);
    vec.extend(std::iter::repeat_n(0f32, len));
    Vec::leak(vec)
}

async fn prepare_context(
    context: AudioContext,
    inputs: usize,
    outputs: usize,
    receiver: mpsc::Receiver<FirewheelProcessor<WebAudioBackend>>,
    alive: ArcGc<AtomicBool>,
    is_dropeed: Rc<AtomicBool>,
    processor_node: Rc<RefCell<Option<AudioWorkletNode>>>,
) -> Result<AudioWorkletNode, String> {
    let mod_url = crate::dynamic_module::dependent_module!("./js/audio-worklet.js")
        .context("loading dynamic context")?;

    wasm_bindgen_futures::JsFuture::from(
        context
            .audio_worklet()
            .context("fetching audio worklet")?
            .add_module(mod_url.trim_start_matches('.'))
            .context("creating audio worklet module")?,
    )
    .await
    .context("creating audio worklet module")?;

    let wrapper = ProcessorHost {
        processor: None,
        receiver,
        alive,
        inputs,
        input_buffers: create_buffer(inputs * crate::BLOCK_FRAMES),
        outputs,
        output_buffers: create_buffer(outputs * crate::BLOCK_FRAMES),
    };
    let wrapper = wrapper.pack();

    let node = web_sys::AudioWorkletNode::new_with_options(&context, "WasmProcessor", &{
        let options = web_sys::AudioWorkletNodeOptions::new();

        let output_channels = js_sys::Array::of1(&outputs.into());
        options.set_output_channel_count(&output_channels);

        options.set_number_of_inputs(if inputs > 0 { 1 } else { 0 });
        options.set_number_of_outputs(1);
        options.set_channel_count(2);

        options.set_processor_options(Some(&js_sys::Array::of3(
            &wasm_bindgen::module(),
            &wasm_bindgen::memory(),
            &wrapper.into(),
        )));
        options
    })
    .context("creating audio worklet instance")?;

    let closure = wasm_bindgen::prelude::Closure::<dyn Fn(web_sys::ErrorEvent)>::new(
        move |data: web_sys::ErrorEvent| {
            let message = data.message();
            is_dropeed.store(true, std::sync::atomic::Ordering::Relaxed);
            log::error!("encountered error in Firewheel `AudioWorkletNode`: {message}");
        },
    );
    node.set_onprocessorerror(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    node.connect_with_audio_node(&context.destination())
        .context("connecting audio worklet to destination")?;

    *processor_node.borrow_mut() = Some(node.clone());

    Ok(node)
}
