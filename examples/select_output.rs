//! This example demonstrates how to play a one-shot sample.

use bevy::prelude::*;
use bevy_seedling::{context::AudioStreamConfig, prelude::*, startup::OutputDeviceInfo};

#[derive(Component)]
struct SelectedOutput;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, SeedlingPlugin::default()))
        .add_systems(Startup, (startup, set_up_ui))
        .add_systems(Update, (select_output, play_sound))
        .add_observer(observe_selection)
        .run();
}

fn startup(
    outputs: Query<(Entity, &OutputDeviceInfo)>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (entity, device) in &outputs {
        info!("device: {}, default: {}", device.name, device.is_default);

        if device.is_default {
            commands.entity(entity).insert(SelectedOutput);
        }
    }

    commands.spawn(
        SamplePlayer::new(server.load("selfless_courage.ogg"))
            .looping()
            .with_volume(Volume::Decibels(-6.0)),
    );
}

fn play_sound(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands, server: Res<AssetServer>) {
    if keys.just_pressed(KeyCode::Space) {
        commands.spawn(SamplePlayer::new(server.load("caw.ogg")));
    }
}

fn select_output(
    keys: Res<ButtonInput<KeyCode>>,
    outputs: Query<(Entity, &OutputDeviceInfo, Has<SelectedOutput>)>,
    mut commands: Commands,
) {
    let mut devices = outputs.iter().collect::<Vec<_>>();
    devices.sort_unstable_by_key(|(_, device, _)| &device.name);

    let Some(mut selected_index) = devices.iter().position(|(.., has_selected)| *has_selected)
    else {
        return;
    };

    if keys.just_pressed(KeyCode::ArrowRight) {
        commands
            .entity(devices[selected_index].0)
            .remove::<SelectedOutput>();
        selected_index = (selected_index + 1) % devices.len();
        commands
            .entity(devices[selected_index].0)
            .insert(SelectedOutput);
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        commands
            .entity(devices[selected_index].0)
            .remove::<SelectedOutput>();
        if selected_index == 0 {
            selected_index = devices.len() - 1;
        } else {
            selected_index -= 1;
        }
        commands
            .entity(devices[selected_index].0)
            .insert(SelectedOutput);
    }
}

fn observe_selection(
    trigger: Trigger<OnAdd, SelectedOutput>,
    outputs: Query<&OutputDeviceInfo>,
    mut stream: ResMut<AudioStreamConfig>,
    mut rate_toggle: Local<bool>,
) -> Result {
    let output = outputs.get(trigger.target())?;
    stream.0.output.device_name = Some(output.name.clone());

    let rate = if *rate_toggle { 48000 } else { 44100 };
    *rate_toggle = !*rate_toggle;
    stream.0.output.desired_sample_rate = Some(rate);

    info!("new output: {:#?}", stream.0.output);

    Ok(())
}

// UI code //

fn set_up_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn((
        Node {
            width: Val::Percent(80.0),
            height: Val::Percent(80.0),
            margin: UiRect::AUTO,
            ..Default::default()
        },
        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
    ));
}
