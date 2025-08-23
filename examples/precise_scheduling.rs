//! This example demonstrates precise audio event scheduling.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{
    node::events::{AudioEvents, VolumeFade},
    prelude::*,
};
use bevy_time::common_conditions::once_after_delay;
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((
            // Without a window, the event loop tends to run quite fast.
            // We'll slow it down so we don't drop any audio events.
            // MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16))),
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(
            PostStartup,
            // Running after stream initialization will give us access to the
            // correct `Time<Audio>` value.
            startup.after(SeedlingStartupSystems::StreamInitialization),
        )
        .add_systems(
            Update,
            fade_out.run_if(once_after_delay(Duration::from_secs_f32(72.0))),
        )
        .add_observer(on_complete)
        .run();
}

fn startup(server: Res<AssetServer>, time: Res<Time<Audio>>, mut commands: Commands) {
    let mut events = AudioEvents::new(&time);

    let settings = PlaybackSettings::default();
    // Depending on how long it takes your system to load the asset,
    // some of the beginning may get cut off. This ensures any simultaneously
    // scheduled events, like the loop point, still occur at exactly the
    // right time.
    settings.play_at(None, time.now(), &mut events);

    // This music has an 11 second intro and lasts about 60 seconds.
    // With precise scheduling, we can make it loop seamlessly while
    // skipping the intro.
    settings.play_at(
        Some(Playhead::Seconds(time.delay(DurationSeconds(11.443)).0)),
        time.delay(DurationSeconds(70.125)),
        &mut events,
    );

    commands.spawn((
        events,
        settings,
        SamplePlayer::new(server.load("midir-chip.ogg")).with_volume(Volume::Decibels(-8.0)),
        sample_effects![fade_in(2.5, &time)],
    ));
}

// Here's how you might build a composable fade in function.
fn fade_in(seconds: f32, time: &Time<Audio>) -> impl Bundle {
    let mut events = AudioEvents::new(time);
    let volume = VolumeNode {
        volume: Volume::SILENT,
        ..Default::default()
    };

    volume.fade_to(
        Volume::UNITY_GAIN,
        DurationSeconds(seconds as f64),
        &mut events,
    );

    (volume, events)
}

fn fade_out(
    player: Single<(&SampleEffects, &PlaybackSettings, &mut AudioEvents)>,
    mut volume: Query<(&VolumeNode, &mut AudioEvents), Without<SampleEffects>>,
    time: Res<Time<Audio>>,
) -> Result {
    let (effects, settings, mut settings_events) = player.into_inner();
    let (volume, mut volume_events) = volume.get_effect_mut(effects)?;

    // Fade out with a little pitch falloff, then stop the music.
    let fade_duration = DurationSeconds(5.0);
    volume.fade_to(Volume::SILENT, fade_duration, &mut volume_events);
    settings.speed_to(0.95, fade_duration, &mut settings_events);
    settings.stop_at(time.delay(fade_duration), &mut settings_events);

    Ok(())
}

fn on_complete(_: Trigger<PlaybackCompletionEvent>) {
    info!("Playback complete!");
}
