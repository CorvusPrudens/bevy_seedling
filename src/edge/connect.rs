use super::{EdgeTarget, NodeMap, PendingEdge};
use crate::{
    context::AudioContext,
    edge::ChannelMapping,
    node::{FirewheelNode, FirewheelNodeInfo},
};
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use core::ops::Deref;

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
///
/// ## Specifying ports
///
/// When connecting, chaining, or disconnecting nodes, you can
/// specify exactly which outputs should be connected to which inputs.
/// For example, if you connect a stereo node to a mono node, you
/// can downmix the stereo signal:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn system(mut commands: Commands) {
/// commands.spawn(VolumeNode::default()).chain_node_with(
///     (
///         LowPassNode::default(),
///         LowPassConfig {
///             channels: NonZeroChannelCount::new(1).unwrap(),
///             ..Default::default()
///         },
///     ),
///     &[(0, 0), (1, 0)],
/// );
/// # }
/// ```
///
/// The tuples represent individual edges in the audio graph. The
/// first element is the output of the source, and the second is
/// the input of the destination. So, in the example above, the
/// left and right channels of [`VolumeNode`] are both connected
/// to the single input of [`LowPassNode`].
///
/// [`VolumeNode`]: crate::prelude::VolumeNode
/// [`LowPassNode`]: crate::prelude::LowPassNode
///
/// The above example is also unnecessary in most circumstances.
/// The [`ChannelMapping`] component, a required component on all
/// nodes, defaults to [`ChannelMapping::Speakers`]. This will automatically
/// upmix or downmix the connections according to common speaker configurations,
/// such as stereo-to-mono, when no explicit mapping is provided. In practice,
/// you'll rarely need to provide these explicit mappings.
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
    fn connect(self, target: impl Into<EdgeTarget>) -> ConnectCommands<'a>;

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
    fn chain_node<B: Bundle>(self, node: B) -> ConnectCommands<'a>;

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

#[cfg_attr(debug_assertions, track_caller)]
fn connect_with_commands(
    target: EdgeTarget,
    connections: Option<Vec<(u32, u32)>>,
    commands: &mut EntityCommands,
) {
    #[cfg(debug_assertions)]
    let location = Location::caller();

    commands
        .entry::<PendingConnections>()
        .or_default()
        .and_modify(|mut pending| {
            pending.push(PendingEdge::new_with_location(
                target,
                connections,
                #[cfg(debug_assertions)]
                location,
            ));
        });
}

