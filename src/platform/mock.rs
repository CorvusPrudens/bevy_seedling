//! A mock backend for testing.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use firewheel::{ActivateInfo, FirewheelConfig, FirewheelContext, node::StreamStatus};
use std::{
    num::{NonZero, NonZeroU32},
    sync,
};

use crate::{
    context::{AudioGraph, SampleRate},
    platform::*,
    prelude::SeedlingStartupSystems,
};

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

fn start_stream(mut context: ResMut<AudioGraph>, server: Res<AssetServer>, commands: Commands) {
    context.with(initialize_mock);

    let sample_rate = SampleRate::new(MOCK_SAMPLE_RATE);
    super::initialize_stream(sample_rate, &server, commands);
}

fn initialize_mock(context: &mut FirewheelContext) {
    const BLOCK_SIZE: usize = 128;
    const CHANNELS: usize = 2;

    let mut processor = context
        .activate(ActivateInfo {
            sample_rate: MOCK_SAMPLE_RATE,
            max_block_frames: NonZero::new(BLOCK_SIZE as u32).unwrap(),
            num_stream_in_channels: 0,
            num_stream_out_channels: 2,
            input_to_output_latency_seconds: 0.0,
        })
        .unwrap();

    std::thread::spawn(move || {
        let block_duration = BLOCK_SIZE as f64 / MOCK_SAMPLE_RATE.get() as f64;
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
}
