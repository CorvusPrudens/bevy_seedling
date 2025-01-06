//! Audio node connections and management.

use crate::node_label::NodeLabels;
use crate::{node_label::InternedNodeLabel, AudioContext, MainBus, NodeLabel, SeedlingSystems};
use bevy_app::Last;
use bevy_ecs::{prelude::*, world::DeferredWorld};
use bevy_log::{error, error_once, warn_once};
use bevy_utils::HashMap;
use firewheel::basic_nodes::MixNode;
use firewheel::node::{AudioNode, NodeEvent, NodeEventType, NodeID};
use firewheel::param::AudioParam;
use firewheel::{ChannelConfig, ChannelCount};

#[derive(Component)]
struct ParamsDiff<T>(pub(crate) T);

/// An event queue.
///
/// When inserted into an entity that contains a [Node],
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

fn generate_param_events<T: AudioParam + Component + Clone + Send + Sync + 'static>(
    mut nodes: Query<(&mut T, &mut ParamsDiff<T>, &mut Events)>,
    mut ctx: ResMut<AudioContext>,
) {
    let now = ctx.now();

    for (mut params, mut diff, mut events) in nodes.iter_mut() {
        // Keep params roughly synched.
        params.tick(now);

        params.diff(
            &diff.0,
            |event| events.push(NodeEventType::Custom(Box::new(event))),
            Default::default(),
        );

        diff.0 = params.clone();
    }
}

fn acquire_id<T: Into<Box<dyn AudioNode>> + Component + Clone>(
    q: Query<(Entity, &T, Option<&NodeLabels>, Option<&ChannelConfig>), Without<Node>>,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
    mut node_map: ResMut<NodeMap>,
) {
    context.with(|context| {
        if let Some(graph) = context.graph_mut() {
            for (entity, container, labels, config) in q.iter() {
                let node = match graph.add_node(container.clone().into(), config.cloned()) {
                    Ok(node) => node,
                    Err(e) => {
                        error!("failed to insert node: {e}");
                        continue;
                    }
                };

                // let num_inputs = config.map(|c| c.num_inputs).unwrap_or(ChannelCount::STEREO);

                // let mix_config = ChannelConfig {
                //     num_inputs,
                //     num_outputs: num_inputs,
                // };
                //
                // let mix_node = match graph.add_node(MixNode.into(), Some(mix_config)) {
                //     Ok(node) => node,
                //     Err(e) => {
                //         error!("failed to insert mix node: {e}");
                //         continue;
                //     }
                // };
                //
                // let connections: Vec<_> = (0..num_inputs.get()).map(|i| (i, i)).collect();
                //
                // if let Err(e) = graph.connect(mix_node, node, &connections, false) {
                //     error!("failed to connect mix node: {e}");
                //     continue;
                // }

                for label in labels.iter().flat_map(|l| l.iter()) {
                    node_map.0.insert(*label, entity);
                }

                commands.entity(entity).insert(Node(node));
            }
        }
    });
}

/// Register an audio node with parameters.
///
/// This will allow audio entities to automatically
/// acquire IDs from the audio graph and perform
/// parameter diffing.
pub trait RegisterParamsNode {
    fn register_params_node<T: Into<Box<dyn AudioNode>> + AudioParam + Component + Clone>(
        &mut self,
    ) -> &mut Self;
}

impl RegisterParamsNode for bevy_app::App {
    fn register_params_node<T: Into<Box<dyn AudioNode>> + AudioParam + Component + Clone>(
        &mut self,
    ) -> &mut Self {
        let world = self.world_mut();

        world.register_component_hooks::<T>().on_insert(
            |mut world: DeferredWorld, entity: Entity, _| {
                let value = world.get::<T>(entity).unwrap().clone();
                world.commands().entity(entity).insert(ParamsDiff(value));
            },
        );
        world.register_required_components::<T, Events>();

        self.add_systems(
            Last,
            (
                acquire_id::<T>.in_set(SeedlingSystems::Acquire),
                generate_param_events::<T>.in_set(SeedlingSystems::Queue),
            ),
        )
    }
}

