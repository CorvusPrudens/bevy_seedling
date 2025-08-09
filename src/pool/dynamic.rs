//! Dynamic sampler pools.
//!
//! *Sampler pools* are `bevy_seedling`'s primary mechanism for playing
//! multiple sounds at once. [`SampleEffects`] can be used to create effectful pools on-the-fly.
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_seedling::prelude::*;
//! fn effects(mut commands: Commands, server: Res<AssetServer>) {
//!     commands.spawn((
//!         SamplePlayer::new(server.load("my_sample.wav")),
//!         sample_effects![
//!             SpatialBasicNode::default(),
//!             LowPassNode { frequency: 500.0 }
//!         ],
//!     ));
//! }
//! ```
//!
//! In the above example, we connect a spatial and low-pass node in series with the sample player.
//! Effects are arranged in the order they're spawned, so the output of the spatial node is
//! connected to the input of the low-pass node.
//!
//! Once per frame, `bevy_seedling` will scan for [`SamplePlayer`]s that request dynamic pools, assigning
//! the sample to an existing dynamic pool or creating a new one if none match. The number of
//! samplers in a dynamic pool is determined by the [`PoolSize`] component, which defaults to
//! [`DefaultPoolSize`].
//! The pool is spawned with the range's `start` value, and as demand increases, the pool
//! grows quadratically until the range's `end`.
//!
//! ## When to use dynamic pools
//!
//! Dynamic pools are a convenient abstraction, but they may not be appropriate for all use-cases.
//! They have three main drawbacks:
//!
//! 1. Dynamic pools cannot have individual routing. They are all connected to the [`DynamicBus`].
//! 2. The number of pools corresponds to the total permutations of effects your project uses,
//!    which could grow fairly large. Silent sampler nodes shouldn't take much CPU time,
//!    but many unused nodes could grow your memory usage by a few megabytes.
//! 3. Dynamic pools are spawned on-the-fly, so you may see a small amount of additional
//!    playback latency as the pool propagates to the audio graph.
//!
//! Dynamic pool are best-suited for sounds that do not need complicated routing or
//! bus configurations and when the kinds of effects you apply are simple and regular.
//! Keep in mind that you can freely mix dynamic and static pools, so you're not restricted
//! to only one or the other!
//!
//! Note that when no effects are applied, your samples will be queued in the
//! [`DefaultPool`][crate::prelude::DefaultPool], not a dynamic pool.

use super::{DefaultPoolSize, PoolSize, SamplerPool, sample_effects::EffectOf};
use crate::{
    edge::Connect,
    node::EffectId,
    pool::{label::PoolLabelContainer, sample_effects::SampleEffects},
    sample::{QueuedSample, SamplePlayer},
};
use bevy_app::prelude::*;
use bevy_ecs::{component::ComponentId, entity::EntityCloner, prelude::*};
use bevy_log::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_seedling_macros::{NodeLabel, PoolLabel};

pub(super) struct DynamicPlugin;

impl Plugin for DynamicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Registries>()
            .add_systems(PostUpdate, update_dynamic_pools);
    }
}

/// The destination for all dynamic pools.
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct DynamicBus;

/// A label reserved for dynamic pools.
#[derive(PoolLabel, Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct DynamicPoolLabel(usize);

struct RegistryEntry {
    label: DynamicPoolLabel,
}

#[derive(Resource, Default)]
struct Registries(HashMap<Vec<ComponentId>, RegistryEntry>);

fn update_dynamic_pools(
    queued_samples: Query<
        (Entity, &SampleEffects),
        (
            With<QueuedSample>,
            With<SamplePlayer>,
            Without<PoolLabelContainer>,
        ),
    >,
    // TODO: make sure to migrate this to `If<Single<_>>` for 0.17
    dynamic_bus: Single<Entity, With<DynamicBus>>,
    mut effects: Query<&EffectId>,
    mut registries: ResMut<Registries>,
    mut commands: Commands,
    dynamic_range: Res<DefaultPoolSize>,
) -> Result {
    for (sample, sample_effects) in queued_samples.iter() {
        let component_ids =
            match super::fetch_effect_ids(sample_effects, &mut effects.as_query_lens()) {
                Ok(ids) => ids,
                Err(e) => {
                    error!("{e}");

                    continue;
                }
            };

        match registries.0.get_mut(&component_ids) {
            Some(entry) => {
                commands.entity(sample).insert(entry.label);
            }
            None => {
                let label = DynamicPoolLabel(registries.0.len());

                let bus = commands
                    .spawn((SamplerPool(label), PoolSize(dynamic_range.0.clone())))
                    .connect(*dynamic_bus)
                    .head();

                let effects: Vec<_> = sample_effects.iter().collect();
                commands.queue(move |world: &mut World| {
                    let mut cloner = EntityCloner::build(world);
                    cloner.deny::<EffectOf>();
                    let mut cloner = cloner.finish();

                    let mut cloned = Vec::new();
                    for effect in effects {
                        let effect = cloner.spawn_clone(world, effect);
                        cloned.push(effect);
                    }

                    world.entity_mut(bus).add_related::<EffectOf>(&cloned);
                });

                registries.0.insert(component_ids, RegistryEntry { label });

                commands.entity(sample).insert(label);
            }
        }
    }

    Ok(())
}
