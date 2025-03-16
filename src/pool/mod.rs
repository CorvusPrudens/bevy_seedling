//! Sampler pools, which represent primary sampler player mechanism.

use std::sync::Arc;

use crate::node::ParamFollower;
use crate::prelude::{AudioContext, Connect, DefaultPool, FirewheelNode, PoolLabel, VolumeNode};
use crate::sample::{
    label::PoolLabelContainer, PlaybackSettings, QueuedSample, Sample, SamplePlayer,
};
use crate::{node::Events, SeedlingSystems};
use auto::AutoPoolRegistry;
use bevy_app::{Last, Plugin, PostUpdate};
use bevy_asset::Assets;
use bevy_ecs::{component::ComponentId, prelude::*, world::DeferredWorld};
use bevy_hierarchy::{BuildChildren, DespawnRecursiveExt};
use firewheel::{
    event::{NodeEventType, SequenceCommand},
    node::AudioNode,
    nodes::sampler::{SamplerNode, SamplerState},
    Volume,
};

pub mod auto;

pub(crate) struct SamplePoolPlugin;

impl Plugin for SamplePoolPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<auto::Registries>()
            .add_systems(
                Last,
                (
                    (remove_finished, assign_default)
                        .before(SeedlingSystems::Queue)
                        .after(SeedlingSystems::Acquire),
                    monitor_active
                        .before(SeedlingSystems::Flush)
                        .after(SeedlingSystems::Queue),
                ),
            )
            .add_systems(PostUpdate, auto::update_auto_pools);
    }
}

/// A sampler pool builder.
#[derive(Debug)]
pub struct Pool<L, C> {
    label: L,
    size: usize,
    effects_chain: C,
}

impl<L: PoolLabel + Component + Clone> Pool<L, ()> {
    pub fn new(label: L, size: usize) -> Self {
        Self {
            label,
            size,
            effects_chain: (),
        }
    }
}

pub trait ExtendTuple {
    type Output<T>;

    fn extend<T>(self, value: T) -> Self::Output<T>;
}

macro_rules! extend {
    ($($A:ident),*) => {
        impl<$($A),*> ExtendTuple for ($($A,)*) {
            type Output<Z> = ($($A,)* Z,);

            #[allow(non_snake_case)]
            fn extend<Z>(self, value: Z) -> Self::Output<Z> {
                let ($($A,)*) = self;

                ($($A,)* value,)
            }
        }
    };
}

bevy_utils::all_tuples!(extend, 0, 15, A);

impl<L, C: ExtendTuple> Pool<L, C> {
    pub fn effect<T: AudioNode + Clone + Component>(self, node: T) -> Pool<L, C::Output<T>> {
        Pool {
            label: self.label,
            size: self.size,
            effects_chain: self.effects_chain.extend(node),
        }
    }
}

pub(crate) fn spawn_pool<
    'a,
    L: PoolLabel + Component + Clone,
    C: Fn(&mut Commands) -> Vec<Entity>,
>(
    label: L,
    size: usize,
    chain_spawner: C,
    defaults: SamplePoolDefaults,
    commands: &'a mut Commands,
) -> EntityCommands<'a> {
    commands.queue(|world: &mut World| {
        world.schedule_scope(Last, |_, schedule| {
            schedule.add_systems(
                (rank_nodes::<L>, assign_work::<L>)
                    .chain()
                    .in_set(SeedlingSystems::Queue),
            );
        });
    });

    let bus = commands
        .spawn((
            VolumeNode {
                volume: Volume::Linear(1.0),
            },
            SamplePoolNode,
            label.clone(),
            defaults,
        ))
        .id();

    let rank = commands.spawn((NodeRank::default(), label.clone())).id();

    let nodes: Vec<_> = (0..size)
        .map(|_| {
            let chain = chain_spawner(commands);

            let source = commands
                .spawn((
                    SamplerNode::default(),
                    SamplePoolNode,
                    label.clone(),
                    EffectsChain(chain.clone()),
                ))
                .add_children(&chain)
                .id();

            let mut chain = chain;
            chain.push(bus);

            commands.entity(source).connect(chain[0]);

            for pair in chain.windows(2) {
                commands.entity(pair[0]).connect(pair[1]);
            }

            source
        })
        .collect();

    let mut bus = commands.entity(bus);
    bus.add_children(&nodes).add_child(rank);

    bus
}

