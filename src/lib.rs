#![doc = include_str!("../README.md")]
#![allow(clippy::type_complexity)]
#![warn(missing_debug_implementations)]

extern crate self as bevy_seedling;

use bevy_app::{Last, Plugin, Startup};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::*;
use firewheel::FirewheelConfig;

pub mod activity;
pub mod bpf;
pub mod context;
pub mod label;
pub mod lpf;
pub mod node;
pub mod sample;
pub mod saw;

#[cfg(feature = "profiling")]
pub mod profiling;

pub use activity::{Pause, Stop};
pub use context::AudioContext;
pub use label::{MainBus, NodeLabel};
pub use node::{ConnectNode, ConnectTarget, Node};
pub use node::{RegisterNode, RegisterParamsNode};

// Re-export firewheel.
//
// This will be convenient during development since
// the version of firewheel tracked by this crate
// may just be an arbitrary commit in a fork.
pub use firewheel;
pub use firewheel::basic_nodes::{MixNode, VolumeNode};

/// Node label derive macro.
///
/// Node labels provide a convenient way to manage
/// connections with frequently used nodes.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::{NodeLabel, VolumeNode, ConnectNode,
/// # sample::SamplePlayer};
/// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct EffectsChain;
///
/// fn system(server: Res<AssetServer>, mut commands: Commands) {
///     commands.spawn((VolumeNode::new(0.25), EffectsChain));
///
///     // Now, any node can simply use `EffectsChain`
///     // as a connection target.
///     commands
///         .spawn(SamplePlayer::new(server.load("sound.wav")))
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
pub use seedling_macros::NodeLabel;

/// Sets for all `bevy_seedling` systems.
///
/// These are all inserted into the [`Last`] schedule.
///
/// [`Last`]: bevy_app::Last
#[derive(Debug, SystemSet, PartialEq, Eq, Hash, Clone)]
pub enum SeedlingSystems {
    /// Entities without audio nodes acquire them from the audio context.
    Acquire,
    /// Pending connections are made.
    Connect,
    /// Queue audio engine events.
    ///
    /// While it's not strictly necessary to separate this
    /// set from [`SeedlingSystems::Connect`], it's a nice
    /// semantic divide.
    Queue,
    /// The audio context is updated and flushed.
    Flush,
}

/// `bevy_seedling`'s top-level plugin.
///
/// This spawns the audio task in addition
/// to inserting `bevy_seedling`'s systems
/// and resources.
#[derive(Debug, Default)]
pub struct SeedlingPlugin {
    /// [`firewheel`]'s config, forwarded directly to
    /// the engine.
    ///
    /// [`firewheel`]: firewheel
    pub settings: FirewheelConfig,
}

impl Plugin for SeedlingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        let mut context = AudioContext::new(self.settings);
        let sample_rate = context.with(|ctx| ctx.stream_info().unwrap().sample_rate);

        app.insert_resource(context)
            .init_resource::<node::NodeMap>()
            .init_resource::<node::PendingRemovals>()
            .init_asset::<sample::Sample>()
            .register_asset_loader(sample::SampleLoader { sample_rate })
            .register_node::<sample::SamplePlayer>()
            .register_params_node::<saw::SawNode>()
            .register_params_node::<lpf::LowPassNode>()
            .register_params_node::<bpf::BandPassNode>()
            .register_params_node::<VolumeNode>()
            .register_node::<MixNode>()
            .configure_sets(
                Last,
                (
                    SeedlingSystems::Connect.after(SeedlingSystems::Acquire),
                    SeedlingSystems::Queue.after(SeedlingSystems::Acquire),
                    SeedlingSystems::Flush
                        .after(SeedlingSystems::Connect)
                        .after(SeedlingSystems::Queue),
                ),
            )
            .add_systems(Startup, label::insert_main_bus)
            .add_systems(
                Last,
                (
                    sample::on_add.in_set(SeedlingSystems::Acquire),
                    node::auto_connect
                        .before(SeedlingSystems::Connect)
                        .after(SeedlingSystems::Acquire),
                    node::process_connections.in_set(SeedlingSystems::Connect),
                    sample::trigger_pending_samples.in_set(SeedlingSystems::Queue),
                    (
                        node::process_removals,
                        node::flush_events,
                        context::update_context,
                    )
                        .chain()
                        .in_set(SeedlingSystems::Flush),
                ),
            );
    }
}
