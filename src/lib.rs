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
//! <details>
//! <summary>Example `Cargo.toml`</summary>
//!
//! ```toml
//! [dependencies]
//! bevy_seedling = "0.8.0"
//! bevy = { version = "0.18.0", default-features = false, features = [
//!   # 2d
//!   "2d_bevy_render",
//!   "default_app",
//!   "picking",
//!   "scene",
//!
//!   # 3d
//!   "3d_bevy_render",
//!
//!   # ui
//!   "ui_api",
//!   "ui_bevy_render",
//!
//!   # default_platform
//!   "android-game-activity",
//!   "bevy_gilrs",
//!   "bevy_winit",
//!   "default_font",
//!   "multi_threaded",
//!   "std",
//!   "sysinfo_plugin",
//!   "wayland",
//!   "webgl2",
//!   "x11",
//! ] }
//! ```
//!
//! </details>
//!
//! Then, you'll need to add the [`SeedlingPlugins`] to your app.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_seedling::prelude::*;
//!
//! fn main() {
//!     App::default()
//!         .add_plugins((DefaultPlugins, SeedlingPlugins))
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
//!         sample_effects![FastLowpassNode::<2>::from_cutoff_hz(500.0)],
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
//! - [Context configuration][crate::context::AudioContextConfig]
//! - [Graph template][crate::context::graph::AudioGraphTemplate]
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
//! | Flag              | Description                                | Default |
//! | ----------------- | ------------------------------------------ | ------- |
//! | `reflect`         | Enable [`bevy_reflect`] derive macros.     | Yes     |
//! | `rand`            | Enable the [`RandomPitch`] component.      | Yes     |
//! | `symphonia`       | Enable symphonia and default asset loader. | Yes     |
//! | `diagnostics`     | Enable audio thread diagnostics.           | Yes     |
//! | `wav`             | Enable WAV format and PCM encoding.        | Yes     |
//! | `ogg`             | Enable Ogg format and Vorbis encoding.     | Yes     |
//! | `mp3`             | Enable mp3 format and encoding.            | No      |
//! | `mkv`             | Enable mkv format.                         | No      |
//! | `adpcm`           | Enable adpcm encoding.                     | No      |
//! | `flac`            | Enable FLAC format and encoding.           | No      |
//! | `web_audio`       | Enable the multi-threading web backend.    | No      |
//! | `rtaudio`         | Enable the native RtAudio backend.         | No      |
//! | `hrtf`            | Enable HRTF Spatialization.                | No      |
//! | `hrtf_subjects`   | Enable all HRTF embedded data.             | No      |
//! | `loudness`        | Enable LUFS analyzer node.                 | No      |
//! | `effects`         | Enable extra effects and analyzers.        | No      |
//! | `resample_inputs` | Enable audio input resampling.             | No      |
//! | `dev`             | Enable helpful features for development.   | No      |
//! | `entity_names`    | Add [`Name`]s to node and sample entities. | No      |
//! | `track_location`  | Track caller locations in diagnostics.     | No      |
//!
//! [`RandomPitch`]: crate::prelude::RandomPitch
//! [`Name`]: bevy_ecs::prelude::Name
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
//!         .add_plugins((DefaultPlugins, SeedlingPlugins))
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

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::type_complexity)]
#![expect(clippy::needless_doctest_main)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

#[cfg(any(feature = "std", test))]
extern crate std;

// Naming trick to facilitate straightforward internal macro usage.
extern crate alloc;
extern crate self as bevy_seedling;

use bevy_app::{plugin_group, prelude::*};
use bevy_asset::prelude::AssetApp;
use bevy_ecs::prelude::*;

// We re-export Firewheel here for convenience.
pub use firewheel;

pub mod context;
pub mod edge;
pub mod error;
pub mod node;
pub mod nodes;
pub mod platform;
pub mod time;
pub mod utils;

#[cfg(feature = "sampler")]
pub mod pool;
#[cfg(feature = "sampler")]
pub mod sample;

#[cfg(feature = "diagnostics")]
pub mod diagnostics;

#[cfg(feature = "spatial")]
pub mod spatial;

pub mod prelude {
    //! All `bevy_seedlings`'s important types and traits.

    pub use crate::context::AudioContext;
    pub use crate::context::graph::{AudioGraphTemplate, SeedlingStartupSystems, SoundEffectsBus};
    #[cfg(feature = "sampler")]
    pub use crate::context::graph::MusicPool;
    #[cfg(all(feature = "sampler", feature = "spatial"))]
    pub use crate::context::graph::SpatialPool;
    pub use crate::edge::{
        AudioGraphInput, AudioGraphOutput, ChannelMapping, Connect, Disconnect, EdgeTarget,
    };
    pub use crate::node::{
        AudioBypass, FirewheelNode, RegisterNode,
        events::{AudioEvents, VolumeFade},
        label::{MainBus, NodeLabel},
    };

    #[cfg(feature = "effects")]
    pub use crate::nodes::effects::*;
    #[cfg(feature = "itd")]
    pub use crate::nodes::itd::{ItdConfig, ItdNode};
    #[cfg(feature = "limiter")]
    pub use crate::nodes::limiter::{LimiterConfig, LimiterNode};
    #[cfg(feature = "loudness")]
    pub use crate::nodes::loudness::{LoudnessConfig, LoudnessNode, LoudnessState};
    pub use crate::nodes::{
        core::*,
        send::{SendConfig, SendNode},
    };

