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
use bevy_time::Time;
use bevy_transform::prelude::*;
use firewheel::{clock::DurationSeconds, nodes::spatial_basic::SpatialBasicNode};

use crate::{
    SeedlingSystems,
    node::events::AudioEvents,
    nodes::itd::{ItdConfig, ItdNode},
    pool::sample_effects::EffectOf,
    sample::{PlaybackSettings, SamplePlayer},
    time::{Audio, AudioTime},
};

pub(crate) struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DefaultSpatialScale>()
            .init_resource::<SpeedOfSound>()
            .add_systems(
                Last,
                (update_doppler, delay_play_for_propagation).before(SeedlingSystems::Pool),
            )
            .add_systems(
                Last,
                (
                    update_basic,
                    update_itd,
                    #[cfg(feature = "hrtf")]
                    spatial_hrtf::update_hrtf,
                    store_previous_global_positions,
                )
                    .chain()
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

/// The speed of sound used by spatial effects.
///
/// This value is expressed in world units per second. The default is `343.0`.
#[derive(Resource, Debug, Clone, Copy)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SpeedOfSound(pub f32);

impl Default for SpeedOfSound {
    fn default() -> Self {
        Self(343.0)
    }
}

/// Applies a Doppler shift effect based on the closest listener.
#[derive(Debug, Clone, Copy, Component)]
#[require(PlaybackSettings, Transform, PreviousGlobalPosition)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct Doppler {
    /// The current Doppler multiplier applied when synchronizing playback speed.
    /// This is automatically set by
    pub factor: f64,
}

impl Default for Doppler {
    fn default() -> Self {
        Self { factor: 1.0 }
    }
}

/// Delays the start of playback based on the initial distance to the nearest listener.
#[derive(Debug, Component, Clone, Copy)]
#[require(PlaybackSettings, Transform)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct PropagationDelay;

/// Stores the previous global position of an entity.
/// This is used to calculate the velocity of the entity.
#[derive(Component, Debug, Clone, Copy, Default)]
struct PreviousGlobalPosition(Option<Vec3>);

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
#[require(Transform, PreviousGlobalPosition)]
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
#[require(Transform, PreviousGlobalPosition)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SpatialListener3D;

#[derive(SystemParam)]
struct SpatialListeners<'w, 's> {
    listeners: Query<
        'w,
        's,
        (
            Entity,
            &'static GlobalTransform,
            AnyOf<(&'static SpatialListener2D, &'static SpatialListener3D)>,
        ),
    >,
}

#[derive(Clone, Copy)]
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

/// Return type of `SpatialListeners::nearest_listener`.
/// 
/// Contains useful information about the nearest listener.
#[derive(Clone, Copy)]
struct NearestListener {
    entity: Entity,
    transform: Transform,
    spatial_kind: SpatialKind,
    distance_squared: f32,
}

impl SpatialListeners<'_, '_> {
    /// Fetch the nearest spatial listener, if any exist.
    ///
    /// This iterates over both 2D and 3D listeners.
    fn nearest_listener(&self, emitter: Vec3) -> Option<NearestListener> {
        self.listeners
            .iter()
            .map(|(entity, transform, kind)| {
                let transform = transform.compute_transform();
                let kind = SpatialKind::from(kind);
                let distance_squared = match kind {
                    // in a 2d context, we need to ignore the z component
                    SpatialKind::Listener2D => {
                        emitter.xy().distance_squared(transform.translation.xy())
                    }
                    SpatialKind::Listener3D => emitter.distance_squared(transform.translation),
                };

                NearestListener {
                    entity,
                    transform,
                    spatial_kind: kind,
                    distance_squared,
                }
            })
            .min_by(|left, right| left.distance_squared.total_cmp(&right.distance_squared))
    }

    /// Calculate the offset between `emitter` and the nearest listener.
    ///
    /// This does not account for spatial scaling.
    fn calculate_offset(&self, emitter: Vec3) -> Option<Vec3> {
        let nearest = self.nearest_listener(emitter)?;
        return Some(listener_offset(
            emitter,
            nearest.transform.translation,
            nearest.transform,
            nearest.spatial_kind,
        ));
    }
}

fn listener_offset(
    emitter: Vec3,
    listener_position: Vec3,
    listener_transform: Transform,
    kind: SpatialKind,
) -> Vec3 {
    let mut world_offset = emitter - listener_position;

    match kind {
        SpatialKind::Listener2D => {
            world_offset.z = 0.0;
            let local_offset = listener_transform.rotation.inverse() * world_offset;
            Vec3::new(local_offset.x, 0.0, local_offset.y)
        }
        SpatialKind::Listener3D => {
            let local_offset = listener_transform.rotation.inverse() * world_offset;
            local_offset
        }
    }
}

