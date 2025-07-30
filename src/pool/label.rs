//! Type-based sample pool labeling.
//!
//! `bevy_seedling` provides a single pool label, [`DefaultPool`].
//! Any node that doesn't provide an explicit pool when spawned
//! and has no effects will be automatically played in the [`DefaultPool`].

use bevy_ecs::{component::ComponentId, intern::Interned, prelude::*};

pub use bevy_seedling_macros::PoolLabel;
use firewheel::nodes::sampler::SamplerConfig;

bevy_ecs::define_label!(
    /// A label for differentiating sample pools.
    ///
    /// When deriving [`PoolLabel`], you'll need to make sure your type implements
    /// a few additional traits.
    ///
    /// ```
    /// # use bevy_seedling::prelude::*;
    /// #[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
    /// struct MyPool;
    /// ```
    PoolLabel,
    POOL_LABEL_INTERNER
);

/// The default sample pool.
///
/// If no pool is specified when spawning a
/// [`SamplePlayer`] and no effects are applied,
/// this label will be inserted automatically.
///
/// [`SamplePlayer`]: crate::sample::SamplePlayer
///
/// You can customize the default sampler pool by setting
/// [`SeedlingPlugin::spawn_default_pool`][crate::prelude::SeedlingPlugin::spawn_default_pool]
/// to `false`, preventing the plugin from spawning it for you.
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_seedling::prelude::*;
///
/// fn main() {
///     App::default()
///         .add_plugins((
///             DefaultPlugins,
///             SeedlingPlugin {
///                 spawn_default_pool: false,
///                 ..Default::default()
///             },
///         ))
///         .add_systems(Startup, |mut commands: Commands| {
///             // Make the default pool provide spatial audio
///             commands.spawn((
///                 SamplerPool(DefaultPool),
///                 sample_effects![SpatialBasicNode::default()],
///             ));
///         })
///         .run();
/// }
/// ```
///
/// You can also simply re-route the default pool.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// fn reroute_default_pool(
///     pool: Single<Entity, (With<DefaultPool>, With<VolumeNode>)>,
///     mut commands: Commands,
/// ) {
///     // Let's splice in a send to a reverb node.
///     let reverb = commands.spawn(FreeverbNode::default()).id();
///
///     commands
///         .entity(*pool)
///         .disconnect(MainBus)
///         .chain_node(SendNode::new(Volume::Decibels(-12.0), reverb));
/// }
/// ```
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct DefaultPool;

/// A type-erased node label.
pub type InternedPoolLabel = Interned<dyn PoolLabel>;

/// A type-erased pool label container.
#[derive(Component, Debug, Clone)]
#[require(SamplerConfig)]
pub struct PoolLabelContainer {
    pub(crate) label: InternedPoolLabel,
    pub(crate) label_id: ComponentId,
}

impl PoolLabelContainer {
    /// Create a new interned pool label.
    pub fn new<T: PoolLabel>(label: &T, id: ComponentId) -> Self {
        Self {
            label: label.intern(),
            label_id: id,
        }
    }

    // TODO: make an issue -- this panics on 0.16
    // fn on_remove(mut world: DeferredWorld, context: HookContext) {
    //     let id = world
    //         .entity(context.entity)
    //         .components::<&PoolLabelContainer>()
    //         .label_id;
    //     world.commands().entity(context.entity).remove_by_id(id);
    // }
}
