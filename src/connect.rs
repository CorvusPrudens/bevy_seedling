//! Node connection utilities.

use crate::{node_label::InternedNodeLabel, AudioContext};
use crate::{MainBus, Node, NodeLabel};
use bevy_ecs::prelude::*;
use bevy_log::{error_once, warn_once};
use bevy_utils::HashMap;
use firewheel::node::NodeID;

/// A target for node connections.
///
/// [`ConnectTarget`] can be constructed manually or
/// used as a part of the [`Connect`] API.
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
pub trait Connect<'a>: Sized {
    /// Queue a connection from this entity to the target.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::{MainBus, VolumeNode, Connect, Volume};
    /// # fn system(mut commands: Commands) {
    /// // Connect a node to the MainBus.
    /// let node = commands
    ///     .spawn(VolumeNode { volume: Volume::Linear(0.5) })
    ///     .connect(MainBus)
    ///     .head();
    ///
    /// // Connect another node to the one we just spawned.
    /// commands
    ///     .spawn(VolumeNode { volume: Volume::Linear(0.25) })
    ///     .connect(node);
    /// # }
    /// ```
    ///
    /// By default, this provides a port connection of `[(0, 0), (1, 1)]`,
    /// which represents a simple stereo connection.
    /// To provide a specific port mapping, use [`connect_with`][Connect::connect_with].
    ///
    /// The connection is deferred, finalizing in the [`SeedlingSystems::Connect`] set.
    fn connect(self, target: impl Into<ConnectTarget>) -> ConnectCommands<'a> {
        self.connect_with(target, DEFAULT_CONNECTION)
    }

    /// Queue a connection from this entity to the target with the provided port mappings.
    ///
    /// The connection is deferred, finalizing in the [`SeedlingSystems::Connect`] set.
    fn connect_with(
        self,
        target: impl Into<ConnectTarget>,
        ports: &[(u32, u32)],
    ) -> ConnectCommands<'a>;

    fn chain_node<B: Bundle>(self, node: B) -> ConnectCommands<'a> {
        self.chain_node_with(node, DEFAULT_CONNECTION)
    }

    fn chain_node_with<B: Bundle>(self, node: B, ports: &[(u32, u32)]) -> ConnectCommands<'a>;

    /// Get the head of this chain.
    fn head(&self) -> Entity;

    /// Get the tail of this chain.
    ///
    /// This will be produce the same value
    /// as [`ConnectCommands::head`] if only one
    /// node has been spawned.
    fn tail(&self) -> Entity;
}

impl<'a> Connect<'a> for EntityCommands<'a> {
    fn connect_with(
        mut self,
        target: impl Into<ConnectTarget>,
        ports: &[(u32, u32)],
    ) -> ConnectCommands<'a> {
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

        ConnectCommands::new(self)
    }

    fn chain_node_with<B: Bundle>(mut self, node: B, ports: &[(u32, u32)]) -> ConnectCommands<'a> {
        let new_id = self.commands().spawn(node).id();

        let mut new_connection = self.connect_with(new_id, ports);
        new_connection.head = new_id;

        new_connection
    }

    fn head(&self) -> Entity {
        self.id()
    }

    fn tail(&self) -> Entity {
        self.id()
    }
}

impl<'a> Connect<'a> for ConnectCommands<'a> {
    fn connect_with(
        mut self,
        target: impl Into<ConnectTarget>,
        ports: &[(u32, u32)],
    ) -> ConnectCommands<'a> {
        let tail = self.tail();

        let mut commands = self.commands.commands();
        let mut commands = commands.entity(tail);

        let target = target.into();
        let ports = ports.to_vec();

        commands
            .entry::<PendingConnections>()
            .or_default()
            .and_modify(|mut pending| {
                pending.push(PendingConnection {
                    target,
                    ports: Some(ports),
                });
            });

        self
    }

    fn chain_node_with<B: Bundle>(mut self, node: B, ports: &[(u32, u32)]) -> ConnectCommands<'a> {
        let new_id = self.commands.commands().spawn(node).id();

        let mut new_connection = self.connect_with(new_id, ports);
        new_connection.tail = Some(new_id);

        new_connection
    }

    fn head(&self) -> Entity {
        <Self>::head(self)
    }

    fn tail(&self) -> Entity {
        <Self>::tail(self)
    }
}

/// A set of commands for connecting nodes and chaining effects.
pub struct ConnectCommands<'a> {
    commands: EntityCommands<'a>,
    head: Entity,
    tail: Option<Entity>,
}

impl<'a> ConnectCommands<'a> {
    pub(crate) fn new(commands: EntityCommands<'a>) -> Self {
        Self {
            head: commands.id(),
            tail: None,
            commands,
        }
    }

    /// Get the head of this chain.
    fn head(&self) -> Entity {
        self.head
    }

