//! [![crates.io](https://img.shields.io/crates/v/bevy_seedling)](https://crates.io/crates/bevy_seedling)
//! [![docs.rs](https://docs.rs/bevy_seedling/badge.svg)](https://docs.rs/bevy_seedling)
//!
//! A sprouting integration of the [Firewheel](https://github.com/BillyDM/firewheel)
//! audio engine for [Bevy](https://bevyengine.org/).
//!
//! `bevy_seedling` is powerful, flexible, and [fast](https://github.com/CorvusPrudens/rust-audio-demo?tab=readme-ov-file#performance).
//! You can [play sounds](prelude::SamplePlayer), [apply effects](prelude::SampleEffects),
//! and [route audio anywhere](crate::edge::Connect). Creating
//! and integrating [custom audio processors](prelude::RegisterNode#creating-and-registering-nodes)
//! is simple.
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
//! bevy_seedling = "0.6.0-rc.1"
//! bevy = { version = "0.17.0-rc.1", default-features = false, features = [
//!   "std",
//!   "async_executor",
//!   "android-game-activity",
//!   "android_shared_stdcxx",
//!   "animation",
//!   "bevy_asset",
//!   "bevy_color",
//!   "bevy_core_pipeline",
//!   "bevy_post_process",
//!   "bevy_anti_alias",
//!   "bevy_gilrs",
//!   "bevy_gizmos",
//!   "bevy_gltf",
//!   "bevy_input_focus",
//!   "bevy_log",
//!   "bevy_mesh_picking_backend",
//!   "bevy_pbr",
//!   "bevy_picking",
//!   "bevy_render",
//!   "bevy_scene",
//!   "bevy_image",
//!   "bevy_mesh",
//!   "bevy_camera",
//!   "bevy_light",
//!   "bevy_shader",
//!   "bevy_sprite",
//!   "bevy_sprite_picking_backend",
//!   "bevy_sprite_render",
//!   "bevy_state",
//!   "bevy_text",
//!   "bevy_ui",
//!   "bevy_ui_picking_backend",
//!   "bevy_ui_render",
//!   "bevy_window",
//!   "bevy_winit",
//!   "custom_cursor",
//!   "default_font",
//!   "hdr",
//!   "ktx2",
//!   "multi_threaded",
//!   "png",
//!   "reflect_auto_register",
//!   "smaa_luts",
//!   "sysinfo_plugin",
//!   "tonemapping_luts",
//!   "webgl2",
//!   "x11",
//!   "wayland",
//!   "debug",
//!   "zstd_rust",
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
//!     commands.spawn((
//!         SamplePlayer::new(server.load("my_ambience.wav")).looping(),
//!         sample_effects![LowPassNode { frequency: 500.0 }],
//!     ));
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
//! - [The `SamplePlayer` component][prelude::SamplePlayer]
//! - [Controlling playback][prelude::PlaybackSettings]
//! - [The sample lifecycle][prelude::SamplePlayer#lifecycle]
//! - [Applying effects][prelude::SamplePlayer#applying-effects]
//!
//! ### Sampler pools
//! - [Dynamic pools][pool::dynamic]
//! - [Static pools][prelude::SamplerPool]
//!   - [Constructing pools][prelude::SamplerPool#constructing-pools]
//!   - [Playing samples in a pool][prelude::SamplerPool#playing-samples-in-a-pool]
//!   - [Pool architecture][prelude::SamplerPool#architecture]
//! - [The default pool][prelude::DefaultPool]
//!
//! ### The audio graph
//! - Routing audio
//!   - [Connecting nodes][crate::edge::Connect]
//!   - [Disconnecting nodes][crate::edge::Disconnect]
//!   - [Sends][prelude::SendNode]
//!   - [The main bus][prelude::MainBus]
//! - [Stream configuration][crate::configuration]
//! - [Graph configuration][crate::configuration::GraphConfiguration]
//!
//! ### Event scheduling
//! - [The `AudioEvents` component][crate::prelude::AudioEvents]
//! - [The audio clock][crate::time]
//!
//! ### Custom nodes
//! - [Creating and registering nodes][prelude::RegisterNode#creating-and-registering-nodes]
//! - [Synchronizing ECS and audio types][prelude::RegisterNode#synchronizing-ecs-and-audio-types]
//!
//! ## Feature flags
//!
//! | Flag            | Description                                | Default |
//! | --------------- | ------------------------------------------ | ------- |
//! | `reflect`       | Enable [`bevy_reflect`] derive macros.     | Yes     |
//! | `rand`          | Enable the [`RandomPitch`] component.      | Yes     |
//! | `wav`           | Enable WAV format and PCM encoding.        | Yes     |
//! | `ogg`           | Enable Ogg format and Vorbis encoding.     | Yes     |
//! | `mp3`           | Enable mp3 format and encoding.            | No      |
//! | `mkv`           | Enable mkv format.                         | No      |
//! | `adpcm`         | Enable adpcm encoding.                     | No      |
//! | `flac`          | Enable FLAC format and encoding.           | No      |
//! | `web_audio`     | Enable the multi-threading web backend.    | No      |
//! | `hrtf`          | Enable HRTF Spatialization.                | No      |
//! | `hrtf_subjects` | Enable all HRTF embedded data.             | No      |
//! | `loudness`      | Enable LUFS analyzer node.                 | Yes     |
//! | `stream`        | Enable CPAL input and output stream nodes. | Yes     |
//!
//! [`RandomPitch`]: crate::prelude::RandomPitch
//!
//! ## Frequently asked questions
//!
//! ### How do I dynamically change a sample's volume?
//!
//! The [`SamplePlayer::volume`][prelude::SamplePlayer::volume] field
//! cannot be changed after spawning or inserting the component. Nonetheless,
//! there are a few ways to manage dynamic volume changes depending on your needs.
//!
//! If you need individual control over each sample's volume, you should add a
//! [`VolumeNode`][prelude::VolumeNode] as an effect.
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_seedling::prelude::*;
//! # fn dynamic(mut commands: Commands, server: Res<AssetServer>) {
//! commands.spawn((
//!     SamplePlayer::new(server.load("my_sample.wav")),
//!     sample_effects![VolumeNode {
//!         volume: Volume::Decibels(-6.0),
//!         ..Default::default()
//!     }],
//! ));
//! # }
//! ```
//!
//! To see how to query for effects, refer to the [`EffectsQuery`][prelude::EffectsQuery]
//! trait.
//!
//! If you want to control groups of samples, such as all music, you'll
//! probably want to spawn a [`SamplerPool`][prelude::SamplerPool] and
//! update the pool's [`VolumeNode`][prelude::VolumeNode] rather than using
//! a node for each sample.
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_seedling::prelude::*;
//! # fn dynamic(mut commands: Commands, server: Res<AssetServer>) {
//! #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
//! struct MusicPool;
//!
//! commands.spawn(SamplerPool(MusicPool));
//!
//! commands.spawn((MusicPool, SamplePlayer::new(server.load("my_music.wav"))));
//!
//! // Update the volume of all music at once
//! fn update_music_volume(mut music: Single<&mut VolumeNode, With<SamplerPool<MusicPool>>>) {
//!     music.volume = Volume::Decibels(-6.0);
//! }
//! # }
//! ```
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
//! ## Glossary
//!
//! ### Bus
//!
//! In general audio processing, a _bus_ is typically some connection point, to which
//! we route many tracks of audio.
//!
//! In `bevy_seedling`, a bus is nothing special; it's really just a label
//! applied to a normal audio node. Since connecting many inputs to a node is
//! trivial, there's no need for special support. All of `bevy_seedling`'s
//! buses use [`VolumeNode`][prelude::VolumeNode], but you can apply a bus label to
//! whatever node you like.
//!
//! ### Node
//!
//! A _node_ is the smallest unit of audio processing.
//! It can receive inputs, produce outputs, or both, meaning nodes
//! can be used as sources, sinks, or effects.
//!
//! Nodes in `bevy_seedling` generally consist of two parts:
//! an ECS handle, like [`VolumeNode`][prelude::VolumeNode], and the
//! actual audio processor that we insert into the real-time audio graph.
//! "Node" may refer to either or both of these.
//!
//! ### [Pool][crate::prelude::SamplerPool]
//!
//! A _pool_ (or sampler pool) is a group of [`SamplerNode`]s connected
//! to a local bus. Sampler pools are roughly analogous to `bevy_kira_audio`'s
//! [tracks](https://docs.rs/bevy_kira_audio/latest/bevy_kira_audio/type.Audio.html),
//! where both allow you to play sounds in the same "place" in the audio graph.
//!
//! [`SamplerNode`]: prelude::SamplerNode
//!
//! ### Routing
//!
//! Digital audio is a relentless stream of discrete values. _Routing_ allows us to
//! direct this stream though various stages (or nodes, in Firewheel's case). Each
//! node has some number of input and output channels, to and from which we can arbitrarily route
//! audio.
//!
//! In the simplest case, we'd route the output of a source like [`SamplerNode`] directly
//! to the graph's output. If we want to change the volume, we could insert a [`VolumeNode`]
//! in between the sampler and the output. If we wanted to add reverb, we could also route
//! the [`SamplerNode`] to a [`FreeverbNode`].
//!
//!```text
//! ┌─────────────┐
//! │SamplerNode  │
//! └┬───────────┬┘
//! ┌▽─────────┐┌▽───────────┐
//! │VolumeNode││FreeverbNode│
//! └┬─────────┘└┬───────────┘
//! ┌▽───────────▽┐
//! │GraphOutput  │
//! └─────────────┘
//! ```
//!
//! As you can see, this routing is very powerful!
//!
//! [`VolumeNode`]: prelude::VolumeNode
//! [`FreeverbNode`]: prelude::FreeverbNode
//!
//! ### Sample
//!
//! In `bevy_seedling`, _sample_ primarily refers to a piece of recorded sound,
//! like an audio file. Samples aren't limited to audio files, however; anything
//! implementing [`SampleResource`] can work with [`AudioSample`].
//!
//! Note that "sample" can also refer to the individual amplitude measurements
//! that make up a sound. "Sample rate," often 44.1kHz or 48kHz, refers to these
//! measurements.
//!
//! [`SampleResource`]: firewheel::core::sample_resource::SampleResource
//! [`AudioSample`]: prelude::AudioSample

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![allow(clippy::type_complexity)]
#![expect(clippy::needless_doctest_main)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