/// Register an audio node.
///
/// This will allow audio entities to automatically
/// acquire IDs from the audio graph.
pub trait RegisterNode {
    fn register_node<T: Into<Box<dyn AudioNode>> + Component + Clone>(&mut self) -> &mut Self;
}

impl RegisterNode for bevy_app::App {
    fn register_node<T: Into<Box<dyn AudioNode>> + Component + Clone>(&mut self) -> &mut Self {
        let world = self.world_mut();
        world.register_required_components::<T, Events>();

        self.add_systems(Last, acquire_id::<T>.in_set(SeedlingSystems::Acquire))
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

#[derive(Clone)]
pub(crate) struct AutoMix {
    id: NodeID,
}

impl Component for AutoMix {
    const STORAGE_TYPE: bevy_ecs::component::StorageType = bevy_ecs::component::StorageType::Table;

    fn register_component_hooks(hooks: &mut bevy_ecs::component::ComponentHooks) {
        hooks.on_remove(|mut world, entity, _| {
            let Some(node) = world.get::<AutoMix>(entity).map(|e| e.id) else {
                return;
            };

            let mut removals = world.resource_mut::<PendingRemovals>();
            removals.push(node);
        });
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

/// A target for node connections.
///
/// [`ConnectTarget`] can be constructed manually or
/// used as a part of the [`ConnectNode`] API.
#[derive(Debug, Clone)]
pub enum ConnectTarget {
    /// A global label such as [`MainBus`].
    Label(InternedNodeLabel),
    /// An audio entity.
    Entity(Entity),
    /// An existing node from the audio graph.
    Node(NodeID),
}

/// A pending connection between two nodes.
#[derive(Debug, Clone)]
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
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::{MainBus, VolumeNode, ConnectNode};
    /// # fn system(mut commands: Commands) {
    /// // Connect a node to the MainBus.
    /// let node = commands.spawn(VolumeNode::new(0.5)).connect(MainBus).id();
    ///
    /// // Connect another node to the one we just spawned.
    /// commands.spawn(VolumeNode::new(0.25)).connect(node);
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
/// entities with both a [`Node`] and [`NodeLabel`].
#[derive(Default, Debug, Resource)]
pub struct NodeMap(HashMap<InternedNodeLabel, Entity>);

impl core::ops::Deref for NodeMap {
    type Target = HashMap<InternedNodeLabel, Entity>;

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

const DEFAULT_CONNECTION: &[(u32, u32)] = &[(0, 0), (1, 1)];

// this has turned into a bit of a monster
pub(crate) fn process_connections(
    mut connections: Query<(&mut PendingConnections, &Node)>,
    mut targets: Query<(Option<&mut AutoMix>, Option<&ChannelConfig>, &Node)>,
    node_map: Res<NodeMap>,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
) {
    let mut batched_connections = HashMap::new();

    context.with(|context| {
        if let Some(graph) = context.graph_mut() {
            for (mut pending, source_node) in connections.iter_mut() {
                pending.0.retain(|connection| {
                    let ports = connection.ports.as_deref().unwrap_or(&[(0, 0), (1, 1)]);

                    let target_entity = match connection.target {
                        ConnectTarget::Entity(entity) => entity,
                        ConnectTarget::Label(label) => {
                            let Some(entity) = node_map.get(&label) else {
                                warn_once!("tried to connect audio node to label with no node");

                                // We may need to wait for the intended label to be spawned.
                                return true;
                            };

                            *entity
                        }
                        ConnectTarget::Node(dest_node) => {
                            // no questions asked, simply connect
                            if let Err(e) = graph.connect(source_node.0, dest_node, ports, false) {
                                error_once!("failed to connect audio node to target: {e}");
                            }

                            // if this fails, the target node must have been removed from the graph
                            return false;
                        }
                    };

                    batched_connections
                        .entry(target_entity)
                        .or_insert(Vec::new())
                        .push((source_node.0, connection.ports.clone()));

                    false
                });
            }

            // We batch the entity-based connections together so that
            // we only need to spawn or modify a mix node once per schedule.
            for (dest, sources) in batched_connections {
                let Ok((auto_mix, channel_config, node)) = targets.get_mut(dest) else {
                    error_once!("entity {dest:#?} has no audio node or does not exist");

                    continue;
                };

                let base_inputs = channel_config.map(|c| c.num_inputs.get()).unwrap_or(2);

                let existing_map = match auto_mix {
                    // first, check if we can simply connect the nodes directly
                    None if !graph.edges().any(|e| e.dst_node == node.0) && sources.len() == 1 => {
                        let (source, ports) = &sources[0];

                        if let Err(e) = graph.connect(
                            *source,
                            node.0,
                            ports.as_deref().unwrap_or(DEFAULT_CONNECTION),
                            false,
                        ) {
                            error_once!("failed to connect audio node to target: {e}");
                        }

                        continue;
                    }
                    // otherwise, we'll create a mix node
                    Some(auto_mix) => {
                        let mut existing_map = HashMap::new();

                        for edge in graph.edges().filter(|e| e.dst_node == auto_mix.id) {
                            existing_map
                                .entry(edge.src_node)
                                .or_insert(Vec::new())
                                // the destination ports should be considered only
                                // as the remainder of the base input channels of the
                                // target node
                                .push((edge.src_port, edge.dst_port % base_inputs));
                        }

                        // disconnect the existing mix node
                        let removals: Vec<_> = graph
                            .edges()
                            .filter(|e| e.src_node == auto_mix.id)
                            .map(|e| e.id)
                            .collect();
                        for id in removals {
                            graph.disconnect_by_edge_id(id);
                        }

                        existing_map
                    }
                    _ => {
                        let mut existing_map = HashMap::new();
                        let mut removals = Vec::new();

                        for edge in graph.edges().filter(|e| e.dst_node == node.0) {
                            existing_map
                                .entry(edge.src_node)
                                .or_insert(Vec::new())
                                .push((edge.src_port, edge.dst_port));
                            removals.push(edge.id);
                        }

                        for id in removals {
                            graph.disconnect_by_edge_id(id);
                        }

                        existing_map
                    }
                };

                // finally, create a new mix node to accomodate the connections
                let total_inputs = base_inputs * (sources.len() + existing_map.len()) as u32;

                let mix_node = graph.add_node(
                    MixNode.into(),
                    Some(ChannelConfig {
                        num_inputs: ChannelCount::new(total_inputs).unwrap(),
                        num_outputs: ChannelCount::new(base_inputs).unwrap(),
                    }),
                );

                let mix_node = match mix_node {
                    Ok(n) => n,
                    Err(e) => {
                        error_once!("failed to create automatic audio node mixing: {e}");
                        continue;
                    }
                };

                let mut port_buffer = Vec::with_capacity(base_inputs as usize);

                for i in 0..base_inputs {
                    port_buffer.push((i, i));
                }

                if let Err(e) = graph.connect(mix_node, node.0, &port_buffer, false) {
                    error_once!("failed to connect to automatic audio node mixing: {e}");
                    continue;
                }

                for (i, (src, ports)) in
                    existing_map
                        .iter()
                        .map(|(src, ports)| (src, ports.as_slice()))
                        .chain(sources.iter().map(|(src, ports)| {
                            (src, ports.as_deref().unwrap_or(DEFAULT_CONNECTION))
                        }))
                        .enumerate()
                {
                    port_buffer.clear();

                    for pair in ports.iter() {
                        port_buffer.push((pair.0, pair.1 + i as u32 * base_inputs));
                    }

                    if let Err(e) = graph.connect(*src, mix_node, &port_buffer, false) {
                        error_once!("failed to connect to automatic audio node mixing: {e}");

                        break;
                    }
                }

                commands.entity(dest).insert(AutoMix { id: mix_node });
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
                        delay: firewheel::clock::EventDelay::Immediate,
                    });
                }
            }
        }
    });
}
