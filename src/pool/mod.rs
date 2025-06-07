//! Sampler pools, `bevy_seedling`'s primary sample playing mechanism.

use crate::{
    SeedlingSystems,
    context::AudioContext,
    edge::{PendingConnections, PendingEdge},
    error::SeedlingError,
    node::{EffectId, FirewheelNode, RegisterNode},
    pool::label::PoolLabelContainer,
    prelude::PoolLabel,
    sample::{OnComplete, PlaybackSettings, QueuedSample, SamplePlayer},
};
use bevy::{
    ecs::{
        component::{ComponentId, HookContext},
        entity::EntityCloner,
        system::QueryLens,
        world::DeferredWorld,
    },
    prelude::*,
};
use core::ops::{Deref, RangeInclusive};
use firewheel::nodes::{
    sampler::{SamplerConfig, SamplerNode, SamplerState},
    volume::VolumeNode,
};
use queue::SkipTimer;
use sample_effects::{EffectOf, SampleEffects};

pub mod dynamic;
mod entity_set;
pub mod label;
mod queue;
pub mod sample_effects;

pub(crate) struct SamplePoolPlugin;

impl Plugin for SamplePoolPlugin {
    fn build(&self, app: &mut App) {
        app.register_node::<SamplerNode>()
            .add_systems(
                Last,
                (
                    (populate_pool, queue::grow_pools)
                        .chain()
                        .before(SeedlingSystems::Acquire),
                    (poll_finished, queue::assign_default, retrieve_state)
                        .before(SeedlingSystems::Pool)
                        .after(SeedlingSystems::Connect),
                    watch_sample_players
                        .before(SeedlingSystems::Queue)
                        .after(SeedlingSystems::Pool),
                    (queue::assign_work, queue::update_followers)
                        .chain()
                        .in_set(SeedlingSystems::Pool),
                    (queue::tick_skipped, queue::mark_skipped)
                        .chain()
                        .after(SeedlingSystems::Pool),
                ),
            )
            .add_observer(remove_finished)
            .add_plugins(dynamic::DynamicPlugin);
    }
}

