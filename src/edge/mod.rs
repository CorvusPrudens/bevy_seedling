//! Node connection and disconnection utilities.

use crate::node::label::InternedNodeLabel;
use crate::prelude::{FirewheelNode, MainBus, NodeLabel};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use firewheel::node::NodeID;

#[cfg(debug_assertions)]
use core::panic::Location;

#[allow(clippy::module_inception)]
mod connect;
mod disconnect;

pub use connect::*;
pub use disconnect::*;

/// A marker component for Firewheel's audio graph input.
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
#[derive(Debug, Component)]
pub struct AudioGraphInput;

pub(crate) fn insert_input(
    mut commands: Commands,
    mut context: ResMut<crate::prelude::AudioContext>,
) {
    context.with(|ctx| {
        commands.spawn((
            AudioGraphInput,
            FirewheelNode(ctx.graph_in_node_id()),
            PendingConnections::default(),
        ));
    });
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

const DEFAULT_CONNECTION: &[(u32, u32)] = &[(0, 0), (1, 1)];

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
pub(crate) fn auto_connect(
    nodes: Query<Entity, (With<FirewheelNode>, Without<PendingConnections>)>,
    mut commands: Commands,
) {
    for node in nodes.iter() {
        commands.entity(node).connect(MainBus);
    }
}
