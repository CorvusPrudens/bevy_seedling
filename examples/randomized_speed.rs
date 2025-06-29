//! This example demonstrates how to slightly randomize
//! the pitch of samples.
//!
//! This is a common technique to break up the monotony of
//! frequently played sounds like footsteps.

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
        .add_systems(Update, (manage_lifetime, play_samples))
        .run();
}

fn play_samples(
    mut local: Local<Option<Timer>>,
    time: Res<Time>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    let timer = match local.as_mut() {
        None => {
            *local = Some(Timer::from_seconds(1.0, TimerMode::Repeating));
            local.as_mut().unwrap()
        }
        Some(l) => l,
    };

    let delta = time.delta();
    if timer.tick(delta).just_finished() {
        commands.spawn((
            SamplePlayer::new(server.load("caw.ogg")),
            RandomPitch::new(0.1),
            Lifetime(Timer::from_seconds(0.4, TimerMode::Once)),
        ));
    }
}

// With this, we can clip off the sample at one caw.
#[derive(Component)]
struct Lifetime(Timer);

fn manage_lifetime(mut q: Query<(Entity, &mut Lifetime)>, time: Res<Time>, mut commands: Commands) {
    let delta = time.delta();
    for (entity, mut lifetime) in q.iter_mut() {
        if lifetime.0.tick(delta).just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
