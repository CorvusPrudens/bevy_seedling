//! All of `bevy_seedling`'s audio nodes.

use crate::{SeedlingSystems, prelude::RegisterNode};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

pub mod send;

#[cfg(feature = "itd")]
pub mod itd;
#[cfg(feature = "limiter")]
pub mod limiter;
#[cfg(feature = "loudness")]
pub mod loudness;

/// Core Firewheel nodes.
pub mod core {
    pub use firewheel::nodes::{
        StereoToMonoNode,
        volume::{VolumeNode, VolumeNodeConfig},
        volume_pan::VolumePanNode,
    };

    #[cfg(feature = "spatial")]
    pub use firewheel::nodes::spatial_basic::SpatialBasicNode;

    #[cfg(feature = "sampler")]
    pub use firewheel::nodes::sampler::{
        PlayFrom, PlaybackSpeedQuality, RepeatMode, SamplerConfig, SamplerNode,
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
        freeverb::FreeverbNode,
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
        app.register_node::<send::SendNode>().add_systems(
            Last,
            (send::connect_sends, send::update_remote_sends).before(SeedlingSystems::Acquire),
        );

        #[cfg(feature = "limiter")]
        app.register_node::<limiter::LimiterNode>();

        #[cfg(feature = "itd")]
        app.register_node::<itd::ItdNode>();

        #[cfg(feature = "loudness")]
        app.register_node::<loudness::LoudnessNode>()
            .register_node_state::<loudness::LoudnessNode, loudness::LoudnessState>();

        #[cfg(feature = "hrtf")]
        app.register_node::<firewheel_ircam_hrtf::HrtfNode>();

        // core Firewheel nodes
        app.register_node::<VolumeNode>()
            .register_node::<VolumePanNode>()
            .register_simple_node::<StereoToMonoNode>();

        #[cfg(feature = "spatial")]
        app.register_node::<SpatialBasicNode>();

        #[cfg(feature = "effects")]
        {
            use effects::*;

            app.register_simple_node::<DelayCompensationNode>()
                .register_node_state::<FastRmsNode, FastRmsState>()
                .register_node_state::<PeakMeterNode, PeakMeterState>()
                .register_node::<FreeverbNode>()
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
