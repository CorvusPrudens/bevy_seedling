//! A mock backend for testing.

use audioadapter_buffers::direct::InterleavedSlice;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use firewheel::{ActivateInfo, FirewheelContext, node::StreamStatus};
use std::num::{NonZero, NonZeroU32};

use crate::{
    context::{AudioContext, SampleRate},
    prelude::SeedlingStartupSystems,
};

/// A mock backend that runs the audio processing in a throw-away thread.
///
/// This is useful for testing since no audio devices are needed.
#[derive(Debug)]
pub struct MockBackendPlugin;

impl Plugin for MockBackendPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostStartup,
            start_stream.in_set(SeedlingStartupSystems::StreamInitialization),
        );
    }
}

const MOCK_SAMPLE_RATE: NonZeroU32 = NonZeroU32::new(48000).unwrap();

fn start_stream(mut context: ResMut<AudioContext>, commands: Commands) {
    context.with(initialize_mock);

    let sample_rate = SampleRate::new(MOCK_SAMPLE_RATE);
    super::initialize_stream(sample_rate, commands);
}

fn initialize_mock(context: &mut FirewheelContext) {
    const BLOCK_SIZE: usize = 128;
    const CHANNELS: usize = 2;

    let mut processor = context
        .activate(ActivateInfo {
            sample_rate: MOCK_SAMPLE_RATE,
            max_block_frames: NonZero::new(BLOCK_SIZE as u32).unwrap(),
            num_stream_in_channels: CHANNELS as u32,
            num_stream_out_channels: CHANNELS as u32,
            input_to_output_latency_seconds: 0.0,
        })
        .unwrap();

    std::thread::spawn(move || {
        let block_duration = BLOCK_SIZE as f64 / MOCK_SAMPLE_RATE.get() as f64;
        let input = [0f32; BLOCK_SIZE * CHANNELS];
        let mut output = [0f32; BLOCK_SIZE * CHANNELS];

        loop {
            let start = std::time::Instant::now();
            let now = std::time::Instant::now();

            let input = InterleavedSlice::new(&input, CHANNELS, BLOCK_SIZE).unwrap();
            let mut output = InterleavedSlice::new_mut(&mut output, CHANNELS, BLOCK_SIZE).unwrap();

            processor.process(
                &input,
                &mut output,
                firewheel::backend::BackendProcessInfo {
                    frames: BLOCK_SIZE,
                    process_timestamp: Some(now),
                    duration_since_stream_start: start - now,
                    input_stream_status: StreamStatus::empty(),
                    output_stream_status: StreamStatus::empty(),
                    dropped_frames: 0,
                    process_to_playback_delay: None,
                },
            );

            std::thread::sleep(std::time::Duration::from_secs_f64(block_duration));
        }
    });
}
