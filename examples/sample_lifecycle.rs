//! This example demonstrates sample lifetimes.

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
        .add_systems(Update, remove_all.run_if(on_timer(Duration::from_secs(7))))
        .add_observer(on_finished)
        .run();
}

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    // The default playback settings (a required component of `SamplePlayer`)
    // will cause the sample to play once, despawning the entity when complete.
    commands.spawn((SamplePlayer::new(server.load("caw.ogg")), OnFinished));
}

#[derive(Component)]
struct OnFinished;

// When the sample above completes, `OnRemove` will be triggered on all its
// components when it gets despawned.
fn on_finished(_: Trigger<OnRemove, OnFinished>, server: Res<AssetServer>, mut commands: Commands) {
    info!("One-shot sample finished, playing looped sample!");

    // A looping sample, on the other hand, will continue
    // playing indefinitely until the sample entity is paused, stopped, or despawned.
    commands.spawn(SamplePlayer::new(server.load("caw.ogg")).looping());
}

fn remove_all(mut q: Query<Entity, With<SamplePlayer>>, mut commands: Commands) {
    for sample in q.iter_mut() {
        info!("Stopping all samples...");
        commands.entity(sample).despawn();
    }
}
