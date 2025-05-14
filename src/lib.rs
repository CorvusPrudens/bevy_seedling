//! [![crates.io](https://img.shields.io/crates/v/bevy_seedling)](https://crates.io/crates/bevy_seedling)
//! [![docs.rs](https://docs.rs/bevy_seedling/badge.svg)](https://docs.rs/bevy_seedling)
//!
//! A sprouting integration of the [Firewheel](https://github.com/BillyDM/firewheel)
//! audio engine for [Bevy](https://bevyengine.org/).
//!
//! ## Getting started
//!
//! First, you'll need to add the dependency to your `Cargo.toml`.
//! Note that you'll need to disable Bevy's `bevy_audio` feature,
//! meaning you'll need to specify quite a few features
//! manually!
//!
//! ```toml
//! [dependencies]
//! bevy_seedling = "0.3"
//! bevy = { version = "0.15", default-features = false, features = [
//!   "animation",
//!   "bevy_asset",
//!   "bevy_color",
//!   "bevy_core_pipeline",
//!   "bevy_gilrs",
//!   "bevy_gizmos",
//!   "bevy_gltf",
//!   "bevy_mesh_picking_backend",
//!   "bevy_pbr",
//!   "bevy_picking",
//!   "bevy_render",
//!   "bevy_scene",
//!   "bevy_sprite",
//!   "bevy_sprite_picking_backend",
//!   "bevy_state",
//!   "bevy_text",
//!   "bevy_ui",
//!   "bevy_ui_picking_backend",
//!   "bevy_window",
//!   "bevy_winit",
//!   "custom_cursor",
//!   "default_font",
//!   "hdr",
//!   "multi_threaded",
//!   "png",
//!   "smaa_luts",
//!   "sysinfo_plugin",
//!   "tonemapping_luts",
//!   "webgl2",
//!   "x11",
//! ] }
//! ```
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
//! Once you've set it all up, playing sounds is easy!
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_seedling::prelude::*;
//! fn play_sound(mut commands: Commands, server: Res<AssetServer>) {
//!     // Play a sound!
//!     commands.spawn(SamplePlayer::new(server.load("my_sample.wav")));
//!
//!     // Play a sound... with effects :O
//!     commands
//!         .spawn((
//!             SamplePlayer::new(server.load("my_ambience.wav")),
//!             PlaybackSettings::LOOP,
//!         ))
//!         .effect(LowPassNode::new(500.0));
//! }
//! ```
//!
//! [The repository's examples](https://github.com/CorvusPrudens/bevy_seedling/tree/master/examples)
//! should help you get up to speed on common usage patterns.
//!
//! ## Table of contents
//!
//! Below is a structured overview of this crate's documentation,
//! arranged to ease you into `bevy_seedling`'s features.
//!
//! ### Playing samples
//! - [The `SamplePlayer` type][prelude::SamplePlayer]
//! - [Controlling playback][prelude::PlaybackSettings]
//! - [The sample lifecycle][prelude::SamplePlayer#lifecycle]
//! - [Applying effects][prelude::SamplePlayer#applying-effects]
//!
//! ### Sampler pools
//! - [Dynamic pools][pool::dynamic]
//! - [Static pools][prelude::Pool]
//!   - [Constructing pools][prelude::Pool#constructing-pools]
//!   - [Playing samples in a pool][prelude::Pool#playing-samples-in-a-pool]
//!   - [Pool architecture][prelude::Pool#architecture]
//! - [The default pool][prelude::DefaultPool]
//!
//! ### Routing audio
//! - [Connecting nodes][crate::edge::Connect]
//! - [Disconnecting nodes][crate::edge::Disconnect]
//! - [Sends][prelude::SendNode]
//! - [The main bus][prelude::MainBus]
//!
//! ### Custom nodes
//! - [Creating and registering nodes][prelude::RegisterNode#creating-and-registering-nodes]
//! - [Synchronizing ECS and audio types][prelude::RegisterNode#synchronizing-ecs-and-audio-types]
//!
//! ## Feature flags
//!
//! | Flag | Description | Default feature |
//! | ---  | ----------- | --------------- |
//! | `wav` | Enable WAV format and PCM encoding. | Yes |
//! | `ogg` | Enable Ogg format and Vorbis encoding. | Yes |
//! | `mp3` | Enable mp3 format and encoding. | No |
//! | `mkv` | Enable mkv format. | No |
//! | `adpcm` | Enable adpcm encoding. | No |
//! | `flac` | Enable FLAC format and encoding. | No |
//! | `stream` | Enable CPAL input and output stream nodes. | Yes |
//!
//! ## Frequently asked questions
//!
//! ### How do I dynamically change player's volume?
//! The first thing to note is that `PlaybackSettings` only defines 
//! what should happen when the audio starts playing. That being said, 
//! the volume defined in it would be referred to as the maximum volume for this player.
//! However, the volume can be changed during the player's lifetime using a `VolumeNote`, for example:
//! ```ignore
//! sample_effects!(VolumeNode { volume: Volume::SILENT })
//! ```
//! These can then be queried and manipulated directly.
//! Note that, like other nodes, 
//! the node points to the sample player through `EffectOf(sample_player_entity)`.
//! 
//! ### Why aren't my mp3 samples making any sound?
//!
//! `bevy_seedling` enables a few formats and encodings by default.
//! If your format isn't included in the [default features][self#feature-flags],
//! you'll need to enable it in your `Cargo.toml`.
//!
//!
//! ```toml
//! [dependencies]
//! bevy_seedling = { version = "0.3.0", features = ["mp3"] }
//! ```
//!
//! ### Why isn't my custom node doing anything?
//!
//! `bevy_seedling` does quite a bit with Firewheel nodes under the hood.
//! To enable this machinery, you need to
//! [register your audio node][prelude::RegisterNode#creating-and-registering-nodes].
//!
//! ```ignore
//! use bevy::prelude::*;
//! use bevy_seedling::prelude::*;
//!
//! // Let's assume the relevant traits are implemented.
//! struct CustomNode;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, SeedlingPlugin::default()))
//!         .register_simple_node::<CustomNode>();
//! }
//! ```
//!
//! ### Why are my custom nodes crunchy (underrunning)?
//!
//! If you compile your project without optimizations, your custom audio nodes
//! may perform poorly enough to frequently underrun. You can compensate for
//! this by moving your audio code into a separate crate, selectively applying
//! optimizations.
//!
//! ```toml
//! // Cargo.toml
//! [dependencies]
//! my_custom_nodes = { path = "my_custom_nodes" }
//!
//! [profile.dev.package.my_custom_nodes]
//! opt-level = 3
//! ```
//!
//! ### Why am I getting "`PlaybackSettings`, `Volume`, etc. is ambiguous" errors?
//!
//! `bevy_seedling` re-uses some type names from `bevy_audio`. To avoid ambiguous imports,
//! you'll need to [prevent `bevy_audio` from being compiled][self#getting-started].
//! You may need to update your `Cargo.lock` file to ensure `bevy_audio` isn't included.
//!
//! It's also possible one of your third-part Bevy dependencies depends directly
//! on the `bevy` crate without disabling default features, causing `bevy_audio` to be
//! transitively enabled. In this case, encourage the crate authors to depend on
//! sub-crates (like `bevy_ecs`) or disable Bevy's default features!
//!
//! ## Architecture
//!
//! `bevy_seedling` provides a thin ECS wrapper over `Firewheel`.
//!
//! A `Firewheel` audio node is typically represented in the ECS as
//! an entity with a [`FirewheelNode`][prelude::FirewheelNode] and a component that can generate
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
#![warn(missing_docs)]

