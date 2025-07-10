//! This example demonstrates how to set up a
//! custom sample pool, a custom bus, and the routing in-between.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{nodes::limiter::LimiterNode, prelude::*};

#[derive(NodeLabel, PartialEq, Eq, Debug, Hash, Clone)]
struct EffectsBus;

#[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
struct EffectsPool;

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
                // Here we create a volume node that acts as the entry to
                // our effects bus.
                //
                // When we spawn it with the `EffectsBus` label, any node
                // can use this type to connect to this node anywhere in
                // the code.
                commands
                    .spawn((VolumeNode::default(), EffectsBus))
                    // Any arbitrary effects chain can go here, but
                    // let's just insert a reverb, a low-pass filter, and finally a limiter.
                    .chain_node(LowPassNode::default())
                    .chain_node(FreeverbNode::default())
                    // This node is implicitly connected to the `MainBus`.
                    .chain_node(LimiterNode::default());

                // Let's create a new sample player pool and route it to our effects bus.
                commands.spawn(SamplerPool(EffectsPool)).connect(EffectsBus);

                // Finally, let's play a sample through the chain.
                commands.spawn((
                    SamplePlayer::new(server.load("caw.ogg")).looping(),
                    EffectsPool,
                ));

                // Once these connections are synchronized with the audio graph,
                // it will look like:
                //
                // SamplePlayer
                // -> VolumeNode (EffectsPool)
                // -> VolumeNode (EffectsBus)
                // -> LowPassNode
                // -> FreeverbNode
                // -> LimiterNode
                // -> VolumeNode (MainBus)
                // -> Audio Output
                //
                // The four sampler nodes in the effects pool are routed to a volume node.
                // We then route that node to our effects bus volume node, passing
                // through the effects to the main bus, which finally reaches the output.
            },
        )
        .add_systems(
            Update,
            (
                // Here we apply some modulation to the frequency of the low-pass filter.
                |mut node: Single<&mut LowPassNode>, mut angle: Local<f32>, time: Res<Time>| {
                    let period = 10.0;
                    *angle += time.delta_secs() * core::f32::consts::TAU / period;

                    let sin = angle.sin() * 0.5 + 0.5;
                    node.frequency = 200.0 + sin * sin * 3500.0;
                },
                // Here we control some modulation to the amplitude of the signal.
                |mut node: Single<&mut VolumeNode, With<EffectsBus>>,
                 mut angle: Local<f32>,
                 time: Res<Time>| {
                    let period = 7.0;
                    *angle += time.delta_secs() * core::f32::consts::TAU / period;

                    let sin = angle.sin() * 0.5 + 0.5;
                    node.volume = Volume::UNITY_GAIN + Volume::Linear(sin.powi(2)) * Volume::Decibels(10.);
                },
            ),
        )
        .run();
}
