//! This example demonstrates how to create a loop region.

use std::ops::Range;

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{pool::Sampler, prelude::*};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(
            Startup,
            |server: Res<AssetServer>, mut commands: Commands| {
                commands.spawn((
                    SamplePlayer::new(server.load("midir-chip.ogg")),
                    LoopRegion(8.391..11.437),
                ));
            },
        )
        .add_systems(Update, manage_loop_regions)
        .run();
}

/// Schedules playhead events to create a looping region in a `SamplePlayer`.
#[derive(Component)]
#[require(LastPosition)]
pub struct LoopRegion(pub Range<f64>);

/// Stores the last observed sample position.
#[derive(Component, Default)]
struct LastPosition(f64);

fn manage_loop_regions(
    samples: Query<(
        &LoopRegion,
        &PlaybackSettings,
        &Sampler,
        &mut LastPosition,
        &mut AudioEvents,
    )>,
    time: Res<Time<Audio>>,
) -> Result {
    for (region, settings, sampler, mut last, mut events) in samples {
        let sample_position = sampler.playhead_seconds();

        // Scheduling the playhead event once we're halfway through the loop
        // should ensure it's reliably observed (unless the loop duration is less than a frame).
        let mid_point = region.0.start + (region.0.end - region.0.start) * 0.5;

        if last.0 <= mid_point && sample_position.0 >= mid_point {
            let remaining_to_loop_point = (region.0.end - sample_position.0).max(0.0);

            // schedule new playhead event
            settings.play_at(
                Some(PlayFrom::Seconds(region.0.start)),
                time.delay(DurationSeconds(remaining_to_loop_point)),
                &mut events,
            );
        }

        last.0 = sample_position.0;
    }

    Ok(())
}
