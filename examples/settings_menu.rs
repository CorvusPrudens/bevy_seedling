//! This example demonstrates a simple master, music, and sfx setup.
//!
//! With the default [`GraphConfiguration`], we have everything we need
//! to create the typical audio settings menu.
//!
//! The `Music` pool, `Sfx` pool, and `DefaultPool` are all routed to the `MainBus` node.
//! Since each pool has a `VolumeNode`, we can control them all individually. And,
//! since they're all routed to the `MainBus`, we can also set the volume of all three
//! at once.
//!
//! You can see this in action in the knob observers: to set the master volume,
//! we adjust the `MainBus` node, and to set the individual volumes, we adjust the
//! pool nodes.

#![allow(clippy::type_complexity)]
use bevy::{
    app::App,
    ecs::{spawn::SpawnWith, system::IntoObserverSystem},
    prelude::*,
};
use bevy_seedling::{
    configuration::{MusicPool, SfxBus},
    prelude::*,
};

fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, SeedlingPlugin::default()))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                update_music_volume_label,
                update_master_volume_label,
                update_sfx_volume_label,
                button_hover,
            ),
        )
        .run();
}

fn setup(mut master: Single<&mut VolumeNode, With<MainBus>>, mut commands: Commands) {
    // Let's reduce the master volume a bit.
    master.volume = CONVERTER.perceptual_to_volume(0.7);

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
            text((
                Text::new("Sound Settings"),
                TextFont {
                    font_size: 32.0,
                    ..Default::default()
                },
            )),
            core_grid(),
            play_buttons(),
        ],
    ));
}

fn play_music(
    _: Trigger<Pointer<Click>>,
    playing: Query<(), (With<MusicPool>, With<SamplePlayer>)>,
    mut commands: Commands,
    server: Res<AssetServer>,
) {
    // We'll only play music if it's not already playing.
    if playing.iter().len() > 0 {
        return;
    }

    let source = server.load("selfless_courage.ogg");
    commands.spawn((
        // Including the `MusicPool` marker queues this sample in the `MusicPool`.
        MusicPool,
        SamplePlayer::new(source).with_volume(Volume::Decibels(-6.0)),
    ));
}

fn play_sfx(_: Trigger<Pointer<Click>>, mut commands: Commands, server: Res<AssetServer>) {
    let source = server.load("caw.ogg");

    // The default pool is routed to the `SfxBus`, so we don't
    // need to include any special markers for sound effects.
    commands.spawn(SamplePlayer::new(source));
}

//  ============================ Control Knob Observers ============================ //

const CONVERTER: PerceptualVolume = PerceptualVolume::new();

const MIN_VOLUME: f32 = 0.0;
const MAX_VOLUME: f32 = 2.0;
const STEP: f32 = 0.1;

fn increment_volume(volume: Volume) -> Volume {
    let perceptual = CONVERTER.volume_to_perceptual(volume);
    let new_perceptual = (perceptual + STEP).min(MAX_VOLUME);
    CONVERTER.perceptual_to_volume(new_perceptual)
}

fn decrement_volume(volume: Volume) -> Volume {
    let perceptual = CONVERTER.volume_to_perceptual(volume);
    let new_perceptual = (perceptual - STEP).max(MIN_VOLUME);
    CONVERTER.perceptual_to_volume(new_perceptual)
}

// Master
fn lower_master(_: Trigger<Pointer<Click>>, mut master: Single<&mut VolumeNode, With<MainBus>>) {
    master.volume = decrement_volume(master.volume);
}

fn raise_master(_: Trigger<Pointer<Click>>, mut master: Single<&mut VolumeNode, With<MainBus>>) {
    master.volume = increment_volume(master.volume);
}

fn update_master_volume_label(
    mut label: Single<&mut Text, With<MasterVolumeLabel>>,
    master: Single<&VolumeNode, (With<MainBus>, Changed<VolumeNode>)>,
) {
    let percent = CONVERTER.volume_to_perceptual(master.volume) * 100.0;
    let text = format!("{}%", percent.round());
    label.0 = text;
}

// Music
fn lower_music(
    _: Trigger<Pointer<Click>>,
    mut music: Single<&mut VolumeNode, With<SamplerPool<MusicPool>>>,
) {
    music.volume = decrement_volume(music.volume);
}

fn raise_music(
    _: Trigger<Pointer<Click>>,
    mut music: Single<&mut VolumeNode, With<SamplerPool<MusicPool>>>,
) {
    music.volume = increment_volume(music.volume);
}

fn update_music_volume_label(
    mut label: Single<&mut Text, With<MusicVolumeLabel>>,
    music: Single<&VolumeNode, (With<SamplerPool<MusicPool>>, Changed<VolumeNode>)>,
) {
    let percent = CONVERTER.volume_to_perceptual(music.volume) * 100.0;
    let text = format!("{}%", percent.round());
    label.0 = text;
}

