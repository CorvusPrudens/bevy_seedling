//! This example demonstrates sample lifetimes.

use bevy::{
    ecs::{component::ComponentId, world::DeferredWorld},
    log::LogPlugin,
    prelude::*,
};
use bevy_seedling::prelude::*;
use std::time::Duration;

#[derive(Component)]
#[component(on_remove = on_remove)]
struct OnFinished;

fn on_remove(mut world: DeferredWorld, _: Entity, _: ComponentId) {
    info!("One-shot sample finished!");

    world.send_event(PlayEvent);
}

#[derive(Event)]
struct PlayEvent;

#[derive(Component)]
struct LoopingRemover {
    timer: Timer,
    sample: Entity,
}

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(Startup, startup)
        .add_systems(Update, (play_event, remove_looping))
        .add_event::<PlayEvent>()
        .run();
}

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    // The default playback settings (a required component of `SamplePlayer`)
    // will cause the sample to play once, despawning the entity when complete.
    commands.spawn((
        SamplePlayer::new(server.load("snd_wobbler.wav")),
        OnFinished,
    ));
}

fn play_event(
    mut reader: EventReader<PlayEvent>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    for _ in reader.read() {
        // A looping sample, on the other hand, will continue
        // playing indefinitely until the sample entity is despawned.
        let sample = commands
            .spawn((
                SamplePlayer::new(server.load("snd_wobbler.wav")),
                PlaybackSettings::LOOP,
            ))
            .id();

        // Here we kick off a timer that will remove the looping sample, stopping playback.
        commands.spawn(LoopingRemover {
            timer: Timer::new(Duration::from_secs(3), TimerMode::Once),
            sample,
        });
    }
}

fn remove_looping(
    mut q: Query<(Entity, &mut LoopingRemover)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut remover) in q.iter_mut() {
        remover.timer.tick(time.delta());

        if remover.timer.just_finished() {
            info!("Stopping looping sample...");

            commands.entity(remover.sample).despawn();
            commands.entity(e).despawn();
        }
    }
}
