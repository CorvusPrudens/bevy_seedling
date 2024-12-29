//! A simple band-pass filter.

use bevy_ecs::prelude::*;
use firewheel::{
    node::{AudioNode, AudioNodeProcessor, EventData, ProcessStatus},
    param::AudioParam,
    param::Timeline,
    ChannelConfig, ChannelCount,
};

/// A simple low-pass filter.
#[derive(seedling_macros::AudioParam, Debug, Clone, Component)]
pub struct BandPassNode {
    /// The cutoff frequency in hertz.
    pub frequency: Timeline<f32>,
    pub q: Timeline<f32>,
}

impl BandPassNode {
    /// Create a new [`BandPassNode`] with an initial cutoff frequency and quality.
    ///
    /// ```
    /// # use bevy_seedling::{*, lpf::BandPassNode};
    /// # use bevy::prelude::*;
    /// # fn system(mut commands: Commands) {
    /// commands.spawn(BandPassNode::new(1000.0));
    /// # }
    /// ```
    pub fn new(frequency: f32, q: f32) -> Self {
        Self {
            frequency: Timeline::new(frequency),
            q: Timeline::new(q),
        }
    }
}

impl From<BandPassNode> for Box<dyn AudioNode> {
    fn from(value: BandPassNode) -> Self {
        Box::new(value)
    }
}

impl AudioNode for BandPassNode {
    fn debug_name(&self) -> &'static str {
        "band pass filter"
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
        Ok(Box::new(BandPassProcessor {
            params: self.clone(),
            channels: vec![
                Bpf::new(
                    stream_info.sample_rate as f32,
                    self.frequency.get(),
                    self.q.get()
                );
                channel_config.num_inputs.get() as usize
            ],
        }))
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
        inputs: &[&[f32]],
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
            let seconds = seconds
                + firewheel::clock::ClockSeconds(sample as f64 * proc_info.sample_rate_recip);
            self.params.tick(seconds);
            let frequency = self.params.frequency.get();

            for (i, channel) in self.channels.iter_mut().enumerate() {
                channel.center_freq = frequency;
                outputs[i][sample] = channel.process(inputs[i][sample]);
            }
        }

        ProcessStatus::outputs_not_silent()
    }
}