// SFX
fn lower_sfx(_: Trigger<Pointer<Click>>, mut sfx: Single<&mut VolumeNode, With<SfxBus>>) {
    sfx.volume = decrement_volume(sfx.volume);
}

fn raise_sfx(_: Trigger<Pointer<Click>>, mut sfx: Single<&mut VolumeNode, With<SfxBus>>) {
    sfx.volume = increment_volume(sfx.volume);
}

fn update_sfx_volume_label(
    mut label: Single<&mut Text, With<SfxVolumeLabel>>,
    sfx: Single<&VolumeNode, (With<SfxBus>, Changed<VolumeNode>)>,
) {
    let percent = CONVERTER.volume_to_perceptual(sfx.volume) * 100.0;
    let text = format!("{}%", percent.round());
    label.0 = text;
}

//  ============================ UI Code ============================ //

fn core_grid() -> impl Bundle {
    (
        Name::new("Sound Grid"),
        Node {
            row_gap: Val::Px(10.0),
            column_gap: Val::Px(30.0),
            display: Display::Grid,
            width: Val::Percent(100.0),
            grid_template_columns: RepeatedGridTrack::percent(2, 50.0),
            ..default()
        },
        children![
            text(Text::new("Master")),
            master_volume(),
            text(Text::new("Music")),
            music_volume(),
            text(Text::new("Sfx")),
            sfx_volume(),
        ],
    )
}

fn play_buttons() -> impl Bundle {
    (
        Node {
            justify_content: JustifyContent::SpaceAround,
            width: Val::Percent(100.0),
            ..Default::default()
        },
        children![btn("Play Music", play_music), btn("Play Sfx", play_sfx),],
    )
}

fn master_volume() -> impl Bundle {
    (
        knobs_container(),
        children![
            btn("-", lower_master),
            knob_label(MasterVolumeLabel),
            btn("+", raise_master),
        ],
    )
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct MasterVolumeLabel;

fn music_volume() -> impl Bundle {
    (
        knobs_container(),
        children![
            btn("-", lower_music),
            knob_label(MusicVolumeLabel),
            btn("+", raise_music),
        ],
    )
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct MusicVolumeLabel;

fn sfx_volume() -> impl Bundle {
    (
        knobs_container(),
        children![
            btn("-", lower_sfx),
            knob_label(SfxVolumeLabel),
            btn("+", raise_sfx),
        ],
    )
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct SfxVolumeLabel;

pub fn btn<E, B, M, I>(t: impl Into<String>, action: I) -> impl Bundle
where
    E: Event,
    B: Bundle,
    I: IntoObserverSystem<E, B, M>,
{
    let action = IntoObserverSystem::into_system(action);
    let t: String = t.into();

    (
        Name::new("Button"),
        Node::default(),
        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
            parent
                .spawn((
                    Button,
                    BorderColor(Color::WHITE),
                    children![Name::new("Button text"), text(Text(t))],
                ))
                .observe(action);
        })),
    )
}

pub fn text(text: impl Bundle) -> impl Bundle {
    (
        Node {
            padding: UiRect::axes(Val::Px(10.0), Val::Px(10.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..Default::default()
        },
        BackgroundColor(Color::srgb(0.9, 0.9, 0.9)),
        BorderRadius::all(Val::Percent(10.0)),
        children![(text, TextColor(Color::srgb(0.1, 0.1, 0.1)))],
    )
}

fn knobs_container() -> impl Bundle {
    Node {
        justify_self: JustifySelf::Center,
        align_content: AlignContent::SpaceEvenly,
        min_width: Val::Px(100.0),
        ..Default::default()
    }
}

fn knob_label(label: impl Component) -> impl Bundle {
    (
        Node {
            padding: UiRect::horizontal(Val::Px(10.0)),
            justify_content: JustifyContent::Center,
            ..Default::default()
        },
        children![text((
            Text::new(""),
            Node {
                min_width: Val::Px(75.0),
                ..Default::default()
            },
            TextLayout {
                justify: JustifyText::Center,
                ..Default::default()
            },
            label
        ))],
    )
}

const NORMAL_BUTTON: Color = Color::srgb(0.9, 0.9, 0.9);
const HOVERED_BUTTON: Color = Color::srgb(0.7, 0.7, 0.7);

fn button_hover(
    interaction_query: Query<(&Interaction, &Children), (Changed<Interaction>, With<Button>)>,
    mut text: Query<&mut BackgroundColor>,
) {
    for (interaction, children) in &interaction_query {
        let Some(mut color) = children.get(1).and_then(|c| text.get_mut(*c).ok()) else {
            continue;
        };

        match *interaction {
            Interaction::Pressed => {
                *color = NORMAL_BUTTON.into();
            }
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
            }
        }
    }
}
