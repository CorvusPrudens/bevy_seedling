//! Type-base sample pool labelling.
//!
//! `bevy_seedling` provides a single label, [MainBus],
//! which represents the terminal node that every other
//! node must eventually reach.
//!
//! Any node that doesn't provide an explicit connection when spawned
//! will be automatically connected to [MainBus].
use bevy_ecs::{intern::Interned, prelude::*};

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
#[derive(crate::PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
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
