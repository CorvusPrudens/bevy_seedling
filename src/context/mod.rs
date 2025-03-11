//! Glue code for interfacing with the underlying audio context.

use bevy_ecs::prelude::*;
use bevy_log::error;
use firewheel::clock::ClockSeconds;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::AudioContext;

#[cfg(not(target_arch = "wasm32"))]
mod os;
#[cfg(not(target_arch = "wasm32"))]
pub use os::AudioContext;

impl AudioContext {
    /// Get an absolute timestamp from the audio thread of the current time.
    ///
    /// This can be used to generate precisely-timed events.
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::{AudioContext, lpf::LowPassNode};
    /// # use firewheel::clock::ClockSeconds;
    /// fn mute_all(mut q: Query<&mut LowPassNode>, mut context: ResMut<AudioContext>) {
    ///     let now = context.now();
    ///     for mut filter in q.iter_mut() {
    ///         filter.
    ///             frequency
    ///             .push_curve(
    ///                 0.,
    ///                 now,
    ///                 now + ClockSeconds(1.),
    ///                 EaseFunction::ExponentialOut,
    ///             )
    ///             .unwrap();
    ///     }
    /// }
    /// ```
    pub fn now(&mut self) -> ClockSeconds {
        self.with(|c| c.clock_now())
    }
}

pub(crate) fn update_context(mut context: ResMut<AudioContext>) {
    context.with(|context| {
        if let Err(e) = context.update() {
            error!("graph error: {:?}", e);
        }
    });
}
