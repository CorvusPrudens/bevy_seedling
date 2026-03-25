//! This example demonstrates how to process input from a microphone or
//! other input device. This example grabs the system default, panicking
//! if no device is available.
//!
//! Note that this may result in positive feedback on some setups.
//! I recommend starting with low volume!

use bevy::{input::InputPlugin, log::LogPlugin, prelude::*};
use bevy_seedling::{context::AudioContextConfig, prelude::*};
use firewheel::cpal::{CpalConfig, CpalInputConfig};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, SeedlingPlugins))
        .add_systems(Startup, route_input)
        .add_systems(Update, bypass)
        .run();
}

/// Here we route the input node, `AudioGraphInput`, to the `MainBus`.
fn route_input(
    input: Single<Entity, With<SamplerPool<MusicPool>>>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands
        .entity(*input)
        .disconnect(MainBus)
        // route the music through a ton of reverb
        .chain_node(FreeverbNode {
            room_size: 0.9,
            damping: 0.8,
            width: 0.8,
            ..Default::default()
        })
        .connect(MainBus);

    commands.spawn((
        MusicPool,
        SamplePlayer::new(server.load("divine_comedy.ogg")),
    ));
}

fn bypass(
    interaction: Res<ButtonInput<KeyCode>>,
    freeverb: Single<(Entity, Has<AudioBypass>), With<FreeverbNode>>,
    mut commands: Commands,
) {
    if interaction.just_pressed(KeyCode::Space) {
        let (entity, bypassed) = freeverb.into_inner();

        if bypassed {
            info!("Removing bypass...");
            commands.entity(entity).remove::<AudioBypass>();
        } else {
            info!("Bypassing reverb...");
            commands.entity(entity).insert(AudioBypass);
        }
    }
}