// Naming trick to facilitate straightforward internal macro usage.
extern crate self as bevy_seedling;

use bevy_app::prelude::*;
use bevy_asset::prelude::AssetApp;
use bevy_ecs::prelude::*;
use context::AudioStreamConfig;
use firewheel::{CpalBackend, backend::AudioBackend};

// We re-export Firewheel here for convenience.
pub use firewheel;

pub mod configuration;
pub mod context;
pub mod edge;
pub mod error;
pub mod node;
pub mod nodes;
pub mod pool;
pub mod sample;
pub mod spatial;
pub mod time;
pub mod utils;

pub mod prelude {
    //! All `bevy_seedlings`'s important types and traits.

    pub use crate::configuration::{
        GraphConfiguration, InputDeviceInfo, MusicPool, OutputDeviceInfo, SeedlingStartupSystems,
        SfxBus, SpatialPool,
    };
    pub use crate::context::AudioContext;
    pub use crate::edge::{AudioGraphInput, AudioGraphOutput, Connect, Disconnect, EdgeTarget};
    pub use crate::node::{
        FirewheelNode, RegisterNode,
        events::{AudioEvents, VolumeFade},
        label::{MainBus, NodeLabel},
    };
    #[cfg(feature = "loudness")]
    pub use crate::nodes::loudness::{LoudnessConfig, LoudnessNode, LoudnessState};
    pub use crate::nodes::{
        bpf::{BandPassConfig, BandPassNode},
        freeverb::FreeverbNode,
        itd::{ItdConfig, ItdNode},
        limiter::{LimiterConfig, LimiterNode},
        lpf::{LowPassConfig, LowPassNode},
        send::{SendConfig, SendNode},
    };
    pub use crate::pool::{
        DefaultPoolSize, PlaybackCompletionEvent, PoolCommands, PoolDespawn, PoolSize, SamplerPool,
        dynamic::DynamicBus,
        label::{DefaultPool, PoolLabel},
        sample_effects::{EffectOf, EffectsQuery, SampleEffects},
    };
    pub use crate::sample::{
        AudioSample, OnComplete, PlaybackSettings, SamplePlayer, SamplePriority,
    };
    pub use crate::sample_effects;
    pub use crate::spatial::{
        DefaultSpatialScale, SpatialListener2D, SpatialListener3D, SpatialScale,
    };
    pub use crate::time::{Audio, AudioTime};
    pub use crate::utils::perceptual_volume::PerceptualVolume;
    pub use crate::{SeedlingPlugin, SeedlingSystems};

