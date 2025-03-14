//! This example demonstrates how to play a one-shot sample.

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
        .add_systems(
            Startup,
            |server: Res<AssetServer>, mut commands: Commands| {
                // Spawning a `SamplePlayer` component will play a sample
                // once as soon as it's loaded. If no pool is specified
                // like in this example, teh sample will be played in
                // the `DefaultPool`.
                commands.spawn(SamplePlayer::new(server.load("snd_wobbler.wav")));
            },
        )
        .run();
}
