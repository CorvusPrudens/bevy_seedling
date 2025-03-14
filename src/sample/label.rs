//! Type-based sample pool labeling.
//!
//! `bevy_seedling` provides a single pool label, [`DefaultPool`].
//! Any node that doesn't provide an explicit pool when spawned
//! will be automatically played in the [`DefaultPool`].
//!
//! You can customize the default sampler pool by preventing
//! automatic spawning.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_seedling::prelude::*;
//!
//! fn main() {
//!     App::default()
//!         .add_plugins((
//!             DefaultPlugins,
//!             SeedlingPlugin {
//!                 sample_pool_size: None,
//!                 ..Default::default()
//!             },
//!         ))
//!         .add_systems(
//!             Startup,
//!             |mut commands: Commands| {
//!                 // Make the default pool provide spatial audio
//!                 Pool::new(DefaultPool, 24)
//!                     .effect(SpatialBasicNode::default())
//!                     .spawn(&mut commands);
//!             }
//!         )
//!         .run();
//! }
//! ```

use bevy_ecs::{intern::Interned, prelude::*};

pub use seedling_macros::PoolLabel;

bevy_ecs::define_label!(
    /// A label for differentiating sample pools.
    PoolLabel,
    POOL_LABEL_INTERNER
);

/// The default sample pool.
///
/// If no pool is specified when spawning a
/// [`SamplePlayer`], this label will be inserted.
///
/// [`SamplePlayer`]: crate::sample::SamplePlayer
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefaultPool;

/// A type-erased node label.
pub type InternedPoolLabel = Interned<dyn PoolLabel>;

/// A type-erased pool label container.
#[derive(Component, Debug)]
#[allow(dead_code)]
pub struct PoolLabelContainer(InternedPoolLabel);

impl PoolLabelContainer {
    pub fn new<T: PoolLabel>(label: &T) -> Self {
        Self(label.intern())
    }
}
