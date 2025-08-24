//! Profiling utilities.

use firewheel::{
    StreamInfo,
    backend::{AudioBackend, DeviceInfo},
    clock::DurationSeconds,
    node::StreamStatus,
    processor::FirewheelProcessor,
};
use std::{
    num::NonZeroU32,
    sync::mpsc::{self, TryRecvError},
};

/// A very simple backend for testing and profiling.
#[allow(dead_code)]
pub struct ProfilingBackend {
    processor: mpsc::Sender<FirewheelProcessor<Self>>,
}

impl core::fmt::Debug for ProfilingBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfilingBackend")
            .field("processor", &())
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

impl AudioBackend for ProfilingBackend {
    type Config = ();
    type Instant = std::time::Instant;

    type StartStreamError = ProfilingError;
    type StreamError = ProfilingError;

    fn available_input_devices() -> Vec<DeviceInfo> {
        vec![]
    }

    fn available_output_devices() -> Vec<DeviceInfo> {
        vec![DeviceInfo {
            name: "default output".into(),
            num_channels: 2,
            is_default: true,
        }]
    }

    fn delay_from_last_process(
        &self,
        process_timestamp: Self::Instant,
    ) -> Option<std::time::Duration> {
        Some(std::time::Instant::now() - process_timestamp)
    }

    fn start_stream(_: Self::Config) -> Result<(Self, StreamInfo), Self::StartStreamError> {
        let sample_rate = NonZeroU32::new(48000).unwrap();
        let (sender, receiver) = mpsc::channel();

        const BLOCK_SIZE: usize = 128;
        const CHANNELS: usize = 2;

        std::thread::spawn(move || {
            let mut processor = None;

            let mut seconds = DurationSeconds(0.0);

            let block_duration = BLOCK_SIZE as f64 / sample_rate.get() as f64;
            let input = [0f32; BLOCK_SIZE * CHANNELS];
            let mut output = [0f32; BLOCK_SIZE * CHANNELS];

            let start = std::time::Instant::now();

            loop {
                match &mut processor {
                    None => {
                        let new_processor: FirewheelProcessor<Self> = receiver.recv().unwrap();
                        processor = Some(new_processor);
                    }
                    Some(processor) => {
                        let now = std::time::Instant::now();

                        processor.process_interleaved(
                            &input,
                            &mut output,
                            firewheel::backend::BackendProcessInfo {
                                num_in_channels: CHANNELS,
                                num_out_channels: CHANNELS,
                                frames: BLOCK_SIZE,
                                process_timestamp: now,
                                duration_since_stream_start: start - now,
                                input_stream_status: StreamStatus::empty(),
                                output_stream_status: StreamStatus::empty(),
                                dropped_frames: 0,
                            }, // CHANNELS,
                               // CHANNELS,
                               // BLOCK_SIZE,
                               // now,
                               // start - now,
                               // StreamStatus::empty(),
                               // StreamStatus::empty(),
                               // 0,
                        );
                        std::thread::sleep(std::time::Duration::from_secs_f64(block_duration));
                        seconds.0 += block_duration;

                        match receiver.try_recv() {
                            Ok(new_processor) => *processor = new_processor,
                            Err(TryRecvError::Empty) => {}
                            Err(TryRecvError::Disconnected) => break,
                        }
                    }
                }
            }
        });

        Ok((
            Self { processor: sender },
            StreamInfo {
                prev_sample_rate: sample_rate,
                sample_rate,
                sample_rate_recip: 1.0 / sample_rate.get() as f64,
                max_block_frames: NonZeroU32::new(BLOCK_SIZE as u32).unwrap(),
                num_stream_in_channels: 0,
                num_stream_out_channels: 2,
                declick_frames: NonZeroU32::new(16).unwrap(),
                input_device_name: None,
                output_device_name: Some("default output".into()),
                input_to_output_latency_seconds: 0.0,
            },
        ))
    }

    fn set_processor(&mut self, processor: FirewheelProcessor<Self>) {
        self.processor.send(processor).unwrap();
    }

    fn poll_status(&mut self) -> Result<(), Self::StreamError> {
        Ok(())
    }
}
