//! Stream management for `cpal`.

use alloc::vec::Vec;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::{error, warn};
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

fn start_stream(
    mut context: AudioContext,
    stream_config: Res<AudioStreamConfig<CpalConfig>>,
    commands: Commands,
) -> Result {
    // TODO: it's not possible for the user to recover if this fails
    let sample_rate = context.with_store(|context, store| {
        #[allow(unused_mut)]
        let mut stream_config = stream_config.0.clone();

        #[cfg(all(feature = "web_audio", target_arch = "wasm32"))]
        {
            // if no host has been specified, we'll choose the worklet host
            if stream_config.output.host.is_none() {
                match cpal::cpal::host_from_id(HostId::AudioWorklet) {
                    Ok(host) => {
                        stream_config.output.host = Some(host.id());
                    }
                    Err(e) => {
                        bevy_log::warn!("Failed to acquire audioworklet host: {e}");
                    }
                }
            }
        }

        let stream = cpal::CpalStream::new(context, stream_config)?;
        let sample_rate = stream.info().sample_rate;

        let previous = store.insert(stream);
        debug_assert!(previous.is_none());

        Ok::<_, StartStreamError>(sample_rate)
    })?;

    super::initialize_stream(SampleRate::new(sample_rate), commands);

    Ok(())
}

fn poll_stream(mut context: AudioContext, mut commands: Commands) -> Result {
    let errors = context.with_store(|_, store| {
        store
            .get_mut::<cpal::CpalStream>()
            .map(|stream| stream.poll_status().collect::<Vec<_>>())
    });

    for e in errors.into_iter().flatten() {
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
    mut graph: AudioContext,
    sample_rate: Res<SampleRate>,
    mut commands: Commands,
) -> Result {
    // drop it like it's hot
    let current_rate = graph.with_store(|context, store| {
        let _ = store.remove::<cpal::CpalStream>();

        let stream = cpal::CpalStream::new(context, stream_config.0.clone())?;
        let sample_rate = stream.info().sample_rate;
        store.insert(stream);

        Ok::<_, StartStreamError>(sample_rate)
    })?;

    let previous_rate = sample_rate.get();
    sample_rate.set(current_rate);

    commands.trigger(StreamRestartEvent {
        previous_rate,
        current_rate,
    });

    Ok(())
}
