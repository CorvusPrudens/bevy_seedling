use bevy_asset::Asset;
use bevy_reflect::TypePath;
use firewheel::{collector::ArcGc, sample_resource::SampleResource};
use std::{num::NonZeroU32, sync::Arc};

/// A type-erased audio sample.
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

#[cfg(feature = "symphonium")]
impl From<firewheel::DecodedAudioF32> for AudioSample {
    fn from(source: firewheel::DecodedAudioF32) -> Self {
        Self {
            original_sample_rate: source.original_sample_rate(),
            sample: ArcGc::new_unsized(|| Arc::new(source) as _),
        }
    }
}

#[cfg(feature = "symphonium")]
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

#[cfg(feature = "symphonium")]
pub use loader::{SampleLoader, SampleLoaderError};

#[cfg(feature = "symphonium")]
mod loader {
    use super::AudioSample;
    use bevy_asset::AssetLoader;
    use bevy_reflect::TypePath;

    /// A simple loader for audio samples.
    ///
    /// Samples are loaded via [`symphonia`] and resampled eagerly.
    /// As a result, you may notice some latency when loading longer
    /// samples with low optimization levels.
    ///
    /// The available containers and formats can be configured with
    /// this crate's feature flags.
    #[derive(Debug, TypePath)]
    pub struct SampleLoader {
        /// The sampling rate of the audio engine.
        sample_rate: crate::context::SampleRate,
    }

    impl SampleLoader {
        /// Create a new sample loader.
        ///
        /// `sample_rate` should be cloned directly from the resource
        /// that lives in the same world.
        pub fn new(sample_rate: crate::context::SampleRate) -> Self {
            Self { sample_rate }
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

    impl SampleLoader {
        pub(crate) const fn extensions() -> &'static [&'static str] {
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

            let mut loader = symphonium::SymphoniumLoader::new();
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
            Self::extensions()
        }
    }
}
