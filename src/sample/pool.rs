use super::{PlaybackSettings, QueuedSample, Sample, SamplePlayer};
use crate::{node::Events, ConnectNode, SamplePoolSize, SeedlingSystems};
use bevy_app::{Last, Plugin, PreStartup};
use bevy_asset::Assets;
use bevy_ecs::{component::ComponentId, prelude::*, world::DeferredWorld};
use bevy_hierarchy::DespawnRecursiveExt;
use firewheel::{
    node::NodeEventType,
    sampler::{SamplerConfig, SamplerNode},
};
use seedling_macros::NodeLabel;
use std::sync::atomic::Ordering;

pub(crate) struct SamplePoolPlugin;

impl Plugin for SamplePoolPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<NodeRank>()
            .add_systems(PreStartup, spawn_pool)
            .add_systems(
                Last,
                (remove_finished, rank_nodes, assign_work, monitor_active)
                    .chain()
                    .in_set(SeedlingSystems::Queue),
            );
    }
}

#[derive(Component)]
struct SamplePoolNode;

/// The bus node that all nodes in the default sample
/// pool are routed to.
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SamplePoolBus;

#[derive(Resource, Default)]
struct NodeRank(Vec<(Entity, u64)>);

fn spawn_pool(mut commands: Commands, size: Res<SamplePoolSize>) {
    // spawn the bus
    commands.spawn((crate::VolumeNode::new(1.0), SamplePoolBus));

    for _ in 0..size.0 {
        commands
            .spawn((
                crate::SamplerNode::new(SamplerConfig::default()),
                SamplePoolNode,
            ))
            .connect(SamplePoolBus);
    }
}

fn rank_nodes(q: Query<(Entity, &SamplerNode), With<SamplePoolNode>>, mut rank: ResMut<NodeRank>) {
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

fn assign_work(
    mut nodes: Query<(Entity, &mut Events), With<SamplePoolNode>>,
    queued_samples: Query<(Entity, &SamplePlayer, &PlaybackSettings), With<QueuedSample>>,
    mut rank: ResMut<NodeRank>,
    assets: Res<Assets<Sample>>,
    mut commands: Commands,
) {
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
