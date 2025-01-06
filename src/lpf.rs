//! One-pole, low-pass filter.

use bevy_ecs::prelude::*;
use firewheel::{
    node::{AudioNode, AudioNodeProcessor, NodeEventType, ProcessStatus},
    param::{AudioParam, ParamEvent, Timeline},
    ChannelConfig, ChannelCount,
};

/// A one-pole, low-pass filter.
#[derive(seedling_macros::AudioParam, Debug, Clone, Component)]
pub struct LowPassNode {
    /// The cutoff frequency in hertz.
    pub frequency: Timeline<f32>,
}

impl LowPassNode {
    /// Create a new [`LowPassNode`] with an initial cutoff frequency.
    ///
    /// ```
    /// # use bevy_seedling::{*, lpf::LowPassNode};
    /// # use bevy::prelude::*;
    /// # fn system(mut commands: Commands) {
    /// commands.spawn(LowPassNode::new(1000.0));
    /// # }
    /// ```
    pub fn new(frequency: f32) -> Self {
        Self {
            frequency: Timeline::new(frequency),
        }
    }
}

impl From<LowPassNode> for Box<dyn AudioNode> {
    fn from(value: LowPassNode) -> Self {
        Box::new(value)
    }
}

impl AudioNode for LowPassNode {
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
        channel_config: ChannelConfig,
    ) -> Result<Box<dyn firewheel::node::AudioNodeProcessor>, Box<dyn std::error::Error>> {
        Ok(Box::new(LowPassProcessor {
            params: self.clone(),
            channels: vec![
                Lpf::new(stream_info.sample_rate.get() as f32, self.frequency.get());
                channel_config.num_inputs.get() as usize
            ],
        }))
    }
}

#[derive(Clone)]
struct Lpf {
    freq: f32,
    prev_out: f32,
    fixed_coeff: f32,
    coeff: f32,
}

impl Lpf {
    fn new(sample_rate: f32, frequency: f32) -> Self {
        let fixed_coeff = core::f32::consts::TAU / sample_rate;

        let mut filter = Self {
            freq: 0.,
            prev_out: 0.,
            fixed_coeff,
            coeff: 0.,
        };

        filter.set_frequency(frequency);

        filter
    }

    /// sets the cutoff frequency, recalculating the required coeff
    pub fn set_frequency(&mut self, freq: f32) {
        if freq != self.freq {
            self.coeff = (freq * self.fixed_coeff).clamp(0.0, 1.0);
            self.freq = freq;
        }
    }

    /// processes a single sample of audio through the filter
    pub fn process(&mut self, input: f32) -> f32 {
        // Recalculate frequency coefficient if it has changed.
        let fb = 1.0 - self.coeff;
        let output = self.coeff * input + fb * self.prev_out;
        self.prev_out = output;
        output
    }
}

struct LowPassProcessor {
    params: LowPassNode,
    channels: Vec<Lpf>,
}

impl AudioNodeProcessor for LowPassProcessor {
    fn process(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        events: firewheel::node::NodeEventIter,
        proc_info: firewheel::node::ProcInfo,
    ) -> ProcessStatus {
        // It would be nice if this process were made a little
        // more smooth, or it should at least be easy to
        // properly report errors without panicking or allocations.
        for event in events {
            if let NodeEventType::Custom(event) = event {
                if let Some(param) = event.downcast_ref::<ParamEvent>() {
                    let _ = self.params.patch(&param.data, &param.path);
                }
            }
        }

        // Actually this won't _technically_ be true, since
        // the filter may cary over a bit of energy from
        // when the inputs were just active.
        //
        // Allowing a bit of settling time would resolve this.
        if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
            // All inputs are silent.
            return ProcessStatus::ClearAllOutputs;
        }

        let seconds = proc_info.clock_seconds;
        for sample in 0..inputs[0].len() {
            if sample % 32 == 0 {
                let seconds = seconds
                    + firewheel::clock::ClockSeconds(sample as f64 * proc_info.sample_rate_recip);
                self.params.tick(seconds);
                let frequency = self.params.frequency.get();

                for channel in self.channels.iter_mut() {
                    channel.set_frequency(frequency);
                }
            }

            for (i, channel) in self.channels.iter_mut().enumerate() {
                outputs[i][sample] = channel.process(inputs[i][sample]);
            }
        }

        ProcessStatus::outputs_not_silent()
    }
}
