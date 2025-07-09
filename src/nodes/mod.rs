//! All of `bevy_seedling`'s audio nodes.

use crate::{SeedlingSystems, prelude::RegisterNode};
use bevy::prelude::*;

pub mod bpf;
pub mod envelopes;
pub mod freeverb;
pub mod lpf;
pub mod send;

/// Registration and logic for `bevy_seedling`'s audio nodes.
pub(crate) struct SeedlingNodesPlugin;

impl Plugin for SeedlingNodesPlugin {
    fn build(&self, app: &mut App) {
        app.register_node::<bpf::BandPassNode>()
            .register_node::<lpf::LowPassNode>()
            .register_node::<send::SendNode>()
            .register_node::<freeverb::FreeverbNode>()
            .register_node::<envelopes::AhdsrVolumeNode>()
            .add_systems(
                Last,
                (send::connect_sends, send::update_remote_sends).before(SeedlingSystems::Acquire),
            );
    }
}
