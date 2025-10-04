//! A Rust implementation of Freeverb by Ian Hobson.
//! The original repo can be found [here](https://github.com/irh/freeverb-rs).

#![allow(missing_docs)]
#![allow(clippy::module_inception)]

use bevy_ecs::component::Component;
use firewheel::{
    channel_config::{ChannelConfig, ChannelCount},
    core::node::ProcInfo,
    diff::{Diff, Notify, Patch},
    dsp::declick::{DeclickValues, Declicker, FadeType},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, EmptyConfig,
        ProcBuffers, ProcExtra, ProcessStatus,
    },
};

mod all_pass;
mod comb;
mod delay_line;
mod freeverb;

/// A simple, relatively cheap stereo reverb.
#[derive(Diff, Patch, Clone, Debug, Component)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct FreeverbNode {
    /// Set the size of the emulated room, expressed from 0 to 1.
    pub room_size: f32,

    /// Set the high-frequency damping, expressed from 0 to 1.
    pub damping: f32,

    /// Set the L/R blending, expressed from 0 to 1.
    pub width: f32,

    /// Pause the reverb processing.
    ///
    /// This prevents a reverb tail from ringing out when you
    /// want all sound to momentarily pause.
    pub pause: bool,

    /// Reset the reverb, clearing its internal state.
    pub reset: Notify<()>,
}

impl Default for FreeverbNode {
    fn default() -> Self {
        FreeverbNode {
            room_size: 0.5,
            damping: 0.5,
            width: 0.5,
            pause: false,
            reset: Notify::new(()),
        }
    }
}

impl AudioNode for FreeverbNode {
    type Configuration = EmptyConfig;

    fn info(&self, _: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("freeverb")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::STEREO,
                num_outputs: ChannelCount::STEREO,
            })
    }

    fn construct_processor(
        &self,
        _: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> impl AudioNodeProcessor {
        let mut freeverb = freeverb::Freeverb::new(cx.stream_info.sample_rate.get() as usize);
        self.apply_params(&mut freeverb);

        FreeverbProcessor {
            freeverb,
            paused: self.pause,
            declicker: if self.pause {
                Declicker::SettledAt0
            } else {
                Declicker::SettledAt1
            },
            values: DeclickValues::new(cx.stream_info.declick_frames),
        }
    }
}

impl FreeverbNode {
    fn apply_params(&self, verb: &mut freeverb::Freeverb) {
        verb.set_dampening(self.damping as f64);
        verb.set_width(self.width as f64);
        verb.set_room_size(self.room_size as f64);
        verb.update_combs();
    }
}

struct FreeverbProcessor {
    freeverb: freeverb::Freeverb,
    paused: bool,
    declicker: Declicker,
    values: DeclickValues,
}

impl AudioNodeProcessor for FreeverbProcessor {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        ProcBuffers { inputs, outputs }: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        let mut update_combs = false;
        for patch in events.drain_patches::<FreeverbNode>() {
            match patch {
                FreeverbNodePatch::Damping(value) => {
                    self.freeverb.set_dampening(value as f64);
                    update_combs = true;
                }
                FreeverbNodePatch::RoomSize(value) => {
                    self.freeverb.set_room_size(value as f64);
                    update_combs = true;
                }
                FreeverbNodePatch::Width(value) => {
                    self.freeverb.set_width(value as f64);
                }
                FreeverbNodePatch::Reset(_) => {
                    self.freeverb.reset();
                }
                FreeverbNodePatch::Pause(value) => {
                    // TODO: perform declicking
                    self.paused = value;

                    if value {
                        self.declicker.fade_to_0(&self.values);
                    } else {
                        self.declicker.fade_to_1(&self.values);
                    }
                }
            }
        }

        if update_combs {
            self.freeverb.update_combs();
        }

        // I don't really want to figure out if the reverb is silent
        // if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
        //     // All inputs are silent.
        //     return ProcessStatus::ClearAllOutputs;
        // }

        if self.paused && self.declicker.is_settled() {
            return ProcessStatus::ClearAllOutputs;
        }

        for frame in 0..proc_info.frames {
            let (left, right) = self
                .freeverb
                .tick((inputs[0][frame] as f64, inputs[1][frame] as f64));

            outputs[0][frame] = left as f32;
            outputs[1][frame] = right as f32;
        }

        if !self.declicker.is_settled() {
            self.declicker.process(
                &mut outputs[..2],
                0..proc_info.frames,
                &self.values,
                1.0,
                FadeType::EqualPower3dB,
            );
        }

        ProcessStatus::outputs_not_silent()
    }

    fn new_stream(&mut self, stream_info: &firewheel::StreamInfo) {
        // TODO: we could probably attempt to smooth the transition here
        self.freeverb.resize(stream_info.sample_rate.get() as usize);
    }
}
