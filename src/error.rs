//! `bevy_seedling`'s error types.

use bevy_ecs::entity::Entity;
use firewheel::diff::PatchError;

// TODO: add location tracking where relevant
/// The set of all errors produced by `bevy_seedling`.
#[derive(Debug)]
pub enum SeedlingError {
    /// An error occurred when applying a Firewheel `Patch`
    /// to an audio node.
    PatchError {
        /// The type name on which the patch failed.
        ty: &'static str,
        /// The Firewheel patch error.
        error: PatchError,
    },
    /// An error occurred when attempting to connect two
    /// audio nodes.
    ConnectionError {
        /// The source entity.
        source: Entity,
        /// The destination entity.
        dest: Entity,
        /// The underlying Firewheel error.
        error: String,
    },
    /// A sample effect relationship was spawned with an empty
    /// effect entity.
    MissingEffect {
        /// The [`EffectOf`][crate::pool::sample_effects::EffectOf] entity missing
        /// an effect.
        empty_entity: Entity,
    },
}

impl core::fmt::Display for SeedlingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PatchError { ty, error } => {
                write!(f, "Failed to apply audio patch to `{ty}`: {error:?}")
            }
            Self::ConnectionError { error, .. } => {
                write!(f, "Failed to connect audio nodes: {error}")
            }
            Self::MissingEffect { .. } => {
                write!(f, "Expected audio node in `SampleEffects` relationship")
            }
        }
    }
}

impl core::error::Error for SeedlingError {}