// Naming trick to facilitate straightforward internal macro usage.
extern crate self as bevy_seedling;

use bevy::prelude::*;
use core::ops::RangeInclusive;
use firewheel::{CpalBackend, backend::AudioBackend};

pub mod context;
pub mod edge;
pub mod error;
pub mod fixed_vec;
pub mod node;
pub mod nodes;
pub mod pool;
pub mod sample;
pub mod spatial;
pub mod timeline;

#[cfg(any(feature = "profiling", test))]
pub mod profiling;

pub mod prelude {
    //! All `bevy_seedlings`'s important types and traits.

    pub use crate::SeedlingPlugin;
    pub use crate::context::AudioContext;
    pub use crate::edge::{Connect, Disconnect, EdgeTarget};
    pub use crate::node::{
        FirewheelNode, RegisterNode,
        label::{MainBus, NodeLabel},
    };
    pub use crate::nodes::{
        bpf::{BandPassConfig, BandPassNode},
        freeverb::FreeverbNode,
        lpf::{LowPassConfig, LowPassNode},
        send::{SendConfig, SendNode},
    };
    pub use crate::pool::{
        PlaybackCompletionEvent, PoolCommands, PoolDespawn, SamplerPool,
        label::{DefaultPool, PoolLabel},
        sample_effects::{EffectOf, EffectsQuery, SampleEffects},
    };
    pub use crate::sample::{
        OnComplete, PlaybackParams, PlaybackSettings, SamplePlayer, SamplePriority,
    };
    pub use crate::sample_effects;
    pub use crate::spatial::{
        DefaultSpatialScale, SpatialListener2D, SpatialListener3D, SpatialScale,
    };

    pub use firewheel::{
        FirewheelConfig, Volume,
        clock::{ClockSamples, ClockSeconds},
        diff::{Memo, Notify},
        nodes::{
            StereoToMonoNode,
            sampler::{PlaybackSpeedQuality, PlaybackState, Playhead, RepeatMode, SamplerNode},
            spatial_basic::{SpatialBasicConfig, SpatialBasicNode},
            volume::{VolumeNode, VolumeNodeConfig},
            volume_pan::{VolumePanNode, VolumePanNodeConfig},
        },
    };

