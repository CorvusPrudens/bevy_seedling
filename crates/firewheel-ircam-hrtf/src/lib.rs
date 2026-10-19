//! A head-related transfer function (HRTF) node for
//! [Firewheel](https://github.com/BillyDM/Firewheel),
//! powered by [Fyrox](https://docs.rs/hrtf/latest/hrtf/)'s
//! [IRCAM](http://recherche.ircam.fr/equipes/salles/listen/download.html)-based HRIR.
//!
//! HRTFs can provide far more convincing spatialization compared to
//! simpler techniques. They simulate the way our bodies filter sounds
//! based on where they're coming from, allowing you to distinguish up/down,
//! front/back, and the more typical left/right.
//!
//! This simulation is moderately expensive. You'll generally want to avoid more
//! than 32-64 HRTF emitters, especially on less powerful devices.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

use firewheel::{
    channel_config::{ChannelConfig, NonZeroChannelCount},
    diff::{Diff, Patch},
    dsp::{coeff_update::CoeffUpdateFactor, distance_attenuation::DistanceAttenuatorStereoDsp},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, NodeError, ProcBuffers, ProcExtra, ProcInfo,
        ProcessStatus,
    },
};
use glam::Vec3;
use hrtf::{HrirSphere, HrtfContext, HrtfProcessor};
use std::io::Cursor;

mod subjects;

pub use firewheel::dsp::distance_attenuation::{DistanceAttenuation, DistanceModel};
pub use subjects::{Subject, SubjectBytes};

/// Head-related transfer function (HRTF) node.
///
/// HRTFs can provide far more convincing spatialization
/// compared to simpler techniques. They simulate the way
/// our bodies filter sounds based on where they’re coming from,
/// allowing you to distinguish up/down, front/back,
/// and the more typical left/right.
///
/// This simulation is moderately expensive. You’ll generally
/// want to avoid more than 32-64 HRTF emitters, especially on
/// less powerful devices.
#[derive(Debug, Clone, Diff, Patch)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
pub struct HrtfNode {
    /// The positional offset from the listener to the emitter.
    pub offset: Vec3,

    /// The amount of muffling (lowpass) in the range `[20.0, 20_480.0]`,
    /// where `20_480.0` is no muffling and `20.0` is maximum muffling.
    ///
    /// This can be used to give the effect of a sound being played behind a wall
    /// or underwater.
    ///
    /// By default this is set to `20_480.0`.
    ///
    /// See <https://www.desmos.com/calculator/jxp8t9ero4> for an interactive graph of
    /// how these parameters affect the final lowpass cuttoff frequency.
    pub muffle_cutoff_hz: f32,

    /// Distance attenuation parameters.
    pub distance_attenuation: DistanceAttenuation,

    /// The time in seconds of the internal smoothing filter.
    ///
    /// By default this is set to `0.015` (15ms).
    pub smooth_seconds: f32,

    /// If the resutling gain (in raw amplitude, not decibels) is less than or equal
    /// to this value, the the gain will be clamped to `0` (silence).
    ///
    /// By default this is set to "0.0001" (-80 dB).
    pub min_gain: f32,

    /// An exponent representing the rate at which DSP coefficients are
    /// updated when parameters are being smoothed.
    ///
    /// Smaller values will produce less "stair-stepping" artifacts,
    /// but will also consume more CPU.
    ///
    /// The resulting number of frames (samples in a single channel of audio)
    /// that will elapse between each update is calculated as
    /// `2^coeff_update_factor`.
    ///
    /// By default this is set to `5`.
    pub coeff_update_factor: CoeffUpdateFactor,
}

impl Default for HrtfNode {
    fn default() -> Self {
        Self {
            offset: Vec3::ZERO,
            muffle_cutoff_hz: 20480.0,
            distance_attenuation: Default::default(),
            smooth_seconds: 0.015,
            min_gain: 0.0001,
            coeff_update_factor: CoeffUpdateFactor(5),
        }
    }
}

/// Configuration for [`HrtfNode`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
pub struct HrtfConfig {
    /// The number of input channels.
    ///
    /// The inputs are downmixed to a mono signal
    /// before spatialization is applied.
    ///
    /// Defaults to [`NonZeroChannelCount::STEREO`].
    pub input_channels: NonZeroChannelCount,

