//! A DSP clock.

use std::time::Duration;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_time::{Time, TimeSystem};
use firewheel::clock::InstantSeconds;

use crate::context::AudioContext;

pub(crate) struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Time<Audio>>()
            .add_systems(First, update_time.in_set(TimeSystem));
    }
}

/// The current time of the audio context.
///
/// This can be used for precise scheduling.
/// The time is compensated, so it doesn't need to wait
/// for the audio context to advance.
#[derive(Debug, Default)]
pub struct Audio {
    instant: InstantSeconds,
}

impl Audio {
    /// Get the underlying [`InstantSeconds`] of the clock.
    pub fn instant(&self) -> InstantSeconds {
        self.instant
    }
}

fn update_time(mut time: ResMut<Time<Audio>>, context: Option<ResMut<AudioContext>>) {
    let Some(mut context) = context else {
        return;
    };

    let last = time.context().instant;
    let now = context.now();
    let delta = (now.seconds.0 - last.0).max(0.0);
    let delta = Duration::from_secs_f64(delta);
    time.advance_by(delta);
    time.context_mut().instant = now.seconds;
}
