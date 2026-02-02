//! This example demonstrates seamless playback of multiple samples.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{context::SampleRate, prelude::*};

const SAMPLE_COUNT: usize = 7;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(
            PostStartup,
            preload_assets.after(SeedlingStartupSystems::StreamInitialization),
        )
        .add_systems(Update, (check_assets_ready, queue_next_sample).chain())
        .add_observer(on_sample_removed)
        .run();
}

#[derive(Resource)]
struct PreloadedSamples {
    handles: Vec<Handle<AudioSample>>,
}

#[derive(Component)]
struct MusicSample {
    index: usize,
    scheduled_start: InstantSeconds,
}

#[derive(Component)]
struct NextSampleQueued;

#[derive(Component)]
struct AssetsNotReady;

fn preload_assets(server: Res<AssetServer>, mut commands: Commands) {
    let handles: Vec<Handle<AudioSample>> = (0..SAMPLE_COUNT)
        .map(|i| server.load(format!("selfless_courage_{:02}.wav", i)))
        .collect();

    commands.insert_resource(PreloadedSamples { handles });
    commands.spawn(AssetsNotReady);
}

fn check_assets_ready(
    mut commands: Commands,
    preloaded: Res<PreloadedSamples>,
    audio_samples: Res<Assets<AudioSample>>,
    time: Res<Time<Audio>>,
    query: Query<Entity, With<AssetsNotReady>>,
) {
    let Ok(entity) = query.single() else {
        return;
    };

    // Check if all assets are loaded.
    for handle in &preloaded.handles {
        if audio_samples.get(handle).is_none() {
            return;
        }
    }

    commands.entity(entity).despawn();

    let mut events = AudioEvents::new(&time);
    let settings = PlaybackSettings::default()
        .with_playback(false)
        .with_on_complete(OnComplete::Despawn);

    // Wait 3s to give some breathing room for the example to start.
    let scheduled_start = time.delay(DurationSeconds(3.0));
    settings.play_at(None, scheduled_start, &mut events);

    commands.spawn((
        MusicSample {
            index: 0,
            scheduled_start,
        },
        events,
        settings,
        SamplePlayer::new(preloaded.handles[0].clone()),
    ));

    info!(
        "All assets loaded. First sample scheduled to play at {:.3}s",
        scheduled_start.0
    );
}

fn queue_next_sample(
    mut commands: Commands,
    preloaded: Res<PreloadedSamples>,
    audio_samples: Res<Assets<AudioSample>>,
    sample_rate: Res<SampleRate>,
    time: Res<Time<Audio>>,
    sample_query: Query<(Entity, &SamplePlayer, &MusicSample), Without<NextSampleQueued>>,
) {
    const PRE_SPAWN_THRESHOLD_SECONDS: f64 = 0.15;

    for (entity, sample_player, music_sample) in sample_query.iter() {
        if music_sample.index >= SAMPLE_COUNT - 1 {
            continue;
        }

        let Some(sample_asset) = audio_samples.get(&sample_player.sample) else {
            continue;
        };

        let sample_resource = sample_asset.get();
        let sample_duration_seconds =
            sample_resource.len_frames() as f64 / sample_rate.get().get() as f64;

        let scheduled_end =
            InstantSeconds(music_sample.scheduled_start.0 + sample_duration_seconds);
        let time_until_end = scheduled_end.0 - time.now().0;

        if time_until_end <= PRE_SPAWN_THRESHOLD_SECONDS && time_until_end > 0.0 {
            let next_index = music_sample.index + 1;

            let mut events = AudioEvents::new(&time);
            let settings = PlaybackSettings::default()
                .with_playback(false)
                .with_on_complete(OnComplete::Despawn);
            settings.play_at(None, scheduled_end, &mut events);

            commands.entity(entity).insert(NextSampleQueued);

            commands.spawn((
                MusicSample {
                    index: next_index,
                    scheduled_start: scheduled_end,
                },
                events,
                settings,
                SamplePlayer::new(preloaded.handles[next_index].clone()),
            ));

            info!(
                "Queued sample {} at {:.3}s (current time: {:.3}s, time until end: {:.3}s)",
                next_index,
                scheduled_end.0,
                time.now().0,
                time_until_end
            );
        }
    }
}

fn on_sample_removed(trigger: On<Remove, MusicSample>, query: Query<&MusicSample>) {
    let Ok(music_sample) = query.get(trigger.entity) else {
        return;
    };

    info!("Sample {} finished playing", music_sample.index);

    if music_sample.index >= SAMPLE_COUNT - 1 {
        info!("All samples completed!");
    }
}