    pub use crate::platform::AudioStreamConfig;

    #[cfg(feature = "sampler")]
    pub use crate::pool::{
        DefaultPoolSize, PlaybackCompletion, PoolCommands, PoolDespawn, PoolSize, SamplerPool,
        dynamic::DynamicBus,
        label::{DefaultPool, PoolLabel},
        sample_effects::{EffectOf, EffectsQuery, SampleEffects},
    };

    #[cfg(feature = "sampler")]
    pub use crate::sample::{
        AudioSample, OnComplete, PlaybackSettings, SamplePlayer, SamplePriority,
    };

    #[cfg(feature = "sampler")]
    pub use crate::sample_effects;

    #[cfg(feature = "spatial")]
    pub use crate::spatial::{
        DefaultSpatialScale, SpatialListener2D, SpatialListener3D, SpatialScale,
    };

    pub use crate::time::{Audio, AudioTime};
    pub use crate::utils::perceptual_volume::PerceptualVolume;
    pub use crate::{SeedlingPlugins, SeedlingSystems};

    #[cfg(feature = "cpal")]
    pub use crate::platform::cpal::CpalStream;

    pub use firewheel::{
        FirewheelConfig, Volume,
        channel_config::{ChannelCount, NonZeroChannelCount},
        clock::{
            DurationMusical, DurationSamples, DurationSeconds, InstantMusical, InstantSamples,
            InstantSeconds,
        },
        diff::{Memo, Notify},
    };

    #[cfg(all(feature = "rtaudio", not(target_arch = "wasm32")))]
    pub use crate::platform::rtaudio::{RtAudioConfig, RtAudioPlatformPlugin};

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
    /// The audio stream is polled and unexpected device changes are handled.
    PollStream,
}

/// `bevy_seedling`'s core plugin.
///
/// This spawns the audio task in addition
/// to inserting `bevy_seedling`'s systems
/// and resources.
#[derive(Debug, Default)]
pub struct SeedlingCorePlugin;

plugin_group! {
    /// `bevy_seedling`'s top-level plugin.
    ///
    /// This spawns the audio task in addition
    /// to inserting `bevy_seedling`'s systems
    /// and resources.
    #[derive(Debug)]
    pub struct SeedlingPlugins {
        :SeedlingCorePlugin,
        #[cfg(feature = "cpal")]
        platform::cpal:::CpalPlatformPlugin,
        #[cfg(feature = "rtaudio")]
        platform::rtaudio:::RtAudioPlatformPlugin,
        #[cfg(feature = "diagnostics")]
        diagnostics:::AudioDiagnosticsPlugin,
    }
}

/// Run a system if the given resource has changed, ignoring
/// change ticks on startup.
pub fn resource_changed_without_insert<R: Resource>(res: Res<R>, mut has_run: Local<bool>) -> bool {
    let changed = res.is_changed() && *has_run;
    *has_run = true;

    changed
}

impl Plugin for SeedlingCorePlugin {
    fn build(&self, app: &mut App) {
        use prelude::*;

        #[cfg(feature = "sampler")]
        app.init_resource::<pool::DefaultPoolSize>()
            .init_asset::<sample::AudioSample>();

        app.configure_sets(
            Last,
            (
                SeedlingSystems::Connect.after(SeedlingSystems::Acquire),
                SeedlingSystems::Pool.after(SeedlingSystems::Connect),
                SeedlingSystems::Queue.after(SeedlingSystems::Pool),
                SeedlingSystems::Flush.after(SeedlingSystems::Queue),
                SeedlingSystems::PollStream.after(SeedlingSystems::Flush),
            ),
        );

        #[cfg(feature = "sampler")]
        app.add_observer(sample::observe_player_insert);

        app.add_plugins((
            context::ContextPlugin,
            node::NodePlugin,
            edge::EdgePlugin,
            nodes::SeedlingNodesPlugin,
            time::TimePlugin,
            #[cfg(feature = "rand")]
            sample::RandomPlugin,
            #[cfg(feature = "symphonia")]
            sample::SymphoniumLoaderPlugin,
        ));

        #[cfg(feature = "sampler")]
        app.add_plugins(pool::SamplePoolPlugin);
        #[cfg(feature = "spatial")]
        app.add_plugins(spatial::SpatialPlugin);

        #[cfg(all(feature = "reflect", feature = "sampler"))]
        app.register_type::<SamplerPool<MusicPool>>()
            .register_type::<SamplerPool<DefaultPool>>();
        #[cfg(all(feature = "reflect", feature = "sampler", feature = "spatial"))]
        app.register_type::<SamplerPool<SpatialPool>>();
    }
}

#[cfg(test)]
mod test {
    use crate::{
        platform::{cpal::CpalPlatformPlugin, mock::MockBackendPlugin},
        prelude::*,
    };
    use bevy::{ecs::system::RunSystemOnce, prelude::*};
    use firewheel::nodes::fast_filters::lowpass::FastLowpassNode;

    pub fn prepare_app<F: IntoSystem<(), (), M>, M>(startup: F) -> App {
        let mut app = App::new();

        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            SeedlingPlugins.build().disable::<CpalPlatformPlugin>(),
            MockBackendPlugin,
            TransformPlugin,
        ))
        .insert_resource(AudioGraphTemplate::Empty)
        .register_node::<FastLowpassNode>()
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
