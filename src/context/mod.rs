//! Glue code for interfacing with the underlying audio context.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_platform::sync;
use firewheel::{FirewheelConfig, FirewheelContext, clock::AudioClock};
use std::num::NonZeroU32;

pub mod graph;

pub(crate) struct ContextPlugin;

impl Plugin for ContextPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioContextConfig>()
            .add_plugins(graph::GraphPlugin)
            .add_systems(PreStartup, initialize_context);
    }
}

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
use web::InnerContext;

#[cfg(not(target_arch = "wasm32"))]
mod os;
#[cfg(not(target_arch = "wasm32"))]
use os::InnerContext;

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
#[derive(Debug, Resource, Component)]
pub struct AudioContext(InnerContext);

impl AudioContext {
    /// Create the audio context.
    ///
    /// This will not start a stream.
    pub fn new(settings: FirewheelConfig) -> Self {
        AudioContext(InnerContext::new(settings))
    }

    /// Get an absolute timestamp from the audio thread of the current time.
    ///
    /// This can be used to generate precisely-timed events.
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn mute_all(mut q: Query<&mut BandPassNode>, mut context: ResMut<AudioContext>) {
    ///     let now = context.now().seconds;
    ///     for mut filter in q.iter_mut() {
    ///         filter
    ///             .frequency
    ///             .push_curve(
    ///                 0.,
    ///                 now,
    ///                 now + DurationSeconds(1.),
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
    pub fn now(&mut self) -> AudioClock {
        self.with(|c| c.audio_clock_corrected())
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
    ///     let stream_info = context.with(|context| context.stream_info().cloned());
    /// }
    /// ```
    pub fn with<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut FirewheelContext) -> O + Send,
        O: Send + 'static,
    {
        self.0.with(f)
    }
}

/// Provides the [`AudioContext`] its [`FirewheelConfig`].
#[derive(Resource, Default, Debug)]
pub struct AudioContextConfig(pub FirewheelConfig);

/// Provides the current audio sample rate.
///
/// This resource becomes available after [`SeedlingStartupSystems::StreamInitialization`]
/// in [`PostStartup`]. Internally, the resource is atomically synchronized,
/// so this can't be used for detecting changes in the sample rate.
///
/// [`SeedlingStartupSystems::StreamInitialization`]:
/// crate::configuration::SeedlingStartupSystems::StreamInitialization
/// [`PostStartup`]: bevy_app::prelude::PostStartup
#[derive(Resource, Debug, Clone)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SampleRate(pub(crate) sync::Arc<sync::atomic::AtomicU32>);

impl SampleRate {
    /// Construct a new, shared [`SampleRate`].
    pub fn new(rate: NonZeroU32) -> Self {
        Self(sync::Arc::new(sync::atomic::AtomicU32::new(rate.get())))
    }

    /// Get the current sample rate.
    pub fn get(&self) -> NonZeroU32 {
        self.0
            .load(sync::atomic::Ordering::Relaxed)
            .try_into()
            .unwrap()
    }

    /// Set the current sample rate.
    pub fn set(&self, rate: NonZeroU32) {
        self.0.store(rate.get(), sync::atomic::Ordering::Relaxed)
    }
}

fn initialize_context(firewheel_config: Res<AudioContextConfig>, mut commands: Commands) -> Result {
    let context = AudioContext::new(firewheel_config.0);
    commands.insert_resource(context);

    Ok(())
}

/// An event triggered when the audio stream first initializes.
#[derive(Event, Debug)]
pub struct StreamStartEvent {
    /// The sample rate of the initialized stream.
    pub sample_rate: NonZeroU32,
}

/// An event triggered just before the audio stream restarts.
///
/// This allows components to temporarily store any state
/// that may be lost if sample rates or other parameters change.
#[derive(Event, Debug)]
pub struct PreStreamRestartEvent;

/// Bookkeepig for pre-restart behavior.
///
/// This should be called by custom backend immediately before
/// restarting a stream.
pub fn pre_restart_stream(mut commands: Commands) {
    commands.trigger(PreStreamRestartEvent);
}

/// An event triggered when the audio stream restarts.
#[derive(Event, Debug)]
pub struct StreamRestartEvent {
    /// The sample rate before the restart, which may or may not match
    /// [`current_rate`][StreamRestartEvent::current_rate].
    pub previous_rate: NonZeroU32,
    /// The current sample rate following the restart.
    pub current_rate: NonZeroU32,
}
