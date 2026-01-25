//! Type-based sample pool labeling.
//!
//! `bevy_seedling` provides a single pool label, [`DefaultPool`].
//! Any node that doesn't provide an explicit pool when spawned
//! and has no effects will be automatically played in the [`DefaultPool`].

use bevy_ecs::{
    component::ComponentId, intern::Interned, lifecycle::HookContext, prelude::*,
    world::DeferredWorld,
};

pub use bevy_seedling_macros::PoolLabel;

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
/// [`SamplePlayer`] and any effects match those
/// on the default pool, this label will be inserted
/// automatically.
///
/// [`SamplePlayer`]: crate::sample::SamplePlayer
///
/// Depending on your [`GraphConfiguration`][crate::prelude::GraphConfiguration], you
/// can customize the default pool or even omit it entirely.
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
///                 graph_config: GraphConfiguration::Empty,
///                 ..Default::default()
///             },
///         ))
///         .add_systems(Startup, |mut commands: Commands| {
///             // Make the default pool provide spatial audio.
///             commands.spawn((
///                 SamplerPool(DefaultPool),
///                 sample_effects![SpatialBasicNode::default()],
///             ));
///
///             commands
///                 .spawn((MainBus, VolumeNode::default()))
///                 .connect(AudioGraphOutput);
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
///         .chain_node(SendNode::new(Volume::Decibels(-12.0), reverb))
///         .connect(SoundEffectsBus);
/// }
/// ```
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct DefaultPool;

/// A type-erased node label.
pub type InternedPoolLabel = Interned<dyn PoolLabel>;

/// A type-erased pool label container.
#[derive(Component, Debug, Clone)]
#[component(on_remove = Self::on_remove)]
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

    fn on_remove(mut world: DeferredWorld, context: HookContext) {
        let id = world
            .get::<PoolLabelContainer>(context.entity)
            .unwrap()
            .label_id;

        world.commands().queue(move |world: &mut World| {
            let Ok(mut entity) = world.get_entity_mut(context.entity) else {
                return;
            };
            entity.remove_by_id(id);
        });
    }
}

/// Insert a type-erased label container.
#[doc(hidden)]
pub fn insert_pool_label<L: PoolLabel + Component>(mut world: DeferredWorld, context: HookContext) {
    let value = world.get::<L>(context.entity).unwrap();
    let container = PoolLabelContainer::new(value, context.component_id);
    world.commands().entity(context.entity).insert(container);
}

/// Remove this label's associated type-erased label container.
#[doc(hidden)]
pub fn remove_pool_label<L: PoolLabel + Component>(mut world: DeferredWorld, context: HookContext) {
    world.commands().queue(move |world: &mut World| {
        let Ok(mut entity) = world.get_entity_mut(context.entity) else {
            return;
        };
        let Some(container) = entity.get::<PoolLabelContainer>() else {
            return;
        };

        if container.label_id == context.component_id {
            entity.remove::<PoolLabelContainer>();
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::prepare_app;

    #[derive(PoolLabel, Debug, PartialEq, Eq, Hash, Clone)]
    struct TestLabel;

    // These are simple test that just confirm the order of
    // hooks _and_ their queued effects works how this module
    // expects.
    #[test]
    fn test_label_removes_container() {
        let mut app = prepare_app(|| ());
        let world = app.world_mut();

        let entity = world.spawn(TestLabel).id();
        assert!(world.entity(entity).contains::<PoolLabelContainer>());
        world.commands().entity(entity).remove::<TestLabel>();
        world.flush();
        assert!(!world.entity(entity).contains::<PoolLabelContainer>());
    }

    #[test]
    fn test_no_spurious_label_remove() {
        let mut app = prepare_app(|| ());
        let world = app.world_mut();

        let entity = world.spawn(TestLabel).id();
        assert!(world.entity(entity).contains::<PoolLabelContainer>());
        world
            .commands()
            .entity(entity)
            .remove::<TestLabel>()
            .insert(TestLabel);
        world.flush();
        assert!(world.entity(entity).contains::<PoolLabelContainer>());
    }

    #[test]
    fn test_container_removes_label() {
        let mut app = prepare_app(|| ());
        let world = app.world_mut();

        let entity = world.spawn(TestLabel).id();
        assert!(world.entity(entity).contains::<PoolLabelContainer>());
        world
            .commands()
            .entity(entity)
            .remove::<PoolLabelContainer>();
        world.flush();
        assert!(!world.entity(entity).contains::<TestLabel>());
    }

    #[test]
    fn test_no_spurious_container_remove() {
        let mut app = prepare_app(|| ());
        let world = app.world_mut();

        let entity = world.spawn(TestLabel).id();
        assert!(world.entity(entity).contains::<PoolLabelContainer>());
        world
            .commands()
            .entity(entity)
            .remove::<PoolLabelContainer>()
            .insert(TestLabel);
        world.flush();
        assert!(world.entity(entity).contains::<PoolLabelContainer>());
    }
}
