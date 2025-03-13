//! Type-base node labelling.
//!
//! `bevy_seedling` provides a single label, [MainBus],
//! which represents the terminal node that every other
//! node must eventually reach.
//!
//! Any node that doesn't provide an explicit connection when spawned
//! will be automatically connected to [MainBus].
use crate::node::NodeMap;
use bevy_ecs::{component::ComponentId, intern::Interned, prelude::*, world::DeferredWorld};
use firewheel::{nodes::volume::VolumeNode, Volume};
use smallvec::SmallVec;

use crate::{AudioContext, ConnectNode};

bevy_ecs::define_label!(
    /// A label for addressing audio nodes.
    ///
    /// Types that implement [NodeLabel] can be used in place of entity IDs
    /// for audio node connections.
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::{NodeLabel, VolumeNode, sample::SamplePlayer, ConnectNode};
    /// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
    /// struct EffectsChain;
    ///
    /// fn system(server: Res<AssetServer>, mut commands: Commands) {
    ///     commands.spawn((VolumeNode { normalized_volume: 0.25 }, EffectsChain));
    ///
    ///     commands
    ///         .spawn(SamplePlayer::new(server.load("sound.wav")))
    ///         .connect(EffectsChain);
    /// }
    /// ```
    NodeLabel,
    NODE_LABEL_INTERNER
);

/// The main audio bus.
///
/// All audio nodes must pass through this bus to
/// reach the output.
///
/// If no connections are specified for an entity
/// with a [Node][crate::Node] component, the
/// node will automatically be routed to this bus.
///
/// [MainBus] is a stereo volume node. To adjust the
/// global volume, you can query for a volume node's parameters
/// filtered on this label.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::{MainBus, VolumeNode};
/// fn mute(mut q: Single<&mut VolumeNode, With<MainBus>>) {
///     let mut params = q.into_inner();
///     params.normalized_volume = 0.0;
/// }
/// ```
#[derive(crate::NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MainBus;

/// A type-erased node label.
pub type InternedNodeLabel = Interned<dyn NodeLabel>;

pub(crate) fn insert_main_bus(mut commands: Commands, mut context: ResMut<AudioContext>) {
    let terminal_node = context.with(|context| context.graph_out_node_id());

    commands
        .spawn((
            VolumeNode {
                volume: Volume::Linear(1.),
            },
            MainBus,
        ))
        .connect(terminal_node);
}

/// A collection of all node labels applied to an entity.
///
/// To associate a label with an audio node,
/// the node entity should be spawned with the label.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::{NodeLabel, VolumeNode};
/// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct MyLabel;
/// # fn system(mut commands: Commands) {
///
/// commands.spawn((VolumeNode { normalized_volume: 0.25 }, MyLabel));
/// # }
#[derive(Debug, Default, Component)]
#[component(on_remove = on_remove)]
pub struct NodeLabels(SmallVec<[InternedNodeLabel; 1]>);

fn on_remove(mut world: DeferredWorld, entity: Entity, _: ComponentId) {
    let Some(labels) = world.get::<NodeLabels>(entity) else {
        return;
    };

    if labels.0.len() == 1 {
        let label = labels.0[0];
        let mut node_map = world.resource_mut::<NodeMap>();

        node_map.remove(&label);
    } else {
        let labels = labels.0.to_vec();
        let mut node_map = world.resource_mut::<NodeMap>();

        node_map.retain(|key, _| !labels.contains(key));
    }
}

impl core::ops::Deref for NodeLabels {
    type Target = [InternedNodeLabel];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl NodeLabels {
    /// Insert an interned node label.
    ///
    /// Returns `true` if the label is newly inserted.
    pub fn insert(&mut self, label: InternedNodeLabel) -> bool {
        if !self.contains(&label) {
            self.0.push(label);
            true
        } else {
            false
        }
    }

    /// Remove a label.
    ///
    /// Returns `true` if the label was in the set.
    pub fn remove(&mut self, label: InternedNodeLabel) -> bool {
        let index = self.iter().position(|l| l == &label);

        match index {
            Some(i) => {
                self.0.remove(i);
                true
            }
            None => false,
        }
    }
}
