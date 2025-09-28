//! This example demonstrates how to set up a
//! crossfade between two samples.

use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use bevy_seedling::prelude::*;
use bevy_time::common_conditions::once_after_delay;
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
        .add_systems(Startup, startup)
        .add_systems(
            Update,
            crossfade.run_if(once_after_delay(Duration::from_secs(1))),
        )
        .run();
}

#[derive(Component)]
struct MusicA;

#[derive(Component)]
struct MusicB;

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        MusicPool, // spawned in the default configuration
        MusicA,
        SamplePlayer::new(server.load("selfless_courage.ogg")),
    ));

    commands.spawn((
        MusicPool,
        MusicB,
        SamplePlayer::new(server.load("midir-chip.ogg")),
        // Each sampler in the music pool has a volume node.
        // We'll initialize this one to zero.
        sample_effects![VolumeNode {
            volume: Volume::SILENT,
            ..default()
        },],
    ));
}

fn crossfade(
    music_a: Single<&SampleEffects, With<MusicA>>,
    music_b: Single<&SampleEffects, With<MusicB>>,
    mut volume_nodes: Query<(&VolumeNode, &mut AudioEvents)>,
) -> Result {
    let fade_duration = DurationSeconds(5.0);

    // fade out A
    let (volume, mut events) = volume_nodes.get_effect_mut(&music_a)?;
    volume.fade_to(Volume::SILENT, fade_duration, &mut events);

    // fade in B
    let (volume, mut events) = volume_nodes.get_effect_mut(&music_b)?;
    volume.fade_to(Volume::UNITY_GAIN, fade_duration, &mut events);

    Ok(())
}