    pub use firewheel::{
        CpalBackend, FirewheelConfig, Volume,
        channel_config::{ChannelCount, NonZeroChannelCount},
        clock::{
            DurationMusical, DurationSamples, DurationSeconds, InstantMusical, InstantSamples,
            InstantSeconds,
        },
        diff::{Memo, Notify},
        nodes::{
            StereoToMonoNode,
            sampler::{
                PlaybackSpeedQuality, PlaybackState, Playhead, RepeatMode, SamplerConfig,
                SamplerNode,
            },
            spatial_basic::SpatialBasicNode,
            volume::{VolumeNode, VolumeNodeConfig},
            volume_pan::VolumePanNode,
        },
    };

    #[cfg(feature = "stream")]
    pub use firewheel::nodes::stream::{
        reader::{StreamReaderConfig, StreamReaderNode},
        writer::{StreamWriterConfig, StreamWriterNode},
    };

    #[cfg(feature = "hrtf")]
    pub use firewheel_ircam_hrtf::{self as hrtf, HrtfConfig, HrtfNode};

    #[cfg(feature = "rand")]
    pub use crate::sample::RandomPitch;
}

/// Sets for all `bevy_seedling` systems.
///
/// These are all inserted into the [`Last`] schedule.
///
/// [`Last`]: bevy_app::prelude::Last
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
pub struct SeedlingPlugin<B: AudioBackend> {
    /// [`firewheel`]'s config, forwarded directly to
    /// the engine.
    ///
    /// [`firewheel`]: firewheel
    pub config: prelude::FirewheelConfig,

