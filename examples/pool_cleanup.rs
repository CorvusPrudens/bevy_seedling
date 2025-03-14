//! This example demonstrates how to
//! create and remove a custom pool.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{
    sample::{
        pool::{Pool, PoolCommands},
        PlaybackSettings, SamplePlayer,
    },
    PoolLabel, SeedlingPlugin,
};
use std::time::Duration;

#[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
struct CustomPool;

#[derive(Component)]
struct PoolRemover(Timer);

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(Startup, startup)
        .add_systems(Update, remove_pool)
        .run();
}

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    // Here we spawn our custom pool with four sampler nodes.
    Pool::new(CustomPool, 4).spawn(&mut commands);

    // And we start playing our sample in the custom pool.
    commands.spawn((
        SamplePlayer::new(server.load("snd_wobbler.wav")),
        PlaybackSettings::LOOP,
        CustomPool,
    ));

    commands.spawn(PoolRemover(Timer::new(
        Duration::from_secs(3),
        TimerMode::Once,
    )));
}

fn remove_pool(mut q: Query<(Entity, &mut PoolRemover)>, time: Res<Time>, mut commands: Commands) {
    for (e, mut remover) in q.iter_mut() {
        remover.0.tick(time.delta());

        if remover.0.just_finished() {
            info!("Cleaning up pool...");

            // This will remove the sampler and volume nodes
            // associated with this pool in both the ECS
            // and audio graph.
            commands.despawn_pool(CustomPool);

            commands.entity(e).despawn();
        }
    }
}
