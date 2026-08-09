//! Stream management for Web Audio worklets.

use bevy_app::prelude::*;

pub use firewheel_web_audio::WebAudioConfig;

/// `bevy_seedling`'s multi-threaded Web Audio platform plugin.
///
/// Currently, this backend only supports stereo inputs and outputs.
///
/// ## Quick start
///
/// To get started with multi-threaded web audio, enable `bevy_seedling`'s
/// `web_audio` feature and ensure you have
/// the [Bevy CLI](https://github.com/theBevyFlock/bevy_cli) installed.
/// Then, run `bevy run web -U multi-threading` to build and serve your
/// project. That's it!
///
/// ## Precise Requirements
///
/// Because this backend relies on Wasm multi-threading, it has
/// some additional requirements.
///
/// 1. A nightly compiler is required along with the Rust standard library source code
///    (with `rustup`, you can add it with `rustup component add rust-src`).
/// 2. You'll need the `atomics` target feature and additional linker settings.
///    These can be enabled with a `.cargo/config.toml` as noted in the
///    [crate docs][firewheel_web_audio]. This is automatically handled by
///    the [Bevy CLI](https://github.com/theBevyFlock/bevy_cli) with the
///    `-U multi-threading` flag, and should be the preferred approach for most projects.
/// 3. Wherever your project is served, the protocol must be secure (usually `https`)
///    and the response must include two security headers:
///
/// ```text
/// Cross-Origin-Opener-Policy: same-origin
/// Cross-Origin-Embedder-Policy: require-corp
/// # or
/// Cross-Origin-Embedder-Policy: credentialless
/// ```
///
/// Conveniently, the CLI also provides these headers for local development.
/// itch.io has a checkbox labeled "`SharedArrayBuffer` support" that provides these headers.
#[derive(Debug, Default)]
pub struct WebAudioPlatformPlugin;

impl Plugin for WebAudioPlatformPlugin {
    fn build(&self, app: &mut App) {
        // this plugin is completely useless outside of wasm,
        // so we'll gate it

        #[cfg(target_arch = "wasm32")]
        inner::build(app);

        #[cfg(not(target_arch = "wasm32"))]
        let _ = app;
    }
}

#[cfg(target_arch = "wasm32")]
mod inner {
    use bevy_app::prelude::*;
    use bevy_ecs::prelude::*;

    use crate::{
        SeedlingSystems,
        context::{AudioContext, SampleRate, StreamRestartEvent},
        platform::*,
        prelude::SeedlingStartupSystems,
        resource_changed_without_insert,
    };

    use firewheel_web_audio::{WebAudioBackend, WebAudioConfig, WebAudioStartError};

    pub fn build(app: &mut App) {
        app.init_resource::<AudioStreamConfig<WebAudioConfig>>()
            .insert_resource(ProcessorActive(false))
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

    fn start_stream(
        mut context: ResMut<AudioContext>,
        stream_config: Res<AudioStreamConfig<WebAudioConfig>>,
        commands: Commands,
    ) -> Result {
        // TODO: it's not possible for the user to recover if this fails
        let sample_rate =
            context.with_store(|context, store| -> Result<_, WebAudioStartError> {
                let stream = WebAudioBackend::new(context, stream_config.0.clone())?;
                let sample_rate = stream.sample_rate();

                let previous = store.insert(stream);
                debug_assert!(previous.is_none());

                Ok(sample_rate)
            })?;

        crate::platform::initialize_stream(SampleRate::new(sample_rate), commands);

        Ok(())
    }

    fn poll_stream(
        mut context: ResMut<AudioContext>,
        mut active: ResMut<ProcessorActive>,
    ) -> Result {
        let new_active = context.with_store(|_, store| {
            store
                .get_mut::<WebAudioBackend>()
                .map(|context| context.poll())
                .unwrap_or(Ok(false))
        })?;

        active.0 = new_active;

        Ok(())
    }

    fn observe_restart(
        _: On<RestartAudioStream>,
        mut config: ResMut<AudioStreamConfig<WebAudioConfig>>,
    ) {
        config.set_changed();
    }

    fn restart_stream(
        stream_config: Res<AudioStreamConfig<WebAudioConfig>>,
        mut graph: ResMut<AudioContext>,
        sample_rate: Res<SampleRate>,
        mut commands: Commands,
    ) -> Result {
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
}
