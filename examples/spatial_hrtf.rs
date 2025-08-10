//! This example demonstrates how to use the HRTF spatial node.
//!
//! An HRTF (head-related transfer function) is a technique for
//! simulating the effect our body has on sounds arriving to our ears.
//! It applies some filter and left/right delay, which should allow
//! you to distinguish not just left from right, but up from down and
//! front from back.
//!
//! HRTFs are fairly expensive, so be careful with the number of
//! actively processing nodes you spawn.

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
            TransformPlugin,
            SeedlingPlugin::default(),
        ))
        .add_systems(Startup, startup)
        .add_systems(Update, spinner)
        .run();
}

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    // Here we spawn a sample player with the HRTF spatial effect,
    // making sure our sample player entity has a transform.
    //
    // This emitter will slowly circle the listener clockwise.
    commands.spawn((
        Spinner(0.0),
        SamplePlayer::new(server.load("divine_comedy.ogg")).looping(),
        sample_effects![HrtfNode::default()],
        // We specifically initialize the transform here because we
        // may otherwise hear a small pop. Our current HRTF is
        // sensitive to sudden, large changes in position.
        Transform::from_xyz(2.0, 0.0, 0.0),
    ));

    // Then, we'll spawn a simple listener.
    //
    // `Transform` is a required component of `SpatialListener3D`, so we
    // don't have to explicitly insert one.
    commands.spawn(SpatialListener3D);
}

#[derive(Component)]
struct Spinner(f32);

fn spinner(mut spinners: Query<(&mut Spinner, &mut Transform), With<Spinner>>, time: Res<Time>) {
    for (mut spinner, mut transform) in spinners.iter_mut() {
        let spin_radius = 2.0;
        let spin_seconds = 15.0;

        let position = Vec2::new(spinner.0.cos() * spin_radius, spinner.0.sin() * spin_radius);

        transform.translation = Vec3::new(position.x, 0.0, position.y);

        spinner.0 += core::f32::consts::TAU * time.delta().as_secs_f32() / spin_seconds;
    }
}
