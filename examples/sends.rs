//! This example demonstrates how to route audio to a send.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(Startup, create_send)
        .run();
}

fn create_send(server: Res<AssetServer>, mut commands: Commands) {
    // Effects like reverb are generally CPU hungry, so
    // you should prefer routing audio to a single reverb,
    // rather than creating many instances for each sample.
    let send = commands
        .spawn(FreeverbNode {
            room_size: 0.9,
            width: 0.75,
            damping: 0.5,
        })
        .id();

    // On the other hand, `Send`s are cheap, so we can insert
    // them anywhere (like directly on a sampler) to send
    // audio to expensive processing chains.
    commands.spawn((
        SamplePlayer::new(server.load("caw.ogg")),
        // Here we send some of the signal to the reverb.
        sample_effects![SendNode::new(Volume::Linear(0.70), send)],
    ));
}
