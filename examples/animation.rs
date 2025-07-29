//! This example demonstrates how to animate parameters.

use bevy::{log::LogPlugin, prelude::*};
use bevy_keyframe::{
    AnimationDuration, Keyframe, animations,
    drivers::{PlaybackMode, TimeDriver},
    lens,
};
use bevy_seedling::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
            bevy_keyframe::KeyframePlugin,
        ))
        .add_systems(Startup, startup)
        .run();
}

fn fade_to(level: f32, seconds: f32) -> impl Bundle {
    (
        lens!(VolumeNode::volume),
        Keyframe(Volume::Linear(level)),
        AnimationDuration::secs(seconds),
    )
}

fn delay(seconds: f32) -> AnimationDuration {
    AnimationDuration::secs(seconds)
}

fn startup(server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        SamplePlayer::new(server.load("selfless_courage.ogg")),
        sample_effects![(
            VolumeNode {
                volume: Volume::SILENT,
            },
            TimeDriver {
                mode: PlaybackMode::Repeat,
                ..Default::default()
            },
            animations![fade_to(1.0, 1.5), delay(0.5), fade_to(0.0, 1.5)],
        )],
    ));
}
