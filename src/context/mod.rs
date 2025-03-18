//! Glue code for interfacing with the underlying audio context.

use bevy_ecs::prelude::*;
use bevy_log::error;
use firewheel::{backend::AudioBackend, clock::ClockSeconds, FirewheelConfig};

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
use web::InnerContext;

#[cfg(not(target_arch = "wasm32"))]
mod os;
#[cfg(not(target_arch = "wasm32"))]
use os::InnerContext;

mod seedling_context;

pub use seedling_context::{
    ErasedNode, SeedlingContext, SeedlingContextError, SeedlingContextWrapper,
};

/// A thread-safe wrapper around the underlying Firewheel audio context.
///
/// After the seedling plugin is initialized, this can be accessed as a resource.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// fn system(mut context: ResMut<AudioContext>) {
///     context.with(|c| {
///         // ...
///     });
/// }
/// ```
#[derive(Debug, Resource)]
pub struct AudioContext(InnerContext);

impl AudioContext {
    /// Initialize the audio process.
    pub fn new<B>(settings: FirewheelConfig, stream_settings: B::Config) -> Self
    where
        B: AudioBackend + 'static,
        B::Config: Send + 'static,
        B::StreamError: Send + Sync + 'static,
    {
        AudioContext(InnerContext::new::<B>(settings, stream_settings))
    }

    /// Get an absolute timestamp from the audio thread of the current time.
    ///
    /// This can be used to generate precisely-timed events.
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn mute_all(mut q: Query<&mut LowPassNode>, mut context: ResMut<AudioContext>) {
    ///     let now = context.now();
    ///     for mut filter in q.iter_mut() {
    ///         filter
    ///             .frequency
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
    ///
    /// Depending on the target platform, this operation can
    /// have moderate overhead. It should not be called
    /// more than once per system.
    pub fn now(&mut self) -> ClockSeconds {
        self.with(|c| c.clock_now())
    }

    /// Operate on the underlying audio context.
    ///
    /// In multi-threaded contexts, this sends `f` to the underlying control thread,
    /// blocking until `f` returns.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn system(mut context: ResMut<AudioContext>) {
    ///     let input_devices = context.with(|context| context.available_input_devices());
    /// }
    /// ```
    pub fn with<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut SeedlingContext) -> O + Send,
        O: Send + 'static,
    {
        self.0.with(f)
    }
}

pub(crate) fn update_context(mut context: ResMut<AudioContext>) {
    context.with(|context| {
        if let Err(e) = context.update() {
            error!("graph error: {:?}", e);
        }
    });
}
