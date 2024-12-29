//! An ADSR (attack, decay, sustain, release) envelope.

use bevy_ecs::prelude::*;
use firewheel::{
    node::{AudioNode, AudioNodeProcessor, EventData, ProcessStatus},
    param::AudioParam,
    param::{Deferred, Timeline},
    ChannelConfig, ChannelCount,
};

#[derive(seedling_macros::AudioParam, Debug, Clone, Component)]
pub struct AdsrNode {
    pub gate: Deferred<bool>,
    pub attack_time: Timeline<f32>,
    pub decay_time: Timeline<f32>,
    pub sustain_level: Timeline<f32>,
    pub release_time: Timeline<f32>,
}

impl From<AdsrNode> for Box<dyn AudioNode> {
    fn from(value: AdsrNode) -> Self {
        Box::new(value)
    }
}

impl AudioNode for AdsrNode {
    fn debug_name(&self) -> &'static str {
        "ADSR envelope"
    }

    fn info(&self) -> firewheel::node::AudioNodeInfo {
        firewheel::node::AudioNodeInfo {
            num_min_supported_inputs: ChannelCount::ZERO,
            num_max_supported_inputs: ChannelCount::ZERO,
            num_min_supported_outputs: ChannelCount::MONO,
            num_max_supported_outputs: ChannelCount::MONO,
            equal_num_ins_and_outs: false,
            default_channel_config: ChannelConfig {
                num_inputs: ChannelCount::ZERO,
                num_outputs: ChannelCount::MONO,
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
        Ok(Box::new(AdsrProcessor::new(
            stream_info.sample_rate as f32,
            self.clone(),
        )))
    }
}

enum AdsrStage {
    Idle,
    Attack,
    Decay,
    Release,
}

struct AdsrProcessor {
    sample_rate: f32,
    trigger: (bool, bool),
    params: AdsrNode,
    stage: AdsrStage,
    x: f32,
    atk_d0: f32,
    dec_d0: f32,
    rel_d0: f32,
}

fn time_constant(sample_rate: f32, time_ms: f32) -> f32 {
    let mut coeff = 1.;
    if time_ms > 0. {
        let target = (1. / core::f32::consts::E).ln();
        coeff = 1. - (target / (time_ms * 0.001 * sample_rate)).exp();
    }
    coeff
}

impl AdsrProcessor {
    pub fn new(sample_rate: f32, params: AdsrNode) -> Self {
        Self {
            sample_rate,
            trigger: (false, false),
            atk_d0: time_constant(sample_rate, params.attack_time.get()),
            dec_d0: time_constant(sample_rate, params.decay_time.get()),
            rel_d0: time_constant(sample_rate, params.release_time.get()),
            params,
            stage: AdsrStage::Idle,
            x: 0.,
        }
    }

    pub fn update_constants(&mut self) {
        self.atk_d0 = time_constant(self.sample_rate, self.params.attack_time.get());
        self.dec_d0 = time_constant(self.sample_rate, self.params.decay_time.get());
        self.rel_d0 = time_constant(self.sample_rate, self.params.release_time.get());
    }

    pub fn process(&mut self) -> f32 {
        let trig = self.params.gate.get();

        // Shuffle edge detection
        self.trigger = (trig, self.trigger.0);
        let (tnow, tprev) = self.trigger;

        if tnow && !tprev {
            // rising edge
            self.stage = AdsrStage::Attack;
        } else if !tnow && tprev {
            // falling edge.
            self.stage = AdsrStage::Release;
        }

        let d0 = match self.stage {
            AdsrStage::Attack => self.atk_d0,
            AdsrStage::Decay => self.dec_d0,
            AdsrStage::Release => self.rel_d0,
            _ => 1.,
        };

        // Attack time must be computed to exceed one due to the
        // asymptopic nature fo the curve -- DaisySP has a configurable curve
        // so the calculation is quite different, but I think for now this is fine.
        // Similarly for the release value, the target is set just below 0
        let target = match self.stage {
            AdsrStage::Attack => 1.001,
            AdsrStage::Decay => self.params.sustain_level.get(),
            _ => -0.001,
        };

        match self.stage {
            AdsrStage::Idle => 0.,
            AdsrStage::Attack => {
                self.x += d0 * (target - self.x);
                if self.x > 1. {
                    self.x = 1.;
                    self.stage = AdsrStage::Decay;
                }

                self.x
            }
            AdsrStage::Decay | AdsrStage::Release => {
                self.x += d0 * (target - self.x);
                if self.x < 0. {
                    self.x = 0.;
                    self.stage = AdsrStage::Idle;
                }

                self.x
            }
        }
    }
}

impl AudioNodeProcessor for AdsrProcessor {
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

            self.params.tick(seconds);
            self.update_constants();

            *output = self.process();
        }

        ProcessStatus::outputs_not_silent()
    }
}
