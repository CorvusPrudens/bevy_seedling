//! Types and traits for managing per-sample effects.

use crate::utils::entity_set::{EntitySet, EntitySetIter};
use bevy_ecs::{
    prelude::*,
    query::{QueryData, QueryFilter, QueryManyUniqueIter, ROQueryItem},
};

/// An effect applied to a sample player.
///
/// This targets the [`SampleEffects`] component.
#[derive(Debug, Component)]
#[relationship(relationship_target = SampleEffects)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct EffectOf(pub Entity);

/// A serial chain of effects applied on a per-sampler basis.
///
/// These effects -- audio nodes with at least two inputs and outputs
/// -- are applied in the order they're spawned. There are two main
/// ways to use [`SampleEffects`].
///
/// ## Dynamic pools
///
/// When applied to a [`SamplePlayer`][crate::prelude::SamplePlayer] without an explicit pool assignment,
/// a pool is dynamically created according to the shape of the effects.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn dynamic(mut commands: Commands, server: Res<AssetServer>) {
/// // Creates a pool on-demand with per-sample volume control.
/// commands.spawn((
///     SamplePlayer::new(server.load("my_sample.wav")),
///     sample_effects![VolumeNode::default()],
/// ));
///
/// // Since this shape already exists, this sample will be queued in the
/// // same dynamic pool we just created.
/// commands.spawn((
///     SamplePlayer::new(server.load("my_other_sample.wav")),
///     // You can always provide arbitrary initial values.
///     sample_effects![VolumeNode {
///         volume: Volume::Decibels(-6.0),
///         ..Default::default()
///     }],
/// ));
/// # }
/// ```
/// By default, these pools are spawned with relatively few samplers,
/// spawning more as demand grows up to some maximum bound.
///
/// Dynamic pools are convenient, especially for prototyping, but
/// may become cumbersome as projects grow. If you'd prefer to disable
/// dynamic pools entirely, insert a [`DefaultPoolSize`] resource of `0..=0`.
///
/// [`DefaultPoolSize`]: crate::prelude::DefaultPoolSize
///
/// ## Static pools
///
/// When applied to a [`SamplerPool`][crate::prelude::SamplerPool], [`SampleEffects`]
/// serves as a template for all samples played in the pool.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn pools(mut commands: Commands, server: Res<AssetServer>) {
/// #[derive(PoolLabel, Clone, PartialEq, Eq, Debug, Hash)]
/// struct MusicPool;
///
/// // Creates a pool where all samplers have volume and spatial processors.
/// commands.spawn((
///     SamplerPool(MusicPool),
///     sample_effects![
///         // The defaults established here will be applied to each
///         // sample player unless explicitly overwritten.
///         VolumeNode {
///             volume: Volume::Decibels(-3.0),
///             ..Default::default()
///         },
///         SpatialBasicNode::default(),
///     ],
/// ));
///
/// // Samples don't need to mention effects.
/// commands.spawn((MusicPool, SamplePlayer::new(server.load("track_one.wav"))));
///
/// // Overwriting just a subset works, too.
/// commands.spawn((
///     SamplePlayer::new(server.load("my_other_sample.wav")),
///     sample_effects![VolumeNode {
///         volume: Volume::Decibels(-6.0),
///         ..Default::default()
///     }],
/// ));
/// # }
/// ```
///
/// Samples played in a pool don't need to respect the ordering
/// or presence of effects; when a sample is queued, missing effects
/// are inserted and the order of effects is corrected. Consequently,
/// the exact index of a particular effect within [`SampleEffects`]
/// may change, so the [`EffectsQuery`] trait is the best way to reliably access
/// them.
///
/// ## Notes
///
/// Rather than existing in the audio graph directly, nodes with [`EffectOf`]
/// components serve as plain-old-data baselines.
/// When a sample is queued in a particular pool, the fully-connected
/// nodes on the selected sampler start tracking the entities in the
/// sample's [`SampleEffects`].
#[derive(Debug, Component)]
#[relationship_target(relationship = EffectOf, linked_spawn)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SampleEffects(EntitySet);

impl core::ops::Deref for SampleEffects {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[doc(hidden)]
pub use bevy_ecs::spawn::Spawn;

/// Returns a spawnable list of [`SampleEffects`].
///
/// This is equivalent to `related!(SampleEffects[/* ... */])`.
///
/// [`SampleEffects`] represents a collection of audio nodes
/// connected in serial, in the order they're spawned, to the
/// underlying sampler node. As effects, `bevy_seedling` expects
/// each node to have at least two input and output channels.
#[macro_export]
macro_rules! sample_effects {
    [$($effect:expr),*$(,)?] => {
        <$crate::pool::sample_effects::SampleEffects>::spawn(($($crate::pool::sample_effects::Spawn($effect)),*))
    };
}

/// Errors for effects queries.
///
/// Since these queries require direct fetching with `get` and
/// related methods, [`EffectsQuery`] are not infallible.
#[derive(Debug)]
pub enum EffectsQueryError {
    /// An effects query that expected a single result matched multiple.
    MatchedMultiple,
    /// An effects query that expected a single result matched none.
    MatchedNone,
}

impl core::fmt::Display for EffectsQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MatchedMultiple => write!(f, "audio effects query matched multiple entities"),
            Self::MatchedNone => write!(f, "audio effects query matched no entities"),
        }
    }
}

impl core::error::Error for EffectsQueryError {}

