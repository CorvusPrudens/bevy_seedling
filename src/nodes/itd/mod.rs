//! Interaural time difference node.

use bevy_ecs::component::Component;
use bevy_math::Vec3;
use delay_line::DelayLine;
use firewheel::{
    channel_config::{ChannelConfig, NonZeroChannelCount},
    diff::{Diff, Patch},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ProcBuffers, ProcExtra, ProcInfo,
        ProcessStatus,
    },
};

mod delay_line;

/// The speed of sound in air, 20 degrees C, at sea level, in meters per second.
const SPEED_OF_SOUND: f32 = 343.0;

/// Interaural time difference node.
///
/// This node simulates the time difference of sounds
/// arriving at each ear, which is on the order of half
/// a millisecond. Since this time difference is
/// one mechanism we use to localize sounds, this node
/// can help build more convincing spatialized audio.
///
/// Note that stereo sounds are converted to mono before applying
/// the spatialization, so some sounds may appear to be "compacted"
/// by the transformation.
#[derive(Debug, Default, Clone, Component, Diff, Patch)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct ItdNode {
    /// The direction vector pointing from the listener to the
    /// emitter.
    pub direction: Vec3,
}

/// Configuration for [`ItdNode`].
#[derive(Debug, Clone, Component, PartialEq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct ItdConfig {
    /// The inter-ear distance in meters.
    ///
    /// This will affect the maximum latency,
    /// though for the normal distribution of head
    /// sizes, it will remain under a millisecond.
    ///
    /// Defaults to `0.22` (22 cm).
    pub inter_ear_distance: f32,

    /// The input configuration.
    ///
    /// Defaults to [`InputConfig::Stereo`].
    pub input_config: InputConfig,
}

impl Default for ItdConfig {
    fn default() -> Self {
        Self {
            inter_ear_distance: 0.22,
            input_config: InputConfig::Stereo,
        }
    }
}

/// The input configuration.
///
/// Defaults to [`NonZeroChannelCount::STEREO`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub enum InputConfig {
    /// Delay the left and right channels without downmixing.
    ///
    /// This is useful for composing spatial effects.
    Stereo,
    /// Downmix the signal to mono, then delay the left and right channels.
    Downmixed(NonZeroChannelCount),
}

impl InputConfig {
    /// Get the number of input channels.
    pub fn input_channels(&self) -> NonZeroChannelCount {
        match self {
            Self::Stereo => NonZeroChannelCount::STEREO,
            Self::Downmixed(c) => *c,
        }
    }
}

struct ItdProcessor {
    left: DelayLine,
    right: DelayLine,
    inter_ear_distance: f32,
    input_config: InputConfig,
}

impl AudioNode for ItdNode {
    type Configuration = ItdConfig;

    fn info(&self, config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("itd node")
            .channel_config(ChannelConfig::new(
                config.input_config.input_channels().get(),
                2,
            ))
    }

    fn construct_processor(
        &self,
        configuration: &Self::Configuration,
        cx: firewheel::node::ConstructProcessorContext,
    ) -> impl firewheel::node::AudioNodeProcessor {
        let maximum_samples = maximum_samples(
            configuration.inter_ear_distance,
            cx.stream_info.sample_rate.get() as f32,
        );

        ItdProcessor {
            left: DelayLine::new(maximum_samples),
            right: DelayLine::new(maximum_samples),
            inter_ear_distance: configuration.inter_ear_distance,
            input_config: configuration.input_config,
        }
    }
}

/// The maximum difference in samples between each ear.
fn maximum_samples(distance: f32, sample_rate: f32) -> usize {
    let maximum_delay = distance / SPEED_OF_SOUND;
    (sample_rate * maximum_delay).ceil() as usize
}

impl AudioNodeProcessor for ItdProcessor {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        ProcBuffers { inputs, outputs }: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        for patch in events.drain_patches::<ItdNode>() {
            let ItdNodePatch::Direction(direction) = patch;
            let direction = direction.normalize_or_zero();

            if direction.length_squared() == 0.0 {
                self.left.read_head = 0.0;
                self.right.read_head = 0.0;
                continue;
            }

            let left_delay =
                Vec3::X.dot(direction).max(0.0) * self.left.len().saturating_sub(1) as f32;
            let right_delay =
                Vec3::NEG_X.dot(direction).max(0.0) * self.right.len().saturating_sub(1) as f32;

            self.left.read_head = left_delay;
            self.right.read_head = right_delay;
        }

        if proc_info.in_silence_mask.all_channels_silent(2) {
            return ProcessStatus::ClearAllOutputs;
        }

        match self.input_config {
            InputConfig::Stereo => {
                // Remove bounds checks inside loop
                let in_left = &inputs[0][..proc_info.frames];
                let in_right = &inputs[1][..proc_info.frames];

                let (out_left, rest) = outputs.split_first_mut().unwrap();

                let out_left = &mut out_left[..proc_info.frames];
                let out_right = &mut rest[0][..proc_info.frames];

                for frame in 0..proc_info.frames {
                    self.left.write(in_left[frame]);
                    self.right.write(in_right[frame]);

                    out_left[frame] = self.left.read();
                    out_right[frame] = self.right.read();
                }
            }
            InputConfig::Downmixed(_) => {
                for frame in 0..proc_info.frames {
                    let mut downmixed = 0.0;
                    for channel in inputs {
                        downmixed += channel[frame];
                    }
                    downmixed /= inputs.len() as f32;

                    self.left.write(downmixed);
                    self.right.write(downmixed);

                    outputs[0][frame] = self.left.read();
                    outputs[1][frame] = self.right.read();
                }
            }
        }

        ProcessStatus::outputs_not_silent()
    }

    fn new_stream(&mut self, stream_info: &firewheel::StreamInfo) {
        if stream_info.sample_rate != stream_info.prev_sample_rate {
            let new_size = maximum_samples(
                self.inter_ear_distance,
                stream_info.sample_rate.get() as f32,
            );

            self.left.resize(new_size);
            self.right.resize(new_size);
        }
    }
}
