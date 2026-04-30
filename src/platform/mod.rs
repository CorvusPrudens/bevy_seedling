//! Components that abstract over different backends.

use bevy_ecs::prelude::*;

use crate::context::{SampleRate, StreamStartEvent};

#[cfg(feature = "cpal")]
pub mod cpal;

#[cfg(feature = "rtaudio")]
pub mod rtaudio;

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
#[derive(Event, Debug)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct RestartAudioStream;

/// Bookkeeping that should be called following stream initialization.
///
/// For example, once a backend has initialized a stream and knows
/// the active sample rate, it can finish startup bookkeeping.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::context::SampleRate;
/// # use bevy_seedling::platform::initialize_stream;
/// # use core::num::NonZeroU32;
/// fn start_stream(commands: Commands) {
///     let sample_rate = SampleRate::new(NonZeroU32::new(48000).unwrap());
///     initialize_stream(sample_rate, commands);
/// }
/// ```
pub fn initialize_stream(sample_rate: SampleRate, mut commands: Commands) {
    let raw_sample_rate = sample_rate.get();
    commands.insert_resource(sample_rate.clone());
    commands.trigger(StreamStartEvent {
        sample_rate: raw_sample_rate,
    });
}
