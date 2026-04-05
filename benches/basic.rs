use std::{hint::black_box, num::NonZeroU32, sync::Arc};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use firewheel::{collector::ArcGc, sample_resource::SampleResource};

pub fn criterion_benchmark(c: &mut Criterion) {
    let test_path = "assets/sine_440hz_1ms.wav";
    let caw_ogg: Arc<[u8]> = std::fs::read(test_path).unwrap().into();

    let mut hint = symphonia::core::probe::Hint::new();
    hint.with_extension(test_path);

    let rate_a = NonZeroU32::new(44100).unwrap();
    let rate_b = NonZeroU32::new(48000).unwrap();

    let mut group = c.benchmark_group("cache metrics");
    for rate in [rate_a, rate_b].into_iter() {
        group.bench_with_input(BenchmarkId::new("without cache", rate), &rate, |b, rate| {
            b.iter(|| {
                let mut loader = symphonium::SymphoniumLoader::new();
                let source = firewheel::load_audio_file_from_source(
                    &mut loader,
                    Box::new(std::io::Cursor::new(caw_ogg.clone())),
                    Some(hint.clone()),
                    Some(*rate),
                    Default::default(),
                );
                black_box(source);
            });
        });

        group.bench_with_input(BenchmarkId::new("with cache", rate), &rate, |b, rate| {
            let mut loader = symphonium::SymphoniumLoader::new();
            b.iter(|| {
                let source = firewheel::load_audio_file_from_source(
                    &mut loader,
                    Box::new(std::io::Cursor::new(caw_ogg.clone())),
                    Some(hint.clone()),
                    Some(*rate),
                    Default::default(),
                );
                black_box(source);
            });
        });
    }
    group.finish();

    let test_path = "assets/caw.ogg";
    let caw_ogg: Arc<[u8]> = std::fs::read(test_path).unwrap().into();

    let mut hint = symphonia::core::probe::Hint::new();
    hint.with_extension(test_path);

    let rate_a = NonZeroU32::new(44100).unwrap();
    let rate_b = NonZeroU32::new(48000).unwrap();

    let mut group = c.benchmark_group("vorbis cache metrics");
    for rate in [rate_a, rate_b].into_iter() {
        group.bench_with_input(BenchmarkId::new("without cache", rate), &rate, |b, rate| {
            b.iter(|| {
                let mut loader = symphonium::SymphoniumLoader::new();
                let source = firewheel::load_audio_file_from_source(
                    &mut loader,
                    Box::new(std::io::Cursor::new(caw_ogg.clone())),
                    Some(hint.clone()),
                    Some(*rate),
                    Default::default(),
                );
                black_box(source);
            });
        });

        group.bench_with_input(BenchmarkId::new("with cache", rate), &rate, |b, rate| {
            let mut loader = symphonium::SymphoniumLoader::new();
            b.iter(|| {
                let source = firewheel::load_audio_file_from_source(
                    &mut loader,
                    Box::new(std::io::Cursor::new(caw_ogg.clone())),
                    Some(hint.clone()),
                    Some(*rate),
                    Default::default(),
                );
                black_box(source);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