impl<'a> Connect<'a> for EntityCommands<'a> {
    fn connect(mut self, target: impl Into<EdgeTarget>) -> ConnectCommands<'a> {
        let target = target.into();
        connect_with_commands(target, None, &mut self);

        ConnectCommands::new(self)
    }

    fn connect_with(
        mut self,
        target: impl Into<EdgeTarget>,
        ports: &[(u32, u32)],
    ) -> ConnectCommands<'a> {
        let target = target.into();
        let ports = ports.to_vec();
        connect_with_commands(target, Some(ports), &mut self);

        ConnectCommands::new(self)
    }

    fn chain_node<B: Bundle>(mut self, node: B) -> ConnectCommands<'a> {
        let new_id = self.commands().spawn(node).id();

        let mut new_connection = self.connect(new_id);
        new_connection.tail = Some(new_id);

        new_connection
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
    fn connect(mut self, target: impl Into<EdgeTarget>) -> ConnectCommands<'a> {
        let tail = self.tail();

        let mut commands = self.commands.commands();
        let mut commands = commands.entity(tail);

        let target = target.into();

        connect_with_commands(target, None, &mut commands);

        self
    }

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

        connect_with_commands(target, Some(ports), &mut commands);

        self
    }

    fn chain_node<B: Bundle>(mut self, node: B) -> ConnectCommands<'a> {
        let new_id = self.commands.commands().spawn(node).id();

        let mut new_connection = self.connect(new_id);
        new_connection.tail = Some(new_id);

        new_connection
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
    mut connections: Query<(
        &mut PendingConnections,
        &FirewheelNode,
        &FirewheelNodeInfo,
        &ChannelMapping,
    )>,
    targets: Query<(&FirewheelNode, &FirewheelNodeInfo)>,
    node_map: Res<NodeMap>,
    mut context: ResMut<AudioContext>,
) {
    let connections = connections
        .iter_mut()
        .filter(|(pending, ..)| !pending.0.is_empty())
        .collect::<Vec<_>>();

    if connections.is_empty() {
        return;
    }

    context.with(|context| {
        for (mut pending, source_node, source_info, source_mapping) in connections.into_iter() {
            pending.0.retain(|connection| {
                let Some((target_node, target_info)) =
                    super::fetch_target(connection, &node_map, &targets, (*context).deref())
                else {
                    return false;
                };

                let inferred_ports;
                let ports = match connection.ports.as_deref() {
                    Some(ports) => ports,
                    None => {
                        let outputs = source_info.channel_config.num_outputs.get();
                        let inputs = target_info.channel_config.num_inputs.get();

                        inferred_ports = source_mapping.map_channels(outputs, inputs);

                        inferred_ports.as_slice()
                    }
                };

                if let Err(e) = context.connect(source_node.0, target_node, ports, false) {
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
        context::AudioContext,
        edge::AudioGraphOutput,
        prelude::MainBus,
        test::{prepare_app, run},
    };

    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use firewheel::{
        channel_config::NonZeroChannelCount,
        nodes::volume::{VolumeNode, VolumeNodeConfig},
    };

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

    #[test]
    fn test_simple_auto_connect() {
        let mut app = prepare_app(|mut commands: Commands| {
            commands
                .spawn((
                    VolumeNode::default(),
                    VolumeNodeConfig {
                        channels: NonZeroChannelCount::new(1).unwrap(),
                    },
                    One,
                ))
                .chain_node((
                    VolumeNode::default(),
                    VolumeNodeConfig {
                        channels: NonZeroChannelCount::new(1).unwrap(),
                    },
                    Two,
                ))
                .connect(AudioGraphOutput);
        });

        let connected = run(
            &mut app,
            |one: Single<&FirewheelNode, With<One>>,
             two: Single<&FirewheelNode, With<Two>>,
             mut context: ResMut<AudioContext>| {
                context.with(|context| {
                    let edges = context.edges();

                    for edge in edges {
                        if edge.src_node == one.0 && edge.dst_node == two.0 {
                            return true;
                        }
                    }

                    false
                })
            },
        );

        assert!(connected);
    }

    #[test]
    fn test_downmix() {
        let mut app = prepare_app(|mut commands: Commands| {
            commands
                .spawn((
                    VolumeNode::default(),
                    VolumeNodeConfig {
                        channels: NonZeroChannelCount::new(2).unwrap(),
                    },
                    One,
                ))
                .chain_node((
                    VolumeNode::default(),
                    VolumeNodeConfig {
                        channels: NonZeroChannelCount::new(1).unwrap(),
                    },
                    Two,
                ))
                .connect(AudioGraphOutput);
        });

        let connected = run(
            &mut app,
            |one: Single<&FirewheelNode, With<One>>,
             two: Single<&FirewheelNode, With<Two>>,
             mut context: ResMut<AudioContext>| {
                context.with(|context| {
                    let edges = context.edges();

                    let mut left = false;
                    let mut right = false;
                    for edge in edges {
                        if edge.src_node == one.0
                            && edge.dst_node == two.0
                            && edge.src_port == 0
                            && edge.dst_port == 0
                        {
                            left = true;
                        }

                        if edge.src_node == one.0
                            && edge.dst_node == two.0
                            && edge.src_port == 1
                            && edge.dst_port == 0
                        {
                            right = true;
                        }
                    }

                    left && right
                })
            },
        );

        assert!(connected);
    }
}