/// A component for building sampler pools.
///
/// *Sampler pools* are `bevy_seedling`'s primary mechanism for playing
/// multiple sounds at once. [`SamplerPool`] allows you to precisely define pools
/// and their routing.
///
/// ## Constructing pools
///
/// To construct a pool, you'll need to provide a [`PoolLabel`].
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// // Note that you'll need a few additional traits to support `PoolLabel`
/// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct SimplePool;
///
/// fn spawn_pool(mut commands: Commands) {
///     commands.spawn(SamplerPool(SimplePool));
/// }
/// ```
///
/// You can also provide an explicit [`PoolSize`], overriding the [`DefaultPoolSize`]
/// resource.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// # struct SimplePool;
/// # fn spawn_pool(mut commands: Commands) {
/// commands.spawn((
///     SamplerPool(SimplePool),
///     // A pool of exactly 16 samplers that cannot grow
///     PoolSize(16..=16),
/// ));
/// # }
/// ```
///
/// Finally, you can insert arbitrary effects.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn spawn_pools(mut commands: Commands) {
/// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct EffectsPool;
///
/// commands.spawn((
///     SamplerPool(EffectsPool),
///     sample_effects![LowPassNode::default(), SpatialBasicNode::default()],
/// ));
/// # }
/// ```
///
/// By default, pools will insert a volume node in the root [`SamplerPool`]
/// entity and connect all its samplers to it. As a result, you
/// can easily route the entire pool with a single [`connect`][crate::prelude::Connect::connect]
/// call.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn spawn_pools(mut commands: Commands) {
/// # #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// # struct SimplePool;
/// let filter = commands.spawn(LowPassNode::default()).id();
///
/// commands.spawn(SamplerPool(SimplePool)).connect(filter);
/// # }
/// ```
///
/// ## Playing samples in a pool
///
/// Once you've spawned a pool, playing samples in it is easy!
/// Just spawn your sample players with the label.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct SimplePool;
///
/// fn spawn_pool_and_play(mut commands: Commands, server: Res<AssetServer>) {
///     commands.spawn(SamplerPool(SimplePool));
///
///     commands.spawn((SimplePool, SamplePlayer::new(server.load("my_sample.wav"))));
/// }
/// ```
///
/// Pools with effects will automatically insert [`SampleEffects`]
/// into queued [`SamplePlayer`]s.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn overriding_effects(mut commands: Commands, server: Res<AssetServer>) {
/// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct SpatialPool;
///
/// commands.spawn((
///     SamplerPool(SpatialPool),
///     sample_effects![SpatialBasicNode::default()],
/// ));
///
/// // Once spawned, this entity will receive a
/// // `SamplerEffects` pointing to a `SpatialBasicNode`
/// commands.spawn((SpatialPool, SamplePlayer::new(server.load("my_sample.wav"))));
/// # }
/// ```
///
/// See [`SampleEffects`][crate::pool::sample_effects::SampleEffects#static-pools] for more details.
///
/// ## Architecture
///
/// Sampler pools are collections of individual
/// sampler nodes, each of which can play a single sample at a time.
/// When samples are queued up for playback, `bevy_seedling` will
/// look for the best sampler in the corresponding pool. If a suitable
/// sampler is found, the sample will begin playback, otherwise
/// waiting until a slot opens up. If the time spent waiting exceeds
/// a sample's [`SampleQueueLifetime`][crate::sample::SampleQueueLifetime],
/// the sample's playback is considered complete, and the [`OnComplete`] effect
/// is applied.
///
/// Each sampler node is routed to a final volume node. For a simple pool:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn simple_pool(mut commands: Commands) {
/// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct SimplePool;
///
/// commands.spawn(SamplerPool(SimplePool));
/// # }
/// ```
///
/// We end up with a graph like:
///
/// ```text
/// ┌───────┐┌───────┐┌───────┐┌───────┐
/// │Sampler││Sampler││Sampler││Sampler│
/// └┬──────┘└┬──────┘└┬──────┘└┬──────┘
/// ┌▽────────▽────────▽────────▽┐
/// │Volume                      │
/// └┬───────────────────────────┘
/// ┌▽──────┐
/// │MainBus│
/// └───────┘
/// ```
///
/// If a pool includes effects, these are inserted in series with each sampler. For a pool
/// with spatial processing:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn spatial_pool(mut commands: Commands) {
/// # #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// # struct SpatialPool;
/// commands.spawn((SpatialPool, sample_effects![SpatialBasicNode::default()]));
/// # }
/// ```
///
/// We end up with a graph like:
///
/// ```text
/// ┌───────┐┌───────┐┌───────┐┌───────┐
/// │Sampler││Sampler││Sampler││Sampler│
/// └┬──────┘└┬──────┘└┬──────┘└┬──────┘
/// ┌▽──────┐┌▽──────┐┌▽──────┐┌▽──────┐
/// │Spatial││Spatial││Spatial││Spatial│
/// └┬──────┘└┬──────┘└┬──────┘└┬──────┘
/// ┌▽────────▽────────▽────────▽┐
/// │Volume                      │
/// └┬───────────────────────────┘
/// ┌▽──────┐
/// │MainBus│
/// └───────┘
/// ```
#[derive(Debug, Component)]
#[component(immutable, on_insert = Self::on_insert_hook)]
#[require(PoolMarker)]
pub struct SamplerPool<T: PoolLabel + Component + Clone>(pub T);

impl<T: PoolLabel + Component + Clone> SamplerPool<T> {
    fn on_insert_hook(mut world: DeferredWorld, context: HookContext) {
        world.commands().queue(move |world: &mut World| {
            let id = match world.component_id::<T>() {
                Some(id) => id,
                None => world.register_component::<T>(),
            };

            let Some(value) = world.get::<SamplerPool<T>>(context.entity) else {
                return;
            };

            let container = PoolLabelContainer::new(&value.0, id);
            world.entity_mut(context.entity).insert(container);
        });
    }
}

/// A simple marker to make it easy to distinguish pools in a type-erased way.
#[derive(Component, Default)]
struct PoolMarker;

#[derive(Debug, Component)]
#[relationship(relationship_target = PoolSamplers)]
struct PoolSamplerOf(pub Entity);