    /// The head-related impulse-response sphere.
    ///
    /// The data for this sphere is captured from subjects. Short
    /// "impulses" are played from all angles and recorded at the
    /// ear canal. The resulting recordings capture how sounds are affected
    /// by the subject's torso, head, and ears.
    ///
    /// Defaults to `HrirSource::Embedded(Subject::Irc1040)`.
    pub hrir_sphere: HrirSource,

    /// The size of the FFT processing block, which can be
    /// tuned for performance.
    pub fft_size: FftSize,
}

impl Default for HrtfConfig {
    fn default() -> Self {
        Self {
            input_channels: NonZeroChannelCount::STEREO,
            hrir_sphere: Subject::Irc1040.into(),
            fft_size: FftSize::default(),
        }
    }
}

/// Describes the size of the FFT processing block.
///
/// Generally, you should try to make the audio's processing buffer size
/// a multiple of [`slice_len`][FftSize::slice_len] if possible.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
pub struct FftSize {
    /// The number of slices a change in direction is interpolated across.
    ///
    /// Defaults to 4.
    pub slice_count: usize,

    /// The size of each slice.
    ///
    /// Defaults to 128.
    pub slice_len: usize,
}

impl Default for FftSize {
    fn default() -> Self {
        Self {
            slice_count: 4,
            slice_len: 128,
        }
    }
}

/// Provides a source for the HRIR sphere data.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
pub enum HrirSource {
    /// Load data from the subjects embedded in the binary itself.
    Embedded(Subject),
    /// Load arbitrary data from an in-memory slice.
    InMemory(SubjectBytes),
}

impl HrirSource {
    fn get_sphere(&self, sample_rate: u32) -> Result<HrirSphere, hrtf::HrtfError> {
        match &self {
            HrirSource::Embedded(subject) => HrirSphere::new(Cursor::new(*subject), sample_rate),
            HrirSource::InMemory(subject) => {
                HrirSphere::new(Cursor::new(subject.clone()), sample_rate)
            }
        }
    }
}

impl From<Subject> for HrirSource {
    fn from(value: Subject) -> Self {
        Self::Embedded(value)
    }
}

impl From<SubjectBytes> for HrirSource {
    fn from(value: SubjectBytes) -> Self {
        Self::InMemory(value)
    }
}

/// A wrapper for [`hrtf::HrtfError`] that implements [`std::error::Error`].
#[derive(Debug)]
pub struct HrtfError(pub hrtf::HrtfError);

impl core::fmt::Display for HrtfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            hrtf::HrtfError::InvalidFileFormat => write!(f, "Failed to parse file as HRIR sphere"),
            hrtf::HrtfError::InvalidLength(len) => write!(f, "Invalid HRIR length `{len}`"),
            hrtf::HrtfError::IoError(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for HrtfError {}

impl AudioNode for HrtfNode {
    type Configuration = HrtfConfig;

    fn info(&self, config: &Self::Configuration) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("hrtf node")
            .channel_config(ChannelConfig::new(config.input_channels.get(), 2)))
    }

    fn construct_processor(
        &self,
        config: &Self::Configuration,
        cx: firewheel::node::ConstructProcessorContext,
    ) -> Result<impl firewheel::node::AudioNodeProcessor, NodeError> {
        let sample_rate = cx.stream_info.sample_rate.get();

        let sphere = config
            .hrir_sphere
            .get_sphere(sample_rate)
            .map_err(HrtfError)?;

        let hrir_len = sphere.len();

        let mut processor = FyroxHrtfProcessor {
            renderer: HrtfProcessor::new(sphere, 1, config.fft_size.slice_len),
            offset: self.offset,
            direction_from: self.offset,
            direction_to: self.offset,
            ramp_frames: 0,
            attenuation: self.distance_attenuation,
            attenuation_processor: DistanceAttenuatorStereoDsp::new(
                firewheel::param::smoother::SmootherConfig {
                    smooth_seconds: self.smooth_seconds,
                    ..Default::default()
                },
                cx.stream_info.sample_rate,
                self.coeff_update_factor,
            ),
            muffle_cutoff_hz: self.muffle_cutoff_hz,
            min_gain: self.min_gain,
            slice: Vec::new(),
            rendered: Vec::new(),
            history: Vec::new(),
            prev_left_samples: Vec::new(),
            prev_right_samples: Vec::new(),
            hrir_len,
            silent_frames: 0,
            prev_input_settled: true,
            sphere_source: config.hrir_sphere.clone(),
            fft_size: config.fft_size.clone(),
        };
        processor.allocate();

        Ok(processor)
    }
}

