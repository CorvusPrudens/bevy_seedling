//! This example demonstrates how to process input from a microphone or
//! other input device. This example grabs the system default, panicking
//! if no device is available.
//!
//! Note that this may result in positive feedback on some setups.
//! I recommend starting with low volume!

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::prelude::*;
use firewheel::cpal::{CpalConfig, CpalInputConfig};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin {
                config: FirewheelConfig {
                    // Ensure the graph has an input
                    num_graph_inputs: ChannelCount::MONO,
                    ..Default::default()
                },
                stream_config: CpalConfig {
                    // Ensure we provide an input config
                    input: Some(CpalInputConfig::default()),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .add_systems(Startup, route_input)
        .run();
}

/// Here we route the input node, `AudioGraphInput`, to the `MainBus`.
fn route_input(input: Single<Entity, With<AudioGraphInput>>, mut commands: Commands) {
    commands
        .entity(*input)
        // route the input through a ton of reverb
        .chain_node(FreeverbNode {
            room_size: 0.9,
            damping: 0.8,
            width: 0.8,
            ..Default::default()
        })
        .connect(MainBus);
}
