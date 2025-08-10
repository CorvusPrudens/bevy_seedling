//! Spatial audio components.
//!
//! To enable spatial audio, three conditions are required:
//!
//! 1. The spatial audio node, [`SpatialBasicNode`], must have
//!    a transform.
//! 2. The spatial listener entity must have a [`SpatialListener2D`]
//!    or [`SpatialListener3D`].
//! 3. The spatial listener entity must have a transform.
//!
//! Typically, you'll want to include a [`SpatialBasicNode`] as an effect.
//!
//! ```
//! # use bevy_seedling::prelude::*;
//! # use bevy::prelude::*;
//! fn spawn_spatial(mut commands: Commands, server: Res<AssetServer>) {
//!     // Spawn a player with a transform (1).
//!     commands.spawn((
//!         SamplePlayer::new(server.load("my_sample.wav")),
//!         Transform::default(),
//!         sample_effects![SpatialBasicNode::default()],
//!     ));
//!
//!     // Then, spawn a listener (2), which automatically inserts
//!     // a transform if it doesn't already exist (3).
//!     commands.spawn(SpatialListener2D);
//! }
//! ```
//!
//! Multiple listeners are supported. `bevy_seedling` will
//! simply select the closest listener for distance
//! calculations.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_transform::prelude::*;
use firewheel::{nodes::spatial_basic::SpatialBasicNode, vector};

use crate::{SeedlingSystems, nodes::itd::ItdNode, pool::sample_effects::EffectOf};

pub(crate) struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DefaultSpatialScale>().add_systems(
            Last,
            (
                update_2d_emitters,
                update_2d_emitters_effects,
                update_3d_emitters,
                update_3d_emitters_effects,
                update_itd_effects,
                #[cfg(feature = "hrtf")]
                spatial_hrtf::update_hrtf_effects,
            )
                .after(SeedlingSystems::Pool)
                .before(SeedlingSystems::Queue),
        );
    }
}

/// A scaling factor applied to the distance between spatial listeners and emitters.
///
/// To override the [global spatial scaling][DefaultSpatialScale] for an entity,
/// simply insert [`SpatialScale`].
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// fn set_scale(mut commands: Commands, server: Res<AssetServer>) {
///     commands.spawn((
///         SamplePlayer::new(server.load("my_sample.wav")),
///         Transform::default(),
///         sample_effects![(SpatialBasicNode::default(), SpatialScale(Vec3::splat(0.25)))],
///     ));
/// }
/// ```
///
/// By default, a spatial signal's amplitude will be cut in half at 10 units. Then,
/// for each doubling in distance, the signal will be successively halved.
///
/// | Distance | Amplitude |
/// | -------- | --------- |
/// | 10       | -6dB      |
/// | 20       | -12dB     |
/// | 40       | -18dB     |
/// | 80       | -24dB     |
///
/// When one unit corresponds to one meter, this is a good default. If
/// your game's scale differs significantly, however, you may need
/// to adjust the spatial scaling.
///
/// The distance between listeners and emitters is multiplied by this
/// factor, so if a meter in your game corresponds to more than one unit, you
/// should provide a spatial scale of less than one to compensate.
#[derive(Component, Debug, Clone, Copy)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SpatialScale(pub Vec3);

impl Default for SpatialScale {
    fn default() -> Self {
        Self(Vec3::ONE)
    }
}

/// The global default spatial scale.
///
/// For more details on spatial scaling, see [`SpatialScale`].
///
/// The default scaling is 1 in every direction, [`Vec3::ONE`].
#[derive(Resource, Debug, Clone, Copy)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct DefaultSpatialScale(pub Vec3);

impl Default for DefaultSpatialScale {
    fn default() -> Self {
        Self(Vec3::ONE)
    }
}

/// A 2D spatial listener.
///
/// When this component is added to an entity with a transform,
/// this transform is used to calculate spatial offsets for all
/// emitters. An emitter is an entity with [`SpatialBasicNode`]
/// and transform components.
///
/// Multiple listeners are supported. `bevy_seedling` will
/// simply select the closest listener for distance
/// calculations.
#[derive(Debug, Default, Component)]
#[require(Transform)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SpatialListener2D;

/// A 3D spatial listener.
///
/// When this component is added to an entity with a transform,
/// this transform is used to calculate spatial offsets for all
/// emitters. An emitter is an entity with [`SpatialBasicNode`]
/// and transform components.
///
/// Multiple listeners are supported. `bevy_seedling` will
/// simply select the closest listener for distance
/// calculations.
#[derive(Debug, Default, Component)]
#[require(Transform)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SpatialListener3D;

