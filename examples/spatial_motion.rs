//! This example demonstrates spatial motion effects: Doppler shift and
//! initial propagation delay.

use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use bevy_seedling::{nodes::itd::ItdNode, prelude::*};
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16))),
            LogPlugin::default(),
            AssetPlugin::default(),
            TransformPlugin,
            SeedlingPlugins,
        ))
        .insert_resource(SpeedOfSound(100.0)) // slower speed of sound to exaggerate the effect
        .add_systems(Startup, startup)
        .add_systems(Update, spinner)
        .run();
}

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        Spinner,
        SamplePlayer::new(server.load("divine_comedy.ogg")).looping(),
        Transform::default(),
        sample_effects![SpatialBasicNode::default(), ItdNode::default()],
        PlaybackSettings::default(),
        Doppler::default(),
        PropagationDelay,
    ));

    commands.spawn(SpatialListener2D);
}

#[derive(Component)]
struct Spinner;

fn spinner(mut spinners: Query<&mut Transform, With<Spinner>>, time: Res<Time>) {
    for mut transform in spinners.iter_mut() {
        let spin_radius = 8.0;

        let t = time.elapsed().as_secs_f32() * 3.0;

        let center = Vec3::new(spin_radius / 1.5, 0.0, 0.0);
        let position =
            Vec2::new(t.cos() * spin_radius, t.sin() * spin_radius).extend(0.0);

        transform.translation = position + center;
    }
}
