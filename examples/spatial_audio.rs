//! This example demonstrates how to use spatial audio.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin {
                level: bevy_log::Level::DEBUG,
                ..Default::default()
            },
            AssetPlugin::default(),
            SeedlingPlugin::default(),
            TransformPlugin,
        ))
        .add_systems(Startup, startup)
        .add_systems(Update, spinner)
        .run();
}

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    #[derive(PoolLabel, Clone, Debug, Hash, PartialEq, Eq)]
    struct MyPool;

    // Here we spawn a pool with a custom label and
    // insert a spatial audio node as a per-sampler effect.
    Pool::new(MyPool, 4)
        .effect(SpatialBasicNode::default())
        .spawn(&mut commands);

    // To play a sound in this pool, we can simply spawn a sample
    // player with the pool label, making sure the entity
    // has a transform.
    commands.spawn((
        MyPool,
        SamplePlayer::new(server.load("snd_wobbler.wav")),
        PlaybackSettings::LOOP,
        // Both the emitter and listener need transforms
        // for spatial information to propagate.
        Transform::default(),
    ));

    // Finally, we'll spawn a simple listener that just circles the emitter.
    commands.spawn((SpatialListener2D, Spinner(0.0), Transform::default()));
}

#[derive(Component)]
struct Spinner(f32);

fn spinner(mut spinners: Query<(&mut Spinner, &mut Transform), With<Spinner>>, time: Res<Time>) {
    for (mut spinner, mut transform) in spinners.iter_mut() {
        let spin_radius = 5.0;
        let spin_seconds = 4.0;

        let position =
            Vec2::new(spinner.0.cos() * spin_radius, spinner.0.sin() * spin_radius).extend(0.0);

        transform.translation = position;

        spinner.0 += core::f32::consts::TAU * time.delta().as_secs_f32() / spin_seconds;
    }
}
