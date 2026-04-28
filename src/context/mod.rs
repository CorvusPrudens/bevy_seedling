//! Glue code for interfacing with the underlying audio context.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_platform::sync;
use firewheel::{FirewheelConfig, FirewheelContext, clock::AudioClock};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    num::NonZeroU32,
};

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
    /// fn mute_all(
    ///     mut q: Query<(&FastBandpassNode, &mut AudioEvents)>,
    ///     mut context: ResMut<AudioContext>,
    /// ) {
    ///     let now = context.now().seconds;
    ///     for (filter, mut events) in q.iter_mut() {
    ///         // In exactly one second from now, set the cutoff to 0.
    ///         events.schedule(now + DurationSeconds(1.0), filter, |f| {
    ///             f.cutoff_hz = 0.0;
    ///         });
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
        self.with_store(|context, _| f(context))
    }

    pub(crate) fn with_store<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut FirewheelContext, &mut LocalStore) -> O + Send,
        O: Send + 'static,
    {
        self.0.with_store(f)
    }
}

pub(crate) struct AudioThreadState {
    context: FirewheelContext,
    store: LocalStore,
}

impl AudioThreadState {
    fn new(settings: FirewheelConfig) -> Self {
        Self {
            context: FirewheelContext::new(settings),
            store: LocalStore::default(),
        }
    }
}

#[cfg_attr(
    not(any(
        feature = "cpal",
        all(feature = "rtaudio", not(target_arch = "wasm32"))
    )),
    allow(dead_code)
)]
#[derive(Default)]
pub(crate) struct LocalStore(HashMap<TypeId, Box<dyn Any>>);

#[cfg_attr(
    not(any(
        feature = "cpal",
        all(feature = "rtaudio", not(target_arch = "wasm32"))
    )),
    allow(dead_code)
)]
impl LocalStore {
    pub(crate) fn insert<T: 'static>(&mut self, value: T) -> Option<T> {
        self.0
            .insert(TypeId::of::<T>(), Box::new(value))
            .map(|value| {
                *value
                    .downcast()
                    .expect("stored type should match its TypeId")
            })
    }

    pub(crate) fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.0.get_mut(&TypeId::of::<T>()).map(|value| {
            value
                .downcast_mut()
                .expect("stored type should match its TypeId")
        })
    }

    pub(crate) fn remove<T: 'static>(&mut self) -> Option<T> {
        self.0.remove(&TypeId::of::<T>()).map(|value| {
            *value
                .downcast()
                .expect("stored type should match its TypeId")
        })
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
/// [`SeedlingStartupSystems::StreamInitialization`]: graph::SeedlingStartupSystems::StreamInitialization
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
