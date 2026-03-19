// use bevy_seedling::{profiling::ProfilingBackend, VolumeNode};
use criterion::{Criterion, criterion_group, criterion_main};
// use firewheel::{channel_config::ChannelCount, FirewheelCtx};

pub fn criterion_benchmark(_c: &mut Criterion) {
    // TODO: why is this all commented out
    // // This benchmarks straightforward processing
    // // with two nodes.
    // c.bench_function("basic processing", {
    //     // let input = [0f32; 256 * 2];
    //     // let mut output = [0f32; 256 * 2];

    //     let mut context = FirewheelCtx::<ProfilingBackend>::new(firewheel::FirewheelConfig {
    //         num_graph_inputs: ChannelCount::ZERO,
    //         num_graph_outputs: ChannelCount::ZERO,
    //         ..Default::default()
    //     });

    //     let out_node = context.graph_out_node_id();
    //     let volume = context.add_node(
    //         VolumeNode {
    //             normalized_volume: 0.5,
    //         },
    //         None,
    //     );

    //     context
    //         .connect(volume, out_node, &[(0, 0), (1, 1)], false)
    //         .unwrap();

    //     // let saw = context.add_node(SawNode::new(440.).into(), None);

    //     // context
    //     //     .connect(saw, volume, &[(0, 0), (0, 1)], false)
    //     //     .unwrap();

    //     context.update().unwrap();

    //     move |b| {
    //         b.iter(|| {
    //             todo!("direct processing");
    //             // context.process_interleaved(black_box(&input), black_box(&mut output));
    //         })
    //     }
    // });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
