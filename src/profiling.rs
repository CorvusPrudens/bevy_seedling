//! Profiling utilities.

use std::num::NonZeroU32;

use firewheel::{
    clock::ClockSeconds,
    node::StreamStatus,
    processor::{FirewheelProcessor, FirewheelProcessorStatus},
    FirewheelGraphCtx, StreamInfo,
};

/// A simple audio context that facilitates
/// focused benchmarking.
#[allow(missing_debug_implementations)]
pub struct ProfilingContext {
    pub context: FirewheelGraphCtx,
    processor: FirewheelProcessor,
    time: ClockSeconds,
    sample_rate_recip: f64,
}

impl ProfilingContext {
    pub fn new(sample_rate: u32) -> Self {
        let mut context = FirewheelGraphCtx::new(Default::default());

        let processor = context
            .activate(StreamInfo {
                sample_rate: NonZeroU32::new(sample_rate).unwrap(),
                declick_frames: NonZeroU32::MIN,
                sample_rate_recip: 1.0 / sample_rate as f64,
                max_block_frames: NonZeroU32::new(256).unwrap(),
                num_stream_in_channels: 2,
                num_stream_out_channels: 2,
                stream_latency_frames: None,
            })
            .unwrap();

        Self {
            context,
            processor,
            time: ClockSeconds(0.),
            sample_rate_recip: 1. / sample_rate as f64,
        }
    }

    pub fn process_interleaved(&mut self, input: &[f32], output: &mut [f32]) {
        let samples = output.len() / 2;

        let status = self.processor.process_interleaved(
            input,
            output,
            2,
            2,
            samples,
            self.time,
            StreamStatus::empty(),
        );

        if matches!(status, FirewheelProcessorStatus::DropProcessor) {
            panic!("received DropProcessor status");
        }

        self.time.0 += self.sample_rate_recip * samples as f64;
    }
}
