//! Types that allow one set of params to track another.

use bevy_ecs::{component::Mutable, prelude::*};
use firewheel::diff::{Diff, Patch, PathBuilder};
use smallvec::SmallVec;

/// A relationship that allows one entity's parameters to track another's.
///
/// This can only support a single rank; cascading
/// is not allowed.
///
/// Within `bevy_seedling`, this is used primarily by sampler
/// pools. When you define a pool with a set of effects,
/// those nodes will be automatically inserted on
/// [`SamplePlayer`][crate::prelude::SamplePlayer] entities
/// queued for that pool. Then, each effect node will
/// have a [`FollowerOf`] component inserted that
/// tracks the [`SamplePlayer`][crate::prelude::SamplePlayer].
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # #[derive(PoolLabel, Clone, Debug, PartialEq, Eq, Hash)]
/// # struct MyLabel;
/// # fn system(mut commands: Commands, server: Res<AssetServer>) {
/// commands.spawn((
///     SamplerPool(MyLabel),
///     sample_effects![SpatialBasicNode::default()],
/// ));
///
/// commands.spawn((MyLabel, SamplePlayer::new(server.load("my_sample.wav"))));
///
/// // Once spawned, these will look something like
/// // Pool: (SamplePlayer) -> (SpatialBasicNode, FollowerOf) -> (VolumeNode) -> (MainBus)
/// // SamplePlayer: (SamplePlayer, SampleEffects)
/// // SpatialBasicNode: (SpatialBasicNode, EffectOf, Followers)
/// # }
/// ```
#[derive(Debug, Component)]
#[relationship(relationship_target = Followers)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct FollowerOf(pub Entity);

/// The relationship target for [`FollowerOf`].
#[derive(Debug, Component)]
#[relationship_target(relationship = FollowerOf)]
pub struct Followers(SmallVec<[Entity; 2]>);

/// Apply diffing and patching between two sets of parameters
/// in the ECS. This allows the engine-connected parameters
/// to follow another set of parameters that may be
/// closer to user code.
///
/// For example, it's much easier for users to set parameters
/// on a sample player entity directly rather than drilling
/// into the sample pool and node the sample is assigned to.
pub(crate) fn param_follower<T: Diff + Patch + Component<Mutability = Mutable>>(
    sources: Query<&T, Without<FollowerOf>>,
    mut followers: Query<(&FollowerOf, &mut T)>,
) -> Result {
    let mut event_queue = Vec::new();
    for (follower, mut params) in followers.iter_mut() {
        let Ok(source) = sources.get(follower.0) else {
            continue;
        };

        source.diff(&params, PathBuilder::default(), &mut event_queue);

        for event in event_queue.drain(..) {
            super::apply_patch(params.as_mut(), &event)?;
        }
    }

    Ok(())
}
