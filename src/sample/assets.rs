use bevy_asset::Asset;
use bevy_reflect::TypePath;
use firewheel::{collector::ArcGc, sample_resource::SampleResource};
use std::{num::NonZeroU32, sync::Arc};

/// A type-erased audio sample.
///
/// Decoding for PCM WAV, Ogg Vorbis, and a number of other
/// formats is supported through `symphonia` and the associated
/// `bevy_seedling` features.
///
/// You can also disable `symphonia` entirely and provide a custom
/// asset loader.
#[derive(Asset, TypePath, Clone)]
pub struct AudioSample {
    sample: ArcGc<dyn SampleResource>,
    original_sample_rate: NonZeroU32,
}

impl AudioSample {
    /// Create a new [`AudioSample`] from a [`SampleResource`] loaded into memory.
    ///
    /// If the sample resource has been resampled, `original_sample_rate` should represent
    /// the sample rate prior to resampling.
    pub fn new<S: SampleResource>(sample: S, original_sample_rate: NonZeroU32) -> Self {
        Self {
            sample: ArcGc::new_unsized(|| Arc::new(sample) as _),
            original_sample_rate,
        }
    }

    /// Share the inner value.
    pub fn get(&self) -> ArcGc<dyn SampleResource> {
        self.sample.clone()
    }

    /// Return the sample resource's original sample rate.
    ///
    /// If the resource has been resampled, this may return
    /// a different value than [`SampleResourceInfo::sample_rate`].
    ///
    /// [`SampleResourceInfo::sample_rate`]: firewheel::sample_resource::SampleResourceInfo::sample_rate
    pub fn original_sample_rate(&self) -> NonZeroU32 {
        self.original_sample_rate
    }
}

#[cfg(feature = "symphonia")]
impl From<firewheel::DecodedAudioF32> for AudioSample {
    fn from(source: firewheel::DecodedAudioF32) -> Self {
        Self {
            original_sample_rate: source.original_sample_rate(),
            sample: ArcGc::new_unsized(|| Arc::new(source) as _),
        }
    }
}

#[cfg(feature = "symphonia")]
impl From<firewheel::DecodedAudio> for AudioSample {
    fn from(source: firewheel::DecodedAudio) -> Self {
        Self {
            original_sample_rate: source.original_sample_rate(),
            sample: ArcGc::new_unsized(|| Arc::new(source) as _),
        }
    }
}

impl core::fmt::Debug for AudioSample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Sample").finish_non_exhaustive()
    }
}

#[cfg(feature = "symphonia")]
pub mod loader {
    use super::AudioSample;
    use bevy_app::prelude::*;
    use bevy_asset::{AssetLoader, AssetServer};
    use bevy_ecs::prelude::*;
    use bevy_reflect::TypePath;
    use symphonia::core::{codecs::CodecRegistry, probe::Probe};

    pub struct SymphoniumLoaderPlugin;

    impl Plugin for SymphoniumLoaderPlugin {
        fn build(&self, app: &mut App) {
            let world = app.world_mut();

            world.init_resource::<AudioLoaderConfig>();
            world.resource_scope::<AudioLoaderConfig, _>(|world, config| {
                world
                    .resource_mut::<AssetServer>()
                    .preregister_loader::<SampleLoader>(config.extensions());
            });

            app.add_observer(init_loader);
        }
    }

    /// A [`Resource`] containing the configuration for [`SampleLoader`].
    ///
    /// New formats and codecs (besides those enabled through this crate's feature flags) can be
    /// added to the [symphonia]'s codec registry by inserting this resource before adding the
    /// plugin.
    ///
    /// For example:
    /// ```no_run
    /// use bevy::prelude::*;
    /// use bevy_seedling::{prelude::*, sample::AudioLoaderConfig};
    /// use symphonia::{
    ///     core::{
    ///         codecs::{CodecRegistry, Decoder},
    ///         probe::{Probe, QueryDescriptor},
    ///     },
    ///     default::{codecs::PcmDecoder, formats::WavReader},
    /// };
    ///
    /// fn main() {
    ///     let mut config = AudioLoaderConfig::default();
    ///     config.register_codec(["wav"], |registry, probe| {
    ///         registry.register_all::<PcmDecoder>();
    ///         probe.register_all::<WavReader>();
    ///     });
    ///
    ///     App::new()
    ///         .insert_resource(config)
    ///         .add_plugins((DefaultPlugins, SeedlingPlugins));
    /// }
    /// ```
    ///
    /// Adding the plugin will pre-register [`SampleLoader`] with the extensions in this config.
    /// If the custom codecs are only available for insertion after adding the plugin,
    /// then [`AssetApp::preregister_asset_loader`] can be called to manually pre-register
    /// the new extensions.
    ///
    /// [`AssetApp::preregister_asset_loader`]: bevy_asset::AssetApp::preregister_asset_loader
    ///
    /// This resource will be removed when the loader is registered
    /// following the [`StreamStartEvent`][crate::context::StreamStartEvent].
    #[derive(Resource)]
    pub struct AudioLoaderConfig {
        /// The registry with codecs to be used for decoding.
        codec_registry: CodecRegistry,
        /// The format probe to be used for probing.
        probe: Probe,
        /// The extensions supported by the formats.
        extensions: Vec<&'static str>,
    }