    /// The stream settings, forwarded directly to the backend.
    ///
    /// After this plugin is added, this configuration is added
    /// as an [`AudioStreamConfig`] resource.
    pub stream_config: B::Config,

    /// The initial graph configuration.
    pub graph_config: configuration::GraphConfiguration,
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
    /// Create a new default [`SeedlingPlugin`] with the specified backend.
    pub fn new() -> Self {
        Self {
            config: prelude::FirewheelConfig::default(),
            stream_config: B::Config::default(),
            graph_config: prelude::GraphConfiguration::default(),
        }
    }
}

#[cfg(feature = "web_audio")]
impl SeedlingPlugin<firewheel_web_audio::WebAudioBackend> {
    /// Create a new default [`SeedlingPlugin`] with the [`firewheel_web_audio`] backend.
    ///
    /// [`firewheel_web_audio`] uses Wasm multi-threading to execute its audio processing
    /// in the browser's high-priority audio thread. This eliminates all stuttering
    /// induced by running the audio processing in the main browser thread, which is
    /// what the default backend, `cpal`, does.
    ///
    /// Wasm multi-threading requires a few
    /// steps, including a nightly compiler, so you'll likely want to feature-gate this
    /// backend.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// # fn plugin(app: &mut App) {
    /// #[cfg(not(feature = "web_audio"))]
    /// app.add_plugins(SeedlingPlugin::default());
    ///
    /// #[cfg(feature = "web_audio")]
    /// app.add_plugins(SeedlingPlugin::new_web_audio());
    /// # }
    /// ```
    ///
    /// To build and run your app, consider using the
    /// [Bevy CLI](https://github.com/TheBevyFlock/bevy_cli).
    ///
    /// ```text
    /// bevy run --features web_audio web -U web-multi-threading
    /// ```
    ///
    /// This automatically enables the required nightly features and
    /// HTTP headers for web multi-threading. To host your game
    /// on a site like [itch.io](itch.io), make sure you enable the
    /// "`SharedArrayBuffer` support" checkbox. For more details,
    /// see the [`firewheel_web_audio`] crate docs.
    pub fn new_web_audio() -> Self {
        Self {
            config: prelude::FirewheelConfig::default(),
            stream_config: <firewheel_web_audio::WebAudioBackend as AudioBackend>::Config::default(
            ),
            graph_config: prelude::GraphConfiguration::default(),
        }
    }
}

/// Run a system if the given resource has changed, ignoring
/// change ticks on startup.
fn resource_changed_without_insert<R: Resource>(res: Res<R>, mut has_run: Local<bool>) -> bool {
    let changed = res.is_changed() && *has_run;
    *has_run = true;

    changed
}

