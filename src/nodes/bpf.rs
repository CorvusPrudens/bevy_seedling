//! A simple band-pass filter.

use crate::utils::timeline::Timeline;
use bevy_ecs::component::Component;
use firewheel::{
    channel_config::ChannelConfig,
    clock::DurationSeconds,
    core::{channel_config::NonZeroChannelCount, node::ProcInfo},
    diff::{Diff, Patch},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, ProcBuffers,
        ProcExtra, ProcessStatus,
    },
};

/// A simple low-pass filter.
#[derive(Diff, Patch, Debug, Clone, Component)]
pub struct BandPassNode {
    /// The cutoff frequency in hertz.
    pub frequency: Timeline<f32>,
    /// The filter's *quality*, or bandwidth.
    pub q: Timeline<f32>,
}

impl Default for BandPassNode {
    fn default() -> Self {
        Self {
            frequency: Timeline::new(1000.0),
            q: Timeline::new(1.0),
        }
    }
}

impl BandPassNode {
    /// Create a new [`BandPassNode`] with an initial cutoff frequency and quality.
    ///
    /// ```
    /// # use bevy_seedling::prelude::*;
    /// # use bevy::prelude::*;
    /// # fn system(mut commands: Commands) {
    /// commands.spawn(BandPassNode::new(1000.0, 1.0));
    /// # }
    /// ```
    pub fn new(frequency: f32, q: f32) -> Self {
        Self {
            frequency: Timeline::new(frequency),
            q: Timeline::new(q),
        }
    }
}

/// [`BandPassNode`]'s configuration.
#[derive(Debug, Component, Clone, PartialEq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct BandPassConfig {
    /// The number of channels to process.
    ///
    /// This node's input and output channel count will always match.
    pub channels: NonZeroChannelCount,
}

impl Default for BandPassConfig {
    fn default() -> Self {
        Self {
            channels: NonZeroChannelCount::STEREO,
        }
    }
}

impl AudioNode for BandPassNode {
    type Configuration = BandPassConfig;

    fn info(&self, config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("band-pass filter")
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
        BandPassProcessor {
            params: self.clone(),
            channels: vec![
                Bpf::new(
                    cx.stream_info.sample_rate.get() as f32,
                    self.frequency.get(),
                    self.q.get()
                );
                config.channels.get().get() as usize
            ],
        }
    }
}

#[derive(Clone)]
struct Bpf {
    sample_rate: f32,
    q: f32,
    x: (f32, f32),
    center_freq: f32,
}

impl Bpf {
    pub fn new(sample_rate: f32, center_freq: f32, q: f32) -> Self {
        Self {
            sample_rate,
            x: (0., 0.),
            q: q.max(0f32),
            center_freq: center_freq.clamp(0., 7e3),
        }
    }

    pub fn process(&mut self, audio: f32) -> f32 {
        use core::f32::consts;

        let omega = self.center_freq * consts::TAU / self.sample_rate;

        let one_minus_r = if self.q < 0.001 { 1. } else { omega / self.q }.min(1.);

        let r = 1. - one_minus_r;

        let q_cos = if (-consts::FRAC_PI_2..=consts::FRAC_PI_2).contains(&omega) {
            let g = omega * omega;

            ((g.powi(3) * (-1.0 / 720.0) + g * g * (1.0 / 24.0)) - g * 0.5) + 1.
        } else {
            0.
        };

        let coefficient_1 = 2. * q_cos * r;
        let coefficient_2 = -r * r;
        let gain = 2. * one_minus_r * (one_minus_r + r * omega);

        let last = self.x.0;
        let previous = self.x.1;

        let bp = audio + coefficient_1 * last + coefficient_2 * previous;

        self.x.1 = self.x.0;
        self.x.0 = bp;

        gain * bp
    }
}

struct BandPassProcessor {
    params: BandPassNode,
    channels: Vec<Bpf>,
}

impl AudioNodeProcessor for BandPassProcessor {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        ProcBuffers { inputs, outputs }: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        for patch in events.drain_patches::<BandPassNode>() {
            self.params.apply(patch);
        }

        if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
            // All inputs are silent.
            return ProcessStatus::ClearAllOutputs;
        }

        let time_range = proc_info.clock_seconds_range();

        let seconds = time_range.start;
        let frame_time = (time_range.end.0 - time_range.start.0) / proc_info.frames as f64;
        for sample in 0..inputs[0].len() {
            if sample % 32 == 0 {
                let seconds = seconds + DurationSeconds(sample as f64 * frame_time);
                self.params.frequency.tick(seconds);
                let frequency = self.params.frequency.get();
                let q = self.params.q.get();

                for channel in self.channels.iter_mut() {
                    channel.center_freq = frequency;
                    channel.q = q;
                }
            }

            for (i, channel) in self.channels.iter_mut().enumerate() {
                outputs[i][sample] = channel.process(inputs[i][sample]);
            }
        }

        ProcessStatus::outputs_not_silent()
    }
}
