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
use bevy_ecs::{prelude::*, query::QueryData, system::SystemParam};
use bevy_math::prelude::*;
use bevy_transform::prelude::*;
use firewheel::nodes::spatial_basic::SpatialBasicNode;

use crate::SeedlingSystems;
#[cfg(feature = "sampler")]
use crate::pool::sample_effects::EffectOf;

pub(crate) struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DefaultSpatialScale>().add_systems(
            Last,
            (
                update_basic,
                #[cfg(feature = "itd")]
                update_itd,
                #[cfg(feature = "hrtf")]
                spatial_hrtf::update_hrtf,
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

#[derive(SystemParam)]
struct SpatialListeners<'w, 's> {
    listeners: Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            AnyOf<(&'static SpatialListener2D, &'static SpatialListener3D)>,
        ),
    >,
}

enum SpatialKind {
    Listener2D,
    Listener3D,
}

impl From<(Option<&'_ SpatialListener2D>, Option<&'_ SpatialListener3D>)> for SpatialKind {
    fn from(value: (Option<&'_ SpatialListener2D>, Option<&'_ SpatialListener3D>)) -> Self {
        match value {
            (Some(_), None) => Self::Listener2D,
            (None, Some(_)) => Self::Listener3D,
            _ => unreachable!(),
        }
    }
}

impl SpatialListeners<'_, '_> {
    /// Fetch the nearest spatial listener, if any exist.
    ///
    /// This iterates over both 2D and 3D listeners.
    fn nearest_listener(&self, emitter: Vec3) -> Option<(Transform, SpatialKind)> {
        // This is linear over the number of listeners, but we
        // expect there to be very few of these at any one time.
        self.listeners
            .iter()
            .map(|(transform, kind)| {
                let transform = transform.compute_transform();
                let kind = SpatialKind::from(kind);
                let distance = match kind {
                    // in a 2d context, we need to ignore the z component
                    SpatialKind::Listener2D => {
                        emitter.xy().distance_squared(transform.translation.xy())
                    }
                    SpatialKind::Listener3D => emitter.distance_squared(transform.translation),
                };

                (transform, kind, distance)
            })
            .min_by(|(.., a), (.., b)| a.total_cmp(b))
            .map(|(transform, kind, ..)| (transform, kind))
    }

    /// Calculate the offset between `emitter` and the nearest listener.
    ///
    /// This does not account for spatial scaling.
    fn calculate_offset(&self, emitter: Vec3) -> Option<Vec3> {
        let (listener, kind) = self.nearest_listener(emitter)?;

        let mut world_offset = emitter - listener.translation;

        match kind {
            SpatialKind::Listener2D => {
                world_offset.z = 0.0;
                let local_offset = listener.rotation.inverse() * world_offset;
                Some(Vec3::new(local_offset.x, 0.0, local_offset.y))
            }
            SpatialKind::Listener3D => {
                let local_offset = listener.rotation.inverse() * world_offset;
                Some(local_offset)
            }
        }
    }
}

#[cfg(feature = "sampler")]
type EffectTransform = AnyOf<(&'static GlobalTransform, &'static EffectOf)>;
#[cfg(not(feature = "sampler"))]
type EffectTransform = &'static GlobalTransform;

#[cfg(feature = "sampler")]
fn extract_effect_transform(
    effect_transform: <EffectTransform as QueryData>::Item<'_, '_>,
    transforms: &Query<&GlobalTransform>,
) -> Option<Vec3> {
    match effect_transform {
        (Some(global), _) => Some(global.translation()),
        (_, Some(parent)) => match transforms.get(parent.0) {
            Ok(global) => Some(global.translation()),
            Err(_) => None,
        },
        _ => unreachable!(),
    }
}

#[cfg(not(feature = "sampler"))]
fn extract_effect_transform(
    effect_transform: <EffectTransform as QueryData>::Item<'_, '_>,
    _transforms: &Query<&GlobalTransform>,
) -> Option<Vec3> {
    Some(effect_transform.translation())
}

fn update_basic(
    listeners: SpatialListeners,
    mut emitters: Query<(
        &mut SpatialBasicNode,
        Option<&SpatialScale>,
        EffectTransform,
    )>,
    transforms: Query<&GlobalTransform>,
    default_scale: Res<DefaultSpatialScale>,
) {
    for (mut spatial, scale, transform) in emitters.iter_mut() {
        if let Some(emitter_pos) = extract_effect_transform(transform, &transforms)
            && let Some(offset) = listeners.calculate_offset(emitter_pos)
        {
            let scale = scale.map(|s| s.0).unwrap_or(default_scale.0);
            spatial.offset = (offset * scale).into();
        }
    }
}

#[cfg(feature = "itd")]
fn update_itd(
    listeners: SpatialListeners,
    mut emitters: Query<(&mut crate::nodes::itd::ItdNode, EffectTransform)>,
    transforms: Query<&GlobalTransform>,
) {
    for (mut spatial, transform) in emitters.iter_mut() {
        if let Some(emitter_pos) = extract_effect_transform(transform, &transforms)
            && let Some(offset) = listeners.calculate_offset(emitter_pos)
        {
            spatial.direction = offset;
        }
    }
}

#[cfg(feature = "hrtf")]
mod spatial_hrtf {
    use super::*;
    use crate::prelude::hrtf::HrtfNode;

    pub(super) fn update_hrtf(
        listeners: SpatialListeners,
        mut emitters: Query<(&mut HrtfNode, Option<&SpatialScale>, EffectTransform)>,
        transforms: Query<&GlobalTransform>,
        default_scale: Res<DefaultSpatialScale>,
    ) {
        for (mut spatial, scale, transform) in emitters.iter_mut() {
            if let Some(emitter_pos) = extract_effect_transform(transform, &transforms)
                && let Some(offset) = listeners.calculate_offset(emitter_pos)
            {
                let scale = scale.map(|s| s.0).unwrap_or(default_scale.0);
                spatial.offset = offset * scale;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use alloc::vec::Vec;
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

        let mut app = prepare_app({
            let positions = positions.clone();
            move |mut commands: Commands| {
                for position in &positions {
                    commands.spawn((SpatialListener3D, *position));
                }
            }
        });

        let closest = run(&mut app, |listeners: SpatialListeners| {
            let emitter = Vec3::splat(0.0);
            listeners.nearest_listener(emitter).unwrap()
        });

        assert_eq!(closest.0, positions[1]);
    }

    #[test]
    fn test_empty() {
        let positions = []
            .into_iter()
            .map(Transform::from_translation)
            .collect::<Vec<_>>();

        let mut app = prepare_app({
            let positions = positions.clone();
            move |mut commands: Commands| {
                for position in &positions {
                    commands.spawn((SpatialListener3D, *position));
                }
            }
        });

        let closest = run(&mut app, |listeners: SpatialListeners| {
            let emitter = Vec3::splat(0.0);
            listeners.nearest_listener(emitter)
        });

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
