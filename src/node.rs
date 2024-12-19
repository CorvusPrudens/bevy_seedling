//! Audio node connections and management.

use crate::{
    label::{InternedLabel, InternedNodeLabel},
    AudioContext, MainBus, NodeLabel, SeedlingSystems,
};
use bevy_app::Last;
use bevy_ecs::{component::ComponentId, prelude::*, world::DeferredWorld};
use bevy_log::{error, warn_once};
use bevy_utils::{HashMap, HashSet};
use core::any::TypeId;
use firewheel::node::{AudioNode, AudioParam, EventData, NodeEvent, NodeID};

pub trait EcsNode: Component {
    fn node(&self) -> Box<dyn AudioNode>;
}

#[derive(Resource, Default)]
pub(crate) struct ParamSystems(HashSet<TypeId>);

#[derive(Component)]
#[component(on_insert = insert_params::<T>)]
#[require(Events)]
pub struct Params<T: AudioParam + Send + Sync + Clone + 'static>(T);

impl<T: AudioParam + Send + Sync + Clone + 'static> Params<T> {
    pub fn new(params: T) -> Self {
        Self(params)
    }
}

impl<T: AudioParam + Send + Sync + Clone + 'static> core::ops::Deref for Params<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: AudioParam + Send + Sync + Clone + 'static> core::ops::DerefMut for Params<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn insert_params<T: AudioParam + Send + Sync + Clone + 'static>(
    mut world: DeferredWorld,
    entity: Entity,
    _: ComponentId,
) {
    let params = world.get::<Params<T>>(entity).unwrap();
    let diff = ParamsDiff(params.0.clone());

    let mut commands = world.commands();
    commands.entity(entity).insert(diff);

    commands.queue(|world: &mut World| {
        let id = TypeId::of::<T>();
        let mut systems = world.get_resource_or_init::<ParamSystems>();
        let added = systems.0.insert(id);

        if added {
            world.schedule_scope(Last, |_, schedule| {
                schedule.add_systems(generate_param_events::<T>);
            });
        }
    });
}

#[derive(Component)]
struct ParamsDiff<T>(T);

/// An event queue.
///
/// When inserted into an entity that contains a [Node],
/// these events will automatically be drained and sent
/// to the audio context in the [SeedlingSystems::Flush] set.
#[derive(Component, Default)]
pub struct Events(Vec<EventData>);

impl Events {
    /// Push a new event.
    pub fn push(&mut self, event: EventData) {
        self.0.push(event);
    }

    /// Push a custom event.
    ///
    /// `value` is boxed and wrapped in [EventData::Custom].
    pub fn push_custom<T: Send + Sync + 'static>(&mut self, value: T) {
        self.0.push(EventData::Custom(Box::new(value)));
    }
}

fn generate_param_events<T: AudioParam + Clone + Send + Sync + 'static>(
    mut nodes: Query<(&mut Params<T>, &mut ParamsDiff<T>, &mut Events)>,
) {
    for (params, mut diff, mut events) in nodes.iter_mut() {
        params.0.to_messages(
            &diff.0,
            |event| events.push(EventData::Parameter(event)),
            Default::default(),
        );

        diff.0 = params.0.clone();
    }
}

fn acquire_id<T: EcsNode>(
    q: Query<(Entity, &T, Option<&InternedLabel>), Without<Node>>,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
    mut node_map: ResMut<NodeMap>,
) {
    context.with(|context| {
        if let Some(graph) = context.graph_mut() {
            for (entity, container, label) in q.iter() {
                let node = match graph.add_node(container.node(), None) {
                    Ok(node) => node,
                    Err(e) => {
                        error!("failed to insert node: {e}");
                        continue;
                    }
                };

                if let Some(label) = label {
                    node_map.0.insert(label.0, node);
                }
                commands.entity(entity).insert(Node(node));
            }
        }
    });
}

/// Register an audio node.
///
/// This will allow audio entities to automatically
/// acquire IDs from the audio graph.
pub trait RegisterNode {
    fn register_node<T: EcsNode>(&mut self) -> &mut Self;
}

