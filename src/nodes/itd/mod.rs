//! Interaural time difference node.

use bevy_ecs::component::Component;
use bevy_math::Vec3;
use delay_line::DelayLine;
use firewheel::{
    channel_config::{ChannelConfig, NonZeroChannelCount},
    diff::{Diff, Patch},
    node::{AudioNode, AudioNodeInfo, AudioNodeProcessor, ProcBuffers, ProcessStatus},
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
#[derive(Debug, Clone, Component)]
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

    /// The number of input channels.
    ///
    /// The inputs are downmixed to a mono signal
    /// before spatialization is applied.
    ///
    /// Defaults to [`NonZeroChannelCount::STEREO`].
    pub input_channels: NonZeroChannelCount,
}

impl Default for ItdConfig {
    fn default() -> Self {
        Self {
            inter_ear_distance: 0.22,
            input_channels: NonZeroChannelCount::STEREO,
        }
    }
}

struct ItdProcessor {
    left: DelayLine,
    right: DelayLine,
    inter_ear_distance: f32,
}

impl AudioNode for ItdNode {
    type Configuration = ItdConfig;

    fn info(&self, config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("itd node")
            .channel_config(ChannelConfig::new(config.input_channels.get(), 2))
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
        ProcBuffers {
            inputs, outputs, ..
        }: ProcBuffers,
        proc_info: &firewheel::node::ProcInfo,
        events: &mut firewheel::event::NodeEventList,
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
