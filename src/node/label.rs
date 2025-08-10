//! Type-base node labelling.
//!
//! `bevy_seedling` provides a single label, [MainBus],
//! which represents the terminal node that every other
//! node must eventually reach.
//!
//! Any node that doesn't provide an explicit connection when spawned
//! will be automatically connected to [MainBus].

use crate::edge::NodeMap;
use bevy_ecs::{intern::Interned, prelude::*};
use bevy_log::prelude::*;
use smallvec::SmallVec;

/// Node label derive macro.
///
/// Node labels provide a convenient way to manage
/// connections with frequently used nodes.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct EffectsChain;
///
/// fn system(server: Res<AssetServer>, mut commands: Commands) {
///     commands.spawn((
///         VolumeNode {
///             volume: Volume::Linear(0.25),
///             ..Default::default()
///         },
///         EffectsChain,
///     ));
///
///     // Now, any node can simply use `EffectsChain`
///     // as a connection target.
///     commands
///         .spawn(SamplePlayer::new(server.load("my_sample.wav")))
///         .connect(EffectsChain);
/// }
/// ```
///
/// [`NodeLabel`] also implements [`Component`] with the
/// required machinery to automatically synchronize itself
/// when inserted and removed. If you want custom component
/// behavior for your node labels, you'll need to derive
/// [`NodeLabel`] manually.
///
/// [`Component`]: bevy_ecs::component::Component
pub use bevy_seedling_macros::NodeLabel;

bevy_ecs::define_label!(
    /// A label for addressing audio nodes.
    ///
    /// Types that implement [NodeLabel] can be used in place of entity IDs
    /// for audio node connections.
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
    /// struct EffectsChain;
    ///
    /// fn system(server: Res<AssetServer>, mut commands: Commands) {
    ///     commands.spawn((
    ///         VolumeNode {
    ///             volume: Volume::Linear(0.25),
    ///             ..Default::default()
    ///         },
    ///         EffectsChain,
    ///     ));
    ///
    ///     commands
    ///         .spawn(SamplePlayer::new(server.load("my_sample.wav")))
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
/// with a [`FirewheelNode`][crate::prelude::FirewheelNode] component, the
/// node will automatically be routed to this bus.
/// For example, if you spawn a [`VolumeNode`][crate::prelude::VolumeNode]:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn spawn(mut commands: Commands) {
/// commands.spawn(VolumeNode::default());
/// # }
/// ```
///
/// it'll produce a graph like
///
/// ```text
/// ┌──────┐
/// │Volume│
/// └┬─────┘
/// ┌▽──────┐
/// │MainBus│
/// └───────┘
/// ```
///
/// [`MainBus`] is a stereo volume node. To adjust the
/// global volume, you can query for a volume node's parameters
/// filtered on this label.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// fn mute(mut q: Single<&mut VolumeNode, With<MainBus>>) {
///     let mut params = q.into_inner();
///     params.volume = Volume::Linear(0.0);
/// }
/// ```
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct MainBus;

/// A type-erased node label.
pub type InternedNodeLabel = Interned<dyn NodeLabel>;

/// A collection of all node labels applied to an entity.
///<
/// To associate a label with an audio node,
/// the node entity should be spawned with the label.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn system(mut commands: Commands) {
/// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct MyLabel;
///
/// commands.spawn((VolumeNode { volume: Volume::Linear(0.25), ..Default::default() }, MyLabel));
/// # }
#[derive(Debug, Default, Component, Clone)]
#[component(immutable)]
pub struct NodeLabels(SmallVec<[InternedNodeLabel; 1]>);

impl NodeLabels {
    pub(crate) fn on_add_observer(
        trigger: Trigger<OnInsert, NodeLabels>,
        labels: Query<&NodeLabels>,
        mut map: ResMut<NodeMap>,
    ) -> Result {
        let labels = labels.get(trigger.target())?;

        for label in labels.iter() {
            if let Some(existing) = map.insert(*label, trigger.target()) {
                if existing != trigger.target() {
                    warn!("node label `{label:?}` has been applied to multiple entities");
                }
            }
        }

        Ok(())
    }

    pub(crate) fn on_replace_observer(
        trigger: Trigger<OnReplace, NodeLabels>,
        labels: Query<&NodeLabels>,
        mut map: ResMut<NodeMap>,
    ) -> Result {
        let labels = labels.get(trigger.target())?;

        for label in labels.iter() {
            map.remove(label);
        }

        Ok(())
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

#[cfg(test)]
mod test {
    use crate::{
        edge::NodeMap,
        prelude::*,
        test::{prepare_app, run},
    };
    use bevy::prelude::*;

    #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
    struct TestLabel;

    #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
    struct TestLabelTwo;

    #[test]
    fn test_label_management() {
        let interned_one = TestLabel.intern();
        let interned_two = TestLabelTwo.intern();

        let mut app = prepare_app(|mut commands: Commands| {
            commands.spawn(SamplerPool(DefaultPool));

            commands
                .spawn((MainBus, VolumeNode::default()))
                .connect(AudioGraphOutput);

            commands.spawn((TestLabel, VolumeNode::default()));
        });

        run(
            &mut app,
            move |node: Query<Entity, With<TestLabel>>,
                  map: Res<NodeMap>,
                  mut commands: Commands| {
                let node = node.single().unwrap();
                assert_eq!(map[&interned_one], node);

                commands.entity(node).insert(TestLabelTwo);
            },
        );

        run(
            &mut app,
            move |node: Query<Entity, With<TestLabel>>,
                  map: Res<NodeMap>,
                  mut commands: Commands| {
                let node = node.single().unwrap();

                assert_eq!(map[&interned_one], node);
                assert_eq!(map[&interned_two], node);

                commands.entity(node).despawn();
            },
        );

        run(&mut app, move |map: Res<NodeMap>| {
            assert!(!map.contains_key(&interned_one));
            assert!(!map.contains_key(&interned_two));
        });
    }
}
