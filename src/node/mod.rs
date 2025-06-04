//! Audio node registration and management.

use crate::edge::NodeMap;
use crate::error::SeedlingError;
use crate::pool::sample_effects::EffectOf;
use crate::{SeedlingSystems, prelude::AudioContext};
use bevy::ecs::component::{ComponentId, HookContext, Mutable};
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use firewheel::{
    diff::{Diff, Patch},
    event::{NodeEvent, NodeEventType},
    node::{AudioNode, NodeID},
};

pub mod follower;
pub mod label;

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

/// An event queue.
///
/// When inserted into an entity that contains a [FirewheelNode],
/// these events will automatically be drained and sent
/// to the audio context in the [SeedlingSystems::Flush] set.
#[derive(Component, Default)]
pub struct Events(Vec<NodeEventType>);

// Not ideal, but we're waiting for Firewheel to implement debug.
impl core::fmt::Debug for Events {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries((0..self.0.len()).map(|_| ()))
            .finish()
    }
}

impl Events {
    /// Push a new event.
    pub fn push(&mut self, event: NodeEventType) {
        self.0.push(event);
    }

    /// Push a custom event.
    ///
    /// `value` is boxed and wrapped in [NodeEventType::Custom].
    pub fn push_custom<T: Send + Sync + 'static>(&mut self, value: T) {
        self.0.push(NodeEventType::Custom(Box::new(value)));
    }
}

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
    mut nodes: Query<(&T, &mut Baseline<T>, &mut Events), (Changed<T>, Without<EffectOf>)>,
) -> Result {
    for (params, mut baseline, mut events) in nodes.iter_mut() {
        // This ensures we only apply patches that were generated here.
        // I'm not sure this is correct in all cases, though.
        let starting_len = events.0.len();

        params.diff(&baseline.0, Default::default(), &mut events.0);

        // Patch the baseline.
        for event in &events.0[starting_len..] {
            apply_patch(&mut baseline.0, event)?;
        }
    }

    Ok(())
}

fn acquire_id<T>(
    q: Query<
        (Entity, &T, Option<&T::Configuration>, Option<&NodeLabels>),
        (Without<FirewheelNode>, Without<EffectOf>),
    >,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
    mut node_map: ResMut<NodeMap>,
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
pub trait RegisterNode {
    /// Register an audio node with automatic diffing.
    ///
    /// This will allow audio entities to automatically
    /// acquire IDs from the audio graph and perform
    /// parameter diffing.
    fn register_node<T>(&mut self) -> &mut Self
    where
        T: AudioNode<Configuration: Component + Clone>
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
        T: AudioNode<Configuration: Component + Clone> + Component + Clone;
}

impl RegisterNode for App {
    fn register_node<T>(&mut self) -> &mut Self
    where
        T: AudioNode<Configuration: Component + Clone>
            + Diff
            + Patch
            + Component<Mutability = Mutable>
            + Clone,
    {
        let world = self.world_mut();

        world.register_component_hooks::<T>().on_insert(
            |mut world: DeferredWorld, context: HookContext| {
                let value = world.get::<T>(context.entity).unwrap().clone();
                world
                    .commands()
                    .entity(context.entity)
                    .insert((Baseline(value), EffectId(context.component_id)));
            },
        );
        world.register_required_components::<T, Events>();
        world.register_required_components::<T, T::Configuration>();

        self.add_systems(
            Last,
            (
                acquire_id::<T>.in_set(SeedlingSystems::Acquire),
                (follower::param_follower::<T>, generate_param_events::<T>)
                    .chain()
                    .in_set(SeedlingSystems::Queue),
            ),
        )
    }

    fn register_simple_node<T>(&mut self) -> &mut Self
    where
        T: AudioNode<Configuration: Component + Clone> + Component + Clone,
    {
        let world = self.world_mut();
        world.register_required_components::<T, Events>();
        world.register_required_components::<T, T::Configuration>();

        self.add_systems(Last, acquire_id::<T>.in_set(SeedlingSystems::Acquire))
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
#[component(on_remove = Self::on_remove_hook, immutable)]
pub struct FirewheelNode(pub NodeID);

impl FirewheelNode {
    fn on_remove_hook(mut world: DeferredWorld, context: HookContext) {
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
    mut nodes: Query<(&FirewheelNode, &mut Events)>,
    mut removals: ResMut<PendingRemovals>,
    mut context: ResMut<AudioContext>,
) {
    context.with(|context| {
        for node in removals.0.drain(..) {
            if context.remove_node(node).is_err() {
                error!("attempted to remove non-existent or invalid node from audio graph");
            }
        }

        for (node, mut events) in nodes.iter_mut() {
            for event in events.0.drain(..) {
                context.queue_event(NodeEvent {
                    node_id: node.0,
                    event,
                });
            }
        }

        if let Err(e) = context.update() {
            error!("graph error: {:?}", e);
        }
    });
}