#[derive(Debug, Component)]
#[relationship_target(relationship = PoolSamplerOf, linked_spawn)]
struct PoolSamplers(Vec<Entity>);

/// A wrapper for Firewheel's sampler state.
#[derive(Component, Clone)]
struct SamplerStateWrapper(SamplerState);

/// A sampler assignment relationships.
///
/// This resides in the [`SamplerNode`] entity, pointing to the
/// [`SamplePlayer`] entity it has been allocated for.
#[derive(Debug, Component)]
#[relationship(relationship_target = Sampler)]
#[component(on_remove = Self::on_remove_hook)]
pub struct SamplerOf(pub Entity);

impl SamplerOf {
    fn on_remove_hook(mut world: DeferredWorld, context: HookContext) {
        if let Some(mut sampler) = world.get_mut::<SamplerNode>(context.entity) {
            sampler.stop();
        }
    }
}

/// A relationship that provides information about a sample player's
/// assigned [`SamplerNode`].
///
/// This component is inserted on a [`SamplePlayer`] entity when a
/// sampler in the corresponding pool has been successfully allocated.
/// [`Sampler`] provides precise information about a sample's playback
/// status using shared atomics. Depending on the audio sample rate,
/// the number of frames in a processing block, and frequency at which
/// this data is checked, you may notice jitter in the playhead.
#[derive(Component)]
#[relationship_target(relationship = SamplerOf)]
#[component(on_insert = Self::on_insert_hook)]
pub struct Sampler {
    #[relationship]
    sampler: Entity,
    state: Option<SamplerState>,
}

impl Sampler {
    /// Returns the underlying sampler entity.
    pub fn sampler(&self) -> Entity {
        self.sampler
    }

    /// Returns whether this sample is currently playing.
    pub fn is_playing(&self) -> bool {
        self.state
            .as_ref()
            .map(|s| !s.stopped())
            .unwrap_or_default()
    }

    /// Returns the current playhead in frames.
    ///
    /// # Panics
    ///
    /// If the sample player has not yet propagated to the audio
    /// graph, this information may not yet be available. For a
    /// fallible method, see [`try_playhead_frames`][Self::try_playhead_frames].
    pub fn playhead_frames(&self) -> u64 {
        self.try_playhead_frames().unwrap()
    }

    /// Returns the current playhead in frames.
    ///
    /// If the sample player has not yet propagated to the audio
    /// graph, this returns `None`.
    pub fn try_playhead_frames(&self) -> Option<u64> {
        self.state.as_ref().map(|s| s.playhead_frames())
    }
}

impl core::fmt::Debug for Sampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamplerAssignment")
            .field("sampler", &self.sampler)
            .finish_non_exhaustive()
    }
}

impl Sampler {
    fn on_insert_hook(mut world: DeferredWorld, context: HookContext) {
        let sampler = world.get::<Sampler>(context.entity).unwrap().sampler;

        // We'll attempt to eagerly fill the state here, otherwise falling
        // back to `retrieve_State` when it's not ready.
        if let Some(state) = world.get::<SamplerStateWrapper>(sampler).cloned() {
            world.get_mut::<Sampler>(context.entity).unwrap().state = Some(state.0);
        }
    }
}

#[derive(Component)]
struct PoolShape(Vec<ComponentId>);

fn fetch_effect_ids(
    effects: &[Entity],
    lens: &mut QueryLens<&EffectId>,
) -> core::result::Result<Vec<ComponentId>, SeedlingError> {
    let query = lens.query();

    let mut effect_ids = Vec::new();
    effect_ids.reserve_exact(effects.len());
    for entity in effects {
        let id = query
            .get(*entity)
            .map_err(|_| SeedlingError::MissingEffect {
                empty_entity: *entity,
            })?;

        effect_ids.push(id.0);
    }

    Ok(effect_ids)
}

