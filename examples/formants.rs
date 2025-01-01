//! This example demonstrates a more complicated graph.
//!
//! Note that you should generally implement things
//! like this within a single, custom node since `bevy_seedling`
//! does not expose a rich set of nodes like Max or Pure Data.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{
    bpf::BandPassNode,
    firewheel::{basic_nodes::VolumeNode, clock::ClockSeconds},
    node::NodeMap,
    sample::SamplePlayer,
    saw::SawNode,
    AudioContext, ConnectNode, MainBus, NodeLabel, SeedlingPlugin,
};
use firewheel::{basic_nodes::MixNode, ChannelConfig, ChannelCount};
use std::time::Duration;

#[derive(NodeLabel, PartialEq, Eq, Debug, Hash, Clone)]
struct EffectsBus;

fn db(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[derive(Component)]
struct FormantGroup(Vec<(Entity, Entity)>);

#[derive(Resource)]
struct VowelSwitch(Timer);

struct Formant {
    frequency: f32,
    amplitude: f32,
    bandwidth: f32,
}

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(
            Startup,
            |server: Res<AssetServer>, mut commands: Commands| {
                let mix = commands
                    .spawn((
                        MixNode,
                        ChannelConfig {
                            num_inputs: ChannelCount::new(TENOR[0].len() as u32).unwrap(),
                            num_outputs: ChannelCount::MONO,
                        },
                    ))
                    .connect_with(MainBus, &[(0, 0), (0, 1)])
                    .id();

                let filters: Vec<_> = TENOR[0]
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let vol = commands
                            .spawn(VolumeNode::new(db(f.amplitude)))
                            .connect_with(mix, &[(0, i as u32)])
                            .id();

                        let filter = commands
                            .spawn(BandPassNode::new(f.frequency, f.frequency / f.bandwidth))
                            .connect_with(vol, &[(0, 0)])
                            .id();

                        (vol, filter)
                    })
                    .collect();

                let mut bus = commands.spawn((VolumeNode::new(0.05), EffectsBus));

                for filter in &filters {
                    bus.connect(filter.1);
                }

                commands.spawn(FormantGroup(filters));

                commands.spawn(SawNode::new(50.)).connect(EffectsBus);
            },
        )
        .insert_resource(VowelSwitch(Timer::new(
            Duration::from_millis(250),
            TimerMode::Repeating,
        )))
        .add_systems(Update, switch_formant)
        .run();
}

/// Switch between formant groups according to the timer.
///
/// A small bit of smoothing is applied to the amplitudes
/// and filter frequencies.
fn switch_formant(
    q: Query<&FormantGroup>,
    mut formants: Query<&mut BandPassNode>,
    mut volumes: Query<&mut VolumeNode>,
    time: Res<Time>,
    mut switch: ResMut<VowelSwitch>,
    mut step: Local<usize>,
    mut ctx: ResMut<AudioContext>,
) {
    let now = ctx.now();
    switch.0.tick(time.delta());

    if switch.0.just_finished() {
        *step = (*step + 1) % TENOR[0].len();

        for group in q.iter() {
            for (i, f) in group.0.iter().enumerate() {
                if let (Ok(mut volume), Ok(mut formant)) =
                    (volumes.get_mut(f.0), formants.get_mut(f.1))
                {
                    let data = &TENOR[*step][i];

                    formant.frequency.push_curve(
                        data.frequency,
                        now,
                        now + ClockSeconds(0.05),
                        EaseFunction::Linear,
                    );

                    formant.q.push_curve(
                        data.frequency / data.bandwidth,
                        now,
                        now + ClockSeconds(0.05),
                        EaseFunction::Linear,
                    );

                    volume.0.push_curve(
                        db(data.amplitude),
                        now,
                        now + ClockSeconds(0.05),
                        EaseFunction::Linear,
                    );
                }
            }
        }
    }
}

const TENOR: [[Formant; 5]; 5] = [
    [
        Formant {
            frequency: 650.0,
            amplitude: 0.0,
            bandwidth: 80.0,
        },
        Formant {
            frequency: 1080.0,
            amplitude: -6.0,
            bandwidth: 90.0,
        },
        Formant {
            frequency: 2650.0,
            amplitude: -7.0,
            bandwidth: 120.0,
        },
        Formant {
            frequency: 2900.0,
            amplitude: -8.0,
            bandwidth: 130.0,
        },
        Formant {
            frequency: 3250.0,
            amplitude: -22.0,
            bandwidth: 140.0,
        },
    ],
    [
        Formant {
            frequency: 400.0,
            amplitude: 0.0,
            bandwidth: 70.0,
        },
        Formant {
            frequency: 1700.0,
            amplitude: -14.0,
            bandwidth: 80.0,
        },
        Formant {
            frequency: 2600.0,
            amplitude: -12.0,
            bandwidth: 100.0,
        },
        Formant {
            frequency: 3200.0,
            amplitude: -14.0,
            bandwidth: 120.0,
        },
        Formant {
            frequency: 3580.0,
            amplitude: -20.0,
            bandwidth: 120.0,
        },
    ],
    [
        Formant {
            frequency: 290.0,
            amplitude: 0.0,
            bandwidth: 40.0,
        },
        Formant {
            frequency: 1870.0,
            amplitude: -15.0,
            bandwidth: 90.0,
        },
        Formant {
            frequency: 2800.0,
            amplitude: -18.0,
            bandwidth: 100.0,
        },
        Formant {
            frequency: 3250.0,
            amplitude: -20.0,
            bandwidth: 120.0,
        },
        Formant {
            frequency: 3540.0,
            amplitude: -30.0,
            bandwidth: 120.0,
        },
    ],
    [
        Formant {
            frequency: 400.0,
            amplitude: 0.0,
            bandwidth: 40.0,
        },
        Formant {
            frequency: 800.0,
            amplitude: -10.0,
            bandwidth: 80.0,
        },
        Formant {
            frequency: 2600.0,
            amplitude: -12.0,
            bandwidth: 100.0,
        },
        Formant {
            frequency: 2800.0,
            amplitude: -12.0,
            bandwidth: 120.0,
        },
        Formant {
            frequency: 3000.0,
            amplitude: -26.0,
            bandwidth: 120.0,
        },
    ],
    [
        Formant {
            frequency: 350.0,
            amplitude: 0.0,
            bandwidth: 40.0,
        },
        Formant {
            frequency: 600.0,
            amplitude: -20.0,
            bandwidth: 60.0,
        },
        Formant {
            frequency: 2700.0,
            amplitude: -17.0,
            bandwidth: 100.0,
        },
        Formant {
            frequency: 2900.0,
            amplitude: -14.0,
            bandwidth: 120.0,
        },
        Formant {
            frequency: 3300.0,
            amplitude: -26.0,
            bandwidth: 120.0,
        },
    ],
];
