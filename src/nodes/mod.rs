//! All of `bevy_seedling`'s audio nodes.

use crate::{SeedlingSystems, prelude::RegisterNode};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

pub mod itd;
pub mod limiter;
pub mod send;

#[cfg(feature = "loudness")]
pub mod loudness;

/// Core Firewheel nodes.
pub mod core {
    pub use firewheel::nodes::{
        StereoToMonoNode,
        freeverb::FreeverbNode,
        sampler::{PlayFrom, PlaybackSpeedQuality, RepeatMode, SamplerConfig, SamplerNode},
        spatial_basic::SpatialBasicNode,
        volume::{VolumeNode, VolumeNodeConfig},
        volume_pan::VolumePanNode,
    };
}

/// Effects and analysis nodes from Firewheel.
#[cfg(feature = "effects")]
pub mod effects {
    pub use firewheel::nodes::{
        convolution::{ConvolutionNode, ConvolutionNodeConfig},
        delay_compensation::{DelayCompNodeConfig, DelayCompensationNode},
        fast_filters::{
            bandpass::FastBandpassNode, highpass::FastHighpassNode, lowpass::FastLowpassNode,
        },
        fast_rms::{FastRmsNode, FastRmsState},
        mix::{MixNode, MixNodeConfig},
        noise_generator::{
            pink::{PinkNoiseGenConfig, PinkNoiseGenNode},
            white::{WhiteNoiseGenConfig, WhiteNoiseGenNode},
        },
        peak_meter::{PeakMeterNode, PeakMeterState},
        svf::{SvfNode, SvfNodeConfig},
    };
}

/// Registration and logic for `bevy_seedling`'s audio nodes.
pub(crate) struct SeedlingNodesPlugin;

impl Plugin for SeedlingNodesPlugin {
    fn build(&self, app: &mut App) {
        use core::*;

        // seedling nodes
        app.register_node::<send::SendNode>()
            .register_node::<limiter::LimiterNode>()
            .register_node::<itd::ItdNode>()
            .add_systems(
                Last,
                (send::connect_sends, send::update_remote_sends).before(SeedlingSystems::Acquire),
            );

        #[cfg(feature = "loudness")]
        app.register_node::<loudness::LoudnessNode>()
            .register_node_state::<loudness::LoudnessNode, loudness::LoudnessState>();

        // third party
        // #[cfg(feature = "hrtf")]
        // app.register_node::<HrtfNode>();

        // core Firewheel nodes
        app.register_node::<VolumeNode>()
            .register_node::<VolumePanNode>()
            .register_node::<SpatialBasicNode>()
            .register_node::<FreeverbNode>()
            .register_simple_node::<StereoToMonoNode>();

        #[cfg(feature = "effects")]
        {
            use effects::*;

            app.register_simple_node::<DelayCompensationNode>()
                .register_node_state::<FastRmsNode, FastRmsState>()
                .register_node_state::<PeakMeterNode, PeakMeterState>()
                .register_node::<SvfNode>()
                .register_node::<FastBandpassNode>()
                .register_node::<FastHighpassNode>()
                .register_node::<FastLowpassNode>()
                .register_node::<MixNode>()
                .register_node::<PinkNoiseGenNode>()
                .register_node::<WhiteNoiseGenNode>()
                .register_node::<ConvolutionNode>();
        }
    }
}
