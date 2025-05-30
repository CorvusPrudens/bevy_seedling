//! This example demonstrates how to set up a
//! custom sample pool, a custom bus, and the routing in-between.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::prelude::*;

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
                    // let's just insert some reverb and a low-pass filter.
                    .chain_node(LowPassNode::default())
                    // This node is implicitly connected to the `MainBus`.
                    .chain_node(FreeverbNode::default());

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
                // SamplePlayer -> VolumeNode (EffectsPool) -> VolumeNode (EffectsBus) -> LowPassNode -> VolumeNode (MainBus) -> Audio Output
                //
                // The four sampler nodes in the effects pool are routed to a volume node.
                // We then route that node to our effects bus volume node, passing
                // through the effects to the main bus, which finally reaches the output.
            },
        )
        .add_systems(
            Update,
            // Here we apply some modulation to the frequency of the low-pass filter.
            |mut node: Single<&mut LowPassNode>, mut angle: Local<f32>, time: Res<Time>| {
                let period = 10.0;
                *angle += time.delta_secs() * core::f32::consts::TAU / period;

                let sin = angle.sin() * 0.5 + 0.5;
                node.frequency = 200.0 + sin * sin * 3500.0;
            },
        )
        .run();
}
