//! `bevy_seedling`'s error types.

use alloc::string::{String, ToString};
use bevy_ecs::prelude::*;
use bevy_utils::prelude::DebugName;
use core::fmt::Display;
use firewheel::{diff::PatchError, error::UpdateError, node::NodeError};

// TODO: add location tracking where relevant
/// The set of all errors produced by `bevy_seedling`.
#[derive(Debug)]
pub enum SeedlingError {
    /// An error occurred when applying a Firewheel `Patch`
    /// to an audio node.
    Patch {
        /// The type name on which the patch failed.
        ty: DebugName,
        /// The Firewheel patch error.
        error: PatchError,
    },
    /// An error occurred when attempting to connect two
    /// audio nodes.
    Connection {
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
    /// An error that occurred during node construction.
    Node(String),
    /// Failed to fetch a node's state from the audio context.
    MissingState {
        /// The node for which state fetching failed.
        node: DebugName,
        /// The state that could not be fetched.
        state: DebugName,
    },
    /// Encountered an error when flushing the audio context.
    Update(UpdateError),
}

impl core::fmt::Display for SeedlingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Patch { ty, error } => {
                write!(f, "Failed to apply audio patch to `{ty}`: {error:?}")
            }
            Self::Connection { error, .. } => {
                write!(f, "Failed to connect audio nodes: {error}")
            }
            Self::MissingEffect { .. } => {
                write!(f, "Expected audio node in `SampleEffects` relationship")
            }
            Self::Node(e) => {
                write!(f, "Failed to construct node: {e}")
            }
            Self::MissingState { node, state } => {
                write!(
                    f,
                    "Failed to fetch the state `{node}` from the audio context for node `{state}`"
                )
            }
            Self::Update(e) => {
                write!(f, "{e}")
            }
        }
    }
}

impl core::error::Error for SeedlingError {}

impl From<NodeError> for SeedlingError {
    fn from(value: NodeError) -> Self {
        SeedlingError::Node(value.to_string())
    }
}

pub(crate) fn render_errors<
    I: IntoIterator<Item: core::fmt::Display, IntoIter: ExactSizeIterator>,
>(
    message: impl Display,
    error_collection: I,
) -> bevy_ecs::error::Result {
    use core::fmt::Write;
    let iterator = error_collection.into_iter();

    if iterator.len() == 0 {
        Ok(())
    } else {
        let mut string = String::new();
        for error in iterator {
            writeln!(&mut string, "{error}").unwrap();
        }

        Err(alloc::format!("{message}: {string}").into())
    }
}
