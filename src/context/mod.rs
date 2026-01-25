//! Glue code for interfacing with the underlying audio context.

use bevy_asset::prelude::*;
use bevy_ecs::prelude::*;
use bevy_platform::sync;
use firewheel::{FirewheelConfig, FirewheelCtx, backend::AudioBackend, clock::AudioClock};
use std::num::NonZeroU32;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
use web::InnerContext;

#[cfg(not(target_arch = "wasm32"))]
mod os;
#[cfg(not(target_arch = "wasm32"))]
use os::InnerContext;

mod seedling_context;

pub use seedling_context::{SeedlingContext, SeedlingContextError, SeedlingContextWrapper};

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
    /// Create the audio context.
    ///
    /// This will not start a stream.
    pub fn new<B>(settings: FirewheelConfig) -> Self
    where
        B: AudioBackend + 'static,
        B::StreamError: Send + Sync + 'static,
    {
        AudioContext(InnerContext::new::<B>(settings))
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
pub struct SampleRate(sync::Arc<sync::atomic::AtomicU32>);

impl SampleRate {
    /// Get the current sample rate.
    pub fn get(&self) -> NonZeroU32 {
        self.0
            .load(sync::atomic::Ordering::Relaxed)
            .try_into()
            .unwrap()
    }
}

/// A [`Resource`] containing the audio context's stream configuration.
///
/// Mutating this resource will cause the audio stream to stop
/// and restart, applying the latest changes.
#[derive(Resource, Debug)]
pub struct AudioStreamConfig<B: AudioBackend = firewheel::cpal::CpalBackend>(pub B::Config);

pub(crate) fn initialize_context<B>(
    firewheel_config: crate::prelude::FirewheelConfig,
    commands: &mut Commands,
) -> Result
where
    B: AudioBackend + 'static,
    B::StreamError: Send + Sync + 'static,
{
    let context = AudioContext::new::<B>(firewheel_config);
    commands.insert_resource(context);

    Ok(())
}

pub(crate) fn start_stream<B>(
    config: Res<AudioStreamConfig<B>>,
    server: Res<AssetServer>,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
) -> Result
where
    B: AudioBackend + 'static,
    B::Config: Clone + Send + Sync + 'static,
{
    context.with(|context| {
        let context = context.downcast_mut::<FirewheelCtx<B>>().expect(
            "Attempted to initialize audio context with unexpected backend type. \
                    `bevy_seedling` expects a single context.",
        );
        context
            .start_stream(config.0.clone())
            .map_err(|e| format!("failed to start audio stream: {e:?}"))?;

        let raw_sample_rate = context.stream_info().unwrap().sample_rate;
        let sample_rate = SampleRate(sync::Arc::new(sync::atomic::AtomicU32::new(
            raw_sample_rate.get(),
        )));

        commands.insert_resource(sample_rate.clone());
        server.register_loader(crate::sample::SampleLoader { sample_rate });

        commands.trigger(StreamStartEvent {
            sample_rate: raw_sample_rate,
        });

        Ok(())
    })
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

pub(crate) fn pre_restart_context(mut commands: Commands) {
    commands.trigger(PreStreamRestartEvent);
}

/// An event triggered when the audio stream restarts.
#[derive(Event, Debug)]
pub struct StreamRestartEvent {
    /// The sample rate before the restart, which may or may not match.
    pub previous_rate: NonZeroU32,
    /// The current sample rate following the restart.
    pub current_rate: NonZeroU32,
}

pub(crate) fn restart_context<B>(
    stream_config: Res<AudioStreamConfig<B>>,
    mut commands: Commands,
    mut audio_context: ResMut<AudioContext>,
    sample_rate: Res<SampleRate>,
) -> Result
where
    B: AudioBackend + 'static,
    B::Config: Clone + Send + Sync + 'static,
    B::StreamError: Send + Sync + 'static,
{
    audio_context.with(|context| {
        let context: &mut FirewheelCtx<B> = context
            .downcast_mut()
            .ok_or("only one audio context should be active at a time")?;

        context.stop_stream();
        context
            .start_stream(stream_config.0.clone())
            .map_err(|e| format!("failed to restart audio stream: {e:?}"))?;

        let previous_rate = sample_rate.get();

        let current_rate = context.stream_info().unwrap().sample_rate;
        sample_rate
            .0
            .store(current_rate.get(), sync::atomic::Ordering::Relaxed);

        commands.trigger(StreamRestartEvent {
            previous_rate,
            current_rate,
        });

        Ok(())
    })
}
