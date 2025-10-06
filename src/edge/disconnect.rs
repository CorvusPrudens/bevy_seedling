use super::{EdgeTarget, NodeMap, PendingEdge};
use crate::{
    context::AudioContext,
    node::{FirewheelNode, FirewheelNodeInfo},
};
use bevy_ecs::prelude::*;
use core::ops::Deref;

#[cfg(debug_assertions)]
use core::panic::Location;

/// The set of all pending disconnections for an entity.
///
/// These disconnections are drained and synchronized with the
/// audio graph in the [`SeedlingSystems::Connect`][crate::SeedlingSystems::Connect]
/// set.
#[derive(Debug, Default, Component)]
pub struct PendingDisconnections(Vec<PendingEdge>);

impl PendingDisconnections {
    /// Push a new pending disconnection.
    pub fn push(&mut self, disconnection: PendingEdge) {
        self.0.push(disconnection)
    }
}

/// An [`EntityCommands`] extension trait for disconnecting node entities.
///
/// Like with [`Connect`][crate::prelude::Connect], this trait accepts
/// both [`Entity`] and [`NodeLabel`] as edge targets.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn system(mut commands: Commands) {
/// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct TargetLabel;
///
/// // For some target node...
/// let target_node = commands.spawn((TargetLabel, VolumeNode::default())).id();
///
/// // We can connect and disconnect from it with either a label...
/// let node_a = commands
///     .spawn(VolumeNode::default())
///     .connect(TargetLabel)
///     .head();
///
/// commands.entity(node_a).disconnect(TargetLabel);
///
/// // or its `Entity`.
/// let node_b = commands
///     .spawn(VolumeNode::default())
///     .connect(target_node)
///     .head();
///
/// commands.entity(node_b).disconnect(target_node);
/// # }
/// ```
///
/// Disconnections are deferred, finalizing in the
/// [`SeedlingSystems::Connect`][crate::SeedlingSystems::Connect] set immediately
/// after connections.
///
/// [`EntityCommands`]: bevy_ecs::prelude::EntityCommands
/// [`NodeLabel`]: crate::prelude::NodeLabel
pub trait Disconnect: Sized {
    /// Queue a disconnection from this entity to the target.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// # fn system(mut commands: Commands) {
    /// // For any node connection...
    /// let node = commands
    ///     .spawn(VolumeNode::default())
    ///     .connect(MainBus)
    ///     .head();
    ///
    /// // We can process a corresponding disconnection.
    /// commands.entity(node).disconnect(MainBus);
    /// # }
    /// ```
    ///
    /// By default, this provides a port disconnection of `[(0, 0), (1, 1)]`,
    /// which represents a simple stereo disconnection.
    /// To provide a specific port mapping, use [`disconnect_with`][Disconnect::disconnect_with].
    ///
    /// The disconnection is deferred, finalizing in the
    /// [`SeedlingSystems::Connect`][crate::SeedlingSystems::Connect] set.
    #[cfg_attr(debug_assertions, track_caller)]
    fn disconnect(self, target: impl Into<EdgeTarget>) -> Self;

    /// Queue a disconnection from this entity to the target with the provided port mappings.
    ///
    /// The disconnection is deferred, finalizing in the
    /// [`SeedlingSystems::Connect`][crate::SeedlingSystems::Connect] set.
    #[cfg_attr(debug_assertions, track_caller)]
    fn disconnect_with(self, target: impl Into<EdgeTarget>, ports: &[(u32, u32)]) -> Self;
}

#[cfg_attr(debug_assertions, track_caller)]
fn disconnect_with_commands(
    target: EdgeTarget,
    ports: Option<Vec<(u32, u32)>>,
    commands: &mut EntityCommands,
) {
    #[cfg(debug_assertions)]
    let location = Location::caller();

    commands
        .entry::<PendingDisconnections>()
        .or_default()
        .and_modify(|mut pending| {
            pending.push(PendingEdge::new_with_location(
                target,
                ports,
                #[cfg(debug_assertions)]
                location,
            ));
        });
}

impl Disconnect for EntityCommands<'_> {
    fn disconnect(mut self, target: impl Into<EdgeTarget>) -> Self {
        let target = target.into();

        disconnect_with_commands(target, None, &mut self);

        self
    }

    fn disconnect_with(mut self, target: impl Into<EdgeTarget>, ports: &[(u32, u32)]) -> Self {
        let target = target.into();
        let ports = ports.to_vec();

        disconnect_with_commands(target, Some(ports), &mut self);

        self
    }
}

pub(crate) fn process_disconnections(
    mut disconnections: Query<(&mut PendingDisconnections, &FirewheelNode)>,
    targets: Query<(&FirewheelNode, &FirewheelNodeInfo)>,
    node_map: Res<NodeMap>,
    mut context: ResMut<AudioContext>,
) {
    let disconnections = disconnections
        .iter_mut()
        .filter(|(pending, _)| !pending.0.is_empty())
        .collect::<Vec<_>>();

    if disconnections.is_empty() {
        return;
    }

    context.with(|context| {
        for (mut pending, source_node) in disconnections.into_iter() {
            pending.0.retain(|disconnections| {
                let Some((target_node, _target_info)) =
                    super::fetch_target(disconnections, &node_map, &targets, (*context).deref())
                else {
                    return false;
                };

                let existing_connections;
                let ports = match disconnections.ports.as_deref() {
                    Some(ports) => ports,
                    None => {
                        existing_connections = context
                            .edges()
                            .into_iter()
                            .filter(|e| e.src_node == source_node.0 && e.dst_node == target_node)
                            .map(|e| (e.src_port, e.dst_port))
                            .collect::<Vec<_>>();

                        existing_connections.as_slice()
                    }
                };

                context.disconnect(source_node.0, target_node, ports);

                false
            });
        }
    });
}

#[cfg(test)]
mod test {
    use crate::{
        context::AudioContext,
        edge::{AudioGraphOutput, Connect},
        prelude::MainBus,
        test::{prepare_app, run},
    };

    use super::*;
    use firewheel::nodes::volume::VolumeNode;

    #[derive(Component)]
    struct One;
    #[derive(Component)]
    struct Two;

    #[test]
    fn test_disconnect() {
        let mut app = prepare_app(|mut commands: Commands| {
            commands
                .spawn((VolumeNode::default(), One))
                .chain_node((VolumeNode::default(), Two));

            commands
                .spawn((VolumeNode::default(), MainBus))
                .connect(AudioGraphOutput);
        });

        // first, verify they're all connected
        run(
            &mut app,
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
        );

        // Then, apply a disconnection
        run(
            &mut app,
            |one: Single<Entity, With<One>>,
             two: Single<Entity, With<Two>>,
             mut commands: Commands| {
                let one = one.into_inner();
                let two = two.into_inner();

                commands.entity(one).disconnect(two);
            },
        );

        app.update();

        // finally, verify one and two are disconnected
        run(
            &mut app,
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

                    assert_eq!(outgoing_edges_one.len(), 0);
                    assert_eq!(outgoing_edges_two.len(), 2);

                    assert!(outgoing_edges_one.iter().all(|e| e.dst_node == two.0));
                    assert!(outgoing_edges_two.iter().all(|e| e.dst_node == main.0));
                });
            },
        );
    }
}