    /// Get the tail of this chain.
    ///
    /// This will be produce the same value
    /// as [`ConnectCommands::head`] if only one
    /// node has been spawned.
    fn tail(&self) -> Entity {
        self.tail.unwrap_or(self.head)
    }
}

impl core::fmt::Debug for ConnectCommands<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainConnection")
            .field("entity", &self.head)
            .finish_non_exhaustive()
    }
}

const DEFAULT_CONNECTION: &[(u32, u32)] = &[(0, 0), (1, 1)];

// this has turned into a bit of a monster
pub(crate) fn process_connections(
    mut connections: Query<(&mut PendingConnections, &Node)>,
    targets: Query<&Node>,
    node_map: Res<NodeMap>,
    mut context: ResMut<AudioContext>,
) {
    context.with(|context| {
        for (mut pending, source_node) in connections.iter_mut() {
            pending.0.retain(|connection| {
                let ports = connection.ports.as_deref().unwrap_or(DEFAULT_CONNECTION);

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
                        if let Err(e) = context.connect(source_node.0, dest_node, ports, false) {
                            error_once!("failed to connect audio node to target: {e}");
                        }

                        // if this fails, the target node must have been removed from the graph
                        return false;
                    }
                };

                let target = match targets.get(target_entity) {
                    Ok(t) => t,
                    Err(_) => {
                        error_once!("failed to fetch audio node entity {target_entity:?}");
                        return false;
                    }
                };

                if let Err(e) = context.connect(source_node.0, target.0, ports, false) {
                    error_once!("failed to connect audio node to target: {e}");
                }

                false
            });
        }
    });
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

#[cfg(test)]
mod test {
    use crate::SeedlingPlugin;

    use super::*;
    use bevy::prelude::*;
    use bevy_ecs::system::RunSystemOnce;
    use firewheel::nodes::volume::VolumeNode;

    #[derive(Component)]
    struct One;
    #[derive(Component)]
    struct Two;
    #[derive(Component)]
    struct Three;

    fn prepare_app<F: IntoSystem<(), (), M>, M>(startup: F) -> App {
        let mut app = App::new();

        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            SeedlingPlugin {
                sample_pool_size: None,
                ..Default::default()
            },
        ))
        .add_systems(Startup, startup);

        app.finish();
        app.cleanup();
        app.update();

        app
    }

    #[test]
    fn test_chain() {
        let mut app = prepare_app(|mut commands: Commands| {
            commands
                .spawn((VolumeNode::default(), One))
                .chain_node((VolumeNode::default(), Two))
                .connect(MainBus);
        });

        app.world_mut()
            .run_system_once(
                |mut context: ResMut<AudioContext>,
                 one: Single<&Node, With<One>>,
                 two: Single<&Node, With<Two>>,
                 main: Single<&Node, With<MainBus>>| {
                    let one = one.into_inner();
                    let two = two.into_inner();
                    let main = main.into_inner();

                    context.with(|context| {
                        // input node, output node, One, Two, and MainBus
                        assert_eq!(context.nodes().count(), 5);

                        let outgoing_edges_one: Vec<_> =
                            context.edges().filter(|e| e.src_node == one.0).collect();
                        let outgoing_edges_two: Vec<_> =
                            context.edges().filter(|e| e.src_node == two.0).collect();

                        assert_eq!(outgoing_edges_one.len(), 2);
                        assert_eq!(outgoing_edges_two.len(), 2);

                        assert!(outgoing_edges_one.iter().all(|e| e.dst_node == two.0));
                        assert!(outgoing_edges_two.iter().all(|e| e.dst_node == main.0));
                    });
                },
            )
            .unwrap();
    }

    #[test]
    fn test_fanout() {
        let mut app = prepare_app(|mut commands: Commands| {
            let a = commands.spawn((VolumeNode::default(), One)).head();
            let b = commands.spawn((VolumeNode::default(), Two)).head();

            commands
                .spawn((VolumeNode::default(), Three))
                .connect(a)
                .connect(b);
        });

        app.world_mut()
            .run_system_once(
                |mut context: ResMut<AudioContext>,
                 one: Single<&Node, With<One>>,
                 two: Single<&Node, With<Two>>,
                 three: Single<&Node, With<Three>>| {
                    let one = one.into_inner();
                    let two = two.into_inner();
                    let three = three.into_inner();

                    context.with(|context| {
                        // input node, output node, One, Two, Three, and MainBus
                        assert_eq!(context.nodes().count(), 6);

                        let outgoing_edges_three: Vec<_> =
                            context.edges().filter(|e| e.src_node == three.0).collect();

                        assert_eq!(
                            outgoing_edges_three
                                .iter()
                                .filter(|e| e.dst_node == one.0)
                                .count(),
                            2
                        );
                        assert_eq!(
                            outgoing_edges_three
                                .iter()
                                .filter(|e| e.dst_node == two.0)
                                .count(),
                            2
                        );
                    });
                },
            )
            .unwrap();
    }
}
