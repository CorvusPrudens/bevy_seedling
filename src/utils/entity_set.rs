use std::iter::Copied;

use bevy_ecs::{
    entity::{Entity, EntityMapper, EntitySetIterator, MapEntities},
    relationship::RelationshipSourceCollection,
};

/// A thin wrapper around `std::vec::Vec<Entity>`.
///
/// This type guarantees that all elements are unique.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::TypePath))]
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

    fn assert_no_duplicates(&self) {
        assert!(
            !self.has_duplicates(),
            "`EntitySet` must not contain duplicate entities"
        )
    }
}

impl MapEntities for EntitySet {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        for entity in self.0.iter_mut() {
            *entity = entity_mapper.get_mapped(*entity);
        }

        self.assert_no_duplicates();
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

#[cfg(feature = "reflect")]
mod reflect {
    use super::*;
    use bevy_reflect::{FromReflect, PartialReflect, SetInfo, Typed};

    impl bevy_reflect::GetTypeRegistration for EntitySet {
        fn get_type_registration() -> bevy_reflect::TypeRegistration {
            let mut registration = bevy_reflect::TypeRegistration::of::<Self>();
            registration
            .insert::<bevy_reflect::ReflectFromPtr>(bevy_reflect::FromType::<Self>::from_type());
            registration.insert::<bevy_reflect::ReflectFromReflect>(
                bevy_reflect::FromType::<Self>::from_type(),
            );
            registration
        }

        #[inline(never)]
        fn register_type_dependencies(registry: &mut bevy_reflect::TypeRegistry) {
            <Entity as bevy_reflect::__macro_exports::RegisterForReflection>::__register(registry);
        }
    }

    impl Typed for EntitySet {
        #[inline]
        fn type_info() -> &'static bevy_reflect::TypeInfo {
            static CELL: bevy_reflect::utility::NonGenericTypeInfoCell =
                bevy_reflect::utility::NonGenericTypeInfoCell::new();
            CELL.get_or_set(|| bevy_reflect::TypeInfo::Set(SetInfo::new::<Self, Entity>()))
        }
    }

    impl bevy_reflect::Reflect for EntitySet {
        #[inline]
        fn into_any(self: Box<Self>) -> Box<dyn ::core::any::Any> {
            self
        }

        #[inline]
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }

