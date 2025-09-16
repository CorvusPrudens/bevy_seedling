use bevy_ecs::prelude::*;
use firewheel::{
    channel_config::ChannelConfig,
    core::{channel_config::NonZeroChannelCount, node::ProcInfo},
    diff::{Diff, Patch},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, ProcBuffers,
        ProcExtra, ProcessStatus,
    },
};

#[derive(Diff, Patch, Debug, Default, Clone, Component)]
pub struct FbInNode;

#[derive(Debug, Component, Clone, PartialEq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct FbConfig {
    /// The number of channels to process.
    ///
    /// This node's input and output channel count will always match.
    pub channels: NonZeroChannelCount,

    /// The [`FbOutNode`] node with which this input is linked.
    pub linked_node: Option<Entity>,
}

impl Default for FbConfig {
    fn default() -> Self {
        Self {
            channels: NonZeroChannelCount::STEREO,
            linked_node: None,
        }
    }
}

impl AudioNode for FbInNode {
    type Configuration = FbConfig;

    fn info(&self, config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("Feedback In")
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
        let channels = (0..config.channels.get().get())
            .map(|_| FbInProc::new(cx.stream_info.max_block_frames.get() as usize))
            .collect();

        FeedbackInProcessor { channels }
    }
}

struct FbInProc {
    delay_line: Vec<f32>,
    buffer: Option<ringbuf::HeapProd<f32>>,
}

impl FbInProc {
    pub fn new(max_block: usize) -> Self {
        let mut delay_line = Vec::new();
        delay_line.reserve_exact(max_block);

        Self {
            delay_line,
            buffer: None,
        }
    }

    // pub fn process(&mut self, audio: f32) -> f32 {
    //     use core::f32::consts;
    //
    //     let omega = self.center_freq * consts::TAU / self.sample_rate;
    //
    //     let one_minus_r = if self.q < 0.001 { 1. } else { omega / self.q }.min(1.);
    //
    //     let r = 1. - one_minus_r;
    //
    //     let q_cos = if (-consts::FRAC_PI_2..=consts::FRAC_PI_2).contains(&omega) {
    //         let g = omega * omega;
    //
    //         ((g.powi(3) * (-1.0 / 720.0) + g * g * (1.0 / 24.0)) - g * 0.5) + 1.
    //     } else {
    //         0.
    //     };
    //
    //     let coefficient_1 = 2. * q_cos * r;
    //     let coefficient_2 = -r * r;
    //     let gain = 2. * one_minus_r * (one_minus_r + r * omega);
    //
    //     let last = self.x.0;
    //     let previous = self.x.1;
    //
    //     let bp = audio + coefficient_1 * last + coefficient_2 * previous;
    //
    //     self.x.1 = self.x.0;
    //     self.x.0 = bp;
    //
    //     gain * bp
    // }
}

struct FeedbackInProcessor {
    channels: Vec<FbInProc>,
}

impl AudioNodeProcessor for FeedbackInProcessor {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        ProcBuffers { inputs, outputs }: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        // for patch in events.drain_patches::<FbInNode>() {
        //     self.params.apply(patch);
        // }

        if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
            // All inputs are silent.
            return ProcessStatus::ClearAllOutputs;
        }

        let time_range = proc_info.clock_seconds_range();

        // let seconds = time_range.start;
        // let frame_time = (time_range.end.0 - time_range.start.0) / proc_info.frames as f64;
        // for sample in 0..inputs[0].len() {
        //     if sample % 32 == 0 {
        //         let seconds = seconds + DurationSeconds(sample as f64 * frame_time);
        //         self.params.frequency.tick(seconds);
        //         let frequency = self.params.frequency.get();
        //         let q = self.params.q.get();
        //
        //         for channel in self.channels.iter_mut() {
        //             channel.center_freq = frequency;
        //             channel.q = q;
        //         }
        //     }
        //
        //     for (i, channel) in self.channels.iter_mut().enumerate() {
        //         outputs[i][sample] = channel.process(inputs[i][sample]);
        //     }
        // }

        ProcessStatus::outputs_not_silent()
    }
}
