//!
//! Simple setup for a game: general, music and sfx channels settings
//!
//! This example set's up the following structure:
//!
//! ```text
//! ┌─────┐┌───┐┌───────┐
//! │Music││Sfx││General│
//! └┬────┘└┬──┘└┬──────┘
//! ┌▽──────▽┐   │
//! │Bus 1   │   │
//! └┬───────┘   │
//! ┌▽───────────▽┐
//! │MainBus      │
//! └─────────────┘
//! ```
//!
//! A "bus" is really just a node that we've given a label, usually a VolumeNode
//! The default pool is already connected to the MainBus,
//! and the Bus node will be automatically connected as well since we didn't specify any connections for it.
//!
//! A sampler pool is basically a collective sound source, so it doesn't really make any sense to route audio "through" it.
//! We don't use relationships right now to represent connections because Bevy's implementation doesn't support M:N-style relationships.
//! So for now, we have to stick to the imperative connect methods.
//!

#![allow(clippy::type_complexity)]
use bevy::{
    app::App,
    ecs::{spawn::SpawnWith, system::IntoObserverSystem},
    prelude::*,
    ui::Val::*,
};
use bevy_seedling::{pool::SamplerPool, prelude::*, sample::Sample};

#[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
struct Sfx;

#[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
struct Music;

#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct General;

#[derive(Resource, Debug, Clone)]
pub struct Sound {
    pub general: f32,
    pub music: f32,
    pub sfx: f32,
}

const MIN_VOLUME: f32 = 0.0;
const MAX_VOLUME: f32 = 3.0;
const STEP: f32 = 0.1;

fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, SeedlingPlugin::default()));

    app.add_systems(Startup, spawn_pools).add_systems(
        Update,
        (
            update_music_volume_label,
            update_general_volume_label,
            update_sfx_volume_label,
        ),
    );

    app.run();
}

fn spawn_pools(mut commands: Commands) {
    commands.insert_resource(Sound {
        general: 1.0,
        music: 0.5,
        sfx: 0.5,
    });

    commands.spawn((General, VolumeNode::default()));
    commands.spawn(SamplerPool(Music)).connect(General);
    commands.spawn(SamplerPool(Sfx)).connect(General);

    commands.spawn(Camera2d);

    commands.spawn((
        BackgroundColor(Color::srgba_u8(170, 200, 250, 200)),
        Node {
            width: Percent(100.0),
            height: Percent(100.0),
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Vh(5.0),
            ..default()
        },
        children![text("Settings".into()), core_grid()],
    ));
}

pub fn music(handle: Handle<Sample>, vol: f32) -> impl Bundle {
    (
        Music,
        SamplePlayer::new(handle).with_volume(Volume::Decibels(vol)),
    )
}

pub fn sfx(handle: Handle<Sample>, vol: f32) -> impl Bundle {
    (
        Sfx,
        SamplePlayer::new(handle).with_volume(Volume::Decibels(vol)),
    )
}

fn play_music(
    _: Trigger<Pointer<Click>>,
    mut commands: Commands,
    sound: Res<Sound>,
    server: Res<AssetServer>,
) {
    let source = server.load("selfless_courage.ogg");
    let vol = sound.general * sound.music;
    commands.spawn(music(source, vol));
}

fn play_sfx(
    _: Trigger<Pointer<Click>>,
    mut commands: Commands,
    sound: Res<Sound>,
    server: Res<AssetServer>,
) {
    let source = server.load("caw.ogg");
    let vol = sound.general * sound.sfx;
    commands.spawn(sfx(source, vol));
}

// ============================ CONTROL KNOB OBSERVERS ============================

// GENERAL
fn lower_general(
    _: Trigger<Pointer<Click>>,
    mut sound: ResMut<Sound>,
    mut general: Single<&mut VolumeNode, With<General>>,
) {
    let new_volume = (sound.general - STEP).max(MIN_VOLUME);
    sound.general = new_volume;
    general.volume = Volume::Linear(new_volume);
}

fn raise_general(
    _: Trigger<Pointer<Click>>,
    mut sound: ResMut<Sound>,
    mut general: Single<&mut VolumeNode, With<General>>,
) {
    let new_volume = (sound.general + STEP).min(MAX_VOLUME);
    sound.general = new_volume;
    general.volume = Volume::Linear(new_volume);
}

fn update_general_volume_label(
    mut label: Single<&mut Text, With<GeneralVolumeLabel>>,
    sound: Res<Sound>,
) {
    let percent = (sound.general * 100.0).round();
    let text = format!("{percent}%");
    label.0 = text;
}

