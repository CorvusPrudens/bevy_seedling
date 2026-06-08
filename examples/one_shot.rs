//! This example demonstrates how to play a one-shot sample.

use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::prelude::*;
use bevy_time::common_conditions::on_timer;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugins,
        ))
        .add_systems(
            Startup,
            |server: Res<AssetServer>, mut commands: Commands| {
                // Spawning a `SamplePlayer` component will play a sample
                // once as soon as it's loaded. If no pool is specified
                // and no effects are applied, the sample will be played in
                // the `DefaultPool`.
                commands.spawn(SamplePlayer::new(server.load("caw.ogg")));
            },
        )
        .add_systems(
            Update,
            make_loop.run_if(on_timer(Duration::from_secs_f32(0.5))),
        )
        .run();
}

fn make_loop(sounds: Query<&mut PlaybackSettings>) {
    for mut settings in sounds {
        settings.play_from = PlayFrom::Seconds(0.5);
        *settings.play = true;
    }
}
