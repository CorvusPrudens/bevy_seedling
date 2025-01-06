//! NOTE: This appears to be non-functional for 0.2.
//! This should at least demonstrate the intended API.
//!
//! This example demonstrates how to use `Pause`
//! to manage audio node activity.

use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{sample::SamplePlayer, Pause, PlaybackSettings, SeedlingPlugin};

#[derive(Resource)]
struct Delay(Timer);

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .insert_resource(Delay(Timer::new(
            Duration::from_millis(500),
            TimerMode::Repeating,
        )))
        .add_systems(
            Startup,
            |server: Res<AssetServer>, mut commands: Commands| {
                commands.spawn((
                    SamplePlayer::new(server.load("snd_wobbler.wav")),
                    PlaybackSettings::LOOP,
                ));
            },
        )
        .add_systems(Update, toggle_activity)
        .run();
}

// Here, we continually pause and unpause the sample node.
fn toggle_activity(
    without_pause: Query<Entity, (With<SamplePlayer>, Without<Pause>)>,
    with_pause: Query<Entity, (With<SamplePlayer>, With<Pause>)>,
    time: Res<Time>,
    mut delay: ResMut<Delay>,
    mut commands: Commands,
) {
    delay.0.tick(time.delta());

    if delay.0.just_finished() {
        for sampler in without_pause.iter() {
            // Once added, `Pause` will emit
            // a `Pause` event.
            commands.entity(sampler).insert(Pause);
        }

        for sampler in with_pause.iter() {
            // Removing it will emit a `Resume` event.
            commands.entity(sampler).remove::<Pause>();
        }
    }
}
