//! This example demonstrates how to use the `LoudnessNode`.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{node::AudioState, prelude::*};
use bevy_time::common_conditions::on_timer;
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(Startup, startup)
        .add_systems(
            Update,
            monitor.run_if(on_timer(Duration::from_secs_f32(0.5))),
        )
        .add_observer(on_complete)
        .run();
}

fn startup(main: Single<Entity, With<MainBus>>, server: Res<AssetServer>, mut commands: Commands) {
    // This sample has lots of silence, so it'll be a good demonstration.
    commands.spawn(SamplePlayer::new(server.load("divine_comedy.ogg")));

    // Here, we add an offshoot from the main bus to this loudness node.
    // ┌──────────┐
    // │MainBus   │
    // └┬────────┬┘
    // ┌▽──────┐┌▽───────────┐
    // │Limiter││LoudnessNode│
    // └┬──────┘└────────────┘
    // ┌▽───────────────┐
    // │AudioGraphOutput│
    // └────────────────┘
    commands.entity(*main).chain_node(LoudnessNode::default());
}

fn monitor(loudness: Single<&AudioState<LoudnessState>>) {
    let integrated = loudness.0.integrated();
    let momentary = loudness.0.momentary();
    let short_term = loudness.0.short_term();
    let peak = loudness.0.true_peak(0).max(loudness.0.true_peak(1));

    info!("---");
    info!("Integrated: {integrated:.2} LUFS");
    info!("Momentary: {momentary:.2} LUFS");
    info!("Short Term: {short_term:.2} LUFS");
    info!("True peak: {peak:.2} dB");
}

/// We'll replay the sound and reset the analyzer on completion.
fn on_complete(
    _: Trigger<PlaybackCompletionEvent>,
    mut loudness: Single<&mut LoudnessNode>,
    mut commands: Commands,
    server: Res<AssetServer>,
) {
    loudness.reset.notify();
    commands.spawn(SamplePlayer::new(server.load("divine_comedy.ogg")));
}
