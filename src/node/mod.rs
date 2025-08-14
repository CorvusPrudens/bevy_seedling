//! Audio node registration and management.

use crate::edge::NodeMap;
use crate::error::SeedlingError;
use crate::pool::sample_effects::EffectOf;
use crate::{SeedlingSystems, prelude::AudioContext};
use bevy_app::prelude::*;
use bevy_ecs::{
    component::{ComponentId, HookContext, Mutable},
    prelude::*,
    world::DeferredWorld,
};
use bevy_log::prelude::*;
use bevy_platform::collections::HashSet;
use firewheel::error::UpdateError;
use firewheel::{
    diff::{Diff, Patch},
    event::{NodeEvent, NodeEventType},
    node::{AudioNode, NodeID},
};
use std::any::TypeId;
use std::ops::DerefMut;

pub mod events;
pub mod follower;
pub mod label;

use events::AudioEvents;
use label::NodeLabels;

/// A node's baseline instance.
///
/// This is used as the baseline for diffing.
#[derive(Component)]
pub(crate) struct Baseline<T>(pub(crate) T);

/// A component that communicates an effect is present on an entity.
///
/// This is used for sample pool bookkeeping.
#[derive(Component, Clone, Copy)]
pub(crate) struct EffectId(pub(crate) ComponentId);

fn apply_patch<T: Patch>(value: &mut T, event: &NodeEventType) -> Result {
    let NodeEventType::Param { data, path } = event else {
        return Ok(());
    };

    let patch = T::patch(data, path).map_err(|e| SeedlingError::PatchError {
        ty: core::any::type_name::<T>(),
        error: e,
    })?;

    value.apply(patch);

    Ok(())
}

fn generate_param_events<T: Diff + Patch + Component + Clone>(
    mut nodes: Query<(&T, &mut Baseline<T>, &mut AudioEvents), (Changed<T>, Without<EffectOf>)>,
) -> Result {
    for (params, mut baseline, mut events) in nodes.iter_mut() {
        // This ensures we only apply patches that were generated here.
        // I'm not sure this is correct in all cases, though.
        let starting_len = events.queue.len();

        params.diff(&baseline.0, Default::default(), events.deref_mut());

        // Patch the baseline.
        for (_, event) in &events.queue[starting_len..] {
            apply_patch(&mut baseline.0, event)?;
        }
    }

    Ok(())
}

fn handle_configuration_changes<
    T: AudioNode<Configuration: Component + PartialEq + Clone> + Component + Clone,
>(
    mut configs: Query<
        (
            Entity,
            &T,
            &FirewheelNode,
            &T::Configuration,
            &mut Baseline<T::Configuration>,
        ),
        Changed<T::Configuration>,
    >,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
) -> Result {
    let changes: Vec<_> = configs.iter_mut().filter(|(.., c, b)| *c != &b.0).collect();
    if changes.is_empty() {
        return Ok(());
    }

    context.with(|context| {
        for (entity, node, node_id, config, mut baseline) in changes {
            // we have to get them every time, which is kind of annoying
            let edges = context.edges();
            let existing_inputs = edges
                .iter()
                .filter(|e| e.dst_node == node_id.0)
                .map(|e| firewheel::graph::Edge::clone(e))
                .collect::<Vec<_>>();
            let existing_outputs = edges
                .iter()
                .filter(|e| e.src_node == node_id.0)
                .map(|e| firewheel::graph::Edge::clone(e))
                .collect::<Vec<_>>();

            let new_node = context.add_node(node.clone(), Some(config.clone()));
            commands.entity(entity).insert(FirewheelNode(new_node));

            for edge in existing_inputs {
                context.connect(
                    edge.src_node,
                    new_node,
                    &[(edge.src_port, edge.dst_port)],
                    true,
                )?;
            }

            for edge in existing_outputs {
                context.connect(
                    new_node,
                    edge.dst_node,
                    &[(edge.src_port, edge.dst_port)],
                    true,
                )?;
            }

            baseline.0 = config.clone();
        }

        Ok(())
    })
}

