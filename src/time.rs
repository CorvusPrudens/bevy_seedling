//! A DSP clock.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_time::{Time, TimeSystem};
use firewheel::clock::{DurationSeconds, InstantSeconds};
use std::time::Duration;

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

/// A trait that provides a frame's audio render range.
///
/// This can be used for rendering timeline events with [`AudioEvents`].
///
/// [`AudioEvents`]: crate::prelude::AudioEvents
pub trait AudioTime {
    /// Get the audio processing thread's compensated current time.
    ///
    /// This instant is measured from the moment the audio thread begins processing,
    /// monotonically counting up. Note that this instant is updated once per frame in the
    /// [`First`] schedule, meaning it may slip behind the audio processing later in the
    /// frame. If you need more precision, prefer reading the exact time from [`AudioContext`].
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn scheduling(
    ///     main: Single<(&VolumeNode, &mut AudioEvents), With<MainBus>>,
    ///     time: Res<Time<Audio>>,
    /// ) {
    ///     // fade out the main bus, silencing all sound
    ///     let (volume, mut events) = main.into_inner();
    ///     volume.fade_at(
    ///         Volume::SILENT,
    ///         time.now(),
    ///         time.delay(DurationSeconds(2.5)),
    ///         &mut events,
    ///     );
    /// }
    /// ```
    fn now(&self) -> InstantSeconds;

    /// Calculate an instant delayed from [`AudioTime::now`] by `duration`.
    ///
    /// Equivalent to
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// # fn delay(duration: DurationSeconds, time: Res<Time<AudioTime>>) -> InstantSeconds {
    /// time.now() + duration
    /// # }
    /// ```
    fn delay(&self, duration: DurationSeconds) -> InstantSeconds;

    /// A frame's audio render range.
    ///
    /// This describes the time elapsed since the last frame from
    /// the perspective of the audio thread.
    fn render_range(&self) -> core::ops::Range<InstantSeconds>;
}

impl AudioTime for Time<Audio> {
    fn now(&self) -> InstantSeconds {
        self.context().instant()
    }

    fn delay(&self, duration: DurationSeconds) -> InstantSeconds {
        self.now() + duration
    }

    fn render_range(&self) -> core::ops::Range<InstantSeconds> {
        let now = self.context().instant();
        let last = self.delta_secs_f64();

        InstantSeconds(now.0 - last)..now
    }
}
