use super::{DEFAULT_CONNECTION, EdgeTarget, NodeMap, PendingEdge};
use crate::{context::AudioContext, node::FirewheelNode};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;

#[cfg(debug_assertions)]
use core::panic::Location;

/// The set of all pending connections for an entity.
///
/// These connections are drained and synchronized with the
/// audio graph in the [`SeedlingSystems::Connect`][crate::SeedlingSystems::Connect]
/// set.
#[derive(Debug, Default, Component)]
pub struct PendingConnections(Vec<PendingEdge>);

impl PendingConnections {
    /// Push a new pending connection.
    pub fn push(&mut self, connection: PendingEdge) {
        self.0.push(connection)
    }
}

/// An [`EntityCommands`] extension trait for connecting Firewheel nodes.
///
/// Firewheel features a node-graph audio architecture. Audio processors like [`VolumeNode`] represent
/// graph _nodes_, and the connections between processors are graph _edges_.
/// `bevy_seedling` exposes this directly, so you can connect nodes however you like.
///
/// [`VolumeNode`]: crate::prelude::VolumeNode
///
/// There are two main ways to connect nodes: with [`Entity`], and with [`NodeLabel`].
///
/// ## Connecting nodes via [`Entity`]
///
/// Any entity with a registered [`FirewheelNode`] is a valid connection target.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn system(mut commands: Commands) {
/// // Spawn a Firewheel node.
/// let node_entity = commands.spawn(VolumeNode::default()).id();
///
/// // Connect another node to it.
/// commands.spawn(LowPassNode::default()).connect(node_entity);
/// # }
/// ```
///
/// In the above example, when the connections are finalized at the end of the frame, the output
/// of the low-pass node will be connected to the input of the volume node:
///
/// ```text
/// ┌───────┐
/// │LowPass│
/// └┬──────┘
/// ┌▽─────────┐
/// │VolumeNode│
/// └┬─────────┘
/// ┌▽──────┐
/// │MainBus│
/// └───────┘
/// ```
///
/// Note how the [`VolumeNode`] is implicitly routed to the [`MainBus`];
/// this is true for _any_ node that has no specified routing.
/// This should keep your connections just a little more terse!
///
/// [`MainBus`]: crate::prelude::MainBus
///
/// ## Connecting via [`NodeLabel`]
///
/// An entity with a component deriving [`NodeLabel`] is also a valid connection target.
/// Since Rust types can have global, static visibility, node labels are especially useful
/// for common connections points like busses or effects chains.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// // Each type that derives `NodeLabel` also needs a few additional traits.
/// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct EffectsChain;
///
/// fn spawn_chain(mut commands: Commands) {
///     // Once spawned with this label, any other node can connect
///     // to this one without knowing its exact `Entity`.
///     commands.spawn((EffectsChain, LowPassNode::default()));
/// }
///
/// fn add_to_chain(mut commands: Commands) {
///     // Let's add even more processing!
///     //
///     // Keep in mind this new connection point is only
///     // visible within this system, since we don't spawn
///     // `BandPassNode` with any labels.
///     let additional_processing = commands
///         .spawn(BandPassNode::default())
///         .connect(EffectsChain);
/// }
/// ```
///
/// ## Chaining nodes
///
/// You'll often find yourself connecting several nodes one after another
/// in a chain. [`Connect`] provides an API to ease this process.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn system(mut commands: Commands) {
/// commands
///     .spawn(VolumeNode::default())
///     .chain_node(LowPassNode::default())
///     .chain_node(SpatialBasicNode::default());
/// # }
/// ```
///
/// When spawning nodes this way, you may want to recover the [`Entity`] of the first node
/// in the chain. [`Connect::head`] provides this information, regardless of how
/// long your chain is.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn system(mut commands: Commands) {
/// let chain_head = commands
///     .spawn(VolumeNode::default())
///     .chain_node(LowPassNode::default())
///     .chain_node(SpatialBasicNode::default())
///     .head();
///
/// commands.spawn(BandPassNode::default()).connect(chain_head);
/// # }
/// ```
///
/// [`EntityCommands`]: bevy_ecs::prelude::EntityCommands
/// [`NodeLabel`]: crate::prelude::NodeLabel
pub trait Connect<'a>: Sized {
    /// Queue a connection from this entity to the target.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// # fn system(mut commands: Commands) {
    /// // Connect a node to the MainBus.
    /// let node = commands
    ///     .spawn(VolumeNode::default())
    ///     .connect(MainBus)
    ///     .head();
    ///
    /// // Connect another node to the one we just spawned.
    /// commands.spawn(VolumeNode::default()).connect(node);
    /// # }
    /// ```
    ///
    /// By default, this provides a port connection of `[(0, 0), (1, 1)]`,
    /// which represents a simple stereo connection.
    /// To provide a specific port mapping, use [`connect_with`][Connect::connect_with].
    ///
    /// The connection is deferred, finalizing in the
    /// [`SeedlingSystems::Connect`][crate::SeedlingSystems::Connect] set.
    #[cfg_attr(debug_assertions, track_caller)]
    #[inline]
    fn connect(self, target: impl Into<EdgeTarget>) -> ConnectCommands<'a> {
        self.connect_with(target, DEFAULT_CONNECTION)
    }

    /// Queue a connection from this entity to the target with the provided port mappings.
    ///
    /// The connection is deferred, finalizing in the
    /// [`SeedlingSystems::Connect`][crate::SeedlingSystems::Connect] set.
    #[cfg_attr(debug_assertions, track_caller)]
    fn connect_with(
        self,
        target: impl Into<EdgeTarget>,
        ports: &[(u32, u32)],
    ) -> ConnectCommands<'a>;

    /// Chain a node's output into this node's input.
    ///
    /// This allows you to easily build up effects chains.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// # fn head(mut commands: Commands, server: Res<AssetServer>) {
    /// commands
    ///     .spawn(LowPassNode::default())
    ///     .chain_node(BandPassNode::default())
    ///     .chain_node(VolumeNode::default());
    /// # }
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    #[inline]
    fn chain_node<B: Bundle>(self, node: B) -> ConnectCommands<'a> {
        self.chain_node_with(node, DEFAULT_CONNECTION)
    }

    /// Chain a node with a manually-specified connection.
    ///
    /// This connection will be made between the previous node's output
    /// and this node's input.
    #[cfg_attr(debug_assertions, track_caller)]
    fn chain_node_with<B: Bundle>(self, node: B, ports: &[(u32, u32)]) -> ConnectCommands<'a>;

    /// Get the head of this chain.
    ///
    /// This makes it easy to recover the input of a chain of nodes.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn head(mut commands: Commands, server: Res<AssetServer>) {
    ///     let chain_input = commands
    ///         .spawn(LowPassNode::default())
    ///         .chain_node(BandPassNode::default())
    ///         .chain_node(VolumeNode::default())
    ///         .head();
    ///
    ///     commands.spawn((
    ///         SamplePlayer::new(server.load("my_sample.wav")),
    ///         sample_effects![SendNode::new(Volume::UNITY_GAIN, chain_input)],
    ///     ));
    /// }
    /// ```
    #[must_use]
    fn head(&self) -> Entity;

    /// Get the tail of this chain.
    ///
    /// This will be produce the same value
    /// as [`Connect::head`] if only one
    /// node has been spawned.
    #[must_use]
    fn tail(&self) -> Entity;
}