impl<B: AudioBackend> Plugin for SeedlingPlugin<B>
where
    B: 'static,
    B::Config: Clone + Send + Sync + 'static,
    B::StreamError: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        use prelude::*;

        app.insert_resource(context::AudioStreamConfig::<B>(self.stream_config.clone()))
            .insert_resource(configuration::ConfigResource(self.graph_config))
            .init_resource::<edge::NodeMap>()
            .init_resource::<node::ScheduleDiffing>()
            .init_resource::<node::AudioScheduleLookahead>()
            .init_resource::<node::PendingRemovals>()
            .init_resource::<pool::DefaultPoolSize>()
            .init_asset::<sample::AudioSample>()
            .register_node::<VolumeNode>()
            .register_node::<VolumePanNode>()
            .register_node::<SpatialBasicNode>()
            .register_simple_node::<StereoToMonoNode>();

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
            PostUpdate,
            (context::pre_restart_context, context::restart_context::<B>)
                .chain()
                .run_if(resource_changed_without_insert::<AudioStreamConfig<B>>),
        )
        .add_observer(node::label::NodeLabels::on_add_observer)
        .add_observer(node::label::NodeLabels::on_replace_observer)
        .add_observer(sample::observe_player_insert);

        app.add_plugins((
            configuration::SeedlingStartup::<B>::new(self.config),
            pool::SamplePoolPlugin,
            nodes::SeedlingNodesPlugin,
            node::events::EventsPlugin,
            spatial::SpatialPlugin,
            time::TimePlugin,
            #[cfg(feature = "rand")]
            sample::RandomPlugin,
        ));

        #[cfg(feature = "stream")]
        app.register_simple_node::<StreamReaderNode>()
            .register_simple_node::<StreamWriterNode>();

        #[cfg(all(feature = "reflect", feature = "stream"))]
        app.register_type::<StreamReaderNode>()
            .register_type::<StreamWriterNode>();

        #[cfg(feature = "hrtf")]
        app.register_node::<HrtfNode>();

        #[cfg(all(feature = "reflect", feature = "hrtf"))]
        app.register_type::<HrtfNode>()
            .register_type::<HrtfConfig>();

        #[cfg(all(feature = "reflect", feature = "rand"))]
        app.register_type::<RandomPitch>();

        #[cfg(feature = "reflect")]
        app.register_type::<FirewheelNode>()
            .register_type::<SamplePlayer>()
            .register_type::<SamplePriority>()
            .register_type::<PlaybackSettings>()
            .register_type::<sample::SampleQueueLifetime>()
            .register_type::<OnComplete>()
            .register_type::<SpatialScale>()
            .register_type::<DefaultSpatialScale>()
            .register_type::<SpatialListener2D>()
            .register_type::<SpatialListener3D>()
            .register_type::<InputDeviceInfo>()
            .register_type::<OutputDeviceInfo>()
            .register_type::<firewheel::node::NodeID>()
            .register_type::<node::follower::FollowerOf>()
            .register_type::<SendNode>()
            .register_type::<LowPassNode>()
            .register_type::<LowPassConfig>()
            .register_type::<BandPassConfig>()
            .register_type::<LimiterNode>()
            .register_type::<LimiterConfig>()
            .register_type::<ItdNode>()
            .register_type::<ItdConfig>()
            .register_type::<LimiterConfig>()
            .register_type::<FreeverbNode>()
            .register_type::<Volume>()
            .register_type::<firewheel::dsp::pan_law::PanLaw>()
            .register_type::<MainBus>()
            .register_type::<PoolSize>()
            .register_type::<DefaultPoolSize>()
            .register_type::<PlaybackCompletionEvent>()
            .register_type::<DefaultPool>()
            .register_type::<SamplerPool<DefaultPool>>()
            .register_type::<DynamicBus>()
            .register_type::<configuration::FetchAudioIoEvent>()
            .register_type::<configuration::RestartAudioEvent>()
            .register_type::<configuration::SfxBus>()
            .register_type::<configuration::GraphConfiguration>()
            .register_type::<configuration::MusicPool>()
            .register_type::<SamplerPool<configuration::MusicPool>>()
            .register_type::<configuration::SpatialPool>()
            .register_type::<SamplerPool<configuration::SpatialPool>>()
            .register_type::<node::ScheduleDiffing>()
            .register_type::<node::AudioScheduleLookahead>()
            .register_type::<NonZeroChannelCount>()
            .register_type::<SamplerConfig>()
            .register_type::<PlaybackState>()
            .register_type::<RepeatMode>()
            .register_type::<Playhead>()
            .register_type::<Notify<f32>>()
            .register_type::<Notify<bool>>()
            .register_type::<Notify<PlaybackState>>()
            .register_type::<InstantMusical>()
            .register_type::<InstantSeconds>()
            .register_type::<InstantSamples>()
            .register_type::<DurationMusical>()
            .register_type::<DurationSeconds>()
            .register_type::<DurationSamples>()
            .register_type::<VolumeNode>()
            .register_type::<VolumeNodeConfig>()
            .register_type::<VolumePanNode>();
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
            SeedlingPlugin::<crate::utils::profiling::ProfilingBackend> {
                graph_config: crate::configuration::GraphConfiguration::Empty,
                ..SeedlingPlugin::<crate::utils::profiling::ProfilingBackend>::new()
            },
            TransformPlugin,
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
