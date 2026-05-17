//! 3D spatial audio with runtime effect toggling.
//!
//! A source orbits a static listener at 2 m while bobbing vertically.
//! Press **S** / **I** / **H** to toggle [`SpatialBasicNode`], [`ItdNode`], and [`HrtfNode`].

use bevy::prelude::*;
use bevy_seedling::{node::follower::FollowerOf, prelude::*};

const EFFECTS: [(&str, &str, KeyCode); 3] = [
    ("[S]", "Spatial basic", KeyCode::KeyS),
    ("[I]", "ITD", KeyCode::KeyI),
    ("[H]", "HRTF", KeyCode::KeyH),
];

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, SeedlingPlugins))
        .add_systems(Startup, (set_up_ui, startup).chain())
        .add_systems(
            Update,
            (
                update_source_motion,
                update_scene_camera,
                toggle_effects,
                update_effect_labels,
            )
                .chain(),
        )
        .run();
}

#[derive(Component)]
struct DemoSound;

#[derive(Component)]
struct SourceMotion {
    orbit_angle: f32,
    bob_phase: f32,
}

#[derive(Component)]
struct SceneCamera;

#[derive(Component)]
struct SpatialBasicLabel;

#[derive(Component)]
struct ItdLabel;

#[derive(Component)]
struct HrtfLabel;

fn set_up_ui(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(14.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.12, 0.14, 0.85)),
        BorderColor::all(Color::srgb(0.75, 0.75, 0.8)),
        children![
            (
                Text::new("Spatial effects"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
            ),
            (
                Text::new("[S] Spatial basic: …"),
                SpatialBasicLabel,
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
            ),
            (
                Text::new("[I] ITD: …"),
                ItdLabel,
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
            ),
            (
                Text::new("[H] HRTF: …"),
                HrtfLabel,
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
            ),
        ],
    ));
}

fn startup(
    server: Res<AssetServer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.1, -0.85)),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d {
            half_size: Vec2::splat(6.0),
            ..default()
        })),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.32, 0.35, 0.4),
            perceptual_roughness: 0.92,
            ..default()
        })),
        Transform::from_xyz(0.0, -5.0, 0.0),
    ));

    commands.spawn((
        SceneCamera,
        Camera3d::default(),
        Camera {
            order: 0,
            ..default()
        },
        Transform::from_xyz(0.0, 3.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DemoSound,
        SourceMotion {
            orbit_angle: 0.0,
            bob_phase: 0.0,
        },
        SamplePlayer::new(server.load("selfless_courage.ogg")).looping(),
        Transform::from_xyz(2.0, 0.0, 0.0),
        Mesh3d(meshes.add(Sphere::new(0.15))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.45, 0.2),
            ..default()
        })),
        sample_effects![
            SpatialBasicNode::default(),
            ItdNode::default(),
            HrtfNode::default(),
        ],
    ));

    commands.spawn((
        SpatialListener3D,
        Transform::default(),
        Mesh3d(meshes.add(Sphere::new(0.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.55, 0.95),
            ..default()
        })),
    ));
}

fn update_source_motion(
    listener: Query<&Transform, (With<SpatialListener3D>, Without<DemoSound>)>,
    mut sources: Query<(&mut SourceMotion, &mut Transform), With<DemoSound>>,
    time: Res<Time>,
) {
    let Ok(listener_transform) = listener.single() else {
        return;
    };

    let listener_position = listener_transform.translation;
    let orbit_radius = 2.0;
    let orbit_period_seconds = 15.0;
    let bob_amplitude = 1.0;
    let bob_period_seconds = 3.0;
    let delta_seconds = time.delta_secs();

    for (mut motion, mut transform) in sources.iter_mut() {
        motion.orbit_angle += core::f32::consts::TAU * delta_seconds / orbit_period_seconds;
        motion.bob_phase += core::f32::consts::TAU * delta_seconds / bob_period_seconds;

        let horizontal_offset = Vec2::new(
            motion.orbit_angle.cos() * orbit_radius,
            motion.orbit_angle.sin() * orbit_radius,
        );

        transform.translation = listener_position
            + Vec3::new(
                horizontal_offset.x,
                motion.bob_phase.sin() * bob_amplitude,
                horizontal_offset.y,
            );
    }
}

fn update_scene_camera(
    listener: Query<&Transform, (With<SpatialListener3D>, Without<SceneCamera>)>,
    mut camera: Query<&mut Transform, (With<SceneCamera>, Without<SpatialListener3D>)>,
) {
    let Ok(listener_transform) = listener.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera.single_mut() else {
        return;
    };

    let listener_position = listener_transform.translation;
    let camera_offset = Vec3::new(0.0, 3.0, 2.5);

    camera_transform.translation = listener_position + camera_offset;
    camera_transform.look_at(listener_position, Vec3::Y);
}

fn update_effect_labels(
    samples: Query<&SampleEffects, With<DemoSound>>,
    followers: Query<(Has<AudioBypass>, &FollowerOf), With<FirewheelNode>>,
    mut labels: ParamSet<(
        Single<&mut Text, With<SpatialBasicLabel>>,
        Single<&mut Text, With<ItdLabel>>,
        Single<&mut Text, With<HrtfLabel>>,
    )>,
) {
    let Ok(effects) = samples.single() else {
        return;
    };

    labels.p0().into_inner().0 =
        effect_status_line(EFFECTS[0].0, EFFECTS[0].1, &followers, effects, 0);
    labels.p1().into_inner().0 =
        effect_status_line(EFFECTS[1].0, EFFECTS[1].1, &followers, effects, 1);
    labels.p2().into_inner().0 =
        effect_status_line(EFFECTS[2].0, EFFECTS[2].1, &followers, effects, 2);
}

fn effect_status_line(
    key: &str,
    name: &str,
    followers: &Query<(Has<AudioBypass>, &FollowerOf), With<FirewheelNode>>,
    effects: &SampleEffects,
    effect_index: usize,
) -> String {
    let Some(&baseline) = effects.get(effect_index) else {
        return format!("{key} {name}: …");
    };

    let Some((bypassed, _)) = followers
        .iter()
        .find(|(_, follower)| follower.0 == baseline)
    else {
        return format!("{key} {name}: loading");
    };

    let state = if bypassed { "bypassed" } else { "on" };
    return format!("{key} {name}: {state}");
}

fn toggle_effects(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    samples: Query<&SampleEffects, With<DemoSound>>,
    followers: Query<(Entity, Has<AudioBypass>, &FollowerOf), With<FirewheelNode>>,
) {
    let Ok(effects) = samples.single() else {
        return;
    };

    for (index, (_, _, key)) in EFFECTS.into_iter().enumerate() {
        if keyboard.just_pressed(key) {
            toggle_effect_bypass(&mut commands, &followers, effects, index);
        }
    }
}

fn toggle_effect_bypass(
    commands: &mut Commands,
    followers: &Query<(Entity, Has<AudioBypass>, &FollowerOf), With<FirewheelNode>>,
    effects: &SampleEffects,
    effect_index: usize,
) {
    let Some(&baseline) = effects.get(effect_index) else {
        return;
    };

    let Some((entity, bypassed, _)) = followers
        .iter()
        .find(|(_, _, follower)| follower.0 == baseline)
    else {
        return;
    };

    if bypassed {
        commands.entity(entity).remove::<AudioBypass>();
    } else {
        commands.entity(entity).insert(AudioBypass);
    }
}
