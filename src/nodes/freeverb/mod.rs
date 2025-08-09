//! A Rust implementation of Freeverb by Ian Hobson.
//! The original repo can be found [here](https://github.com/irh/freeverb-rs).

#![allow(missing_docs)]
#![allow(clippy::module_inception)]

use bevy_ecs::component::Component;
use firewheel::{
    channel_config::{ChannelConfig, ChannelCount},
    core::node::ProcInfo,
    diff::{Diff, Patch},
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
}

impl Default for FreeverbNode {
    fn default() -> Self {
        FreeverbNode {
            room_size: 0.5,
            damping: 0.5,
            width: 0.5,
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
            params: self.clone(),
            freeverb,
        }
    }
}

impl FreeverbNode {
    fn apply_params(&self, verb: &mut freeverb::Freeverb) {
        verb.set_dampening(self.damping as f64);
        verb.set_width(self.width as f64);
        verb.set_room_size(self.room_size as f64);
    }
}

struct FreeverbProcessor {
    params: FreeverbNode,
    freeverb: freeverb::Freeverb,
}

impl AudioNodeProcessor for FreeverbProcessor {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        ProcBuffers { inputs, outputs }: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        let mut changed = false;

        for patch in events.drain_patches::<FreeverbNode>() {
            changed = true;
            self.params.apply(patch);
        }

        if changed {
            self.params.apply_params(&mut self.freeverb);
        }

        // I don't really want to figure out if the reverb is silent
        // if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
        //     // All inputs are silent.
        //     return ProcessStatus::ClearAllOutputs;
        // }

        for frame in 0..proc_info.frames {
            let (left, right) = self
                .freeverb
                .tick((inputs[0][frame] as f64, inputs[1][frame] as f64));

            outputs[0][frame] = left as f32;
            outputs[1][frame] = right as f32;
        }

        ProcessStatus::outputs_not_silent()
    }

    fn new_stream(&mut self, stream_info: &firewheel::StreamInfo) {
        // Note: for a proper implementation, we should change the sample rate in-place
        // rather than creating entirely new buffers.
        self.freeverb = freeverb::Freeverb::new(stream_info.sample_rate.get() as usize);
        self.params.apply_params(&mut self.freeverb);
    }
}