fn acquire_id<T>(
    q: Query<
        (Entity, &T, Option<&T::Configuration>, Option<&NodeLabels>),
        (Without<FirewheelNode>, Without<EffectOf>),
    >,
    mut context: ResMut<AudioContext>,
    mut node_map: ResMut<NodeMap>,
    mut commands: Commands,
) where
    T: AudioNode<Configuration: Component + Clone> + Component + Clone,
{
    if q.iter().len() == 0 {
        return;
    }

    context.with(|context| {
        for (entity, container, config, labels) in q.iter() {
            let node = context.add_node(container.clone(), config.cloned());

            for label in labels.iter().flat_map(|l| l.iter()) {
                node_map.insert(*label, entity);
            }

            commands.entity(entity).insert(FirewheelNode(node));
        }
    });
}

fn insert_baseline<T: Component + Clone>(
    trigger: Trigger<OnInsert, T>,
    q: Query<&T>,
    mut commands: Commands,
) -> Result {
    let value = q.get(trigger.target())?;
    commands
        .entity(trigger.target())
        .insert(Baseline(value.clone()));

    Ok(())
}

/// A container for an audio node's state type.
#[derive(Debug, Component)]
// TODO: manage reflect
// #[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct AudioState<T>(pub T);

fn fetch_state<T, S>(
    q: Query<(Entity, &FirewheelNode), (Changed<FirewheelNode>, With<T>)>,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
) where
    T: AudioNode + Component,
    S: Clone + Send + Sync + 'static,
{
    // likely not expensive enough to matter, relative to context switching
    if q.iter().count() == 0 {
        return;
    }

    context.with(|context| {
        for (entity, node) in q.iter() {
            match context.node_state::<S>(node.0) {
                Some(state) => {
                    commands.entity(entity).insert(AudioState(state.clone()));
                }
                None => {
                    bevy_log::error!(
                        "Failed to fetch state `{}` for node `{}`",
                        core::any::type_name::<S>(),
                        core::any::type_name::<T>(),
                    );
                }
            }
        }
    });
}

#[derive(Resource, Default)]
struct RegisteredNodes(HashSet<TypeId>);

impl RegisteredNodes {
    /// Insert the `TypeId` of `T`.
    ///
    /// Returns `true` if the ID wasn't already present.
    fn insert<T: core::any::Any>(&mut self) -> bool {
        self.0.insert(TypeId::of::<T>())
    }
}

#[derive(Resource, Default)]
struct RegisteredConfigs(HashSet<TypeId>);

impl RegisteredConfigs {
    /// Insert the `TypeId` of `T`.
    ///
    /// Returns `true` if the ID wasn't already present.
    fn insert<T: core::any::Any>(&mut self) -> bool {
        self.0.insert(TypeId::of::<T>())
    }
}

#[derive(Resource, Default)]
struct RegisteredState(HashSet<(TypeId, TypeId)>);

impl RegisteredState {
    /// Insert the `TypeId` of `T` and `U`.
    ///
    /// Returns `true` if the tuple wasn't already present.
    fn insert<T: core::any::Any, U: core::any::Any>(&mut self) -> bool {
        self.0.insert((TypeId::of::<T>(), TypeId::of::<U>()))
    }
}