// MUSIC
fn lower_music(
    _: Trigger<Pointer<Click>>,
    mut sound: ResMut<Sound>,
    mut music: Single<&mut VolumeNode, (With<SamplerPool<Music>>, Without<SamplerPool<Sfx>>)>,
) {
    let new_volume = (sound.music - STEP).max(MIN_VOLUME);
    sound.music = new_volume;
    music.volume = Volume::Linear(new_volume * sound.general);
}

fn raise_music(
    _: Trigger<Pointer<Click>>,
    mut sound: ResMut<Sound>,
    mut music: Single<&mut VolumeNode, (With<SamplerPool<Music>>, Without<SamplerPool<Sfx>>)>,
) {
    let new_volume = (sound.music + STEP).min(MAX_VOLUME);
    sound.music = new_volume;
    music.volume = Volume::Linear(new_volume * sound.general);
}

fn update_music_volume_label(
    mut label: Single<&mut Text, With<MusicVolumeLabel>>,
    sound: Res<Sound>,
) {
    let percent = (sound.music * 100.0).round();
    let text = format!("{percent}%");
    label.0 = text;
}

// SFX
fn lower_sfx(
    _: Trigger<Pointer<Click>>,
    mut sound: ResMut<Sound>,
    mut sfx: Single<&mut VolumeNode, (With<SamplerPool<Sfx>>, Without<SamplerPool<Music>>)>,
) {
    let new_volume = (sound.sfx - STEP).max(MIN_VOLUME);
    sound.sfx = new_volume;
    sfx.volume = Volume::Linear(new_volume * sound.general);
}

fn raise_sfx(
    _: Trigger<Pointer<Click>>,
    mut sound: ResMut<Sound>,
    mut sfx: Single<&mut VolumeNode, (With<SamplerPool<Sfx>>, Without<SamplerPool<Music>>)>,
) {
    let new_volume = (sound.sfx + STEP).min(MAX_VOLUME);
    sound.sfx = new_volume;
    sfx.volume = Volume::Linear(new_volume * sound.general);
}

fn update_sfx_volume_label(mut label: Single<&mut Text, With<SfxVolumeLabel>>, sound: Res<Sound>) {
    let percent = (sound.sfx * 100.0).round();
    let text = format!("{percent}%");
    label.0 = text;
}

// ============================ UI STUFF, NEVERMIND THIS PILE OF CODE ============================

fn core_grid() -> impl Bundle {
    (
        Name::new("Sound Grid"),
        Node {
            row_gap: Px(10.0),
            column_gap: Px(30.0),
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::px(2, 400.0),
            ..default()
        },
        children![
            text("General".into()),
            general_volume(),
            text("Music".into()),
            music_volume(),
            text("Sfx".into()),
            sfx_volume(),
            btn("Play Music".into(), play_music),
            btn("Play Sfx".into(), play_sfx),
        ],
    )
}
fn general_volume() -> impl Bundle {
    (
        knobs_container(),
        children![
            btn("-".into(), lower_general),
            knob_label(GeneralVolumeLabel),
            btn("+".into(), raise_general),
        ],
    )
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct GeneralVolumeLabel;

fn music_volume() -> impl Bundle {
    (
        knobs_container(),
        children![
            btn("-".into(), lower_music),
            knob_label(MusicVolumeLabel),
            btn("+".into(), raise_music),
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
            btn("-".into(), lower_sfx),
            knob_label(SfxVolumeLabel),
            btn("+".into(), raise_sfx),
        ],
    )
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct SfxVolumeLabel;

pub fn btn<E, B, M, I>(t: String, action: I) -> impl Bundle
where
    E: Event,
    B: Bundle,
    I: IntoObserverSystem<E, B, M>,
{
    let action = IntoObserverSystem::into_system(action);

    (
        Name::new("Button"),
        Node::default(),
        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
            parent
                .spawn((
                    Button,
                    BorderColor(Color::WHITE),
                    children![Name::new("Button text"), text(t)],
                ))
                .observe(action);
        })),
    )
}

pub fn text(t: String) -> impl Bundle {
    (
        BackgroundColor(Color::WHITE),
        Text(t),
        TextColor(Color::BLACK),
    )
}

fn knobs_container() -> impl Bundle {
    Node {
        justify_self: JustifySelf::Center,
        align_content: AlignContent::SpaceEvenly,
        min_width: Px(100.0),
        ..Default::default()
    }
}

fn knob_label(label: impl Component) -> impl Bundle {
    (
        Node {
            padding: UiRect::horizontal(Px(10.0)),
            justify_content: JustifyContent::Center,
            ..Default::default()
        },
        children![(text("".into()), label)],
    )
}
