//! Stream management for Web Audio worklets.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use crate::{
    context::{AudioContext, SampleRate, StreamRestartEvent},
    platform::*,
    prelude::SeedlingStartupSystems,
    resource_changed_without_insert, SeedlingSystems,
};

pub use firewheel_web_audio::WebAudioConfig;

use firewheel_web_audio::{WebAudioBackend, WebAudioStartError};

/// `bevy_seedling`'s multi-threaded Web Audio platform plugin.
#[derive(Debug, Default)]
pub struct WebAudioPlatformPlugin;

impl Plugin for WebAudioPlatformPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioStreamConfig<WebAudioConfig>>()
            .add_systems(
                PostStartup,
                start_stream.in_set(SeedlingStartupSystems::StreamInitialization),
            )
            .add_systems(
                PostUpdate,
                (crate::context::pre_restart_stream, restart_stream)
                    .chain()
                    .run_if(resource_changed_without_insert::<AudioStreamConfig<WebAudioConfig>>),
            )
            .add_systems(Last, poll_stream.in_set(SeedlingSystems::PollStream))
            .add_observer(observe_restart);
    }
}

fn start_stream(
    mut context: ResMut<AudioContext>,
    stream_config: Res<AudioStreamConfig<WebAudioConfig>>,
    commands: Commands,
) -> Result {
    // TODO: it's not possible for the user to recover if this fails
    let sample_rate = context.with_store(|context, store| -> Result<_, WebAudioStartError> {
        let stream = WebAudioBackend::new(context, stream_config.0.clone())?;
        let sample_rate = stream.sample_rate();

        let previous = store.insert(stream);
        debug_assert!(previous.is_none());

        Ok(sample_rate)
    })?;

    super::initialize_stream(SampleRate::new(sample_rate), commands);

    Ok(())
}

fn poll_stream(mut context: ResMut<AudioContext>, mut commands: Commands) -> Result {
    // TODO: we should probably poll at least something here

    Ok(())
}

fn observe_restart(_: On<RestartAudioStream>, mut config: ResMut<AudioStreamConfig<WebAudioConfig>>) {
    config.set_changed();
}

fn restart_stream(
    stream_config: Res<AudioStreamConfig<WebAudioConfig>>,
    mut graph: ResMut<AudioContext>,
    sample_rate: Res<SampleRate>,
    mut commands: Commands,
) -> Result {
    // drop it like it's hot
    let current_rate = graph.with_store(|context, store| -> Result<_, WebAudioStartError> {
        let _ = store.remove::<WebAudioBackend>();

        let stream = WebAudioBackend::new(context, stream_config.0.clone())?;
        let sample_rate = stream.sample_rate();
        store.insert(stream);

        Ok(sample_rate)
    })?;

    let previous_rate = sample_rate.get();
    sample_rate.set(current_rate);

    commands.trigger(StreamRestartEvent {
        previous_rate,
        current_rate,
    });

    Ok(())
}