impl<'a> Connect<'a> for EntityCommands<'a> {
    fn connect_with(
        mut self,
        target: impl Into<EdgeTarget>,
        ports: &[(u32, u32)],
    ) -> ConnectCommands<'a> {
        let target = target.into();
        let ports = ports.to_vec();

        #[cfg(debug_assertions)]
        let location = Location::caller();

        self.entry::<PendingConnections>()
            .or_default()
            .and_modify(|mut pending| {
                pending.push(PendingEdge::new_with_location(
                    target,
                    Some(ports),
                    #[cfg(debug_assertions)]
                    location,
                ));
            });

        ConnectCommands::new(self)
    }

    fn chain_node_with<B: Bundle>(mut self, node: B, ports: &[(u32, u32)]) -> ConnectCommands<'a> {
        let new_id = self.commands().spawn(node).id();

        let mut new_connection = self.connect_with(new_id, ports);
        new_connection.tail = Some(new_id);

        new_connection
    }

    #[inline(always)]
    fn head(&self) -> Entity {
        self.id()
    }

    #[inline(always)]
    fn tail(&self) -> Entity {
        self.id()
    }
}

impl<'a> Connect<'a> for ConnectCommands<'a> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn connect_with(
        mut self,
        target: impl Into<EdgeTarget>,
        ports: &[(u32, u32)],
    ) -> ConnectCommands<'a> {
        let tail = self.tail();

        let mut commands = self.commands.commands();
        let mut commands = commands.entity(tail);

        let target = target.into();
        let ports = ports.to_vec();

        #[cfg(debug_assertions)]
        let location = Location::caller();

        commands
            .entry::<PendingConnections>()
            .or_default()
            .and_modify(|mut pending| {
                pending.push(PendingEdge::new_with_location(
                    target,
                    Some(ports),
                    #[cfg(debug_assertions)]
                    location,
                ));
            });

        self
    }

    fn chain_node_with<B: Bundle>(mut self, node: B, ports: &[(u32, u32)]) -> ConnectCommands<'a> {
        let new_id = self.commands.commands().spawn(node).id();

        let mut new_connection = self.connect_with(new_id, ports);
        new_connection.tail = Some(new_id);

        new_connection
    }

    #[inline(always)]
    fn head(&self) -> Entity {
        <Self>::head(self)
    }

    #[inline(always)]
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
        f.debug_struct("ConnectCommands")
            .field("entity", &self.head)
            .finish_non_exhaustive()
    }
}

