//! A band-limited sawtooth oscillator.

use bevy_ecs::prelude::*;
use firewheel::{
    clock::ClockSeconds,
    node::{AudioNode, AudioNodeProcessor, EventData, ProcessStatus},
    param::AudioParam,
    param::Timeline,
    ChannelConfig, ChannelCount,
};

/// A band-limited sawtooth oscillator.
#[derive(seedling_macros::AudioParam, Debug, Clone, Component)]
pub struct SawNode {
    /// The frequency in hertz.
    pub frequency: Timeline<f32>,
}

impl SawNode {
    /// Create a new [`SawNode`] with an initial frequency.
    ///
    /// ```
    /// # use bevy_seedling::{*, lpf::SawNode};
    /// # use bevy::prelude::*;
    /// # fn system(mut commands: Commands) {
    /// commands.spawn(SawNode::new(440.0));
    /// # }
    /// ```
    pub fn new(frequency: f32) -> Self {
        Self {
            frequency: Timeline::new(frequency),
        }
    }
}

impl From<SawNode> for Box<dyn AudioNode> {
    fn from(value: SawNode) -> Self {
        Box::new(value)
    }
}

impl AudioNode for SawNode {
    fn debug_name(&self) -> &'static str {
        "low pass filter"
    }

    fn info(&self) -> firewheel::node::AudioNodeInfo {
        firewheel::node::AudioNodeInfo {
            num_min_supported_inputs: ChannelCount::MONO,
            num_max_supported_inputs: ChannelCount::MAX,
            num_min_supported_outputs: ChannelCount::MONO,
            num_max_supported_outputs: ChannelCount::MAX,
            equal_num_ins_and_outs: true,
            default_channel_config: ChannelConfig {
                num_inputs: ChannelCount::STEREO,
                num_outputs: ChannelCount::STEREO,
            },
            updates: false,
            uses_events: true,
        }
    }

    fn activate(
        &mut self,
        stream_info: &firewheel::StreamInfo,
        _: ChannelConfig,
    ) -> Result<Box<dyn firewheel::node::AudioNodeProcessor>, Box<dyn std::error::Error>> {
        Ok(Box::new(SawProcessor::new(
            stream_info.sample_rate as f32,
            self.clone(),
        )))
    }
}

#[derive(Clone)]
struct SawProcessor {
    sample_rate: f32,
    phase: f32,
    params: SawNode,
}

fn polyblep(phase_inc: f32, t: f32) -> f32 {
    let dt = phase_inc;
    if t < dt {
        let t_div_dt = t / dt;
        t_div_dt + t_div_dt - t_div_dt.powi(2) - 1.0
    } else if t > 1.0 - dt {
        let t_norm = (t - 1.0) / dt;
        t_norm.powi(2) + t_norm + t_norm + 1.0
    } else {
        0.0
    }
}

impl SawProcessor {
    fn new(sample_rate: f32, params: SawNode) -> Self {
        Self {
            sample_rate,
            phase: 0.,
            params,
        }
    }

    /// processes a single sample of audio through the filter
    pub fn process(&mut self, time: ClockSeconds) -> f32 {
        self.params.tick(time);

        let phase_increment = self.params.frequency.get() * (1.0 / self.sample_rate);

        let mut out = (2.0 * self.phase) - 1.0;
        out -= polyblep(phase_increment, self.phase);
        out *= -1.0;

        self.phase = (self.phase + phase_increment) % 1.0;

        out
    }
}

impl AudioNodeProcessor for SawProcessor {
    fn process(
        &mut self,
        _: &[&[f32]],
        outputs: &mut [&mut [f32]],
        events: firewheel::node::NodeEventIter,
        proc_info: firewheel::node::ProcInfo,
    ) -> ProcessStatus {
        // It would be nice if this process were made a little
        // more smooth, or it should at least be easy to
        // properly report errors without panicking or allocations.
        for event in events {
            if let EventData::Parameter(p) = event {
                let _ = self.params.patch(&p.data, &p.path);
            }
        }

        let seconds = proc_info.clock_seconds;

        for (i, output) in outputs[0].iter_mut().enumerate() {
            let seconds =
                seconds + firewheel::clock::ClockSeconds(i as f64 * proc_info.sample_rate_recip);

            *output = self.process(seconds);
        }

        ProcessStatus::outputs_not_silent()
    }
}
