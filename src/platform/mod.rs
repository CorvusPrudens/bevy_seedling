//! Components that abstract over different backends.

use bevy_asset::AssetServer;
use bevy_ecs::prelude::*;

use crate::context::{SampleRate, StreamStartEvent};

#[cfg(feature = "cpal")]
pub mod cpal;

#[cfg(any(feature = "profiling", test))]
pub mod mock;

/// A [`Resource`] containing the audio context's stream configuration.
///
/// Mutating this resource will cause the audio stream to stop
/// and restart, applying the latest changes.
#[derive(Resource, Component, Debug, Default)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct AudioStreamConfig<C>(pub C);

/// When triggered globally, this attempts to automatically
/// restart the audio stream.
///
/// If the current devices are no longer available, this will
/// attempt to select the default input and output.
///
/// This only works with the default `cpal` backend.
#[derive(Event, Debug)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct RestartAudioStream;

/// Bookkeeping that should be called following stream initialization.
///
/// For example, here's how the `cpal` stream is initialized.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::{prelude::*, platform::cpal::CpalConfig, context::SampleRate};
/// # use bevy_seedling::platform::initialize_stream;
/// fn start_stream(
///     mut context: ResMut<AudioContext>,
///     stream_config: Res<AudioStreamConfig<CpalConfig>>,
///     server: Res<AssetServer>,
///     mut commands: Commands,
/// ) -> Result {
///     let stream = context
///         .with(|context| firewheel::cpal::CpalStream::new(context, stream_config.0.clone()))?;
///
///     let sample_rate = SampleRate::new(stream.info().sample_rate);
///
///     commands.insert_resource(CpalStream::new(stream));
///     initialize_stream(sample_rate, &server, commands);
///
///     Ok(())
/// }
/// ```
pub fn initialize_stream(sample_rate: SampleRate, server: &AssetServer, mut commands: Commands) {
    let raw_sample_rate = sample_rate.get();
    commands.insert_resource(sample_rate.clone());
    server.register_loader(crate::sample::SampleLoader::new(sample_rate));
    commands.trigger(StreamStartEvent {
        sample_rate: raw_sample_rate,
    });
}
