//! [![crates.io](https://img.shields.io/crates/v/bevy_seedling)](https://crates.io/crates/bevy_seedling)
//! [![docs.rs](https://docs.rs/bevy_seedling/badge.svg)](https://docs.rs/bevy_seedling)
//!
//! A sprouting integration of the [Firewheel](https://github.com/BillyDM/firewheel)
//! audio engine for [Bevy](https://bevyengine.org/).
//!
//! ## Getting started
//!
//! First, you'll need to add the dependency to your `Cargo.toml`.
//! Note that you'll need to disable Bevy's "bevy_audio" feature,
//! meaning you'll need to specify quite a few features
//! manually!
//! ```toml
//! [dependencies]
//! bevy_seedling = "0.3"
//! bevy = { version = "0.15", default-features = false, features = [
//!   "bevy_asset",
//!   "bevy_state",
//!   # ...
//! ] }
//! ```
//! [See here](https://docs.rs/crate/bevy/latest/features) for a list
//! of Bevy's default features.
//!
//! Then, you'll need to add the [`SeedlingPlugin`] to your app.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_seedling::prelude::*;
//!
//! fn main() {
//!     App::default()
//!         .add_plugins((DefaultPlugins, SeedlingPlugin::default()))
//!         .run();
//! }
//! ```
//!
//! [The repository's examples](https://github.com/CorvusPrudens/bevy_seedling/tree/master/examples)
//! should help you get up to speed on common usage patterns.
//!
//! ## Overview
//!
//! Once you've registered the plugin, playing a sample is easy!
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_seedling::prelude::*;
//! fn play(mut commands: Commands, server: Res<AssetServer>) {
//!     commands.spawn(SamplePlayer::new(server.load("my_sample.wav")));
//! }
//! ```
//!
//! [`PlaybackSettings`][prelude::PlaybackSettings] gives you some
//! control over how your samples are played.
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_seedling::prelude::*;
//! fn play_with_settings(mut commands: Commands, server: Res<AssetServer>) {
//!     commands.spawn((
//!         SamplePlayer::new(server.load("my_sample.wav")),
//!         PlaybackSettings::LOOP,
//!     ));
//! }
//! ```
//!
//! By default, sample players are queued up in the default sample pool,
//! [`DefaultPool`][prelude::DefaultPool]. If you'd like to apply effects to your
//! samples, you can define a new pool with per-sampler effects.
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_seedling::prelude::*;
//! fn custom_pool(mut commands: Commands, server: Res<AssetServer>) {
//!     // First, you'll need a label.
//!     #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
//!     struct MyPool;
//!
//!     // Let's spawn a pool with spatial audio and four samplers.
//!     Pool::new(MyPool, 4)
//!         .effect(SpatialBasicNode::default())
//!         .spawn(&mut commands);
//!
//!     // To play a sample in this pool, just spawn a sample
//!     // player with its label.
//!     commands.spawn((
//!         MyPool,
//!         SamplePlayer::new(server.load("my_sample.wav")),
//!     ));
//! }
//! ```
//!
//! You can also define free-standing effects chains and
//! connect multiple pools to it.
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_seedling::prelude::*;
//! fn chains(mut commands: Commands, server: Res<AssetServer>) {
//!     // We can also define labels for individual nodes.
//!     #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
//!     struct UnderwaterEffects;
//!
//!     commands.spawn((
//!         UnderwaterEffects,
//!         // We'll use a low-pass filter to simulate sounds underwater
//!         LowPassNode::new(1000.0),
//!     ))
//!     // Let's chain it into a volume node so everything's
//!     // a little quieter.
//!     .chain_node(VolumeNode {
//!         volume: Volume::Linear(0.5),
//!     });
//!
//!     // Finally, we'll create a couple sample pools and connect
//!     // them to our water effects.
//!     #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
//!     struct MusicPool;
//!
//!     Pool::new(MusicPool, 1)
//!         .spawn(&mut commands)
//!         .connect(UnderwaterEffects);
//!
//!     #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
//!     struct SfxPool;
//!
//!     Pool::new(SfxPool, 16)
//!         .spawn(&mut commands)
//!         .connect(UnderwaterEffects);
//! }
//! ```
//!
//! So far this has been fairly high-level, but
//! `bevy_seedling` still allows you to drop down very close
//! to Firewheel when needed. See [`Connect::connect_with`][prelude::Connect::connect_with]
//! for how to manage nodes with different channel shapes.
//!
//! ## Design
//!
//! `bevy_seedling` provides a thin ECS wrapper over `Firewheel`.
//!
//! A `Firewheel` audio node is typically represented in the ECS as
//! an entity with a [`Node`][prelude::Node] and a component that can generate
//! `Firewheel` events, such as [`VolumeNode`][prelude::VolumeNode].
//!
//! Interactions with the audio engine are buffered.
//! This includes inserting nodes into the audio graph,
//! removing nodes from the graph, making connections
//! between nodes, and sending node events. This provides
//! a few advantages:
//!
//! 1. Audio entities do not need to wait until
//!    they have Firewheel IDs before they can
//!    make connections or generate events.
//! 2. Systems that spawn or interact with
//!    audio entities can be trivially parallelized.
//! 3. Graph-mutating interactions are properly deferred
//!    while the audio graph isn't ready, for example
//!    if it's been temporarily deactiviated.