/// Register audio nodes in the ECS.
///
/// ## Creating and registering nodes
///
/// A Firewheel *node* is the smallest unit of audio processing.
/// It can receive inputs, produce outputs, or both, meaning nodes
/// can be used as sources, sinks, or effects.
///
/// The core trait for nodes is Firewheel's [`AudioNode`]. For examples
/// on how to create nodes, see
/// [`bevy_seedling`'s custom node example](https://github.com/CorvusPrudens/bevy_seedling/blob/master/examples/custom_node.rs),
/// as well as [Firewheel's examples](https://github.com/BillyDM/Firewheel/tree/main/examples/custom_nodes).
/// Note that you'll need to depend on Firewheel separately to get access
/// to all its node traits and types.
///
/// Once you've implemented [`AudioNode`] on a type, there are two ways to register it:
/// - [`RegisterNode::register_node`] for nodes that also implement [`Diff`] and [`Patch`]
/// - [`RegisterNode::register_simple_node`] for nodes that do not implement [`Diff`] and [`Patch`]
///
/// ```ignore
/// use bevy::prelude::*;
/// use bevy_seedling::prelude::*;
///
/// // Let's assume the relevant traits are implemented.
/// struct CustomNode;
///
/// fn main() {
///     App::new()
///         .add_plugins((DefaultPlugins, SeedlingPlugin::default()))
///         .register_simple_node::<CustomNode>();
/// }
/// ```
///
/// Once registered, you can use your nodes like any other
/// built-in Firewheel or `bevy_seedling` node.
///
/// ## Synchronizing ECS and audio types
///
/// For nodes with parameters, you'll probably want to implement Firewheel's [`Diff`]
/// and [`Patch`] traits. These are `bevy_seedling`'s primary mechanism for Synchronizing
/// data.
///
/// ```
/// use firewheel::diff::{Diff, Patch};
///
/// #[derive(Diff, Patch)]
/// struct FilterNode {
///     pub frequency: f32,
///     pub q: f32,
/// }
/// ```
///
/// When you register a node like `FilterNode`, `bevy_seedling` will register a
/// special *baseline* component. A node's baseline is compared with the real
/// value once per frame, and any differences are sent as patches directly to the
/// corresponding node in the audio graph. In other words, any changes
/// you make to a node in Bevy systems will be automatically
/// synchronized with the audio graph.
///
/// This *diffing* isn't just useful for ECS-to-Audio communications; `bevy_seedling`
/// also uses it to power the [`SampleEffects`][crate::prelude::SampleEffects] abstraction,
/// which makes it easy to modify parameters directly adjacent to sample players.
///
/// Diffing occurs in the [`SeedlingSystems::Queue`] system set during
/// the [`Last`] schedule. Diffing will only be applied to nodes that have
/// been mutated according to Bevy's [`Changed`] filter.
///
/// ## Audio node configuration
///
/// All Firewheel nodes have a configuration struct: the [`AudioNode::Configuration`]
/// associated type. When you register a node, its configuration
/// is added as a required component. Following the initial insertion
/// of the processor, any changes to the configuration component will
/// trigger automatic recreation and reinsertion.
pub trait RegisterNode {
    /// Register an audio node with automatic diffing.
    ///
    /// This will allow audio entities to automatically
    /// acquire IDs from the audio graph and perform
    /// parameter diffing.
    fn register_node<T>(&mut self) -> &mut Self
    where
        T: AudioNode<Configuration: Component + Clone + PartialEq>
            + Diff
            + Patch
            + Component<Mutability = Mutable>
            + Clone;

    /// Register an audio node without automatic diffing.
    ///
    /// This will allow audio entities to automatically
    /// acquire IDs from the audio graph and perform
    /// parameter diffing.
    fn register_simple_node<T>(&mut self) -> &mut Self
    where
        T: AudioNode<Configuration: Component + Clone + PartialEq> + Component + Clone;

    /// Register a state type for an audio node.
    ///
    /// After a node is inserted into the audio graph, its state is fetched and
    /// inserted on the node component in a [`NodeState`] wrapper.
    ///
    /// A node's state is constructed in Firewheel's [AudioNode::construct_processor]
    /// trait method, and subsequently inserted into the audio context. Nodes like
    /// [`SamplerNode`] and [`LoudnessNode`] use their state as a container for
    /// atomics that communicate their current state in the audio graph.
    ///
    /// [`SamplerNode`]: crate::prelude::SamplerNode
    /// [`LoudnessNode`]: crate::prelude::LoudnessNode
    fn register_node_state<T, S>(&mut self) -> &mut Self
    where
        T: AudioNode + Component,
        S: Clone + Send + Sync + 'static;
}