struct FyroxHrtfProcessor {
    renderer: HrtfProcessor,
    offset: Vec3,
    // direction changes are interpolated over `slice_count` slices
    direction_from: Vec3,
    direction_to: Vec3,
    ramp_frames: usize,
    attenuation: DistanceAttenuation,
    attenuation_processor: DistanceAttenuatorStereoDsp,
    muffle_cutoff_hz: f32,
    min_gain: f32,
    slice: Vec<f32>,
    rendered: Vec<(f32, f32)>,
    // the last `hrir_len - 1` input samples, since `hrtf` would otherwise
    // keep the padding of a partial slice as history
    history: Vec<f32>,
    prev_left_samples: Vec<f32>,
    prev_right_samples: Vec<f32>,
    hrir_len: usize,
    silent_frames: usize,
    prev_input_settled: bool,
    sphere_source: HrirSource,
    fft_size: FftSize,
}

impl FyroxHrtfProcessor {
    fn ramp_len(&self) -> usize {
        self.fft_size.slice_count * self.fft_size.slice_len
    }

    // the HRIR has finished ringing out
    fn at_rest(&self) -> bool {
        self.silent_frames >= self.history.len()
    }

    fn allocate(&mut self) {
        let slice_len = self.fft_size.slice_len;
        let history_len = self.hrir_len - 1;

        self.slice = vec![0.0; slice_len];
        self.rendered = vec![(0.0, 0.0); slice_len];
        self.history = vec![0.0; history_len];
        // `hrtf` reallocates these if the length is wrong
        self.prev_left_samples = vec![0.0; history_len];
        self.prev_right_samples = vec![0.0; history_len];

        self.clear();
    }

    // must not allocate, since this runs on the audio thread
    fn clear(&mut self) {
        self.history.fill(0.0);
        self.silent_frames = self.history.len();
        self.prev_input_settled = true;

        self.snap_direction();
        self.attenuation_processor.reset();
    }

    fn snap_direction(&mut self) {
        self.direction_from = self.offset;
        self.direction_to = self.offset;
        self.ramp_frames = self.ramp_len();
    }

    fn direction(&self) -> Vec3 {
        let t = self.ramp_frames as f32 / self.ramp_len() as f32;

        // matches `hrtf`'s own lerp
        self.direction_from + (self.direction_to - self.direction_from) * t
    }

    fn render(&mut self, frames: usize, left: &mut [f32], right: &mut [f32]) {
        if self.direction_to != self.offset {
            self.direction_from = if self.ramp_frames < self.ramp_len() {
                self.direction()
            } else {
                self.direction_to
            };
            self.direction_to = self.offset;
            self.ramp_frames = 0;
        }

        self.ramp_frames = (self.ramp_frames + frames).min(self.ramp_len());
        let direction = self.direction();
        let direction = hrtf::Vec3::new(direction.x, direction.y, direction.z);

        self.slice[frames..].fill(0.0);
        self.rendered.fill((0.0, 0.0));
        self.prev_left_samples.copy_from_slice(&self.history);
        self.prev_right_samples.copy_from_slice(&self.history);

        self.renderer.process_samples(HrtfContext {
            source: &self.slice,
            output: &mut self.rendered,
            new_sample_vector: direction,
            prev_sample_vector: direction,
            prev_left_samples: &mut self.prev_left_samples,
            prev_right_samples: &mut self.prev_right_samples,
            new_distance_gain: 1.0,
            prev_distance_gain: 1.0,
        });

        for ((left, right), &(rendered_left, rendered_right)) in
            left.iter_mut().zip(right.iter_mut()).zip(&self.rendered)
        {
            *left = rendered_left;
            *right = rendered_right;
        }

        let input = &self.slice[..frames];
        let history_len = self.history.len();
        if frames >= history_len {
            self.history.copy_from_slice(&input[frames - history_len..]);
        } else {
            self.history.copy_within(frames.., 0);
            self.history[history_len - frames..].copy_from_slice(input);
        }

        self.silent_frames = match input.iter().rposition(|sample| *sample != 0.0) {
            Some(last) => frames - 1 - last,
            None => self.silent_frames.saturating_add(frames),
        };
    }
}

