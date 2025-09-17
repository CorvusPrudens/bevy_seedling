use std::iter::Copied;

use bevy_ecs::{
    entity::{Entity, EntityMapper, EntitySetIterator, MapEntities},
    relationship::RelationshipSourceCollection,
};

/// A thin wrapper around `std::vec::Vec<Entity>`.
///
/// This type guarantees that all elements are unique.
#[derive(Debug)]
pub struct EntitySet(Vec<Entity>);

impl EntitySet {
    fn has_duplicates(&self) -> bool {
        for i in 1..self.len() {
            if self[i..].contains(&self[i - 1]) {
                return true;
            }
        }
        false
    }
}

impl MapEntities for EntitySet {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        for entity in self.0.iter_mut() {
            *entity = entity_mapper.get_mapped(*entity);
        }

        // Now, verify all items are unique.
        if self.has_duplicates() {
            panic!("`EntityMapper` produced duplicate elements in `EffectsSet`");
        }
    }
}

impl core::ops::Deref for EntitySet {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RelationshipSourceCollection for EntitySet {
    type SourceIter<'a> = EntitySetIter<'a>;

    fn new() -> Self {
        EntitySet(Vec::new())
    }

    fn with_capacity(capacity: usize) -> Self {
        EntitySet(Vec::with_capacity(capacity))
    }

    fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    fn add(&mut self, entity: Entity) -> bool {
        if self.0.contains(&entity) {
            return false;
        }

        self.0.push(entity);
        true
    }

    fn remove(&mut self, entity: Entity) -> bool {
        if let Some(index) = self.iter().position(|e| e == entity) {
            self.0.remove(index);
            return true;
        }

        false
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        EntitySetIter {
            iter: Vec::iter(&self.0),
        }
    }

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        let entities = entities.into_iter();
        if let Some(size) = entities.size_hint().1 {
            self.0.reserve(size);
        }

        // This has O(n * m) time complexity.
        // For a large n or m, it may be better
        // to create a temporary hash set.
        for entity in entities {
            self.add(entity);
        }
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn clear(&mut self) {
        self.0.clear();
    }

    fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }
}

#[derive(Debug)]
pub struct EntitySetIter<'a> {
    iter: Copied<core::slice::Iter<'a, Entity>>,
}

impl Iterator for EntitySetIter<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl DoubleEndedIterator for EntitySetIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

/// # Safety
///
/// Because [`EntitySet`] cannot be mutated in any way
/// that will introduce duplicate elements, this must be safe.
unsafe impl EntitySetIterator for EntitySetIter<'_> {}
