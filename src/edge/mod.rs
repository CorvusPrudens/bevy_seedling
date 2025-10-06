//! Node connection and disconnection utilities.

use crate::context::{AudioContext, SeedlingContextWrapper};
use crate::node::FirewheelNodeInfo;
use crate::node::label::InternedNodeLabel;
use crate::prelude::{FirewheelNode, MainBus, NodeLabel};
use bevy_ecs::prelude::*;
use bevy_log::error_once;
use bevy_platform::collections::HashMap;
use firewheel::node::NodeID;

#[cfg(debug_assertions)]
use core::panic::Location;

#[allow(clippy::module_inception)]
mod connect;
mod disconnect;

pub use connect::*;
pub use disconnect::*;

/// A node label for Firewheel's audio graph input.
///
/// To route the graph's input, you'll need to query for this entity.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::{prelude::*, edge::AudioGraphInput};
/// fn route_input(input: Single<Entity, With<AudioGraphInput>>, mut commands: Commands) {
///     let my_node = commands.spawn(VolumeNode::default()).id();
///
///     commands.entity(*input).connect(my_node);
/// }
/// ```
///
/// By default, Firewheel's graph will have no inputs. Make sure your
/// selected backend and [`FirewheelConfig`][firewheel::FirewheelConfig] are
/// configured for input.
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct AudioGraphInput;

/// A node label for Firewheel's audio graph output.
///
/// To route to the graph's output, simply call [connect][connect::Connect::connect]!
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::{prelude::*, edge::AudioGraphOutput};
/// fn route_output(mut commands: Commands) {
///     commands
///         .spawn(VolumeNode::default())
///         .connect(AudioGraphOutput);
/// }
/// ```
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct AudioGraphOutput;

/// Describes how a node's outputs are mapped to the inputs
/// of its connections.
///
/// Defaults to [`ChannelMapping::Speakers`].
///
/// This is applied when no explicit channel mapping is provided.
/// When the output and input channel count between two nodes
/// matches, all inputs and outputs will be connected in order
/// regardless of this setting.
#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub enum ChannelMapping {
    /// Uses a set of standard mappings for combinations of common speaker
    /// I/O setups (mono, stereo, quad, and 5.1). For example, when connecting
    /// a mono output to a stereo input, each stereo input will receive a connection.
    ///
    /// Non-standard configurations will fall back to [`ChannelMapping::Discrete`].
    #[default]
    Speakers,

    /// Outputs are mapped to inputs in-order up to the maximum number of inputs
    /// or outputs. Any additional channels are dropped.
    Discrete,
}

impl ChannelMapping {
    /// Maps the input channels to the output channels according
    /// to the variant.
    pub fn map_channels(&self, outputs: u32, inputs: u32) -> Vec<(u32, u32)> {
        let map_min = || (0..outputs.min(inputs)).map(|i| (i, i)).collect();

        match self {
            ChannelMapping::Discrete => map_min(),
            ChannelMapping::Speakers => {
                match (outputs, inputs) {
                    // Mono -> Stereo / Mono -> Quad
                    (1, 2) | (1, 4) => {
                        vec![(0, 0), (0, 1)]
                    }
                    // Mono -> 5.1
                    (1, 6) => {
                        vec![(0, 2)]
                    }
                    // Stereo -> Mono
                    (2, 1) => {
                        vec![(0, 0), (1, 0)]
                    }
                    // Stereo -> Quad / Stereo -> 5.1
                    (2, 4) | (2, 6) => {
                        vec![(0, 0), (1, 1)]
                    }
                    // Quad -> Mono
                    (4, 1) => {
                        vec![(0, 0), (1, 0), (2, 0), (3, 0)]
                    }
                    // Quad -> Stereo
                    (4, 2) => {
                        vec![(0, 0), (1, 1), (2, 0), (3, 1)]
                    }
                    // Quad -> 5.1
                    (4, 6) => {
                        vec![(0, 0), (1, 1), (2, 4), (3, 5)]
                    }
                    // 5.1 -> Mono
                    (6, 1) => {
                        vec![(0, 0), (1, 0), (2, 0), (4, 0), (5, 0)]
                    }
                    // 5.1 -> Stereo
                    (6, 2) => {
                        vec![(0, 0), (2, 0), (4, 0), (1, 1), (2, 1), (5, 1)]
                    }
                    // 5.1 -> Quad
                    (6, 4) => {
                        vec![(0, 0), (2, 0), (1, 1), (2, 1), (4, 2), (5, 3)]
                    }
                    _ => map_min(),
                }
            }
        }
    }
}

/// A target for node connections.
///
/// [`EdgeTarget`] can be constructed manually or
/// used as a part of the [`Connect`] and [`Disconnect`] APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeTarget {
    /// A global label such as [`MainBus`].
    Label(InternedNodeLabel),
    /// An audio entity.
    Entity(Entity),
    /// An existing node from the audio graph.
    Node(NodeID),
}

