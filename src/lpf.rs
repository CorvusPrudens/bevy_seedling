//! One-pole, low-pass filter.

use std::any::Any;

use crate::timeline::Timeline;
use bevy_ecs::prelude::*;
use firewheel::{
    channel_config::{ChannelConfig, NonZeroChannelCount},
    clock::ClockSeconds,
    diff::{Diff, Patch},
    event::NodeEventList,
    node::{AudioNode, AudioNodeInfo, AudioNodeProcessor, ProcBuffers, ProcInfo, ProcessStatus},
    StreamInfo,
};

/// A one-pole, low-pass filter.
#[derive(Diff, Patch, Debug, Clone, Component)]
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

#[derive(Debug, Component, Clone)]
pub struct LowPassConfig {
    pub channels: NonZeroChannelCount,
}

impl Default for LowPassConfig {
    fn default() -> Self {
        Self {
            channels: NonZeroChannelCount::STEREO,
        }
    }
}

impl AudioNode for LowPassNode {
    type Configuration = LowPassConfig;

    fn info(&self, config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("low-pass filter")
            .channel_config(ChannelConfig {
                num_inputs: config.channels.get(),
                num_outputs: config.channels.get(),
            })
            .uses_events(true)
    }

    fn processor(
        &self,
        config: &Self::Configuration,
        stream_info: &StreamInfo,
        _: &mut Option<Box<dyn Any>>,
    ) -> impl AudioNodeProcessor {
        LowPassProcessor {
            params: self.clone(),
            channels: vec![
                Lpf::new(stream_info.sample_rate.get() as f32, self.frequency.get());
                config.channels.get().get() as usize
            ],
        }
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
        ProcBuffers {
            inputs, outputs, ..
        }: ProcBuffers,
        proc_info: &ProcInfo,
        events: NodeEventList,
    ) -> ProcessStatus {
        self.params.patch_list(events);

        // Actually this won't _technically_ be true, since
        // the filter may cary over a bit of energy from
        // when the inputs were just active.
        //
        // Allowing a bit of settling time would resolve this.
        if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
            // All inputs are silent.
            return ProcessStatus::ClearAllOutputs;
        }

        let seconds = proc_info.clock_seconds.start;
        let frame_time = (proc_info.clock_seconds.end.0 - proc_info.clock_seconds.start.0)
            / proc_info.frames as f64;
        for sample in 0..inputs[0].len() {
            if sample % 32 == 0 {
                let seconds = seconds + ClockSeconds(sample as f64 * frame_time);
                self.params.frequency.tick(seconds);
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