fn retrieve_state(
    q: Query<
        (Entity, &FirewheelNode, Option<&SamplerOf>),
        (With<SamplerNode>, Without<SamplerStateWrapper>),
    >,
    mut samples: Query<&mut Sampler>,
    mut commands: Commands,
    mut context: ResMut<AudioContext>,
) -> Result {
    if q.iter().len() == 0 {
        return Ok(());
    }

    context.with(|ctx| {
        for (entity, node_id, sampler_of) in q.iter() {
            let Some(state) = ctx.node_state::<SamplerState>(node_id.0) else {
                continue;
            };
            commands
                .entity(entity)
                .insert(SamplerStateWrapper(state.clone()));

            // If the sampler already has an assignment, we'll need to
            // provide the state here since it couldn't have been eagerly
            // fetched.
            if let Some(sampler_of) = sampler_of {
                let mut source = samples.get_mut(sampler_of.0)?;
                source.state = Some(state.clone());
            }
        }

        Ok(())
    })
}

/// A kind of specialization of [`FollowerOf`][crate::node::follower::FollowerOf] for
/// sampler nodes.
fn watch_sample_players(
    mut q: Query<(&mut SamplerNode, &SamplerOf)>,
    samples: Query<&PlaybackSettings>,
) {
    for (mut sampler_node, sample) in q.iter_mut() {
        let Ok(settings) = samples.get(sample.0) else {
            continue;
        };

        sampler_node.playhead = settings.playhead.clone();
        sampler_node.playback = settings.playback.clone();
        sampler_node.speed = settings.speed;
    }
}

fn spawn_chain(
    bus: Entity,
    config: Option<SamplerConfig>,
    effects: &[Entity],
    commands: &mut Commands,
) -> Entity {
    let sampler = commands
        .spawn((
            SamplerNode::default(),
            config.unwrap_or_default(),
            PoolSamplerOf(bus),
        ))
        .id();

    let effects = effects.to_vec();
    commands.queue(move |world: &mut World| -> Result {
        let mut cloner = EntityCloner::build(world);
        cloner.deny::<EffectOf>();
        let mut cloner = cloner.finish();

        let mut chain = Vec::new();
        chain.reserve_exact(effects.len() + 1);
        for effect in effects {
            chain.push(cloner.spawn_clone(world, effect));
        }
        chain.push(bus);

        // Until we come up with a good way to implement the
        // connect trait for `WorldEntityMut`, we're stuck with
        // a bit of boilerplate.
        world
            .get_entity_mut(sampler)?
            .add_children(&chain)
            .entry::<PendingConnections>()
            .or_default()
            .into_mut()
            .push(PendingEdge::new(chain[0], None));

        for pair in chain.windows(2) {
            world
                .get_entity_mut(pair[0])?
                .entry::<PendingConnections>()
                .or_default()
                .into_mut()
                .push(PendingEdge::new(pair[1], None));
        }

        Ok(())
    });

    sampler
}