fn velocity_from_positions(previous: Option<Vec3>, current: Vec3, delta_seconds: f32) -> Vec3 {
    if let Some(previous) = previous
        && delta_seconds > 0.0
    {
        return (current - previous) / delta_seconds;
    } else {
        return Vec3::ZERO;
    }
}

fn doppler_factor(
    speed_of_sound: f32,
    displacement: Vec3, // listener to source
    source_velocity: Vec3,
    listener_velocity: Vec3,
) -> f64 {
    let speed_of_sound = speed_of_sound.max(0.01);
    let direction = displacement.normalize_or_zero();
    if direction.length_squared() == 0.0 {
        return 1.0;
    }

    let source_velocity_radial = source_velocity.dot(direction);
    let listener_velocity_radial = listener_velocity.dot(direction);
    let denominator = speed_of_sound + source_velocity_radial;
    let denominator = denominator.max(0.01);

    let factor = (speed_of_sound + listener_velocity_radial) / denominator;
    return factor.max(0.0) as f64;
}

type EffectTransform = AnyOf<(&'static GlobalTransform, &'static EffectOf)>;

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

fn update_itd(
    listeners: SpatialListeners,
    speed_of_sound: Res<SpeedOfSound>,
    mut emitters: Query<(&mut ItdNode, &mut ItdConfig, EffectTransform)>,
    transforms: Query<&GlobalTransform>,
) {
    for (mut spatial, mut config, transform) in emitters.iter_mut() {
        if config.speed_of_sound != speed_of_sound.0 {
            config.speed_of_sound = speed_of_sound.0;
        }

        if let Some(emitter_pos) = extract_effect_transform(transform, &transforms)
            && let Some(offset) = listeners.calculate_offset(emitter_pos)
        {
            spatial.direction = offset;
        }
    }
}

fn update_doppler(
    listeners: SpatialListeners,
    time: Res<Time>,
    speed_of_sound: Res<SpeedOfSound>,
    listener_positions: Query<
        &PreviousGlobalPosition,
        Or<(With<SpatialListener2D>, With<SpatialListener3D>)>,
    >,
    mut sources: Query<(&GlobalTransform, &PreviousGlobalPosition, &mut Doppler)>,
) {
    if time.delta_secs() <= 0.0 {
        return;
    }

    for (transform, previous_position, mut doppler) in sources.iter_mut() {
        let Some(nearest) = listeners.nearest_listener(transform.translation()) else {
            doppler.factor = 1.0;
            continue;
        };

        let displacement = transform.translation() - nearest.transform.translation;
        let source_velocity =
            velocity_from_positions(previous_position.0, transform.translation(), time.delta_secs());
        let listener_velocity = listener_positions
            .get(nearest.entity)
            .ok()
            .map(|previous| {
                velocity_from_positions(previous.0, nearest.transform.translation, time.delta_secs())
            })
            .unwrap_or(Vec3::ZERO);

        doppler.factor = doppler_factor(
            speed_of_sound.0,
            displacement,
            source_velocity,
            listener_velocity,
        );
    }
}

fn delay_play_for_propagation(
    listeners: SpatialListeners,
    audio_time: Res<Time<Audio>>,
    speed_of_sound: Res<SpeedOfSound>,
    mut source_query: Query<
        (&GlobalTransform, &mut PlaybackSettings, &mut AudioEvents),
        (With<PropagationDelay>, Added<SamplePlayer>),
    >,
) {
    for (transform, mut settings, mut events) in source_query.iter_mut() {
        if !*settings.play {
            continue;
        }

        let emitter_position = transform.translation();
        let Some(offset) = listeners.calculate_offset(emitter_position) else {
            continue;
        };

        let delay_seconds = offset.length() / speed_of_sound.0.max(0.01);
        if delay_seconds <= 0.0 {
            continue;
        }

        settings.pause();
        settings.play_at(
            None,
            audio_time.delay(DurationSeconds(delay_seconds as f64)),
            &mut events,
        );
    }
}

fn store_previous_global_positions(
    mut tracked: Query<(&GlobalTransform, &mut PreviousGlobalPosition)>,
) {
    for (transform, mut previous) in tracked.iter_mut() {
        previous.0 = Some(transform.translation());
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
            listeners.nearest_listener(emitter).unwrap().transform
        });

        assert_eq!(closest, positions[1]);
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
