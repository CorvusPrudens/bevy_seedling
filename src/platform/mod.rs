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
pub struct AudioStreamConfig<C>(pub C);

pub fn initialize_stream(sample_rate: SampleRate, server: &AssetServer, mut commands: Commands) {
    let raw_sample_rate = sample_rate.get();
    commands.insert_resource(sample_rate.clone());
    server.register_loader(crate::sample::SampleLoader::new(sample_rate));
    commands.trigger(StreamStartEvent {
        sample_rate: raw_sample_rate,
    });
}