impl RegisterNode for App {
    #[cfg_attr(debug_assertions, track_caller)]
    fn register_node<T>(&mut self) -> &mut Self
    where
        T: AudioNode<Configuration: Component + Clone + PartialEq>
            + Diff
            + Patch
            + Component<Mutability = Mutable>
            + Clone,
    {
        let world = self.world_mut();
        let mut nodes = world.get_resource_or_init::<RegisteredNodes>();

        if nodes.insert::<T>() {
            world.register_component_hooks::<T>().on_insert(
                |mut world: DeferredWorld, context: HookContext| {
                    let value = world.get::<T>(context.entity).unwrap().clone();
                    world
                        .commands()
                        .entity(context.entity)
                        .insert((Baseline(value), EffectId(context.component_id)));
                },
            );
            world.register_required_components::<T, AudioEvents>();
            world.register_required_components::<T, T::Configuration>();
        } else {
            // TODO: we'll need to be more careful about getting type names
            // for upstreaming.
            #[cfg(debug_assertions)]
            {
                bevy_log::warn!(
                    "Audio node `{}` was registered more than once at {}",
                    core::any::type_name::<T>(),
                    std::panic::Location::caller(),
                );
            }

            #[cfg(not(debug_assertions))]
            bevy_log::warn!(
                "Audio node `{}` was registered more than once",
                core::any::type_name::<T>(),
            );

            return self;
        }

        // Different nodes may share configuration structs, so we need
        // to make sure this isn't registered more than once.
        let mut configs = world.get_resource_or_init::<RegisteredConfigs>();
        if configs.insert::<T::Configuration>() {
            world.add_observer(insert_baseline::<T::Configuration>);
        }

        self.add_systems(
            Last,
            (
                (acquire_id::<T>, handle_configuration_changes::<T>)
                    .chain()
                    .in_set(SeedlingSystems::Acquire),
                (follower::param_follower::<T>, generate_param_events::<T>)
                    .chain()
                    .in_set(SeedlingSystems::Queue),
            ),
        )
    }

    #[cfg_attr(debug_assertions, track_caller)]
    fn register_simple_node<T>(&mut self) -> &mut Self
    where
        T: AudioNode<Configuration: Component + Clone + PartialEq> + Component + Clone,
    {
        let world = self.world_mut();
        let mut nodes = world.get_resource_or_init::<RegisteredNodes>();

        if nodes.insert::<T>() {
            world.register_required_components::<T, AudioEvents>();
            world.register_required_components::<T, T::Configuration>();
        } else {
            #[cfg(debug_assertions)]
            {
                bevy_log::warn!(
                    "Audio node `{}` was registered more than once at {}",
                    core::any::type_name::<T>(),
                    std::panic::Location::caller(),
                );
            }

            #[cfg(not(debug_assertions))]
            bevy_log::warn!(
                "Audio node `{}` was registered more than once",
                core::any::type_name::<T>(),
            );

            return self;
        }

        // Different nodes may share configuration structs, so we need
        // to make sure this isn't registered more than once.
        let mut configs = world.get_resource_or_init::<RegisteredConfigs>();
        if configs.insert::<T::Configuration>() {
            world.add_observer(insert_baseline::<T::Configuration>);
        }

        self.add_systems(
            Last,
            (acquire_id::<T>, handle_configuration_changes::<T>)
                .chain()
                .in_set(SeedlingSystems::Acquire),
        )
    }

    #[cfg_attr(debug_assertions, track_caller)]
    fn register_node_state<T, S>(&mut self) -> &mut Self
    where
        T: AudioNode + Component,
        S: Clone + Send + Sync + 'static,
    {
        let world = self.world_mut();
        let mut nodes = world.get_resource_or_init::<RegisteredState>();

        if !nodes.insert::<T, S>() {
            #[cfg(debug_assertions)]
            {
                bevy_log::warn!(
                    "State `{}` was registered for node `{}` at {}",
                    core::any::type_name::<S>(),
                    core::any::type_name::<T>(),
                    std::panic::Location::caller(),
                );
            }

            #[cfg(not(debug_assertions))]
            bevy_log::warn!(
                "State `{}` registered more than once for node `{}`",
                core::any::type_name::<S>(),
                core::any::type_name::<T>(),
            );

            return self;
        }

        self.add_systems(
            Last,
            fetch_state::<T, S>
                .after(SeedlingSystems::Acquire)
                .before(SeedlingSystems::Connect),
        )
    }
}

/// An ECS handle for an audio node.
///
/// Firewheel nodes [registered with `bevy_seedling`][crate::prelude::RegisterNode]
/// will automatically acquire a [`FirewheelNode`] during the [`SeedlingSystems::Acquire`] set
/// in the [`Last`] schedule.
///
/// When this component is removed, the underlying
/// audio node is removed from the graph.
#[derive(Debug, Clone, Copy, Component)]
#[component(on_replace = Self::on_replace_hook, immutable)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct FirewheelNode(pub NodeID);