#![allow(clippy::type_complexity)]
#![expect(clippy::needless_doctest_main)]
#![warn(missing_debug_implementations)]

extern crate self as bevy_seedling;

use bevy_app::{Last, Plugin, PreStartup};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::*;

pub mod bpf;
pub mod connect;
pub mod context;
pub mod fixed_vec;
pub mod lpf;
pub mod node;
pub mod sample;
pub mod spatial;
pub mod timeline;

#[cfg(feature = "profiling")]
pub mod profiling;

pub mod prelude {
    //! All `bevy_seedlings`'s important types and traits.

    pub use crate::bpf::BandPassNode;
    pub use crate::connect::{Connect, ConnectTarget};
    pub use crate::context::AudioContext;
    pub use crate::lpf::LowPassNode;
    pub use crate::node::{
        label::{MainBus, NodeLabel},
        Node, RegisterNode,
    };
    pub use crate::sample::{
        label::{DefaultPool, PoolLabel},
        pool::{Pool, PoolCommands, PoolDespawn},
        PlaybackSettings, SamplePlayer,
    };
    pub use crate::spatial::{SpatialListener2D, SpatialListener3D};
    pub use crate::SeedlingPlugin;

    pub use firewheel::{
        clock::{ClockSamples, ClockSeconds},
        nodes::{
            sampler::SamplerNode,
            spatial_basic::{SpatialBasicConfig, SpatialBasicNode},
            volume::{VolumeNode, VolumeNodeConfig},
            volume_pan::{VolumePanNode, VolumePanNodeConfig},
            StereoToMonoNode,
        },
        FirewheelConfig, Volume,
    };

    #[cfg(feature = "stream")]
    pub use firewheel::nodes::stream::{
        reader::{StreamReaderConfig, StreamReaderNode},
        writer::{StreamWriterConfig, StreamWriterNode},
    };
}

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
#[derive(Debug)]
pub struct SeedlingPlugin {
    /// [`firewheel`]'s config, forwarded directly to
    /// the engine.
    ///
    /// [`firewheel`]: firewheel
    pub config: prelude::FirewheelConfig,

    /// The number of sampler nodes for the default
    /// sampler pool. If `None` is provided,
    /// the default pool will not be spawned, allowing
    /// you to set it up how you like.
    pub sample_pool_size: Option<usize>,
}

impl Default for SeedlingPlugin {
    fn default() -> Self {
        Self {
            config: Default::default(),
            sample_pool_size: Some(24),
        }
    }
}

impl Plugin for SeedlingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        use prelude::*;

        let mut context = AudioContext::new(self.config);
        let sample_rate = context.with(|ctx| ctx.stream_info().unwrap().sample_rate);
        let sample_pool_size = self.sample_pool_size;

        app.insert_resource(context)
            .init_resource::<connect::NodeMap>()
            .init_resource::<node::PendingRemovals>()
            .init_asset::<sample::Sample>()
            .register_asset_loader(sample::SampleLoader { sample_rate })
            .register_node::<lpf::LowPassNode>()
            .register_node::<bpf::BandPassNode>()
            .register_node::<VolumeNode>()
            .register_node::<VolumePanNode>()
            .register_node::<SpatialBasicNode>()
            .register_simple_node::<StereoToMonoNode>()
            .register_simple_node::<SamplerNode>();

        #[cfg(feature = "stream")]
        app.register_simple_node::<StreamReaderNode>()
            .register_simple_node::<StreamWriterNode>();

        app.configure_sets(
            Last,
            (
                SeedlingSystems::Connect.after(SeedlingSystems::Acquire),
                SeedlingSystems::Queue.after(SeedlingSystems::Acquire),
                SeedlingSystems::Flush
                    .after(SeedlingSystems::Connect)
                    .after(SeedlingSystems::Queue),
            ),
        )
        .add_systems(
            Last,
            (
                (spatial::update_2d_emitters, spatial::update_3d_emitters)
                    .before(SeedlingSystems::Acquire),
                connect::auto_connect
                    .before(SeedlingSystems::Connect)
                    .after(SeedlingSystems::Acquire),
                connect::process_connections.in_set(SeedlingSystems::Connect),
                (
                    node::process_removals,
                    node::flush_events,
                    context::update_context,
                )
                    .chain()
                    .in_set(SeedlingSystems::Flush),
            ),
        )
        .add_systems(
            PreStartup,
            (
                node::label::insert_main_bus,
                move |mut commands: Commands| {
                    if let Some(size) = sample_pool_size {
                        Pool::new(DefaultPool, size).spawn(&mut commands);
                    }
                },
            ),
        );

        app.add_plugins(sample::pool::SamplePoolPlugin);
    }
}