/// An extension trait for simplifying [`SampleEffects`] queries.
///
/// Since Bevy does not yet support sophisticated relationship queries,
/// it can be cumbersome to manually join relationship-dependent queries.
/// This trait eases the burden with a few convenience methods.
///
/// For example, if you want to query over all [`LowPassNode`]s whose
/// sample entity contains some marker:
///
/// [`LowPassNode`]: crate::prelude::LowPassNode
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn example(mut commands: Commands, server: Res<AssetServer>) {
/// #[derive(Component)]
/// struct Marker;
///
/// commands.spawn((
///     Marker,
///     SamplePlayer::new(server.load("my_sample.wav")),
///     sample_effects![LowPassNode::default()],
/// ));
///
/// fn effects_of_marker(
///     samples: Query<&SampleEffects, With<Marker>>,
///     low_pass: Query<&LowPassNode>,
/// ) -> Result {
///     for effects in samples {
///         let low_pass = low_pass.get_effect(effects)?;
///         // ...
///     }
///
///     Ok(())
/// }
/// # }
/// ```
///
/// When Bevy's related queries story matures, this trait will likely be deprecated.
pub trait EffectsQuery<'s, D, F>
where
    D: QueryData,
    F: QueryFilter,
{
    /// Get a single effect.
    ///
    /// An error is returned if the query doesn't return exactly one entity.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// # fn example(mut commands: Commands, server: Res<AssetServer>) {
    /// #[derive(Component)]
    /// struct UnderwaterSound;
    ///
    /// fn log_underwater_freq(
    ///     samples: Query<&SampleEffects, With<UnderwaterSound>>,
    ///     low_pass: Query<&LowPassNode>,
    /// ) -> Result {
    ///     for effects in samples {
    ///         let frequency = low_pass.get_effect(effects)?.frequency;
    ///         info!("Frequency: {frequency}hz");
    ///     }
    ///
    ///     Ok(())
    /// }
    /// # }
    /// ```
    fn get_effect(
        &self,
        effects: &SampleEffects,
    ) -> Result<ROQueryItem<'_, 's, D>, EffectsQueryError>;

    /// Get a mutable reference to a single effect.
    ///
    /// An error is returned if the query doesn't return exactly one entity.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// # fn example(mut commands: Commands, server: Res<AssetServer>) {
    /// #[derive(Component)]
    /// struct UnderwaterSound;
    ///
    /// fn set_underwater_freq(
    ///     samples: Query<&SampleEffects, With<UnderwaterSound>>,
    ///     mut low_pass: Query<&mut LowPassNode>,
    /// ) -> Result {
    ///     for effects in samples {
    ///         low_pass.get_effect_mut(effects)?.frequency = 500.0;
    ///     }
    ///
    ///     Ok(())
    /// }
    /// # }
    /// ```
    fn get_effect_mut(
        &mut self,
        effects: &SampleEffects,
    ) -> Result<D::Item<'_, 's>, EffectsQueryError>;

    /// Iterate over all effects entities that match the query.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// #[derive(Component)]
    /// struct UnderwaterSound;
    ///
    /// fn log_underwater_freq(
    ///     samples: Query<&SampleEffects, With<UnderwaterSound>>,
    ///     low_pass: Query<&LowPassNode>,
    /// ) {
    ///     for node in samples.iter().flat_map(|s| low_pass.iter_effects(s)) {
    ///         info!("Frequency: {}", node.frequency);
    ///     }
    /// }
    /// ```
    fn iter_effects<'a>(
        &self,
        effects: &'a SampleEffects,
    ) -> QueryManyUniqueIter<'_, 's, D::ReadOnly, F, EntitySetIter<'a>>;

    /// Mutably iterate over all effects entities that match the query.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// #[derive(Component)]
    /// struct UnderwaterSound;
    ///
    /// fn set_underwater_freq(
    ///     samples: Query<&SampleEffects, With<UnderwaterSound>>,
    ///     mut low_pass: Query<&mut LowPassNode>,
    /// ) {
    ///     for effects in samples {
    ///         for mut node in low_pass.iter_effects_mut(effects) {
    ///             node.frequency = 500.0;
    ///         }
    ///     }
    /// }
    /// ```
    fn iter_effects_mut<'a>(
        &mut self,
        effects: &'a SampleEffects,
    ) -> QueryManyUniqueIter<'_, 's, D, F, EntitySetIter<'a>>;
}

impl<'s, D, F> EffectsQuery<'s, D, F> for Query<'_, 's, D, F>
where
    D: QueryData,
    F: QueryFilter,
{
    fn get_effect(
        &self,
        effects: &SampleEffects,
    ) -> Result<ROQueryItem<'_, 's, D>, EffectsQueryError> {
        if self.iter_many_unique(effects.iter()).count() > 1 {
            return Err(EffectsQueryError::MatchedMultiple);
        }

        self.iter_many_unique(effects.iter())
            .next()
            .ok_or(EffectsQueryError::MatchedNone)
    }

    fn get_effect_mut(
        &mut self,
        effects: &SampleEffects,
    ) -> Result<D::Item<'_, 's>, EffectsQueryError> {
        if self.iter_many_unique(effects.iter()).count() > 1 {
            return Err(EffectsQueryError::MatchedMultiple);
        }

        self.iter_many_unique_mut(effects.iter())
            .next()
            .ok_or(EffectsQueryError::MatchedNone)
    }

    fn iter_effects<'a>(
        &self,
        effects: &'a SampleEffects,
    ) -> QueryManyUniqueIter<'_, 's, D::ReadOnly, F, EntitySetIter<'a>> {
        self.iter_many_unique(effects.iter())
    }

    fn iter_effects_mut<'a>(
        &mut self,
        effects: &'a SampleEffects,
    ) -> QueryManyUniqueIter<'_, 's, D, F, EntitySetIter<'a>> {
        self.iter_many_unique_mut(effects.iter())
    }
}
