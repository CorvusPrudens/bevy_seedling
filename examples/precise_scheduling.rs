//! This example demonstrates precise audio event scheduling.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{
    node::events::{AudioEvents, VolumeFade},
    prelude::*,
};
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
        .add_systems(
            PostStartup,
            (
                |server: Res<AssetServer>, mut commands: Commands| {
                    commands
                        .spawn(SamplePlayer::new(server.load("selfless_courage.ogg")).looping());
                },
                precise_scheduling.after(SeedlingStartupSystems::StreamInitialization),
            ),
        )
        .add_systems(
            Update,
            precise_scheduling.run_if(on_timer(Duration::from_secs_f32(7.0))),
        )
        .run();
}

fn precise_scheduling(
    node: Single<(&VolumeNode, &mut AudioEvents), With<MainBus>>,
    time: Res<Time<Audio>>,
) {
    let (node, mut events) = node.into_inner();

    node.fade_to(Volume::SILENT, DurationSeconds(2.5), &mut events);

    let now = time.context().instant();
    node.fade_at(
        Volume::UNITY_GAIN,
        now + DurationSeconds(2.5),
        now + DurationSeconds(5.0),
        &mut events,
    );
}
