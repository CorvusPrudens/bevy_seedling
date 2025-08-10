//! All of `bevy_seedling`'s audio nodes.

use crate::{SeedlingSystems, prelude::RegisterNode};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

pub mod bpf;
pub mod freeverb;
pub mod itd;
pub mod limiter;
pub mod lpf;
pub mod send;

#[cfg(feature = "loudness")]
pub mod loudness;

/// Registration and logic for `bevy_seedling`'s audio nodes.
pub(crate) struct SeedlingNodesPlugin;

impl Plugin for SeedlingNodesPlugin {
    fn build(&self, app: &mut App) {
        app.register_node::<bpf::BandPassNode>()
            .register_node::<lpf::LowPassNode>()
            .register_node::<send::SendNode>()
            .register_node::<freeverb::FreeverbNode>()
            .register_node::<limiter::LimiterNode>()
            .register_node::<itd::ItdNode>()
            .add_systems(
                Last,
                (send::connect_sends, send::update_remote_sends).before(SeedlingSystems::Acquire),
            );

        #[cfg(feature = "loudness")]
        app.register_simple_node::<loudness::LoudnessNode>();

        #[cfg(all(feature = "reflect", feature = "loudness"))]
        app.register_type::<loudness::LoudnessNode>();
    }
}
