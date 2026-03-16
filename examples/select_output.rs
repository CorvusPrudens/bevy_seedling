//! This example demonstrates how to play a one-shot sample.

use bevy::prelude::*;
use bevy_seedling::{
    context::{StreamRestartEvent, StreamStartEvent},
    platform::{AudioStreamConfig, cpal::DeviceInfo},
    prelude::*,
};
use firewheel::cpal::{CpalConfig, default_host_enumerator};

#[derive(Component)]
struct SelectedOutput;

#[derive(Component)]
struct OutputDevice(DeviceInfo);

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, SeedlingPlugins))
        .add_systems(Startup, set_up_ui)
        .add_systems(
            PostStartup,
            device_setup.after(SeedlingStartupSystems::StreamInitialization),
        )
        .add_systems(Update, (select_output, play_sound))
        .add_observer(observe_selection)
        .add_observer(observe_init)
        .add_observer(observe_restart)
        .run();
}

fn device_setup(mut commands: Commands) {
    let outputs = default_host_enumerator().output_devices();

    for device in outputs {
        info!("device: {:?}, default: {}", device.name, device.is_default);

        let is_default = device.is_default;
        commands
            .spawn(OutputDevice(device))
            .insert_if(SelectedOutput, || is_default);
    }
}

fn play_sound(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands, server: Res<AssetServer>) {
    if keys.just_pressed(KeyCode::Space) {
        commands.spawn(SamplePlayer::new(server.load("caw.ogg")));
    }
}

fn select_output(
    keys: Res<ButtonInput<KeyCode>>,
    outputs: Query<(Entity, &OutputDevice, Has<SelectedOutput>)>,
    mut commands: Commands,
) {
    let mut devices = outputs.iter().collect::<Vec<_>>();
    devices.sort_unstable_by_key(|(_, device, _)| &device.0.name);

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
    trigger: On<Add, SelectedOutput>,
    outputs: Query<&OutputDevice>,
    mut text: Query<&mut Text, With<SelectedTextNode>>,
    mut stream: ResMut<AudioStreamConfig<CpalConfig>>,
) -> Result {
    let output = outputs.get(trigger.event_target())?;

    stream.0.output.device_id = Some(output.0.id.clone());

    let name = match &output.0.name {
        Some(name) => name,
        None => "<unknown device>",
    };

    let new_string = if output.0.is_default {
        format!("{} (default)", name)
    } else {
        name.to_string()
    };
    text.single_mut()?.0 = new_string;

    Ok(())
}

fn observe_init(
    trigger: On<StreamStartEvent>,
    mut text: Query<&mut Text, With<SampleRateNode>>,
) -> Result {
    let new_text = format!("Sample rate: {}", trigger.sample_rate.get());
    text.single_mut()?.0 = new_text;

    Ok(())
}

fn observe_restart(
    trigger: On<StreamRestartEvent>,
    mut text: Query<&mut Text, With<SampleRateNode>>,
) -> Result {
    let new_text = format!("Sample rate: {}", trigger.current_rate.get());
    text.single_mut()?.0 = new_text;

    Ok(())
}

// UI code //
#[derive(Component)]
struct SelectedTextNode;

#[derive(Component)]
struct SampleRateNode;

fn set_up_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn((
        BackgroundColor(Color::srgb(0.23, 0.23, 0.23)),
        Node {
            width: Val::Percent(80.0),
            height: Val::Percent(80.0),
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Vh(8.0),
            margin: UiRect::AUTO,
            padding: UiRect::axes(Val::Px(50.0), Val::Px(50.0)),
            border: UiRect::axes(Val::Px(2.0), Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(25.0)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.9, 0.9, 0.9)),
        children![
            (
                Text::new("Device Selection"),
                TextFont {
                    font_size: 32.0,
                    ..Default::default()
                },
            ),
            (
                Text::new(
                    "Use the arrow keys to swap output devices.\nUse the spacebar to play sounds."
                ),
                TextLayout {
                    justify: Justify::Center,
                    ..Default::default()
                }
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Vh(2.0),
                    ..default()
                },
                children![
                    Text::new("Selected device:"),
                    (Text::new("N/A"), SelectedTextNode),
                    (Text::new("Sample rate: N/A"), SampleRateNode),
                ]
            )
        ],
    ));
}
