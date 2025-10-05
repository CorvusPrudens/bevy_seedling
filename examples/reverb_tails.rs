//! This example demonstrates how to correctly pause reverbs.

use bevy::prelude::*;
use bevy_log::{Level, LogPlugin};
use bevy_seedling::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(LogPlugin {
                level: Level::DEBUG,
                ..Default::default()
            }),
            SeedlingPlugin::default(),
        ))
        .add_systems(Startup, (set_up_ui, startup).chain())
        .add_systems(
            Update,
            (
                (toggle_playback, adjust_room_size),
                (set_playback, set_room_size),
            )
                .chain(),
        )
        .run();
}

fn startup(
    pool: Single<Entity, With<SamplerPool<DefaultPool>>>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    // set up the reverb
    let reverb = commands
        .spawn(FreeverbNode {
            room_size: 0.8,
            damping: 0.5,
            width: 0.8,
            ..Default::default()
        })
        .id();

    // Connect the default pool to the reverb
    commands
        .entity(*pool)
        .chain_node(VolumeNode {
            volume: Volume::Decibels(-6.0),
            ..Default::default()
        })
        .connect(reverb);

    // play some sound!
    commands.spawn(
        SamplePlayer::new(server.load("divine_comedy.ogg"))
            .looping()
            .with_volume(Volume::Decibels(-6.0)),
    );
}

fn toggle_playback(
    keys: Res<ButtonInput<KeyCode>>,
    mut player: Single<&mut PlaybackSettings>,
    mut reverb: Single<&mut FreeverbNode>,
) {
    if keys.just_pressed(KeyCode::Space) {
        if *player.playback == PlaybackState::Pause {
            player.play();
            reverb.pause = false;
        } else {
            player.pause();
            // This is the key -- when you pause sample playback, make sure
            // you also pause any active reverbs!
            reverb.pause = true;
        }
    }
}

fn adjust_room_size(keys: Res<ButtonInput<KeyCode>>, mut reverb: Single<&mut FreeverbNode>) {
    if keys.just_pressed(KeyCode::ArrowRight) {
        reverb.room_size = (reverb.room_size + 0.1).min(0.9);
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        reverb.room_size = (reverb.room_size - 0.1).max(0.0);
    }
}

// UI code //
#[derive(Component)]
struct PlaybackItem;

#[derive(Component)]
struct RoomSizeItem;

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
        BorderColor::all(Color::srgb(0.9, 0.9, 0.9)),
        BorderRadius::all(Val::Px(25.0)),
        children![
            (
                Text::new("Reverb Tails"),
                TextFont {
                    font_size: 32.0,
                    ..Default::default()
                },
            ),
            (
                Text::new(
                    "Use the arrow keys to adjust the room size.\nUse the spacebar to toggle playback.\
                    \nNotice how the reverb doesn't ring out when the playback is paused."
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
                    (Text::new("Playback:"), PlaybackItem),
                    (Text::new("Room Size:"), RoomSizeItem),
                ]
            )
        ],
    ));
}

fn set_playback(
    player: Single<&PlaybackSettings>,
    mut item: Single<&mut Text, With<PlaybackItem>>,
) {
    let state = if player.playback.is_playing() {
        "Playing"
    } else {
        "Paused"
    };
    item.0 = format!("Playback: {state}");
}

fn set_room_size(reverb: Single<&FreeverbNode>, mut item: Single<&mut Text, With<RoomSizeItem>>) {
    item.0 = format!("Room Size: {:.2}", reverb.room_size);
}
