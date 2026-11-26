//! Interaural time difference node.

use bevy_ecs::component::Component;
use bevy_math::Vec3;
use delay_line::DelayLine;
use firewheel::{
    channel_config::{ChannelConfig, NonZeroChannelCount},
    diff::{Diff, Patch},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, NodeError, ProcBuffers, ProcExtra, ProcInfo,
        ProcStreamCtx, ProcessStatus,
    },
};

mod delay_line;

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

    /// The speed of sound in world units per second.
    ///
    /// This is synchronized to the global [`SpeedOfSound`] resource by
    /// `bevy_seedling`'s spatial systems.
    ///
    /// Defaults to `343.0`.
    ///
    /// [`SpeedOfSound`]: crate::spatial::SpeedOfSound
    pub speed_of_sound: f32,

    /// The input configuration.
    ///
    /// Defaults to [`InputConfig::Stereo`].
    pub input_config: InputConfig,
}

impl Default for ItdConfig {
    fn default() -> Self {
        Self {
            inter_ear_distance: 0.22,
            speed_of_sound: 343.0,
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
    speed_of_sound: f32,
    input_config: InputConfig,
}

impl AudioNode for ItdNode {
    type Configuration = ItdConfig;

    fn info(&self, config: &Self::Configuration) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("itd node")
            .channel_config(ChannelConfig::new(
                config.input_config.input_channels().get(),
                2,
            )))
    }

    fn construct_processor(
        &self,
        configuration: &Self::Configuration,
        cx: firewheel::node::ConstructProcessorContext,
    ) -> Result<impl firewheel::node::AudioNodeProcessor, NodeError> {
        let maximum_samples = maximum_samples(
            configuration.inter_ear_distance,
            configuration.speed_of_sound,
            cx.stream_info.sample_rate.get() as f32,
        );

        Ok(ItdProcessor {
            left: DelayLine::new(maximum_samples),
            right: DelayLine::new(maximum_samples),
            inter_ear_distance: configuration.inter_ear_distance,
            speed_of_sound: configuration.speed_of_sound,
            input_config: configuration.input_config,
        })
    }
}

/// The maximum difference in samples between each ear.
fn maximum_samples(distance: f32, speed_of_sound: f32, sample_rate: f32) -> usize {
    let maximum_delay = distance / speed_of_sound.max(0.01);
    (sample_rate * maximum_delay).ceil() as usize
}

impl AudioNodeProcessor for ItdProcessor {
    fn events(&mut self, _info: &ProcInfo, events: &mut ProcEvents, _extra: &mut ProcExtra) {
        for patch in events.drain_patches::<ItdNode>() {
            let ItdNodePatch::Direction(direction) = patch;
            let direction = direction.normalize_or_zero();

            if direction.length_squared() == 0.0 {
                self.left.set_read_head(0.0);
                self.right.set_read_head(0.0);
                continue;
            }

            self.left.set_read_head(Vec3::X.dot(direction));
            self.right.set_read_head(Vec3::NEG_X.dot(direction));
        }
    }

    fn process(
        &mut self,
        proc_info: &ProcInfo,
        ProcBuffers { inputs, outputs }: ProcBuffers,
        // events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
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

        ProcessStatus::OutputsModified
    }

    fn new_stream(&mut self, stream_info: &firewheel::StreamInfo, _: &mut ProcStreamCtx) {
        if stream_info.sample_rate != stream_info.prev_sample_rate {
            let new_size = maximum_samples(
                self.inter_ear_distance,
                self.speed_of_sound,
                stream_info.sample_rate.get() as f32,
            );

            self.left.resize(new_size);
            self.right.resize(new_size);
        }
    }
}
