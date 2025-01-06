//! Audio sample components.

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use firewheel::node::RepeatMode;

mod assets;
pub mod label;
pub mod pool;

pub use assets::{Sample, SampleLoader, SampleLoaderError};

/// A component that queues sample playback.
///
/// When the sample asset loads, `bevy_seedling` will assign
/// the playback to the best fitting node in the default
/// sample pool.
#[derive(Debug, Component, Clone)]
#[require(PlaybackSettings, QueuedSample)]
pub struct SamplePlayer(pub(crate) Handle<Sample>);

impl SamplePlayer {
    pub fn new(handle: Handle<Sample>) -> Self {
        Self(handle)
    }
}

#[derive(Debug, Component, Clone)]
pub struct PlaybackSettings {
    pub mode: RepeatMode,
    pub volume: f32,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            mode: Default::default(),
            volume: 1.0,
        }
    }
}

impl PlaybackSettings {
    /// Play the audio source once, despawning
    /// this entity when complete or interrupted.
    pub const ONCE: Self = Self {
        mode: RepeatMode::RepeatEndlessly,
        volume: 1.0,
    };

    /// Repeatedly loop the audio source until
    /// this entity is despawned.
    pub const LOOP: Self = Self {
        mode: RepeatMode::RepeatEndlessly,
        volume: 1.0,
    };
}

/// A marker struct for entities that are waiting
/// for asset loading and playback assignment.
#[derive(Debug, Component, Default)]
#[component(storage = "SparseSet")]
pub struct QueuedSample;
