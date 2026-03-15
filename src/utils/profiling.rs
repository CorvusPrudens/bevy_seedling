//! Profiling utilities.

use firewheel::{ActivateInfo, FirewheelConfig, FirewheelContext, node::StreamStatus};
use std::num::{NonZero, NonZeroU32};

/// A very simple backend for testing and profiling.
pub struct ProfilingBackend {
    context: FirewheelContext,
}

impl core::fmt::Debug for ProfilingBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfilingBackend")
            .field("context", &())
            .finish()
    }
}

#[derive(Debug)]
#[allow(missing_docs)]
pub struct ProfilingError;

impl core::fmt::Display for ProfilingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <_ as core::fmt::Debug>::fmt(self, f)
    }
}

impl std::error::Error for ProfilingError {}

impl ProfilingBackend {
    pub fn new() -> Self {
        let mut context = FirewheelContext::new(FirewheelConfig::default());

        let sample_rate = NonZeroU32::new(48000).unwrap();

        const BLOCK_SIZE: usize = 128;
        const CHANNELS: usize = 2;

        let mut processor = context
            .activate(ActivateInfo {
                sample_rate,
                max_block_frames: NonZero::new(BLOCK_SIZE as u32).unwrap(),
                num_stream_in_channels: 0,
                num_stream_out_channels: 2,
                input_to_output_latency_seconds: 0.0,
            })
            .unwrap();

        std::thread::spawn(move || {
            let block_duration = BLOCK_SIZE as f64 / sample_rate.get() as f64;
            let input = [0f32; BLOCK_SIZE * CHANNELS];
            let mut output = [0f32; BLOCK_SIZE * CHANNELS];

            let start = std::time::Instant::now();

            let now = std::time::Instant::now();

            processor.process_interleaved(
                &input,
                &mut output,
                firewheel::backend::BackendProcessInfo {
                    num_in_channels: CHANNELS,
                    num_out_channels: CHANNELS,
                    frames: BLOCK_SIZE,
                    process_timestamp: Some(now),
                    duration_since_stream_start: start - now,
                    input_stream_status: StreamStatus::empty(),
                    output_stream_status: StreamStatus::empty(),
                    dropped_frames: 0,
                },
            );

            std::thread::sleep(std::time::Duration::from_secs_f64(block_duration));
        });

        Self { context }
    }
}
