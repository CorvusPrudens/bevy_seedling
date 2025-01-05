use bevy_asset::{Asset, AssetLoader};
use bevy_reflect::TypePath;
use firewheel::sample_resource::SampleResource;
use std::num::NonZeroU32;
use std::sync::Arc;

/// An audio sample.
#[derive(Asset, TypePath, Clone)]
pub struct Sample(Arc<dyn SampleResource>);

impl Sample {
    /// Share the inner value.
    pub fn get(&self) -> Arc<dyn SampleResource> {
        self.0.clone()
    }
}

impl core::fmt::Debug for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Sample").finish_non_exhaustive()
    }
}

/// A simple loader for audio samples.
#[derive(Debug)]
pub struct SampleLoader {
    /// The sampling rate of the audio engine.
    ///
    /// This must be kept in sync with the engine if
    /// the sample rate changes.
    pub sample_rate: NonZeroU32,
}

/// Errors produced while loading samples.
#[derive(Debug)]
pub enum SampleLoaderError {
    StdIo(std::io::Error),
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
    type Asset = Sample;
    type Settings = ();
    type Error = SampleLoaderError;

    async fn load(
        &self,
        reader: &mut dyn bevy_asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy_asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        // Unfortunately, we need to bridge the gap between sync and async APIs here.
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let mut hint = symphonia::core::probe::Hint::new();
        hint.with_extension(&load_context.path().to_string_lossy());

        let mut loader = symphonium::SymphoniumLoader::new();
        let source = firewheel::load_audio_file_from_source(
            &mut loader,
            Box::new(std::io::Cursor::new(bytes)),
            Some(hint),
            self.sample_rate,
            Default::default(),
        )?;

        Ok(Sample(Arc::new(source)))
    }

    fn extensions(&self) -> &[&str] {
        &["wav"]
    }
}
