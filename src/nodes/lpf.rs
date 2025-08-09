//! One-pole, low-pass filter.

use bevy_ecs::component::Component;
use firewheel::{
    channel_config::{ChannelConfig, NonZeroChannelCount},
    diff::{Diff, Patch},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, ProcBuffers,
        ProcExtra, ProcInfo, ProcessStatus,
    },
    param::smoother::{SmoothedParam, SmootherConfig},
};

/// A one-pole, low-pass filter.
#[derive(Diff, Patch, Debug, Clone, Component)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct LowPassNode {
    /// The cutoff frequency in hertz.
    pub frequency: f32,
}

impl Default for LowPassNode {
    fn default() -> Self {
        Self { frequency: 1000.0 }
    }
}

/// [`LowPassNode`]'s configuration.
#[derive(Debug, Component, Clone, PartialEq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct LowPassConfig {
    /// The parameter smoothing config used for frequency.
    pub smoother_config: SmootherConfig,
    /// The number of input and output channels.
    pub channels: NonZeroChannelCount,
}

impl Default for LowPassConfig {
    fn default() -> Self {
        Self {
            smoother_config: Default::default(),
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
    }

    fn construct_processor(
        &self,
        config: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> impl AudioNodeProcessor {
        LowPassProcessor {
            frequency: SmoothedParam::new(
                self.frequency,
                config.smoother_config,
                cx.stream_info.sample_rate,
            ),
            channels: vec![
                Lpf::new(cx.stream_info.sample_rate.get() as f32, self.frequency);
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
    frequency: SmoothedParam,
    channels: Vec<Lpf>,
}

impl AudioNodeProcessor for LowPassProcessor {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        ProcBuffers { inputs, outputs }: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        for patch in events.drain_patches::<LowPassNode>() {
            match patch {
                LowPassNodePatch::Frequency(f) => self.frequency.set_value(f.clamp(0.0, 20_000.0)),
            }
        }

        // Actually this won't _technically_ be true, since
        // the filter may cary over a bit of energy from
        // when the inputs were just active.
        //
        // Allowing a bit of settling time would resolve this.
        if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
            self.frequency.reset();

            // All inputs are silent.
            return ProcessStatus::ClearAllOutputs;
        }

        if self.frequency.is_smoothing() {
            for sample in 0..inputs[0].len() {
                let freq = self.frequency.next_smoothed();

                for channel in self.channels.iter_mut() {
                    channel.set_frequency(freq);
                }

                for (i, channel) in self.channels.iter_mut().enumerate() {
                    outputs[i][sample] = channel.process(inputs[i][sample]);
                }
            }

            self.frequency.settle();
        } else {
            let freq = self.frequency.target_value();
            for channel in self.channels.iter_mut() {
                channel.set_frequency(freq);
            }

            for sample in 0..inputs[0].len() {
                for (i, channel) in self.channels.iter_mut().enumerate() {
                    outputs[i][sample] = channel.process(inputs[i][sample]);
                }
            }
        }

        ProcessStatus::outputs_not_silent()
    }

    fn new_stream(&mut self, stream_info: &firewheel::StreamInfo) {
        self.frequency.update_sample_rate(stream_info.sample_rate);
    }
}