pub(crate) fn process_connections(
    mut connections: Query<(&mut PendingConnections, &FirewheelNode)>,
    targets: Query<&FirewheelNode>,
    node_map: Res<NodeMap>,
    mut context: ResMut<AudioContext>,
) {
    let connections = connections
        .iter_mut()
        .filter(|(pending, _)| !pending.0.is_empty())
        .collect::<Vec<_>>();

    if connections.is_empty() {
        return;
    }

    context.with(|context| {
        for (mut pending, source_node) in connections.into_iter() {
            pending.0.retain(|connection| {
                let ports = connection.ports.as_deref().unwrap_or(DEFAULT_CONNECTION);

                let target_entity = match connection.target {
                    EdgeTarget::Entity(entity) => entity,
                    EdgeTarget::Label(label) => {
                        let Some(entity) = node_map.get(&label) else {
                            #[cfg(debug_assertions)]
                            {
                                let location = connection.origin;
                                error_once!("failed to connect to node label `{label:?}` at {location}: no associated Firewheel node found");
                            }
                            #[cfg(not(debug_assertions))]
                            error_once!("failed to connect to node label `{label:?}`: no associated Firewheel node found");

                            // We may need to wait for the intended label to be spawned.
                            return true;
                        };

                        *entity
                    }
                    EdgeTarget::Node(dest_node) => {
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
                        #[cfg(debug_assertions)]
                        {
                            let location = connection.origin;
                            error_once!("failed to connect to entity `{target_entity:?}` at {location}: no Firewheel node found");
                        }
                        #[cfg(not(debug_assertions))]
                        error_once!("failed to connect to entity `{target_entity:?}`: no Firewheel node found");

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

#[cfg(test)]
mod test {
    use crate::{
        context::AudioContext, edge::AudioGraphOutput, prelude::MainBus, test::prepare_app,
    };

    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use firewheel::nodes::volume::VolumeNode;

    #[derive(Component)]
    struct One;
    #[derive(Component)]
    struct Two;
    #[derive(Component)]
    struct Three;

    #[test]
    fn test_chain() {
        let mut app = prepare_app(|mut commands: Commands| {
            commands
                .spawn((VolumeNode::default(), One))
                .chain_node((VolumeNode::default(), Two));

            commands
                .spawn((VolumeNode::default(), MainBus))
                .connect(AudioGraphOutput);
        });

        app.world_mut()
            .run_system_once(
                |mut context: ResMut<AudioContext>,
                 one: Single<&FirewheelNode, With<One>>,
                 two: Single<&FirewheelNode, With<Two>>,
                 main: Single<&FirewheelNode, With<MainBus>>| {
                    let one = one.into_inner();
                    let two = two.into_inner();
                    let main = main.into_inner();

                    context.with(|context| {
                        // input node, output node, One, Two, and MainBus
                        assert_eq!(context.nodes().len(), 5);

                        let outgoing_edges_one: Vec<_> = context
                            .edges()
                            .into_iter()
                            .filter(|e| e.src_node == one.0)
                            .collect();
                        let outgoing_edges_two: Vec<_> = context
                            .edges()
                            .into_iter()
                            .filter(|e| e.src_node == two.0)
                            .collect();

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

            commands
                .spawn((VolumeNode::default(), MainBus))
                .connect(AudioGraphOutput);
        });

        app.world_mut()
            .run_system_once(
                |mut context: ResMut<AudioContext>,
                 one: Single<&FirewheelNode, With<One>>,
                 two: Single<&FirewheelNode, With<Two>>,
                 three: Single<&FirewheelNode, With<Three>>| {
                    let one = one.into_inner();
                    let two = two.into_inner();
                    let three = three.into_inner();

                    context.with(|context| {
                        // input node, output node, One, Two, Three, and MainBus
                        assert_eq!(context.nodes().len(), 6);

                        let outgoing_edges_three: Vec<_> = context
                            .edges()
                            .into_iter()
                            .filter(|e| e.src_node == three.0)
                            .collect();

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
