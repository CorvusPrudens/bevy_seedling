//! This example demonstrates the interaural time difference (ITD) node.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{nodes::itd::ItdNode, prelude::*};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
            TransformPlugin,
        ))
        .add_systems(Startup, startup)
        .add_systems(Update, spinner)
        .run();
}

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    // Here we spawn a sample player with a spatial effect,
    // making sure our sample player entity has a transform.
    commands.spawn((
        SamplePlayer::new(server.load("caw.ogg")).looping(),
        Transform::default(),
        sample_effects![
            ItdNode {
                ..Default::default()
            },
            SpatialBasicNode {
                // This should make the panning obvious.
                panning_threshold: 0.8,
                ..Default::default()
            }
        ],
    ));

    // Then, we'll spawn a simple listener that just circles the emitter.
    //
    // `Transform` is a required component of `SpatialListener2D`, so we
    // don't have to explicitly insert one.
    commands.spawn((SpatialListener2D, Spinner(0.0)));
}

#[derive(Component)]
struct Spinner(f32);

fn spinner(mut spinners: Query<(&mut Spinner, &mut Transform), With<Spinner>>, time: Res<Time>) {
    for (mut spinner, mut transform) in spinners.iter_mut() {
        let spin_radius = 2.0;
        let spin_seconds = 5.0;

        let position =
            Vec2::new(spinner.0.cos() * spin_radius, spinner.0.sin() * spin_radius).extend(0.0);

        transform.translation = position;

        spinner.0 += core::f32::consts::TAU * time.delta().as_secs_f32() / spin_seconds;
    }
}
