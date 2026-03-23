//! Stream management for `cpal`.

use bevy_app::prelude::*;
use bevy_asset::AssetServer;
use bevy_ecs::prelude::*;
use bevy_log::{error, warn};
use bevy_platform::cell::SyncCell;
use firewheel::cpal::{self};

use crate::{
    SeedlingSystems,
    context::{AudioContext, SampleRate, StreamRestartEvent},
    platform::*,
    prelude::SeedlingStartupSystems,
    resource_changed_without_insert,
};

pub use firewheel::cpal::*;

/// `bevy_seedling`'s `cpal` platform plugin.
///
/// This plugin spawns and manages a `cpal` audio stream.
#[derive(Debug, Default)]
pub struct CpalPlatformPlugin;

impl Plugin for CpalPlatformPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioStreamConfig<CpalConfig>>()
            .add_systems(
                PostStartup,
                start_stream.in_set(SeedlingStartupSystems::StreamInitialization),
            )
            .add_systems(
                PostUpdate,
                (crate::context::pre_restart_stream, restart_stream)
                    .chain()
                    .run_if(resource_changed_without_insert::<AudioStreamConfig<CpalConfig>>),
            )
            .add_systems(Last, poll_stream.in_set(SeedlingSystems::PollStream))
            .add_observer(observe_restart);
    }
}

/// A wrapper around [`CpalStream`][cpal::CpalStream], safely
/// implementing `Sync`.
#[derive(Resource)]
pub struct CpalStream(Option<SyncCell<cpal::CpalStream>>);

impl CpalStream {
    /// Construct a new, inhabited [`CpalStream`].
    pub fn new(stream: cpal::CpalStream) -> Self {
        Self(Some(SyncCell::new(stream)))
    }

    /// Returns a mutable reference to [`CpalStream`][cpal::CpalStream].
    ///
    /// This is the only way to access the stream.
    pub fn get(&mut self) -> &mut cpal::CpalStream {
        self.0
            .as_mut()
            .expect("`CpalStream` should never be `None`")
            .get()
    }

    fn take(&mut self) -> Option<cpal::CpalStream> {
        self.0.take().map(SyncCell::to_inner)
    }

    fn replace(&mut self, stream: cpal::CpalStream) -> Option<cpal::CpalStream> {
        let old = self.0.take().map(SyncCell::to_inner);
        self.0 = Some(SyncCell::new(stream));
        old
    }
}

impl core::fmt::Debug for CpalStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpalStream").finish_non_exhaustive()
    }
}

fn start_stream(
    mut context: ResMut<AudioContext>,
    stream_config: Res<AudioStreamConfig<CpalConfig>>,
    server: Res<AssetServer>,
    mut commands: Commands,
) -> Result {
    // TODO: it's not possible for the user to recover if this fails
    let stream = context.with(|context| cpal::CpalStream::new(context, stream_config.0.clone()))?;

    let sample_rate = SampleRate::new(stream.info().sample_rate);
    commands.insert_resource(CpalStream::new(stream));

    super::initialize_stream(sample_rate, &server, commands);

    Ok(())
}

fn poll_stream(mut stream: ResMut<CpalStream>, mut commands: Commands) -> Result {
    let stream = stream.get();
    if let Err(e) = stream.poll_status() {
        match e {
            StreamError::StreamInvalidated | StreamError::DeviceNotAvailable => {
                warn!("Audio stream stopped: {e:?}");
                commands.trigger(RestartAudioStream);
            }
            StreamError::BufferUnderrun => {
                warn!("audio stream encountered underrun");
            }
            StreamError::BackendSpecific { err } => {
                error!("unexpected audio backend error: {err}");
            }
        }
    }

    Ok(())
}

fn observe_restart(_: On<RestartAudioStream>, mut config: ResMut<AudioStreamConfig<CpalConfig>>) {
    config.set_changed();
}

fn restart_stream(
    stream_config: Res<AudioStreamConfig<CpalConfig>>,
    mut stream: ResMut<CpalStream>,
    mut graph: ResMut<AudioContext>,
    sample_rate: Res<SampleRate>,
    mut commands: Commands,
) -> Result {
    // drop it like it's hot
    let _ = stream.take();

    let new_stream =
        graph.with(|context| cpal::CpalStream::new(context, stream_config.0.clone()))?;

    let previous_rate = sample_rate.get();
    let current_rate = new_stream.info().sample_rate;
    sample_rate.set(current_rate);

    stream.replace(new_stream);
    commands.trigger(StreamRestartEvent {
        previous_rate,
        current_rate,
    });

    Ok(())
}
