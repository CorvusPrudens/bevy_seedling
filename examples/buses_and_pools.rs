//! This example demonstrates how to set up a
//! custom sample pool, a custom bus, and the routing in-between.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{
    firewheel::{basic_nodes::VolumeNode, clock::ClockSeconds},
    lpf::LowPassNode,
    sample::{pool::SpawnPool, SamplePlayer},
    AudioContext, ConnectNode, NodeLabel, PlaybackSettings, PoolLabel, SeedlingPlugin,
};

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
                // Any arbitrary effects chain can go here, but
                // let's just insert a low-pass filter.
                //
                // This node is implicitly connected to the `MainBus`.
                let effects = commands.spawn(LowPassNode::new(10000.)).id();

                // Here we create a volume node that acts as the entry to
                // our effects bus and we connect it to the effects.
                //
                // When we spawn it with the `EffectsBus` label, any node
                // can use this type to connect to this node anywhere in
                // the code.
                commands
                    .spawn((VolumeNode::new(1.), EffectsBus))
                    .connect(effects);

                // Let's create a new sample player pool and route it to our effects bus.
                commands.spawn_pool(EffectsPool, 4).connect(EffectsBus);

                // Finally, let's play a sample through the chain.
                commands
                    .spawn((
                        SamplePlayer::new(server.load("snd_wobbler.wav")),
                        PlaybackSettings::LOOP,
                        EffectsPool,
                    ))
                    .connect(EffectsBus);

                // Once these connections are synchronized with the audio graph,
                // it will look like:
                //
                // SamplePlayer -> VolumeNode (EffectsPool) -> VolumeNode (EffectsBus) -> LowPassNode -> VolumeNode (MainBus) -> Audio Output
                //
                // The four sampler nodes in the effects pool are routed to a volume node.
                // We then route that node to our effects bus volume node, passing
                // through the effects to the main bus, which finally reaches the output.
            },
        )
        .add_systems(
            PostStartup,
            // Here we apply some modulation to the frequency of the low-pass filter.
            |q: Single<&mut LowPassNode>, mut context: ResMut<AudioContext>| {
                let now = context.now();
                let mut node = q.into_inner();

                node.frequency
                    .push_curve(
                        80.,
                        now,
                        now + ClockSeconds(4.0),
                        EaseFunction::ExponentialOut,
                    )
                    .unwrap();

                node.frequency
                    .push_curve(
                        10000.,
                        now + ClockSeconds(4.0),
                        now + ClockSeconds(8.0),
                        EaseFunction::ExponentialOut,
                    )
                    .unwrap();
            },
        )
        .run();
}