fn update_2d_emitters(
    listeners: Query<&GlobalTransform, With<SpatialListener2D>>,
    mut emitters: Query<(
        &mut SpatialBasicNode,
        Option<&SpatialScale>,
        &GlobalTransform,
    )>,
    default_scale: Res<DefaultSpatialScale>,
) {
    for (mut spatial, scale, transform) in emitters.iter_mut() {
        let emitter_pos = transform.translation();
        let closest_listener = find_closest_listener(
            emitter_pos,
            listeners.iter().map(GlobalTransform::compute_transform),
        );

        let Some(listener) = closest_listener else {
            continue;
        };

        let scale = scale.map(|s| s.0).unwrap_or(default_scale.0);

        let mut world_offset = emitter_pos - listener.translation;
        world_offset.z = 0.0;
        let local_offset = (listener.rotation.inverse() * world_offset) * scale;
        spatial.offset = vector::Vec3::new(local_offset.x, 0.0, local_offset.y);
    }
}

// TODO: is there a good way to consolidate this?
fn update_2d_emitters_effects(
    listeners: Query<&GlobalTransform, With<SpatialListener2D>>,
    mut emitters: Query<(&mut SpatialBasicNode, Option<&SpatialScale>, &EffectOf)>,
    effect_parents: Query<&GlobalTransform>,
    default_scale: Res<DefaultSpatialScale>,
) {
    for (mut spatial, scale, effect_of) in emitters.iter_mut() {
        let Ok(transform) = effect_parents.get(effect_of.0) else {
            continue;
        };

        let emitter_pos = transform.translation();
        let closest_listener = find_closest_listener(
            emitter_pos,
            listeners.iter().map(GlobalTransform::compute_transform),
        );

        let Some(listener) = closest_listener else {
            continue;
        };

        let scale = scale.map(|s| s.0).unwrap_or(default_scale.0);

        let mut world_offset = emitter_pos - listener.translation;
        world_offset.z = 0.0;
        let local_offset = (listener.rotation.inverse() * world_offset) * scale;
        spatial.offset = vector::Vec3::new(local_offset.x, 0.0, local_offset.y);
    }
}

fn update_itd_effects(
    listeners: Query<&GlobalTransform, Or<(With<SpatialListener2D>, With<SpatialListener3D>)>>,
    mut emitters: Query<(&mut ItdNode, &EffectOf)>,
    effect_parents: Query<&GlobalTransform>,
) {
    for (mut spatial, effect_of) in emitters.iter_mut() {
        let Ok(transform) = effect_parents.get(effect_of.0) else {
            continue;
        };

        let emitter_pos = transform.translation();
        let closest_listener = find_closest_listener(
            emitter_pos,
            listeners.iter().map(GlobalTransform::compute_transform),
        );

        let Some(listener) = closest_listener else {
            continue;
        };

        let world_offset = emitter_pos - listener.translation;
        let local_offset = listener.rotation.inverse() * world_offset;
        spatial.direction = local_offset;
    }
}

fn update_3d_emitters(
    listeners: Query<&GlobalTransform, With<SpatialListener3D>>,
    mut emitters: Query<(
        &mut SpatialBasicNode,
        Option<&SpatialScale>,
        &GlobalTransform,
    )>,
    default_scale: Res<DefaultSpatialScale>,
) {
    for (mut spatial, scale, transform) in emitters.iter_mut() {
        let emitter_pos = transform.translation();
        let closest_listener = find_closest_listener(
            emitter_pos,
            listeners.iter().map(GlobalTransform::compute_transform),
        );

        let Some(listener) = closest_listener else {
            continue;
        };

        let scale = scale.map(|s| s.0).unwrap_or(default_scale.0);

        let world_offset = emitter_pos - listener.translation;
        let local_offset = listener.rotation.inverse() * world_offset;
        spatial.offset = (local_offset * scale).into();
    }
}