impl RegisterNode for bevy_app::App {
    fn register_node<T: EcsNode>(&mut self) -> &mut Self {
        self.add_systems(
            Last,
            (
                acquire_id::<T>.in_set(SeedlingSystems::Acquire),
                // generate_param_events::<T::Params>.in_set(SeedlingSystems::Queue),
            ),
        )
    }
}

/// An ECS handle for an audio node.
///
/// [`Node`] may not necessarily be available immediately
/// upon spawning audio entities; [`Node`]s are acquired
/// during the [`SeedlingSystems::Acquire`] set. Node
/// acquisition will also be deferred if the audio context
/// is disabled.
///
/// When this component is removed, the underlying
/// audio node is removed from the graph.
#[derive(Debug, Clone, Copy)]
pub struct Node(pub NodeID);

impl Component for Node {
    const STORAGE_TYPE: bevy_ecs::component::StorageType = bevy_ecs::component::StorageType::Table;

    fn register_component_hooks(hooks: &mut bevy_ecs::component::ComponentHooks) {
        hooks.on_remove(|mut world, entity, _| {
            let Some(node) = world.get::<Node>(entity).copied() else {
                return;
            };

            let mut removals = world.resource_mut::<PendingRemovals>();
            removals.0.push(node.0);
        });
    }
}

/// Queued audio node removals.
///
/// This resource allows us to defer audio node removals
/// until the audio graph is ready.
#[derive(Debug, Default, Resource)]
pub(crate) struct PendingRemovals(Vec<NodeID>);

/// A target for node connections.
///
/// [`ConnectTarget`] can be constructed manually or
/// used as a part of the [`ConnectNode`] API.
/// ```
#[derive(Debug)]
pub enum ConnectTarget {
    /// A global label such as [`MainBus`].
    Label(InternedNodeLabel),
    /// An audio entity.
    Entity(Entity),
    /// An existing node from the audio graph.
    Node(NodeID),
}

/// A pending connection between two nodes.
#[derive(Debug)]
pub struct PendingConnection {
    pub target: ConnectTarget,
    /// An optional [`firewheel`] port mapping.
    ///
    /// The first tuple element represents the source output,
    /// and the second tuple element represents the sink input.
    ///
    /// If an explicit port mapping is not provided,
    /// `[(0, 0), (1, 1)]` is used.
    pub ports: Option<Vec<(u32, u32)>>,
}

impl From<NodeID> for ConnectTarget {
    fn from(value: NodeID) -> Self {
        Self::Node(value)
    }
}

impl<T> From<T> for ConnectTarget
where
    T: NodeLabel,
{
    fn from(value: T) -> Self {
        Self::Label(value.intern())
    }
}

impl From<Entity> for ConnectTarget {
    fn from(value: Entity) -> Self {
        Self::Entity(value)
    }
}

/// The set of all [`PendingConnection`]s for an entity.
///
/// These connections are drained and synchronized with the
/// audio graph in the [SeedlingSystems::Connect] set.
#[derive(Debug, Default, Component)]
pub struct PendingConnections(Vec<PendingConnection>);

impl PendingConnections {
    /// Push a new pending connection.
    pub fn push(&mut self, connection: PendingConnection) {
        self.0.push(connection)
    }
}

/// An [`EntityCommands`] extension trait for connecting node entities.
///
/// These methods provide only source -> sink connections. The source
/// is the receiver and the sink is the provided target.
///
/// [`EntityCommands`]: bevy_ecs::prelude::EntityCommands
pub trait ConnectNode {
    /// Queue a connection from this entity to the target.
    ///
    /// ```
    /// # use crate::label::MainBus;
    /// # fn system(mut commands: Commands) {
    /// // Connect a node to the MainBus.
    /// let node = commands.spawn(Volume::new(0.5)).connect(MainBus).id();
    ///
    /// // Connect another node to the one we just spawned.
    /// commands.spawn(Volume::new(0.25)).connect(node);
    /// # }
    /// ```
    ///
    /// By default, this provides a port connection of `[(0, 0), (1, 1)]`,
    /// which represents a simple stereo connection.
    /// To provide a specific port mapping, use [`connect_with`][ConnectNode::connect_with].
    ///
    /// The connection is deferred, finalizing in the [`SeedlingSystems::Connect`] set.
    fn connect(&mut self, target: impl Into<ConnectTarget>) -> &mut Self;

