//! Stream management for `cpal`.

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

use cpal::cpal::ErrorKind;

/// `bevy_seedling`'s `cpal` platform plugin.
///
/// This plugin spawns and manages a `cpal` audio stream.
#[derive(Debug, Default)]
pub struct CpalPlatformPlugin;

impl Plugin for CpalPlatformPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(all(feature = "web_audio", target_arch = "wasm32"))]
        if app
            .world()
            .contains_resource::<AudioStreamConfig<crate::platform::web_audio::WebAudioConfig>>()
        {
            return;
        }

        #[cfg(feature = "rtaudio")]
        if app
            .world()
            .contains_resource::<AudioStreamConfig<crate::platform::rtaudio::RtAudioConfig>>()
        {
            return;
        }

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
    mut context: ResMut<AudioContext>,
    stream_config: Res<AudioStreamConfig<CpalConfig>>,
    commands: Commands,
) -> Result {
    // TODO: it's not possible for the user to recover if this fails
    let sample_rate = context.with_store(|context, store| {
        let stream = cpal::CpalStream::new(context, stream_config.0.clone())?;
        let sample_rate = stream.info().sample_rate;

        let previous = store.insert(stream);
        debug_assert!(previous.is_none());

        Ok::<_, StartStreamError>(sample_rate)
    })?;

    super::initialize_stream(SampleRate::new(sample_rate), commands);

    Ok(())
}

fn poll_stream(mut context: ResMut<AudioContext>, mut commands: Commands) -> Result {
    let errors = context.with_store(|_, store| {
        store
            .get_mut::<cpal::CpalStream>()
            .map(|stream| stream.poll_status().collect::<Vec<_>>())
    });

    for error in errors.into_iter().flatten() {
        match error {
            IoStreamError::Input(error) => match error.kind() {
                // nothing to do here
                ErrorKind::DeviceChanged => {}
                ErrorKind::Xrun => {
                    warn!("audio input stream encountered underrun or overrun");
                }
                ErrorKind::StreamInvalidated | ErrorKind::DeviceNotAvailable => {
                    warn!("audio input stream stopped: {error:?}");
                }
                kind => match error.message() {
                    Some(message) => {
                        error!("audio input error: {message}");
                    }
                    None => {
                        error!("audio input error: {kind}");
                    }
                },
            },
            IoStreamError::Output(error) => match error.kind() {
                // nothing to do here
                ErrorKind::DeviceChanged => {}
                ErrorKind::Xrun => {
                    warn!("audio output stream encountered underrun or overrun");
                }
                ErrorKind::StreamInvalidated
                | ErrorKind::DeviceNotAvailable
                | ErrorKind::DeviceBusy
                | ErrorKind::HostUnavailable => {
                    warn!("audio stream stopped: {error:?}");
                    commands.trigger(RestartAudioStream);
                }
                kind => match error.message() {
                    Some(message) => {
                        error!("audio output error: {message}");
                    }
                    None => {
                        error!("audio output error: {kind}");
                    }
                },
            },
        }
    }

    Ok(())
}

fn observe_restart(_: On<RestartAudioStream>, mut config: ResMut<AudioStreamConfig<CpalConfig>>) {
    config.set_changed();
}

fn restart_stream(
    stream_config: Res<AudioStreamConfig<CpalConfig>>,
    mut graph: ResMut<AudioContext>,
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