macro_rules! spawn_impl {
    ($($ty:ident),*) => {
        impl<L: PoolLabel + Component + Clone, $($ty),*> Pool<L, ($($ty,)*)>
        where $($ty: Component + Clone),*
        {
            #[allow(non_snake_case)]
            pub fn spawn<'a>(
                self,
                commands: &'a mut Commands,
            ) -> EntityCommands<'a> {
                let Self {
                    label,
                    size,
                    effects_chain,
                } = self;

                let defaults = {
                    let ($($ty,)*) = effects_chain.clone();
                    let mut defaults = SamplePoolDefaults::default();

                    #[allow(unused)]
                    defaults.push(move |commands: &mut EntityCommands| {
                        $(commands.entry::<$ty>().or_insert($ty.clone());)*
                    });

                    defaults
                };

                #[allow(unused)]
                let chain_spawner = {
                    let ($($ty,)*) = effects_chain;
                    let label = label.clone();

                    move |commands: &mut Commands| vec![$(
                        commands.spawn((
                            <$ty as ::core::clone::Clone>::clone(&$ty),
                            SamplePoolNode,
                            label.clone()
                        )).id(),
                    )*]
                };

                spawn_pool(label, size, chain_spawner, defaults, commands)
            }
        }
    };
}

bevy_utils::all_tuples!(spawn_impl, 0, 15, A);

#[derive(Component)]
struct SamplePoolNode;

#[derive(Component)]
struct EffectsChain(Vec<Entity>);

// #[derive(Component)]
// struct SamplePoolDefaults(Box<dyn Fn(&mut EntityCommands) + Send + Sync + 'static>);