    /// Queue a connection from this entity to the target with the provided port mappings.
    ///
    /// The connection is deferred, finalizing in the [`SeedlingSystems::Connect`] set.
    fn connect_with(&mut self, target: impl Into<ConnectTarget>, ports: &[(u32, u32)])
        -> &mut Self;
}

impl ConnectNode for EntityCommands<'_> {
    fn connect(&mut self, target: impl Into<ConnectTarget>) -> &mut Self {
        let target = target.into();

        self.entry::<PendingConnections>()
            .or_default()
            .and_modify(|mut pending| {
                pending.push(PendingConnection {
                    target,
                    ports: None,
                });
            });

        self
    }

    fn connect_with(
        &mut self,
        target: impl Into<ConnectTarget>,
        ports: &[(u32, u32)],
    ) -> &mut Self {
        let target = target.into();
        let ports = ports.to_vec();

        self.entry::<PendingConnections>()
            .or_default()
            .and_modify(|mut pending| {
                pending.push(PendingConnection {
                    target,
                    ports: Some(ports),
                });
            });

        self
    }
}

/// A map that associates [`NodeLabel`]s with audio
/// graph nodes.
///
/// This will be automatically synchronized for
/// entities with both a [`Node`] and [`InternedLabel`].
#[derive(Default, Debug, Resource)]
pub struct NodeMap(HashMap<InternedNodeLabel, NodeID>);

impl core::ops::Deref for NodeMap {
    type Target = HashMap<InternedNodeLabel, NodeID>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for NodeMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Automatically connect nodes without manual connections to the main bus.
pub(crate) fn auto_connect(
    nodes: Query<Entity, (With<Node>, Without<PendingConnections>)>,
    mut commands: Commands,
) {
    for node in nodes.iter() {
        commands.entity(node).connect(MainBus);
    }
}

pub(crate) fn process_removals(
    mut removals: ResMut<PendingRemovals>,
    mut context: ResMut<AudioContext>,
) {
    context.with(|context| {
        if let Some(graph) = context.graph_mut() {
            for node in removals.0.drain(..) {
                if graph.remove_node(node).is_err() {
                    error!("attempted to remove non-existent or invalid node from audio graph");
                }
            }
        }
    });
}

pub(crate) fn process_connections(
    mut conn: Query<(&mut PendingConnections, &Node)>,
    targets: Query<&Node>,
    node_map: Res<NodeMap>,
    mut context: ResMut<AudioContext>,
) {
    context.with(|context| {
        if let Some(graph) = context.graph_mut() {
            for (mut connections, source_node) in conn.iter_mut() {
                connections.0.retain(|connection| {
                    let dest_node = match connection.target {
                        ConnectTarget::Entity(entity) => {
                            let Ok(dest_node) = targets.get(entity) else {
                                warn_once!("no target {entity:?} found for audio node connection");
                                return true;
                            };

                            dest_node.0
                        }
                        ConnectTarget::Label(label) => {
                            let Some(dest_node) = node_map.get(&label) else {
                                warn_once!("no active label found for audio node connection");

                                return true;
                            };

                            *dest_node
                        }
                        ConnectTarget::Node(node) => node,
                    };

                    let ports = connection.ports.as_deref().unwrap_or(&[(0, 0), (1, 1)]);

                    match graph.connect(source_node.0, dest_node, ports, false) {
                        Ok(_) => false,
                        Err(e) => {
                            error!("failed to connect audio node to target: {e}");

                            true
                        }
                    }
                });
            }
        }
    });
}

pub(crate) fn flush_events(
    mut nodes: Query<(&Node, &mut Events)>,
    mut context: ResMut<AudioContext>,
) {
    context.with(|context| {
        if let Some(graph) = context.graph_mut() {
            for (node, mut events) in nodes.iter_mut() {
                for event in events.0.drain(..) {
                    graph.queue_event(NodeEvent {
                        node_id: node.0,
                        event,
                    });
                }
            }
        }
    });
}
