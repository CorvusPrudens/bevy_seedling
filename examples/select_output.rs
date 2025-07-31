//! This example demonstrates how to play a one-shot sample.

use bevy::prelude::*;
use bevy_seedling::{
    context::{AudioStreamConfig, StreamRestartEvent, StreamStartEvent},
    prelude::*,
};

#[derive(Component)]
struct SelectedOutput;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, SeedlingPlugin::default()))
        .add_systems(Startup, (set_up_ui, startup).chain())
        .add_systems(Update, (select_output, play_sound))
        .add_observer(observe_selection)
        .add_observer(observe_init)
        .add_observer(observe_restart)
        .run();
}

fn startup(outputs: Query<(Entity, &OutputDeviceInfo)>, mut commands: Commands) {
    for (entity, device) in &outputs {
        info!("device: {}, default: {}", device.name, device.is_default);

        if device.is_default {
            commands.entity(entity).insert(SelectedOutput);
        }
    }
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
    mut text: Query<&mut Text, With<SelectedTextNode>>,
    mut stream: ResMut<AudioStreamConfig>,
) -> Result {
    let output = outputs.get(trigger.target())?;

    stream.0.output.device_name = Some(output.name.clone());

    let new_string = if output.is_default {
        format!("{} (default)", output.name)
    } else {
        output.name.clone()
    };
    text.single_mut()?.0 = new_string;

    Ok(())
}

fn observe_init(
    trigger: Trigger<StreamStartEvent>,
    mut text: Query<&mut Text, With<SampleRateNode>>,
) -> Result {
    let new_text = format!("Sample rate: {}", trigger.sample_rate.get());
    text.single_mut()?.0 = new_text;

    Ok(())
}

fn observe_restart(
    trigger: Trigger<StreamRestartEvent>,
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
            ..default()
        },
        BorderColor(Color::srgb(0.9, 0.9, 0.9)),
        BorderRadius::all(Val::Px(25.0)),
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
                    justify: JustifyText::Center,
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
