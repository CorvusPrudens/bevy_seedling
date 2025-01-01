//! Profiling utilities.

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
                sample_rate,
                max_block_samples: 256,
                num_stream_in_channels: 2,
                num_stream_out_channels: 2,
                stream_latency_samples: None,
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

        match self.processor.process_interleaved(
            input,
            output,
            2,
            2,
            samples,
            self.time,
            StreamStatus::empty(),
        ) {
            FirewheelProcessorStatus::DropProcessor => {
                panic!("received DropProcessor status");
            }
            _ => {}
        }

        self.time.0 += self.sample_rate_recip * samples as f64;
    }
}