/// A pending edge between two nodes.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PendingEdge {
    /// The edge target.
    ///
    /// The connection will be made between this entity's output
    /// and the target's input.
    pub target: EdgeTarget,

    /// An optional [`firewheel`] port mapping.
    ///
    /// The first tuple element represents the source output,
    /// and the second tuple element represents the sink input.
    ///
    /// If an explicit port mapping is not provided,
    /// `[(0, 0), (1, 1)]` is used.
    pub ports: Option<Vec<(u32, u32)>>,

    #[cfg(debug_assertions)]
    pub(crate) origin: &'static Location<'static>,
}

impl PendingEdge {
    /// Construct a new [`PendingEdge`].
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn new(target: impl Into<EdgeTarget>, ports: Option<Vec<(u32, u32)>>) -> Self {
        Self {
            target: target.into(),
            ports,
            #[cfg(debug_assertions)]
            origin: Location::caller(),
        }
    }

    /// An internal constructor for passing context through closures.
    fn new_with_location(
        target: impl Into<EdgeTarget>,
        ports: Option<Vec<(u32, u32)>>,
        #[cfg(debug_assertions)] location: &'static Location<'static>,
    ) -> Self {
        Self {
            target: target.into(),
            ports,
            #[cfg(debug_assertions)]
            origin: location,
        }
    }
}

impl From<NodeID> for EdgeTarget {
    fn from(value: NodeID) -> Self {
        Self::Node(value)
    }
}

impl<T> From<T> for EdgeTarget
where
    T: NodeLabel,
{
    fn from(value: T) -> Self {
        Self::Label(value.intern())
    }
}

impl From<Entity> for EdgeTarget {
    fn from(value: Entity) -> Self {
        Self::Entity(value)
    }
}

/// A map that associates [`NodeLabel`]s with audio
/// graph nodes.
///
/// This will be automatically synchronized for
/// entities with both a [`FirewheelNode`] and [`NodeLabel`]
/// component.
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
///
/// Importantly, this should _only_ apply connections to nodes that have
/// outputs.
pub(crate) fn auto_connect(
    nodes: Query<(Entity, &FirewheelNode), Without<PendingConnections>>,
    mut context: ResMut<AudioContext>,
    mut commands: Commands,
) {
    if nodes.iter().len() == 0 {
        return;
    }

    context.with(|context| {
        for (entity, node) in nodes.iter() {
            let Some(info) = context.node_info(node.0) else {
                continue;
            };

            let outputs = info.info.channel_config.num_outputs.get();
            if outputs == 0 {
                continue;
            }

            commands.entity(entity).connect(MainBus);
        }
    });
}

fn lookup_node<'a>(
    target_entity: Entity,
    connection: &PendingEdge,
    targets: &'a Query<(&FirewheelNode, &FirewheelNodeInfo)>,
) -> Option<(&'a FirewheelNode, &'a FirewheelNodeInfo)> {
    match targets.get(target_entity) {
        Ok(t) => Some(t),
        Err(_) => {
            #[cfg(debug_assertions)]
            {
                let location = connection.origin;
                error_once!(
                    "failed to connect to entity `{target_entity:?}` at {location}: no Firewheel node found"
                );
            }
            #[cfg(not(debug_assertions))]
            error_once!("failed to connect to entity `{target_entity:?}`: no Firewheel node found");

            None
        }
    }
}

fn fetch_target(
    connection: &PendingEdge,
    node_map: &NodeMap,
    targets: &Query<(&FirewheelNode, &FirewheelNodeInfo)>,
    context: &dyn SeedlingContextWrapper,
) -> Option<(NodeID, FirewheelNodeInfo)> {
    match connection.target {
        EdgeTarget::Entity(entity) => {
            let Some((node, info)) = lookup_node(entity, connection, targets) else {
                return None;
            };

            Some((node.0, *info))
        }
        EdgeTarget::Label(label) => {
            let Some(entity) = node_map.get(&label) else {
                #[cfg(debug_assertions)]
                {
                    let location = connection.origin;
                    error_once!(
                        "failed to connect to node label `{label:?}` at {location}: no associated Firewheel node found"
                    );
                }
                #[cfg(not(debug_assertions))]
                error_once!(
                    "failed to connect to node label `{label:?}`: no associated Firewheel node found"
                );

                return None;
            };

            let Some((node, info)) = lookup_node(*entity, connection, &targets) else {
                return None;
            };

            Some((node.0, *info))
        }
        EdgeTarget::Node(dest_node) => {
            let Some(info) = context.node_info(dest_node) else {
                error_once!(
                    "failed to connect audio node to target: the target `NodeID` doesn't exist"
                );
                return None;
            };
            let info = FirewheelNodeInfo::new(info);

            Some((dest_node, info))
        }
    }
}