    #[cfg(feature = "stream")]
    pub use firewheel::nodes::stream::{
        reader::{StreamReaderConfig, StreamReaderNode},
        writer::{StreamWriterConfig, StreamWriterNode},
    };

    #[cfg(feature = "rand")]
    pub use crate::sample::PitchRange;
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
    /// Process sample pool operations.
    Pool,
    /// Queue audio engine events.
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
pub struct SeedlingPlugin<B: AudioBackend = CpalBackend> {
    /// [`firewheel`]'s config, forwarded directly to
    /// the engine.
    ///
    /// [`firewheel`]: firewheel
    pub config: prelude::FirewheelConfig,

    /// The stream settings, forwarded directly to the backend.
    pub stream_config: B::Config,

    /// Set whether to spawn the [`DefaultPool`].
    ///
    /// This allows you to define the default pool manually.
    pub spawn_default_pool: bool,

    /// Sets the default size range for sample pools.
    pub pool_size: RangeInclusive<usize>,
}

impl Default for SeedlingPlugin<CpalBackend> {
    fn default() -> Self {
        SeedlingPlugin::<CpalBackend>::new()
    }
}

impl<B: AudioBackend> SeedlingPlugin<B>
where
    B::Config: Default,
{
    /// Create a new default [`SeedlingPlugin`].
    pub fn new() -> Self {
        Self {
            config: Default::default(),
            stream_config: Default::default(),
            spawn_default_pool: true,
            pool_size: 4..=32,
        }
    }
}

impl<B: AudioBackend> Plugin for SeedlingPlugin<B>
where
    B: 'static,
    B::Config: Clone + Send + Sync + 'static,
    B::StreamError: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        use prelude::*;

        let mut context = AudioContext::new::<B>(self.config, self.stream_config.clone());
        let sample_rate = context.with(|ctx| ctx.stream_info().unwrap().sample_rate);
        let spawn_default = self.spawn_default_pool;

        app.insert_resource(context)
            .init_resource::<edge::NodeMap>()
            .init_resource::<node::PendingRemovals>()
            .init_resource::<spatial::DefaultSpatialScale>()
            .insert_resource(pool::DefaultPoolSize(self.pool_size.clone()))
            .init_asset::<sample::Sample>()
            .register_asset_loader(sample::SampleLoader { sample_rate })
            .register_node::<VolumeNode>()
            .register_node::<VolumePanNode>()
            .register_node::<SpatialBasicNode>()
            .register_simple_node::<StereoToMonoNode>();

        #[cfg(feature = "stream")]
        app.register_simple_node::<StreamReaderNode>()
            .register_simple_node::<StreamWriterNode>();

        app.configure_sets(
            Last,
            (
                SeedlingSystems::Connect.after(SeedlingSystems::Acquire),
                SeedlingSystems::Pool.after(SeedlingSystems::Connect),
                SeedlingSystems::Queue.after(SeedlingSystems::Pool),
                SeedlingSystems::Flush.after(SeedlingSystems::Queue),
            ),
        )
        .add_systems(
            Last,
            (
                (
                    spatial::update_2d_emitters,
                    spatial::update_2d_emitters_effects,
                    spatial::update_3d_emitters,
                    spatial::update_3d_emitters_effects,
                )
                    .before(SeedlingSystems::Acquire),
                edge::auto_connect
                    .before(SeedlingSystems::Connect)
                    .after(SeedlingSystems::Acquire),
                (edge::process_connections, edge::process_disconnections)
                    .chain()
                    .in_set(SeedlingSystems::Connect),
                node::flush_events.in_set(SeedlingSystems::Flush),
            ),
        )
        .add_systems(
            PreStartup,
            (
                node::label::insert_main_bus,
                move |mut commands: Commands| {
                    if spawn_default {
                        commands.spawn(SamplerPool(DefaultPool));
                    }
                },
            ),
        );

        app.add_plugins((
            pool::SamplePoolPlugin,
            nodes::SeedlingNodesPlugin,
            sample::RandomPlugin,
        ));
    }
}

#[cfg(test)]
mod test {
    use crate::prelude::*;
    use bevy::{ecs::system::RunSystemOnce, prelude::*};

    pub fn prepare_app<F: IntoSystem<(), (), M>, M>(startup: F) -> App {
        let mut app = App::new();

        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            SeedlingPlugin::<crate::profiling::ProfilingBackend> {
                spawn_default_pool: false,
                ..SeedlingPlugin::<crate::profiling::ProfilingBackend>::new()
            },
        ))
        .add_systems(Startup, startup);

        app.finish();
        app.cleanup();
        app.update();

        app
    }

    pub fn run<F: IntoSystem<(), O, M>, O, M>(app: &mut App, system: F) -> O {
        let world = app.world_mut();
        world.run_system_once(system).unwrap()
    }
}
