use bevy_seedling::{profiling::ProfilingContext, saw::SawNode, VolumeNode};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use firewheel::param::Timeline;

pub fn criterion_benchmark(c: &mut Criterion) {
    // This benchmarks straightforward processing
    // with two nodes.
    c.bench_function("basic processing", {
        let input = [0f32; 256 * 2];
        let mut output = [0f32; 256 * 2];

        let mut context = ProfilingContext::new(48000);
        let graph = context.context.graph_mut().unwrap();

        let out_node = graph.graph_out_node();
        let volume = graph
            .add_node(VolumeNode(Timeline::new(0.5f32)).into(), None)
            .unwrap();

        graph
            .connect(volume, out_node, &[(0, 0), (1, 1)], false)
            .unwrap();

        let saw = graph.add_node(SawNode::new(440.).into(), None).unwrap();

        graph
            .connect(saw, volume, &[(0, 0), (0, 1)], false)
            .unwrap();

        context.context.flush_events();
        context.context.update();

        move |b| {
            b.iter(|| {
                context.process_interleaved(black_box(&input), black_box(&mut output));
            })
        }
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