impl FirewheelNode {
    fn on_replace_hook(mut world: DeferredWorld, context: HookContext) {
        let Some(node) = world.get::<FirewheelNode>(context.entity).copied() else {
            return;
        };

        let mut removals = world.resource_mut::<PendingRemovals>();
        removals.push(node.0);
    }
}

/// Queued audio node removals.
///
/// This resource allows us to defer audio node removals
/// until the audio graph is ready.
#[derive(Debug, Default, Resource)]
pub(crate) struct PendingRemovals(Vec<NodeID>);

impl PendingRemovals {
    pub fn push(&mut self, node: NodeID) {
        self.0.push(node);
    }
}

pub(crate) fn flush_events(
    mut nodes: Query<(&FirewheelNode, &mut AudioEvents)>,
    mut removals: ResMut<PendingRemovals>,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
) {
    context.with(|context| {
        for node in removals.0.drain(..) {
            if context.remove_node(node).is_err() {
                error!("attempted to remove non-existent or invalid node from audio graph");
            }
        }

        for (node, mut events) in nodes.iter_mut() {
            for (time, event) in events.queue.drain(..) {
                context.queue_event(NodeEvent {
                    node_id: node.0,
                    event,
                    time,
                });
            }
        }

        let result = context.update();

        match result {
            Err(UpdateError::StreamStoppedUnexpectedly(e)) => {
                // For now, we'll assume this is always due to a device becoming unavailable.
                // As such, we'll attempt a reinitialization.
                warn!("Audio stream stopped: {e:?}");

                // First, we'll want to make sure the devices are up-to-date.
                commands.trigger(crate::configuration::FetchAudioIoEvent);
                // Then, we'll attempt a restart.
                commands.trigger(crate::configuration::RestartAudioEvent);
            }
            Err(e) => {
                error!("graph error: {e:?}");
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        prelude::*,
        test::{prepare_app, run},
    };

    #[derive(Component)]
    struct TestMarker;

    #[test]
    fn test_config_reinsertion() {
        let mut app = prepare_app(|mut commands: Commands| {
            commands
                .spawn(VolumeNode::default())
                .chain_node((VolumeNode::default(), TestMarker))
                .chain_node(VolumeNode::default());
        });

        let initial_id = run(
            &mut app,
            |q: Query<&FirewheelNode, With<TestMarker>>, mut context: ResMut<AudioContext>| {
                let node = q.single().unwrap().0;

                let total_nodes = context.with(|context| {
                    let edges = context.edges();

                    let inputs = edges.iter().filter(|e| e.src_node == node).count();
                    let outputs = edges.iter().filter(|e| e.dst_node == node).count();

                    assert_eq!(inputs, 2);
                    assert_eq!(outputs, 2);
                    context.nodes().len()
                });

                // 3 + input and output
                assert_eq!(total_nodes, 5);

                node
            },
        );

        // now, we modify the configuration
        run(
            &mut app,
            |mut q: Query<&mut VolumeNodeConfig, With<TestMarker>>| {
                let mut config = q.single_mut().unwrap();
                config.channels = NonZeroChannelCount::new(3).unwrap();
            },
        );

        app.update();

        // finally, if the ID is different but still has the appropriate connections, our
        // splicing has succeeded
        run(
            &mut app,
            move |q: Query<&FirewheelNode, With<TestMarker>>, mut context: ResMut<AudioContext>| {
                let node = q.single().unwrap().0;

                assert_ne!(initial_id, node);

                let total_nodes = context.with(|context| {
                    let edges = context.edges();

                    let inputs = edges.iter().filter(|e| e.src_node == node).count();
                    let outputs = edges.iter().filter(|e| e.dst_node == node).count();

                    assert_eq!(inputs, 2);
                    assert_eq!(outputs, 2);

                    context.nodes().len()
                });

                // 3 + input and output
                assert_eq!(total_nodes, 5);

                node.0
            },
        );
    }
}