/// A collections of functions that insert a node's default value into an entity.
#[derive(Component, Default, Clone)]
pub(crate) struct SamplePoolDefaults(Vec<Arc<dyn Fn(&mut EntityCommands) + Send + Sync + 'static>>);

impl SamplePoolDefaults {
    pub fn push<F>(&mut self, f: F)
    where
        F: Fn(&mut EntityCommands) + Send + Sync + 'static,
    {
        self.0.push(Arc::new(f));
    }
}

#[derive(Default, Component)]
struct NodeRank(Vec<(Entity, u64)>);

fn rank_nodes<T: Component>(
    q: Query<(Entity, &SamplerNode, &FirewheelNode), (With<SamplePoolNode>, With<T>)>,
    mut rank: Query<&mut NodeRank, With<T>>,
    mut context: ResMut<AudioContext>,
) {
    let Ok(mut rank) = rank.get_single_mut() else {
        return;
    };

    rank.0.clear();

    context.with(|c| {
        for (e, params, node) in q.iter() {
            let Some(state) = c.node_state::<SamplerState>(node.0) else {
                continue;
            };

            let score = state.worker_score(params);

            rank.0.push((e, score));
        }
    });

    rank.0
        .sort_unstable_by_key(|pair| std::cmp::Reverse(pair.1));
}

#[derive(Component, Clone, Copy)]
#[component(on_remove = on_remove_active)]
struct ActiveSample {
    sample_entity: Entity,
    despawn: bool,
}

fn on_remove_active(mut world: DeferredWorld, entity: Entity, _: ComponentId) {
    let active = *world.entity(entity).components::<&ActiveSample>();

    if active.despawn {
        if let Some(commands) = world.commands().get_entity(active.sample_entity) {
            commands.despawn_recursive();
        }
    }
}

/// Automatically remove or despawn sampler players when their
/// sample has finished playing.
fn remove_finished(
    nodes: Query<(Entity, &EffectsChain, &FirewheelNode), (With<ActiveSample>, With<SamplerNode>)>,
    mut commands: Commands,
    mut context: ResMut<AudioContext>,
) {
    context.with(|context| {
        for (entity, effects_chain, node) in nodes.iter() {
            let Some(state) = context.node_state::<SamplerState>(node.0) else {
                continue;
            };

            let state = state.playback_state();

            // TODO: this will remove samples when paused
            if !state.is_playing() {
                commands.entity(entity).remove::<ActiveSample>();

                for effect in effects_chain.0.iter() {
                    commands.entity(*effect).remove::<ParamFollower>();
                }
            }
        }
    });
}

/// Scan through the set of pending sample players
/// and assign work to the most appropriate sampler node.
fn assign_work<T: Component>(
    mut nodes: Query<
        (
            Entity,
            &mut SamplerNode,
            &mut Events,
            &EffectsChain,
            &FirewheelNode,
        ),
        (With<SamplePoolNode>, With<T>),
    >,
    queued_samples: Query<
        (Entity, &SamplePlayer, &PlaybackSettings),
        (With<QueuedSample>, With<T>),
    >,
    mut rank: Query<&mut NodeRank, With<T>>,
    defaults: Query<&SamplePoolDefaults, With<T>>,
    assets: Res<Assets<Sample>>,
    mut commands: Commands,
    mut context: ResMut<AudioContext>,
) {
    let Ok(mut rank) = rank.get_single_mut() else {
        return;
    };

    context.with(|context| {
        for (sample, player, settings) in queued_samples.iter() {
            let Some(asset) = assets.get(&player.0) else {
                continue;
            };

            // get the best candidate
            let Some((node_entity, _)) = rank.0.first() else {
                continue;
            };

            let Ok((node_entity, mut params, mut events, effects_chain, sampler_id)) =
                nodes.get_mut(*node_entity)
            else {
                continue;
            };

            let Some(sampler_state) = context.node_state::<SamplerState>(sampler_id.0) else {
                continue;
            };

            params.set_sample(asset.get(), settings.volume, settings.mode);
            let event = sampler_state.sync_params_event(&params, true);
            events.push(event);

            // redirect all parameters to follow the sample source
            for effect in effects_chain.0.iter() {
                commands.entity(*effect).insert(ParamFollower(sample));
            }

            // Insert default pool parameters if not present.
            if let Ok(defaults) = defaults.get_single() {
                for item in defaults.0.iter() {
                    item(&mut commands.entity(sample));
                }
            }

            rank.0.remove(0);
            commands.entity(sample).remove::<QueuedSample>();
            commands.entity(node_entity).insert(ActiveSample {
                sample_entity: sample,
                despawn: true,
            });
        }
    });
}

// Stop playback if the source entity no longer exists.
fn monitor_active(
    mut nodes: Query<(Entity, &ActiveSample, &mut Events, &EffectsChain)>,
    samples: Query<&SamplePlayer>,
    mut commands: Commands,
) {
    for (node_entity, active, mut events, effects_chain) in nodes.iter_mut() {
        if samples.get(active.sample_entity).is_err() {
            events.push(NodeEventType::SequenceCommand(SequenceCommand::Stop));

            commands.entity(node_entity).remove::<ActiveSample>();

            for effect in effects_chain.0.iter() {
                commands.entity(*effect).remove::<ParamFollower>();
            }
        }
    }
}

/// Assign the default pool label to a sample player that has no label.
fn assign_default(
    samples: Query<
        Entity,
        (
            With<SamplePlayer>,
            Without<PoolLabelContainer>,
            Without<AutoPoolRegistry>,
        ),
    >,
    mut commands: Commands,
) {
    for sample in samples.iter() {
        commands.entity(sample).insert(DefaultPool);
    }
}

/// A pool despawner command.
///
/// Despawn a sample pool, cleaning up its resources
/// in the ECS and audio graph.
///
/// Despawning the terminal volume node recursively
/// will produce the same effect.
///
/// This can be used directly or via the [`PoolCommands`] trait.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_seedling::prelude::*;
/// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct MyLabel;
///
/// fn system(mut commands: Commands) {
///     commands.queue(PoolDespawn::new(MyLabel));
/// }
/// ```
#[derive(Debug)]
pub struct PoolDespawn<T>(T);

impl<T: PoolLabel + Component> PoolDespawn<T> {
    pub fn new(label: T) -> Self {
        Self(label)
    }
}

impl<T: PoolLabel + Component> Command for PoolDespawn<T> {
    fn apply(self, world: &mut World) {
        let mut roots =
            world.query_filtered::<Entity, (With<T>, With<SamplePoolNode>, With<VolumeNode>)>();

        let roots: Vec<_> = roots.iter(world).collect();

        let mut commands = world.commands();

        for root in roots {
            commands.entity(root).despawn_recursive();
        }
    }
}

/// Provides methods on [`Commands`] to manage sample pools.
pub trait PoolCommands {
    /// Despawn a sample pool, cleaning up its resources
    /// in the ECS and audio graph.
    ///
    /// Despawning the terminal volume node recursively
    /// will produce the same effect.
    fn despawn_pool<T: PoolLabel + Component>(&mut self, label: T);
}

impl PoolCommands for Commands<'_, '_> {
    fn despawn_pool<T: PoolLabel + Component>(&mut self, label: T) {
        self.queue(PoolDespawn::new(label));
    }
}