/// The size of a [`SamplerPool`].
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn spawn_pool_and_play(mut commands: Commands, server: Res<AssetServer>) {
/// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct SimplePool;
///
/// commands.spawn((SamplerPool(SimplePool), PoolSize(4..=16)));
/// # }
/// ```
///
/// This size is expressed as a range so that [`SamplerPool`]s can
/// grow to meet demand when necessary, and otherwise claim as few
/// resources as necessary. If a size isn't explicitly provided,
/// it'll be initialized according to the [`DefaultPoolSize`] resource.
///
/// Pools are grown quadratically, so the cost of queuing samples
/// is roughly amortized constant.
#[derive(Debug, Clone, Component)]
pub struct PoolSize(pub RangeInclusive<usize>);

/// The default [`PoolSize`] applied to [`SamplerPool`]s.
///
/// The default is `4..=32`.
/// When set to `0..=0`, dynamic pools are disabled.
#[derive(Debug, Clone, Resource)]
pub struct DefaultPoolSize(pub RangeInclusive<usize>);

impl Default for DefaultPoolSize {
    fn default() -> Self {
        Self(4..=32)
    }
}

fn populate_pool(
    q: Query<
        (
            Entity,
            &SamplerConfig,
            Option<&PoolSize>,
            Option<&SampleEffects>,
            Option<&EffectId>,
        ),
        (
            With<PoolLabelContainer>,
            With<PoolMarker>,
            Without<PoolSamplers>,
        ),
    >,
    mut effects: Query<&EffectId>,
    default_pool_size: Res<DefaultPoolSize>,
    mut commands: Commands,
) -> Result {
    for (pool, config, size, pool_effects, effect_id) in &q {
        if effect_id.is_none() {
            commands.entity(pool).insert(VolumeNode::default());
        }

        let component_ids = fetch_effect_ids(
            pool_effects.map(|e| e.deref()).unwrap_or(&[]),
            &mut effects.as_query_lens(),
        )?;

        let size = size
            .map(|p| p.0.clone())
            .unwrap_or(default_pool_size.0.clone());

        commands
            .entity(pool)
            .insert((PoolShape(component_ids), PoolSize(size.clone())));

        let size = (*size.start()).max(1);
        let config = config.clone();
        for _ in 0..size {
            spawn_chain(
                pool,
                Some(config.clone()),
                pool_effects.map(|e| e.deref()).unwrap_or(&[]),
                &mut commands,
            );
        }
    }

    Ok(())
}

/// An event triggered on [`SamplePlayer`] entities when
/// their playback completes.
///
/// Note that this may be triggered even when the sample isn't
/// played, including when its playback is set to
/// [`PlaybackState::Stop`][crate::prelude::PlaybackState] or
/// when it can't find space in a sampler pool.
#[derive(Debug, Event)]
pub struct PlaybackCompletionEvent;

/// Clean up sample resources according to their playback settings.
fn remove_finished(
    trigger: Trigger<PlaybackCompletionEvent>,
    samples: Query<(&PlaybackSettings, &PoolLabelContainer)>,
    mut commands: Commands,
) -> Result {
    let sample_entity = trigger.target();
    let (settings, container) = samples.get(sample_entity)?;

    match settings.on_complete {
        OnComplete::Preserve => {
            commands
                .entity(sample_entity)
                .remove::<(Sampler, QueuedSample, SkipTimer)>();
        }
        OnComplete::Remove => {
            commands
                .entity(sample_entity)
                .remove_by_id(container.label_id)
                .remove_with_requires::<(
                    SampleEffects,
                    SamplePlayer,
                    PoolLabelContainer,
                    Sampler,
                    QueuedSample,
                    SkipTimer,
                )>();
        }
        OnComplete::Despawn => {
            commands.entity(sample_entity).despawn();
        }
    }

    Ok(())
}

/// Automatically remove or despawn sample players when their
/// sample has finished playing.
fn poll_finished(
    nodes: Query<(&SamplerNode, &SamplerOf, &SamplerStateWrapper)>,
    mut commands: Commands,
) {
    for (node, active, state) in nodes.iter() {
        let finished = state.0.finished() == node.sequence.id();

        if finished {
            commands.entity(active.0).trigger(PlaybackCompletionEvent);
        }
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
/// # use bevy::prelude::*;
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

impl<T: PoolLabel + Component + Clone> PoolDespawn<T> {
    /// Construct a new [`PoolDespawn`] with the provided label.
    pub fn new(label: T) -> Self {
        Self(label)
    }
}

impl<T: PoolLabel + Component + Clone> Command for PoolDespawn<T> {
    fn apply(self, world: &mut World) {
        let mut roots = world.query_filtered::<(Entity, &PoolLabelContainer), (
            With<SamplerPool<T>>,
            With<PoolSamplers>,
            With<FirewheelNode>,
        )>();

        let roots: Vec<_> = roots
            .iter(world)
            .map(|(root, label)| (root, label.clone()))
            .collect();

        let mut commands = world.commands();

        let interned = self.0.intern();
        for (root, label) in roots {
            if label.label == interned {
                commands.entity(root).despawn();
            }
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
    fn despawn_pool<T: PoolLabel + Component + Clone>(&mut self, label: T);
}

impl PoolCommands for Commands<'_, '_> {
    fn despawn_pool<T: PoolLabel + Component + Clone>(&mut self, label: T) {
        self.queue(PoolDespawn::new(label));
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        prelude::LowPassNode,
        sample_effects,
        test::{prepare_app, run},
    };
    use bevy_seedling_macros::PoolLabel;

    #[derive(PoolLabel, Clone, Debug, PartialEq, Eq, Hash)]
    struct TestPool;

    #[test]
    fn test_spawn() {
        let mut app = prepare_app(|mut commands: Commands| {
            commands.spawn((
                SamplerPool(TestPool),
                sample_effects![LowPassNode::default()],
            ));
        });

        run(
            &mut app,
            |q: Query<&PoolSamplers, With<SamplerPool<TestPool>>>| {
                assert_eq!(q.iter().len(), 1);
            },
        );
    }

    #[test]
    fn test_despawn() {
        let mut app = prepare_app(|mut commands: Commands| {
            commands.spawn((
                SamplerPool(TestPool),
                PoolSize(4..=32),
                sample_effects![LowPassNode::default()],
            ));
        });

        run(&mut app, |pool_nodes: Query<&FirewheelNode>| {
            // 2 * 4 (sampler and low pass nodes) + (pool volume) + 1 (global volume) + 1 (input)
            assert_eq!(pool_nodes.iter().count(), 11);
        });

        run(&mut app, |mut commands: Commands| {
            commands.despawn_pool(TestPool);
        });

        app.update();

        run(&mut app, |pool_nodes: Query<&FirewheelNode>| {
            // 1 (global volume) + 1 (input)
            assert_eq!(pool_nodes.iter().count(), 2);
        });
    }

    #[test]
    fn test_playback_starts() {
        let mut app = prepare_app(|mut commands: Commands, server: Res<AssetServer>| {
            commands.spawn((
                SamplerPool(TestPool),
                sample_effects![LowPassNode::default()],
            ));
            commands.spawn((
                TestPool,
                SamplePlayer::new(server.load("caw.ogg")).looping(),
                EmptyComponent,
            ));
        });

        loop {
            let players = run(
                &mut app,
                |q: Query<Entity, (With<SamplePlayer>, With<Sampler>)>| q.iter().len(),
            );

            if players == 1 {
                break;
            }

            app.update();
        }
    }

    #[derive(Component)]
    struct EmptyComponent;

    #[test]
    fn test_remove_in_dynamic() {
        let mut app = prepare_app(|mut commands: Commands, server: Res<AssetServer>| {
            // We'll play a short sample
            commands.spawn((
                SamplePlayer::new(server.load("sine_440hz_1ms.wav")),
                EmptyComponent,
                PlaybackSettings {
                    on_complete: OnComplete::Remove,
                    ..Default::default()
                },
                sample_effects![LowPassNode::default()],
            ));
        });

        // Then wait until the sample player is removed.
        loop {
            let players = run(
                &mut app,
                |q: Query<Entity, (With<SamplePlayer>, With<EmptyComponent>)>| q.iter().len(),
            );

            if players == 0 {
                break;
            }

            app.update();
        }

        // Once removed, we'll verify that _all_ audio-related components are removed.
        let world = app.world_mut();
        let mut q = world.query_filtered::<EntityRef, With<EmptyComponent>>();
        let entity = q.single(world).unwrap();

        let archetype = entity.archetype();

        assert_eq!(archetype.components().count(), 1);
        assert!(entity.contains::<EmptyComponent>());
    }

    #[test]
    fn test_remove_in_pool() {
        let mut app = prepare_app(|mut commands: Commands, server: Res<AssetServer>| {
            commands.spawn((
                SamplerPool(TestPool),
                sample_effects![LowPassNode::default()],
            ));

            commands.spawn((
                TestPool,
                SamplePlayer::new(server.load("sine_440hz_1ms.wav")),
                EmptyComponent,
                PlaybackSettings {
                    on_complete: OnComplete::Remove,
                    ..Default::default()
                },
            ));
        });

        // Then wait until the sample player is removed.
        loop {
            let players = run(
                &mut app,
                |q: Query<Entity, (With<SamplePlayer>, With<EmptyComponent>)>| q.iter().len(),
            );

            if players == 0 {
                break;
            }

            app.update();
        }

        // Once removed, we'll verify that _all_ audio-related components are removed.
        let world = app.world_mut();
        let mut q = world.query_filtered::<EntityRef, With<EmptyComponent>>();
        let entity = q.single(world).unwrap();

        let archetype = entity.archetype();

        assert_eq!(archetype.components().count(), 1);
        assert!(entity.contains::<EmptyComponent>());
    }
}
