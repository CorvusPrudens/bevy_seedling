use firewheel::{
    backend::BackendProcessInfo, collector::ArcGc, node::StreamStatus,
    processor::FirewheelProcessor,
};
use js_sys::{Array, Float32Array};
use std::sync::atomic::AtomicBool;
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};

use crate::{WebAudioBackend, instant::Instant};

#[wasm_bindgen]
pub(crate) struct ProcessorHost {
    pub(crate) processor: Option<FirewheelProcessor<WebAudioBackend>>,
    pub(crate) receiver: std::sync::mpsc::Receiver<FirewheelProcessor<WebAudioBackend>>,
    pub(crate) alive: ArcGc<AtomicBool>,
    pub(crate) inputs: usize,
    pub(crate) outputs: usize,
    pub(crate) input_buffers: &'static mut [f32],
    pub(crate) output_buffers: &'static mut [f32],
}

impl ProcessorHost {
    fn process_fallible(
        &mut self,
        inputs: js_sys::Array,
        outputs: js_sys::Array,
        current_time: f64,
    ) -> Result<bool, wasm_bindgen::JsValue> {
        if !self.alive.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(false);
        }

        if let Ok(processor) = self.receiver.try_recv() {
            self.processor = Some(processor);
        }

        // interleave
        let mut temp_buffer = [0f32; crate::BLOCK_FRAMES];
        for i in 0..self.inputs {
            let Ok(inputs) = inputs.get(0).dyn_into::<Array>() else {
                continue;
            };
            let Ok(channel_array) = inputs.get(i as u32).dyn_into::<Float32Array>() else {
                continue;
            };

            if channel_array.is_undefined() {
                return Err(wasm_bindgen::JsValue::undefined());
            }

            channel_array.copy_to(&mut temp_buffer);
            for (j, input) in temp_buffer.iter().enumerate() {
                let buffer_index = i + j * self.inputs;
                self.input_buffers[buffer_index] = *input;
            }
        }

        if let Some(processor) = &mut self.processor {
            processor.process_interleaved(
                self.input_buffers,
                self.output_buffers,
                BackendProcessInfo {
                    num_in_channels: self.inputs,
                    num_out_channels: self.outputs,
                    frames: crate::BLOCK_FRAMES,
                    process_timestamp: Instant(current_time),
                    duration_since_stream_start: std::time::Duration::from_secs_f64(current_time),
                    input_stream_status: StreamStatus::empty(),
                    output_stream_status: StreamStatus::empty(),
                    dropped_frames: 0,
                },
            );
        }

        // deinterleave
        for i in 0..self.outputs {
            let Ok(outputs) = outputs.get(0).dyn_into::<Array>() else {
                continue;
            };
            let Ok(channel_array) = outputs.get(i as u32).dyn_into::<Float32Array>() else {
                continue;
            };

            if channel_array.is_undefined() {
                return Err(wasm_bindgen::JsValue::undefined());
            }

            for (j, output) in temp_buffer.iter_mut().enumerate() {
                let buffer_index = i + j * self.outputs;
                *output = self.output_buffers[buffer_index];
            }

            channel_array.copy_from(&temp_buffer);
        }

        Ok(true)
    }
}

#[wasm_bindgen]
#[allow(dead_code)]
impl ProcessorHost {
    /// Pack the object to send through the web audio worklet constructor
    pub fn pack(self) -> usize {
        Box::into_raw(Box::new(self)) as usize
    }

    /// Unpack the object from the worklet constructor
    /// # Safety
    /// This should only be called within the worklet constructor from a known
    /// good pointer
    pub unsafe fn unpack(ptr: usize) -> Self {
        unsafe { *Box::from_raw(ptr as *mut Self) }
    }

    pub fn process(
        &mut self,
        inputs: js_sys::Array,
        outputs: js_sys::Array,
        current_time: f64,
    ) -> bool {
        // since we're in the audio context, it's difficult to
        // do anything but ignore the error
        self.process_fallible(inputs, outputs, current_time)
            .unwrap_or(true)
    }
}
