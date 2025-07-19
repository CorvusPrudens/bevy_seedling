//! This example demonstrates how to manage sample pausing and playing.

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_seedling::prelude::*;
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(Startup, startup)
        .add_systems(
            Update,
            toggle_playback.run_if(on_timer(Duration::from_millis(1500))),
        )
        .run();
}

// Let's start playing a couple samples.
fn startup(server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn(SamplePlayer::new(server.load("caw.ogg")).looping());
    commands.spawn(SamplePlayer::new(server.load("crow_ambience.ogg")).looping());
}

fn toggle_playback(
    // With this, we can iterate over _all_ sample players.
    mut settings: Query<&mut PlaybackSettings, With<SamplePlayer>>,
) {
    info!("toggled playback!");

    // The pause and play methods queue up audio events that
    // are sent at the end of the frame.
    for mut settings in settings.iter_mut() {
        if matches!(*settings.playback, PlaybackState::Play { .. }) {
            settings.pause();
        } else {
            settings.play();
        }
    }
}