fn update_3d_emitters_effects(
    listeners: Query<&GlobalTransform, With<SpatialListener3D>>,
    mut emitters: Query<(&mut SpatialBasicNode, Option<&SpatialScale>, &EffectOf)>,
    effect_parents: Query<&GlobalTransform>,
    default_scale: Res<DefaultSpatialScale>,
) {
    for (mut spatial, scale, effect_of) in emitters.iter_mut() {
        let Ok(transform) = effect_parents.get(effect_of.0) else {
            continue;
        };

        let emitter_pos = transform.translation();
        let closest_listener = find_closest_listener(
            emitter_pos,
            listeners.iter().map(GlobalTransform::compute_transform),
        );

        let Some(listener) = closest_listener else {
            continue;
        };

        let scale = scale.map(|s| s.0).unwrap_or(default_scale.0);

        let world_offset = emitter_pos - listener.translation;
        let local_offset = listener.rotation.inverse() * world_offset;
        spatial.offset = (local_offset * scale).into();
    }
}

fn find_closest_listener(
    emitter_pos: Vec3,
    listeners: impl Iterator<Item = Transform>,
) -> Option<Transform> {
    let mut closest_listener: Option<(f32, Transform)> = None;

    for listener in listeners {
        let listener_pos = listener.translation;
        let distance = emitter_pos.distance_squared(listener_pos);

        match &mut closest_listener {
            None => closest_listener = Some((distance, listener)),
            Some((old_distance, old_transform)) => {
                if distance < *old_distance {
                    *old_distance = distance;
                    *old_transform = listener;
                }
            }
        }
    }

    closest_listener.map(|l| l.1)
}

#[cfg(feature = "hrtf")]
mod spatial_hrtf {
    use super::*;
    use crate::prelude::hrtf::HrtfNode;

    pub(super) fn update_hrtf_effects(
        listeners: Query<&GlobalTransform, Or<(With<SpatialListener2D>, With<SpatialListener3D>)>>,
        mut emitters: Query<(&mut HrtfNode, Option<&SpatialScale>, &EffectOf)>,
        effect_parents: Query<&GlobalTransform>,
        default_scale: Res<DefaultSpatialScale>,
    ) {
        for (mut spatial, scale, effect_of) in emitters.iter_mut() {
            let Ok(transform) = effect_parents.get(effect_of.0) else {
                continue;
            };

            let emitter_pos = transform.translation();
            let closest_listener = find_closest_listener(
                emitter_pos,
                listeners.iter().map(GlobalTransform::compute_transform),
            );

            let Some(listener) = closest_listener else {
                continue;
            };

            let scale = scale.map(|s| s.0).unwrap_or(default_scale.0);

            let world_offset = emitter_pos - listener.translation;
            let local_offset = listener.rotation.inverse() * world_offset;
            let local_offset = local_offset * scale;

            spatial.offset = local_offset * scale;
        }
    }
}

#[cfg(test)]
mod test {
    use bevy_asset::AssetServer;

    use super::*;
    use crate::{
        node::follower::FollowerOf,
        pool::Sampler,
        prelude::*,
        test::{prepare_app, run},
    };

    #[test]
    fn test_closest() {
        let positions = [Vec3::splat(5.0), Vec3::splat(4.0), Vec3::splat(6.0)]
            .into_iter()
            .map(Transform::from_translation)
            .collect::<Vec<_>>();
        let emitter = Vec3::splat(0.0);
        let closest = find_closest_listener(emitter, positions.iter().copied()).unwrap();

        assert_eq!(closest, positions[1]);
    }

    #[test]
    fn test_empty() {
        let positions = [];

        let emitter = Vec3::splat(0.0);
        let closest = find_closest_listener(emitter, positions.iter().copied());

        assert!(closest.is_none());
    }

    #[derive(PoolLabel, PartialEq, Eq, Hash, Clone, Debug)]
    struct TestPool;

    /// Ensure that transform updates are propagated immediately when
    /// queued in a pool.
    #[test]
    fn test_immediate_positioning() {
        let position = Vec3::splat(3.0);
        let mut app = prepare_app(move |mut commands: Commands, server: Res<AssetServer>| {
            commands.spawn((
                SamplerPool(TestPool),
                sample_effects![SpatialBasicNode::default()],
            ));

            commands.spawn((SpatialListener3D, Transform::default()));

            commands.spawn((
                TestPool,
                Transform::from_translation(position),
                SamplePlayer::new(server.load("sine_440hz_1ms.wav")).looping(),
            ));
        });

        loop {
            let complete = run(
                &mut app,
                move |player: Query<&Sampler>,
                      effect: Query<&SpatialBasicNode, With<FollowerOf>>| {
                    if player.iter().len() == 1 {
                        let effect: Vec3 = effect.single().unwrap().offset.into();
                        assert_eq!(effect, position);
                        true
                    } else {
                        false
                    }
                },
            );

            if complete {
                break;
            }

            app.update();
        }
    }
}
