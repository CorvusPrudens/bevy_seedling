//! Stream management for `rtaudio`.

use bevy_app::prelude::*;

/// `bevy_seedling`'s `rtaudio` platform plugin.
///
/// This plugin spawns and manages an `rtaudio` audio stream.
#[derive(Debug, Default)]
pub struct RtAudioPlatformPlugin;

#[cfg(not(target_arch = "wasm32"))]
pub use firewheel::rtaudio::*;

#[cfg(all(not(feature = "cpal"), not(target_arch = "wasm32")))]
mod inner {
    use super::*;
    use bevy_ecs::prelude::*;
    use bevy_log::{error, warn};
    use core::num::NonZeroU32;
    use firewheel::rtaudio::rtaudio::RtAudioErrorType;

    use crate::{
        SeedlingSystems,
        context::{AudioContext, SampleRate, StreamRestartEvent},
        platform::{self, *},
        prelude::SeedlingStartupSystems,
        resource_changed_without_insert,
    };

    pub(super) fn build_plugin(app: &mut App) {
        app.init_resource::<AudioStreamConfig<RtAudioConfig>>()
            .add_systems(
                PostStartup,
                start_stream.in_set(SeedlingStartupSystems::StreamInitialization),
            )
            .add_systems(
                PostUpdate,
                (crate::context::pre_restart_stream, restart_stream)
                    .chain()
                    .run_if(resource_changed_without_insert::<AudioStreamConfig<RtAudioConfig>>),
            )
            .add_systems(Last, poll_stream.in_set(SeedlingSystems::PollStream))
            .add_observer(observe_restart);
    }

    fn start_stream(
        mut context: ResMut<AudioContext>,
        stream_config: Res<AudioStreamConfig<RtAudioConfig>>,
        commands: Commands,
    ) -> Result {
        let sample_rate = context.with_store(|context, store| {
            let stream = RtAudioStream::new(context, stream_config.0.clone())?;
            let sample_rate = stream_sample_rate(&stream);

            let previous = store.insert(stream);
            debug_assert!(previous.is_none());

            Ok::<_, StartStreamError>(sample_rate)
        })?;

        platform::initialize_stream(SampleRate::new(sample_rate), commands);

        Ok(())
    }

    fn poll_stream(mut context: ResMut<AudioContext>, mut commands: Commands) -> Result {
        let status = context.with_store(|_, store| {
            store.get_mut::<RtAudioStream>().map(|stream| {
                let errors = stream.poll_status();
                let is_running = stream.is_running();

                (errors, is_running)
            })
        });

        if let Some((errors, is_running)) = status {
            for error in errors {
                match error.type_ {
                    RtAudioErrorType::Warning => {
                        warn!("RtAudio stream warning: {error}");
                    }
                    _ => {
                        error!("RtAudio stream error: {error}");
                    }
                }
            }

            if !is_running {
                warn!("RtAudio stream stopped");
                commands.trigger(RestartAudioStream);
            }
        }

        Ok(())
    }

    fn observe_restart(
        _: On<RestartAudioStream>,
        mut config: ResMut<AudioStreamConfig<RtAudioConfig>>,
    ) {
        config.set_changed();
    }

    fn restart_stream(
        stream_config: Res<AudioStreamConfig<RtAudioConfig>>,
        mut context: ResMut<AudioContext>,
        sample_rate: Res<SampleRate>,
        mut commands: Commands,
    ) -> Result {
        let previous_rate = sample_rate.get();
        let current_rate = context.with_store(|context, store| {
            let _ = store.remove::<RtAudioStream>();

            let stream = RtAudioStream::new(context, stream_config.0.clone())?;
            let sample_rate = stream_sample_rate(&stream);
            store.insert(stream);

            Ok::<_, StartStreamError>(sample_rate)
        })?;

        sample_rate.set(current_rate);
        commands.trigger(StreamRestartEvent {
            previous_rate,
            current_rate,
        });

        Ok(())
    }

    fn stream_sample_rate(stream: &RtAudioStream) -> NonZeroU32 {
        NonZeroU32::new(stream.stream_info().sample_rate)
            .expect("RtAudio streams should always report a non-zero sample rate")
    }
}

impl RtAudioPlatformPlugin {
    #[cfg(all(not(feature = "cpal"), not(target_arch = "wasm32")))]
    fn build_plugin(&self, app: &mut App) {
        inner::build_plugin(app);
    }
}

impl Plugin for RtAudioPlatformPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(all(not(feature = "cpal"), not(target_arch = "wasm32")))]
        self.build_plugin(app);

        #[cfg(any(feature = "cpal", target_arch = "wasm32"))]
        let _ = app;
    }
}
