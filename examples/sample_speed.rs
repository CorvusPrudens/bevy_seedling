//! This example demonstrates how to adjust a sample's speed
//! (and therefore pitch) during playback.

use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use bevy_seedling::prelude::*;
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((
            // Without a window, the event loop tends to run quite fast.
            // We'll slow it down so we don't drop any audio events.
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16))),
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(
            Startup,
            |server: Res<AssetServer>, mut commands: Commands| {
                commands.spawn(SamplePlayer {
                    sample: server.load("selfless_courage.ogg"),
                    volume: Volume::Decibels(-6.0),
                    repeat_mode: RepeatMode::RepeatEndlessly,
                });
            },
        )
        .add_systems(Update, modulate_speed)
        .run();
}

// The key component is `PlaybackSettings`. It's a set of parameters
// that can be changed during playback.
fn modulate_speed(player: Single<&mut PlaybackSettings>, mut angle: Local<f32>, time: Res<Time>) {
    let mut params = player.into_inner();

    params.speed = angle.sin() as f64 * 0.05 + 1.0;

    let period = 10.0;
    *angle += time.delta_secs() * core::f32::consts::TAU / period;
}