impl AudioNodeProcessor for FyroxHrtfProcessor {
    fn events(&mut self, info: &ProcInfo, events: &mut ProcEvents, _extra: &mut ProcExtra) {
        let mut updated = false;

        for patch in events.drain_patches::<HrtfNode>() {
            updated = true;

            match patch {
                HrtfNodePatch::Offset(offset) => {
                    let distance = offset.length().max(0.01);

                    self.attenuation_processor.compute_values(
                        distance,
                        &self.attenuation,
                        self.muffle_cutoff_hz,
                        self.min_gain,
                    );

                    self.offset = offset.normalize_or(Vec3::Y);
                }
                HrtfNodePatch::MuffleCutoffHz(muffle) => {
                    self.muffle_cutoff_hz = muffle;
                }
                HrtfNodePatch::DistanceAttenuation(a) => {
                    self.attenuation.apply(a);
                }
                HrtfNodePatch::SmoothSeconds(s) => {
                    self.attenuation_processor
                        .set_smooth_seconds(s, info.sample_rate);
                }
                HrtfNodePatch::MinGain(g) => {
                    self.min_gain = g;
                }
                HrtfNodePatch::CoeffUpdateFactor(c) => {
                    self.attenuation_processor.set_coeff_update_factor(c);
                }
            }
        }

        // snap to the new gain between sounds, like `SpatialBasicNode`
        if updated && self.prev_input_settled {
            self.attenuation_processor.reset();
        }
    }

    fn bypassed(&mut self, _bypassed: bool) {
        self.clear();
    }

    fn process(
        &mut self,
        proc_info: &ProcInfo,
        buffers: ProcBuffers,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        if proc_info
            .in_silence_mask
            .all_channels_silent(buffers.inputs.len())
        {
            self.prev_input_settled = true;
            self.attenuation_processor.reset();

            if self.at_rest() {
                return ProcessStatus::ClearAllOutputs;
            }
        } else {
            self.prev_input_settled = buffers.inputs_settled_at_zero();
        }

        if self.at_rest() {
            self.snap_direction();
        }

        let ProcBuffers { inputs, outputs } = buffers;
        let (left, rest) = outputs.split_first_mut().unwrap();
        let right = &mut *rest[0];

        // render each slice right away, zero-padding a partial one, so the
        // output is never delayed
        let frames = proc_info.frames;
        let slice_len = self.fft_size.slice_len;
        for start in (0..frames).step_by(slice_len) {
            let end = (start + slice_len).min(frames);

            for (frame, sample) in (start..end).zip(self.slice.iter_mut()) {
                *sample =
                    inputs.iter().map(|channel| channel[frame]).sum::<f32>() / inputs.len() as f32;
            }

            self.render(end - start, &mut left[start..end], &mut right[start..end]);
        }

        let clear_outputs =
            self.attenuation_processor
                .process(frames, left, right, proc_info.sample_rate_recip);

        if clear_outputs {
            self.attenuation_processor.reset();
            ProcessStatus::ClearAllOutputs
        } else {
            ProcessStatus::OutputsModified
        }
    }

    fn new_stream(
        &mut self,
        stream_info: &firewheel::StreamInfo,
        _store: &mut firewheel::node::ProcStreamCtx,
    ) {
        if stream_info.prev_sample_rate != stream_info.sample_rate {
            let sample_rate = stream_info.sample_rate.get();

            let sphere = self
                .sphere_source
                .get_sphere(sample_rate)
                .expect("HRIR data should be in a valid format");

            self.hrir_len = sphere.len();
            self.renderer = HrtfProcessor::new(sphere, 1, self.fft_size.slice_len);
            self.attenuation_processor
                .update_sample_rate(stream_info.sample_rate);

            self.allocate();
        } else {
            self.clear();
        }
    }
}
