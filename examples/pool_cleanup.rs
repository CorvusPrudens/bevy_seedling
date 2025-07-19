//! This example demonstrates how to
//! create and remove a custom pool.

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
        .add_systems(Startup, startup)
        .add_systems(Update, remove_pool.run_if(on_timer(Duration::from_secs(5))))
        .run();
}

#[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
struct AmbiencePool;

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    // Here we spawn our custom ambience pool.
    commands.spawn(SamplerPool(AmbiencePool));

    // And we start playing our sample in the pool.
    commands.spawn((
        AmbiencePool,
        SamplePlayer::new(server.load("crow_ambience.ogg")).looping(),
    ));
}

fn remove_pool(mut commands: Commands) {
    info_once!("Cleaning up pool...");

    // This will remove the sampler and volume nodes
    // associated with this pool in both the ECS
    // and audio graph.
    commands.despawn_pool(AmbiencePool);
}