        #[inline]
        fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
            self
        }

        #[inline]
        fn into_reflect(self: Box<Self>) -> Box<dyn bevy_reflect::Reflect> {
            self
        }

        #[inline]
        fn as_reflect(&self) -> &dyn bevy_reflect::Reflect {
            self
        }

        #[inline]
        fn as_reflect_mut(&mut self) -> &mut dyn bevy_reflect::Reflect {
            self
        }

        #[inline]
        fn set(
            &mut self,
            value: Box<dyn bevy_reflect::Reflect>,
        ) -> core::result::Result<(), Box<dyn bevy_reflect::Reflect>> {
            *self = <dyn bevy_reflect::Reflect>::take(value)?;

            Ok(())
        }
    }

    impl bevy_reflect::Set for EntitySet {
        fn len(&self) -> usize {
            self.0.len()
        }

        fn get(
            &self,
            value: &dyn bevy_reflect::PartialReflect,
        ) -> Option<&dyn bevy_reflect::PartialReflect> {
            self.0
                .as_slice()
                .iter()
                .find(|e| e.reflect_partial_eq(value).unwrap_or_default())
                .map(|e| e as &dyn PartialReflect)
        }

        fn is_empty(&self) -> bool {
            self.0.is_empty()
        }

        fn iter(&self) -> Box<dyn Iterator<Item = &dyn PartialReflect> + '_> {
            Box::new(self.0.as_slice().iter().map(|e| e as &dyn PartialReflect))
        }

        fn drain(&mut self) -> Vec<Box<dyn PartialReflect>> {
            self.0
                .drain(..)
                .map(|e| Box::new(e) as Box<dyn PartialReflect>)
                .collect()
        }

        fn retain(&mut self, f: &mut dyn FnMut(&dyn PartialReflect) -> bool) {
            self.0.retain(|e| f(e))
        }

        fn to_dynamic_set(&self) -> bevy_reflect::DynamicSet {
            let mut set = bevy_reflect::DynamicSet::default();
            set.set_represented_type(Some(Self::type_info()));

            for value in &self.0 {
                set.insert(*value);
            }

            set
        }

        fn insert_boxed(&mut self, value: Box<dyn PartialReflect>) -> bool {
            let Some(entity) = Entity::from_reflect(value.as_ref()) else {
                return false;
            };

            self.0.add(entity)
        }

        fn remove(&mut self, value: &dyn PartialReflect) -> bool {
            match self
                .0
                .as_slice()
                .iter()
                .position(|e| e.reflect_partial_eq(value).unwrap_or_default())
            {
                Some(index) => {
                    self.0.remove(index);
                    true
                }
                None => false,
            }
        }

        fn contains(&self, value: &dyn PartialReflect) -> bool {
            self.0
                .as_slice()
                .iter()
                .any(|e| e.reflect_partial_eq(value).unwrap_or_default())
        }
    }

    impl bevy_reflect::PartialReflect for EntitySet {
        #[inline]
        fn get_represented_type_info(&self) -> Option<&'static bevy_reflect::TypeInfo> {
            Some(Self::type_info())
        }

        #[inline]
        fn try_apply(
            &mut self,
            value: &dyn bevy_reflect::PartialReflect,
        ) -> core::result::Result<(), bevy_reflect::ApplyError> {
            let new_set =
                Self::from_reflect(value).ok_or(bevy_reflect::ApplyError::MismatchedKinds {
                    from_kind: value.reflect_kind(),
                    to_kind: self.reflect_kind(),
                })?;

            *self = new_set;
            Ok(())
        }

        #[inline]
        fn reflect_kind(&self) -> bevy_reflect::ReflectKind {
            bevy_reflect::ReflectKind::Set
        }

        #[inline]
        fn reflect_ref(&self) -> bevy_reflect::ReflectRef<'_> {
            bevy_reflect::ReflectRef::Set(self)
        }

        #[inline]
        fn reflect_mut(&mut self) -> bevy_reflect::ReflectMut<'_> {
            bevy_reflect::ReflectMut::Set(self)
        }

        #[inline]
        fn reflect_owned(self: Box<Self>) -> bevy_reflect::ReflectOwned {
            bevy_reflect::ReflectOwned::Set(self)
        }

        #[inline]
        fn try_into_reflect(
            self: Box<Self>,
        ) -> core::result::Result<
            Box<dyn bevy_reflect::Reflect>,
            Box<dyn bevy_reflect::PartialReflect>,
        > {
            Ok(self)
        }

        #[inline]
        fn try_as_reflect(&self) -> Option<&dyn bevy_reflect::Reflect> {
            Some(self)
        }

        #[inline]
        fn try_as_reflect_mut(&mut self) -> Option<&mut dyn bevy_reflect::Reflect> {
            Some(self)
        }

        #[inline]
        fn into_partial_reflect(self: Box<Self>) -> Box<dyn bevy_reflect::PartialReflect> {
            self
        }

        #[inline]
        fn as_partial_reflect(&self) -> &dyn bevy_reflect::PartialReflect {
            self
        }

        #[inline]
        fn as_partial_reflect_mut(&mut self) -> &mut dyn bevy_reflect::PartialReflect {
            self
        }

        fn reflect_partial_eq(&self, value: &dyn bevy_reflect::PartialReflect) -> Option<bool> {
            bevy_reflect::set_partial_eq(self, value)
        }

        #[inline]
        fn reflect_clone(
            &self,
        ) -> core::result::Result<Box<dyn bevy_reflect::Reflect>, bevy_reflect::ReflectCloneError>
        {
            Ok(Box::new(self.clone()))
        }
    }

    impl FromReflect for EntitySet {
        fn from_reflect(reflect: &dyn bevy_reflect::PartialReflect) -> Option<Self> {
            if let bevy_reflect::ReflectRef::TupleStruct(ref_struct) =
                bevy_reflect::PartialReflect::reflect_ref(reflect)
            {
                let set = Self(<_ as bevy_reflect::FromReflect>::from_reflect(
                    ref_struct.field(0)?,
                )?);
                set.assert_no_duplicates();

                Some(set)
            } else {
                None
            }
        }
    }
}