    impl AudioLoaderConfig {
        /// Constructs a new, empty config.
        ///
        /// This will not include `bevy_seedling`'s feature-gated codecs.
        pub fn empty() -> Self {
            Self {
                codec_registry: CodecRegistry::new(),
                probe: Probe::default(),
                extensions: Vec::new(),
            }
        }

        /// Register a new codec along with its associated extensions.
        pub fn register_codec<I, F>(&mut self, extensions: I, f: F)
        where
            I: IntoIterator<Item = &'static str>,
            F: FnOnce(&mut CodecRegistry, &mut Probe),
        {
            f(&mut self.codec_registry, &mut self.probe);
            self.extensions.extend(extensions);
        }

        /// Returns this config's registered extensions.
        pub fn extensions(&self) -> &[&'static str] {
            &self.extensions
        }

        const fn default_extensions() -> &'static [&'static str] {
            &[
                #[cfg(feature = "wav")]
                "wav",
                #[cfg(feature = "ogg")]
                "ogg",
                #[cfg(feature = "mp3")]
                "mp3",
                #[cfg(feature = "flac")]
                "flac",
                #[cfg(feature = "mkv")]
                "mkv",
            ]
        }
    }

    impl Default for AudioLoaderConfig {
        fn default() -> Self {
            let mut config = Self::empty();

            symphonia::default::register_enabled_codecs(&mut config.codec_registry);
            symphonia::default::register_enabled_formats(&mut config.probe);
            config
                .extensions
                .extend_from_slice(Self::default_extensions());

            config
        }
    }

    impl std::fmt::Debug for AudioLoaderConfig {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AudioLoaderConfig")
                .field(
                    "codec_registry",
                    &std::fmt::from_fn(|f| write!(f, "CodegRegistry")),
                )
                .field("probe", &std::fmt::from_fn(|f| write!(f, "Probe")))
                .field("extensions", &self.extensions)
                .finish()
        }
    }

    /// A simple loader for audio samples.
    ///
    /// Samples are loaded via [`symphonia`] and resampled eagerly.
    /// As a result, you may notice some latency when loading longer
    /// samples with low optimization levels.
    ///
    /// The available containers and formats can be configured with
    /// this crate's feature flags (and additionally [AudioLoaderConfig]).
    #[derive(TypePath, Debug)]
    pub struct SampleLoader {
        sample_rate: crate::context::SampleRate,
        config: &'static AudioLoaderConfig,
    }

    impl SampleLoader {
        /// Create a new sample loader.
        ///
        /// `sample_rate` should be cloned directly from the resource
        /// that lives in the same world.
        pub fn new(sample_rate: crate::context::SampleRate, config: AudioLoaderConfig) -> Self {
            Self {
                sample_rate,
                // we leak the config here to satisfy symphoium's `&'static` requirements
                // NOTE: remove this when symphonium relaxes its lifetimes
                config: Box::leak(Box::new(config)),
            }
        }
    }

    /// Errors produced while loading samples.
    #[derive(Debug)]
    pub enum SampleLoaderError {
        /// An I/O error, such as missing files.
        StdIo(std::io::Error),
        /// An error directly from `symphonium`.
        Symphonium(String),
    }

    impl From<std::io::Error> for SampleLoaderError {
        fn from(value: std::io::Error) -> Self {
            Self::StdIo(value)
        }
    }

    impl From<symphonium::error::LoadError> for SampleLoaderError {
        fn from(value: symphonium::error::LoadError) -> Self {
            Self::Symphonium(value.to_string())
        }
    }

    impl std::error::Error for SampleLoaderError {}

    impl std::fmt::Display for SampleLoaderError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::StdIo(stdio) => stdio.fmt(f),
                Self::Symphonium(sy) => f.write_str(sy),
            }
        }
    }

    impl AssetLoader for SampleLoader {
        type Asset = AudioSample;
        type Settings = ();
        type Error = SampleLoaderError;

        async fn load(
            &self,
            reader: &mut dyn bevy_asset::io::Reader,
            _settings: &Self::Settings,
            load_context: &mut bevy_asset::LoadContext<'_>,
        ) -> Result<Self::Asset, Self::Error> {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;

            let mut hint = symphonia::core::probe::Hint::new();
            hint.with_extension(&load_context.path().to_string());

            let mut loader = symphonium::SymphoniumLoader::with_codec_registry_and_probe(
                &self.config.codec_registry,
                &self.config.probe,
            );
            let source = firewheel::load_audio_file_from_source(
                &mut loader,
                Box::new(std::io::Cursor::new(bytes)),
                Some(hint),
                Some(self.sample_rate.get()),
                Default::default(),
            )?;

            Ok(source.into())
        }

        fn extensions(&self) -> &[&str] {
            self.config.extensions()
        }
    }

    fn init_loader(_: On<crate::context::StreamStartEvent>, mut commands: Commands) {
        commands.queue(|world: &mut World| -> Result {
            let sample_rate = world
                .get_resource::<crate::context::SampleRate>()
                .ok_or("expected `SampleRate` resource")?
                .clone();
            let config = world
                .remove_resource::<AudioLoaderConfig>()
                .ok_or("expected `AudioLoaderConfig` resource")?;
            world
                .resource::<AssetServer>()
                .register_loader(SampleLoader::new(sample_rate.clone(), config));

            Ok(())
        });
    }
}
