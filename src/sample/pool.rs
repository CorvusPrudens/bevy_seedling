use super::{label::PoolLabelContainer, PlaybackSettings, QueuedSample, Sample, SamplePlayer};
use crate::{node::Events, ConnectNode, DefaultPool, PoolLabel, SeedlingSystems};
use bevy_app::{Last, Plugin};
use bevy_asset::Assets;
use bevy_ecs::{component::ComponentId, prelude::*, world::DeferredWorld};
use bevy_hierarchy::{BuildChildren, DespawnRecursiveExt};
use firewheel::{
    node::NodeEventType,
    sampler::{SamplerConfig, SamplerNode},
};
use std::sync::atomic::Ordering;

pub(crate) struct SamplePoolPlugin;

impl Plugin for SamplePoolPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            Last,
            (
                (remove_finished, assign_default)
                    .before(SeedlingSystems::Queue)
                    .after(SeedlingSystems::Connect),
                monitor_active
                    .before(SeedlingSystems::Flush)
                    .after(SeedlingSystems::Queue),
            ),
        );
    }
}

/// Provides methods on [`Commands`] to spawn new sample pools.
pub trait SpawnPool {
    /// Spawn a sample pool, returning the [`EntityCommands`] for
    /// the terminal volume node.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// use bevy_seedling::{SpawnPool, PoolLabel, SamplePlayer};
    ///
    /// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
    /// struct CustomPool;
    ///
    /// fn spawn_custom_pool(server: Res<AssetServer>, mut commands: Commands) {
    ///     // Spawn a custom sample pool
    ///     commands.spawn_pool(CustomPool, 16);
    ///
    ///     // Trigger sample playback in the custom pool
    ///     commands.spawn((
    ///         SamplePlayer::new(server.load("my_sample.wav")),
    ///         CustomPool,
    ///     ));
    /// }
    /// ```
    fn spawn_pool<T: PoolLabel + Component + Clone>(
        &mut self,
        marker: T,
        size: usize,
    ) -> EntityCommands<'_> {
        self.spawn_pool_with(marker, size, 1.0)
    }

    /// Spawn a sample pool with an initial volume, returning the [`EntityCommands`] for
    /// the terminal volume node.
    fn spawn_pool_with<T: PoolLabel + Component + Clone>(
        &mut self,
        marker: T,
        size: usize,
        volume: f32,
    ) -> EntityCommands<'_>;

    /// Despawn a sample pool, cleaning up its resources
    /// in the ECS and audio graph.
    ///
    /// Despawning the terminal volume node recursively
    /// will produce the same effect.
    fn despawn_pool<T: PoolLabel + Component>(&mut self);
}

impl SpawnPool for Commands<'_, '_> {
    fn spawn_pool_with<T: PoolLabel + Component + Clone>(
        &mut self,
        marker: T,
        size: usize,
        volume: f32,
    ) -> EntityCommands<'_> {
        self.queue(|world: &mut World| {
            world.schedule_scope(Last, |_, schedule| {
                schedule.add_systems(
                    (rank_nodes::<T>, assign_work::<T>)
                        .chain()
                        .in_set(SeedlingSystems::Queue),
                );
            });
        });

        let bus = self
            .spawn((
                crate::VolumeNode::new(volume),
                SamplePoolNode,
                marker.clone(),
            ))
            .id();

        let rank = self.spawn((NodeRank::default(), marker.clone())).id();

        let nodes: Vec<_> = (0..size)
            .map(|_| {
                self.spawn((
                    crate::SamplerNode::new(SamplerConfig::default()),
                    SamplePoolNode,
                    marker.clone(),
                ))
                .connect(bus)
                .id()
            })
            .collect();

        let mut commands = self.entity(bus);

        commands.add_children(&nodes).add_child(rank);

        commands
    }

    fn despawn_pool<T: PoolLabel + Component>(&mut self) {
        self.queue(|world: &mut World| {
            let mut roots = world
                .query_filtered::<Entity, (With<T>, With<SamplePoolNode>, With<crate::VolumeNode>)>(
                );

            let roots: Vec<_> = roots.iter(&world).collect();

            let mut commands = world.commands();

            for root in roots {
                commands.entity(root).despawn_recursive();
            }
        });
    }
}

#[derive(Component)]
struct SamplePoolNode;

#[derive(Default, Component)]
struct NodeRank(Vec<(Entity, u64)>);

fn rank_nodes<T: Component>(
    q: Query<(Entity, &SamplerNode), (With<SamplePoolNode>, With<T>)>,
    mut rank: Query<&mut NodeRank, With<T>>,
) {
    let Ok(mut rank) = rank.get_single_mut() else {
        return;
    };

    rank.0.clear();

    for (e, sampler) in q.iter() {
        let state = sampler.state();
        let score = state
            .status
            .load()
            .new_work_score(state.playhead_frames.load(Ordering::Relaxed));

        rank.0.push((e, score));
    }

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

fn remove_finished(
    nodes: Query<(Entity, &SamplerNode), With<ActiveSample>>,
    mut commands: Commands,
) {
    for (entity, sampler) in nodes.iter() {
        let state = sampler.state().status.load();

        if state.finished() {
            commands.entity(entity).remove::<ActiveSample>();
        }
    }
}

fn assign_work<T: Component>(
    mut nodes: Query<(Entity, &mut Events), (With<SamplePoolNode>, With<T>)>,
    queued_samples: Query<
        (Entity, &SamplePlayer, &PlaybackSettings),
        (With<QueuedSample>, With<T>),
    >,
    mut rank: Query<&mut NodeRank, With<T>>,
    assets: Res<Assets<Sample>>,
    mut commands: Commands,
) {
    let Ok(mut rank) = rank.get_single_mut() else {
        return;
    };

    for (sample, player, settings) in queued_samples.iter() {
        let Some(asset) = assets.get(&player.0) else {
            continue;
        };

        // get the best candidate
        let Some((node_entity, _)) = rank.0.first() else {
            continue;
        };

        let Ok((node_entity, mut events)) = nodes.get_mut(*node_entity) else {
            continue;
        };

        events.push(NodeEventType::NewSample {
            sample: asset.get(),
            normalized_volume: settings.volume,
            repeat_mode: settings.mode,
        });
        events.push(NodeEventType::StartOrRestart);

        rank.0.remove(0);
        commands.entity(sample).remove::<QueuedSample>();
        commands.entity(node_entity).insert(ActiveSample {
            sample_entity: sample,
            despawn: true,
        });
    }
}

// Stop playback if the source entity no longer exists.
fn monitor_active(
    mut nodes: Query<(Entity, &ActiveSample, &mut Events)>,
    samples: Query<&SamplePlayer>,
    mut commands: Commands,
) {
    for (node_entity, active, mut events) in nodes.iter_mut() {
        if samples.get(active.sample_entity).is_err() {
            events.push(NodeEventType::Stop);

            commands.entity(node_entity).remove::<ActiveSample>();
        }
    }
}

fn assign_default(
    samples: Query<Entity, (With<SamplePlayer>, Without<PoolLabelContainer>)>,
    mut commands: Commands,
) {
    for sample in samples.iter() {
        commands.entity(sample).insert(DefaultPool);
    }
}
