//! This example demonstrates how to slightly randomize
//! the pitch of samples.
//!
//! This is a common technique to break up the monotony of
//! frequently played sounds like footsteps.

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
        .add_systems(
            Update,
            (
                manage_lifetime,
                play_samples.run_if(on_timer(Duration::from_millis(750))),
            ),
        )
        .run();
}

fn play_samples(server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        SamplePlayer::new(server.load("caw.ogg")),
        RandomPitch::new(0.15),
        Lifetime(Timer::from_seconds(0.4, TimerMode::Once)),
    ));
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
