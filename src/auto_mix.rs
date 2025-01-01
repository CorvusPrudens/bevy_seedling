use crate::{
    node::{NodeMap, PendingConnection},
    ConnectTarget, Node,
};
use bevy_ecs::prelude::*;
use bevy_utils::HashMap;
use firewheel::{graph::AudioGraph, node::NodeID};

#[derive(Clone)]
struct MixerInput {
    target: ConnectTarget,
    // The intended connection
    transparent: Option<Vec<(u32, u32)>>,
    // The actual connection through the mixer
    actual: Vec<(u32, u32)>,
}

#[derive(Component)]
struct Mixer {
    inputs: Vec<MixerInput>,
}

#[derive(Clone, Copy)]
struct MixerRef(Entity);

fn promote_connection(
    nodes: &Query<&Node>,
    map: &NodeMap,
    source: NodeID,
    connection: &PendingConnection,
    graph: &mut AudioGraph,
    commands: &mut Commands,
) {
    let Some(dest) = connection.target.get(nodes, map) else {
        return;
    };

    // rebuilding this information is a bit tedious
    let existing_connections: Vec<_> = graph
        .edges()
        .filter(|e| e.dst_node == dest)
        .copied()
        .collect();

    let mut existing_connections = HashMap::new();
    for edge in graph.edges().filter(|e| e.dst_node == dest) {
        let entry = existing_connections
            .entry(edge.src_node)
            .or_insert(Vec::new());

        entry.push(*edge);
    }

    // This organizes them more like we expect, making
    // comparisons easy later on.
    for groups in existing_connections.values_mut() {
        groups.sort_unstable_by_key(|e| (e.src_port, e.dst_port));
    }

    // // for each existing connection, first disconnect it from the intended node
    // for (_, edge) in existing_connections {
    //     graph.disconnect_by_edge_id(edge.id);
    // }

    let mut mixer = Mixer {
        inputs: Default::default(),
    };

    // then, create an equivalent connection to the new mixer
    // for (_)

    commands.spawn(mixer);
}
