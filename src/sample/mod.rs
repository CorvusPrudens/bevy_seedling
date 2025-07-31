//! Audio sample components.

use crate::prelude::Volume;
use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use firewheel::{
    diff::Notify,
    nodes::sampler::{PlaybackState, Playhead, RepeatMode},
};
use std::time::Duration;

mod assets;

pub use assets::{Sample, SampleLoader, SampleLoaderError};

/// A component that queues sample playback.
///
/// ## Playing sounds
///
/// Playing a sound is very simple!
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// fn play_sound(mut commands: Commands, server: Res<AssetServer>) {
///     commands.spawn(SamplePlayer::new(server.load("my_sample.wav")));
/// }
/// ```
///
/// This queues playback in a [`SamplerPool`][crate::prelude::SamplerPool].
/// When no effects are applied, samples are played in the
/// [`DefaultPool`][crate::prelude::DefaultPool].
///
/// The [`SamplePlayer`] component includes two fields that cannot change during
/// playback: `repeat_mode` and `volume`. Because [`SamplePlayer`] is immutable,
/// these can only be changed by re-inserting, which subsequently stops and restarts
/// playback. To update a sample's volume dynamically, consider adding a
/// [`VolumeNode`][crate::prelude::VolumeNode] as an effect.
///
/// ## Lifecycle
///
/// By default, entities with a [`SamplePlayer`] component are despawned when
/// playback completes. If you insert [`SamplePlayer`] components on gameplay entities
/// such as the player or enemies, you'll probably want to set [`PlaybackSettings::on_complete`]
/// to [`OnComplete::Remove`] or even [`OnComplete::Preserve`].
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// #[derive(Component)]
/// struct Player;
///
/// fn play_sound_on_player(
///     player: Single<Entity, With<Player>>,
///     server: Res<AssetServer>,
///     mut commands: Commands,
/// ) {
///     commands.entity(*player).insert((
///         SamplePlayer::new(server.load("my_sample.wav")),
///         PlaybackSettings {
///             on_complete: OnComplete::Remove,
///             ..Default::default()
///         },
///     ));
/// }
/// ```
///
/// ## Applying effects
///
/// Effects can be applied directly to a sample entity with
/// [`SampleEffects`][crate::prelude::SampleEffects].
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// fn play_with_effects(mut commands: Commands, server: Res<AssetServer>) {
///     commands.spawn((
///         SamplePlayer::new(server.load("my_sample.wav")),
///         sample_effects![
///             SpatialBasicNode::default(),
///             LowPassNode { frequency: 500.0 }
///         ],
///     ));
/// }
/// ```
///
/// In the above example, we connect a spatial and low-pass node in series with the sample player.
/// Effects are arranged in the order they're spawned, so the output of the spatial node is
/// connected to the input of the low-pass node.
///
/// When you apply effects to a sample player, the node components are added using the
/// [`SampleEffects`][crate::prelude::SampleEffects] relationships. If you want to access
/// the effects in terms of the sample they're applied to, you can break up your
/// queries and use the [`EffectsQuery`][crate::prelude::EffectsQuery] trait.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn play_sound(mut commands: Commands, server: Res<AssetServer>) {
/// commands.spawn((
///     // We'll look for sample player entities with the name "dynamic"
///     Name::new("dynamic"),
///     SamplePlayer::new(server.load("my_sample.wav")),
///     sample_effects![VolumeNode::default()],
/// ));
/// # }
///
/// fn update_volume(
///     sample_players: Query<(&Name, &SampleEffects)>,
///     mut volume: Query<&mut VolumeNode>,
/// ) -> Result {
///     for (name, effects) in &sample_players {
///         if name.as_str() == "dynamic" {
///             // Once we've found the target entity, we can get at
///             // its effects with `EffectsQuery`
///             volume.get_effect_mut(effects)?.volume = Volume::Decibels(-6.0);
///         }
///     }
///
///     Ok(())
/// }
/// ```
///
/// Applying effects directly to a [`SamplePlayer`] is simple, but it
/// [has some tradeoffs][crate::pool::dynamic#when-to-use-dynamic-pools], so you may
/// find yourself gravitating towards manually defined [`SamplerPool`][crate::prelude::SamplerPool]s as your
/// requirements grow.
///
/// ## Supporting components
///
/// A [`SamplePlayer`] can be spawned with a number of components:
/// - Any component that implements [`PoolLabel`][crate::prelude::PoolLabel]
/// - [`PlaybackSettings`]
/// - [`SamplePriority`]
/// - [`SampleQueueLifetime`]
/// - [`SampleEffects`][crate::prelude::SampleEffects]
///
/// Altogether, that would look like:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::{prelude::*, sample::SampleQueueLifetime};
/// # fn spatial_pool(mut commands: Commands, server: Res<AssetServer>) {
/// commands.spawn((
///     DefaultPool,
///     SamplePlayer {
///         sample: server.load("my_sample.wav"),
///         repeat_mode: RepeatMode::PlayOnce,
///         volume: Volume::UNITY_GAIN,
///     },
///     PlaybackSettings {
///         playback: Notify::new(PlaybackState::Play { delay: None }),
///         playhead: Notify::new(Playhead::Seconds(0.0)),
///         speed: 1.0,
///         on_complete: OnComplete::Remove,
///     },
///     SamplePriority(0),
///     SampleQueueLifetime(std::time::Duration::from_millis(100)),
///     sample_effects![SpatialBasicNode::default()],
/// ));
/// # }
/// ```
///
/// Once a sample has been queued in a pool, the [`Sampler`][crate::pool::Sampler] component
/// will be inserted, which provides information about the
/// playhead position and playback status.
#[derive(Debug, Component, Clone)]
#[require(PlaybackSettings, SamplePriority, SampleQueueLifetime, QueuedSample)]
#[component(immutable)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SamplePlayer {
    /// The sample to play.
    pub sample: Handle<Sample>,

    /// Sets the sample's [`RepeatMode`].
    ///
    /// Defaults to [`RepeatMode::PlayOnce`].
    ///
    /// The [`RepeatMode`] can only be configured once at the beginning of playback.
    pub repeat_mode: RepeatMode,

    /// Sets the volume of the sample.
    ///
    /// Defaults to [`Volume::UNITY_GAIN`].
    ///
    /// This volume can only be configured once at the beginning of playback.
    /// For dynamic volume, consider routing to buses or applying [`VolumeNode`]
    /// as an effect.
    ///
    /// [`VolumeNode`]: crate::prelude::VolumeNode
    pub volume: Volume,
}

impl Default for SamplePlayer {
    fn default() -> Self {
        Self {
            sample: Default::default(),
            repeat_mode: RepeatMode::PlayOnce,
            volume: Volume::UNITY_GAIN,
        }
    }
}

impl SamplePlayer {
    /// Construct a new [`SamplePlayer`].
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn play_sound(mut commands: Commands, server: Res<AssetServer>) {
    ///     commands.spawn(SamplePlayer::new(server.load("my_sample.wav")));
    /// }
    /// ```
    ///
    /// This immediately queues up the sample for playback.
    pub fn new(handle: Handle<Sample>) -> Self {
        Self {
            sample: handle,
            ..Default::default()
        }
    }

    /// Enable looping playback.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn play_sound(mut commands: Commands, server: Res<AssetServer>) {
    ///     commands.spawn(SamplePlayer::new(server.load("my_sample.wav")).looping());
    /// }
    /// ```
    ///
    /// Looping can only be configured once at the beginning of playback.
    pub fn looping(self) -> Self {
        Self {
            repeat_mode: RepeatMode::RepeatEndlessly,
            ..self
        }
    }

    /// Set the overall sample volume.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn play_sound(mut commands: Commands, server: Res<AssetServer>) {
    ///     commands.spawn(
    ///         SamplePlayer::new(server.load("my_sample.wav")).with_volume(Volume::Decibels(-6.0)),
    ///     );
    /// }
    /// ```
    ///
    /// This volume can only be configured once at the beginning of playback.
    /// For dynamic volume, consider routing to buses or applying [`VolumeNode`]
    /// as an effect.
    ///
    /// [`VolumeNode`]: crate::prelude::VolumeNode
    pub fn with_volume(self, volume: Volume) -> Self {
        Self { volume, ..self }
    }
}

/// Provide explicit priorities for samples.
///
/// Samples with higher priorities are queued before, and cannot
/// be interrupted by, those with lower priorities. This allows you
/// to confidently play music, stingers, and key sound effects even in
/// highly congested pools.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// # fn priority(mut commands: Commands, server: Res<AssetServer>) {
/// commands.spawn((
///     SamplePlayer::new(server.load("important_music.wav")).looping(),
///     // Ensure this sample is definitely played and without interruption
///     SamplePriority(10),
/// ));
/// # }
/// ```
#[derive(Debug, Default, Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[component(immutable)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SamplePriority(pub i32);

/// The maximum duration of time that a sample will wait for an available sampler.
///
/// The timer begins once the sample asset has loaded and after the sample player has been skipped
/// at least once. If the sample player is not queued for playback within this duration,
/// it will be considered to have completed playback.
///
/// The default lifetime is 100ms.
#[derive(Debug, Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[component(immutable)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SampleQueueLifetime(pub Duration);

impl Default for SampleQueueLifetime {
    fn default() -> Self {
        Self(Duration::from_millis(100))
    }
}

/// Determines what happens when a sample completes playback.
///
/// This will not trigger for looping samples unless they are stopped.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub enum OnComplete {
    /// Preserve the entity and components, leaving them untouched.
    Preserve,
    /// Remove the [`SamplePlayer`] and related components.
    Remove,
    /// Despawn the [`SamplePlayer`] entity.
    ///
    /// Since spawning sounds as their own isolated entity is so
    /// common, this is the default.
    #[default]
    Despawn,
}

/// Sample parameters that can change during playback.
///
/// These parameters will apply to samples immediately, so
/// you can choose to begin playback wherever you'd like,
/// or even start with the sample paused.
///
/// ```
/// # use bevy_seedling::prelude::*;
/// # use bevy::prelude::*;
/// fn play_with_params(mut commands: Commands, server: Res<AssetServer>) {
///     commands.spawn((
///         SamplePlayer::new(server.load("my_sample.wav")),
///         // You can start one second in
///         PlaybackSettings {
///             playhead: Notify::new(Playhead::Seconds(1.0)),
///             ..Default::default()
///         },
///     ));
///
///     commands.spawn((
///         SamplePlayer::new(server.load("my_sample.wav")),
///         // Or even spawn with paused playback
///         PlaybackSettings {
///             playback: Notify::new(PlaybackState::Pause),
///             ..Default::default()
///         },
///     ));
/// }
/// ```
#[derive(Component, Debug)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct PlaybackSettings {
    /// Sets the playback state, allowing you to play, pause or stop samples.
    ///
    /// This field provides only one-way communication with the
    /// audio processor. To get whether the sample is playing,
    /// see [`Sampler::is_playing`][crate::pool::Sampler::is_playing].
    pub playback: Notify<PlaybackState>,

    /// Sets the playback speed.
    pub speed: f64,

    /// Determines this sample's behavior on playback completion.
    pub on_complete: OnComplete,
}

// const _: () = {
//     #[allow(unused_mut)]
//     impl<T> bevy_reflect::GetTypeRegistration for Notify2<T>
//     where
//         Notify2<T>: ::core::any::Any + ::core::marker::Send + ::core::marker::Sync,
//         T: bevy_reflect::FromReflect
//             + bevy_reflect::TypePath
//             + bevy_reflect::MaybeTyped
//             + bevy_reflect::__macro_exports::RegisterForReflection,
//     {
//         fn get_type_registration() -> bevy_reflect::TypeRegistration {
//             let mut registration = bevy_reflect::TypeRegistration::of::<Self>();
//             registration.insert:: <bevy_reflect::ReflectFromPtr>(bevy_reflect::FromType:: <Self> ::from_type());
//             registration.insert::<bevy_reflect::ReflectFromReflect>(
//                 bevy_reflect::FromType::<Self>::from_type(),
//             );
//             registration
//         }
//         #[inline(never)]
//         fn register_type_dependencies(registry: &mut bevy_reflect::TypeRegistry) {
//             <T as bevy_reflect::__macro_exports::RegisterForReflection>::__register(registry);
//             <u64 as bevy_reflect::__macro_exports::RegisterForReflection>::__register(registry);
//         }
//     }
//     impl<T> bevy_reflect::Typed for Notify2<T>
//     where
//         Notify2<T>: ::core::any::Any + ::core::marker::Send + ::core::marker::Sync,
//         T: bevy_reflect::FromReflect
//             + bevy_reflect::TypePath
//             + bevy_reflect::MaybeTyped
//             + bevy_reflect::__macro_exports::RegisterForReflection,
//     {
//         #[inline]
//         fn type_info() -> &'static bevy_reflect::TypeInfo {
//             static CELL: bevy_reflect::utility::GenericTypeInfoCell =
//                 bevy_reflect::utility::GenericTypeInfoCell::new();
//             CELL.get_or_insert::<Self, _>(|| {
//                 bevy_reflect::TypeInfo::Struct(
//                     bevy_reflect::StructInfo::new::<Self>(&[
//                         bevy_reflect::NamedField::new::<T>("value").with_custom_attributes(
//                             bevy_reflect::attributes::CustomAttributes::default(),
//                         ),
//                         bevy_reflect::NamedField::new::<u64>("counter").with_custom_attributes(
//                             bevy_reflect::attributes::CustomAttributes::default(),
//                         ),
//                     ])
//                     .with_custom_attributes(bevy_reflect::attributes::CustomAttributes::default())
//                     .with_generics(bevy_reflect::Generics::from_iter([
//                         bevy_reflect::GenericInfo::Type(bevy_reflect::TypeParamInfo::new::<T>(
//                             bevy_reflect::__macro_exports::alloc_utils::Cow::Borrowed("T"),
//                         )),
//                     ])),
//                 )
//             })
//         }
//     }
//     const _: () = {
//         extern crate alloc;
//         impl<T> bevy_reflect::TypePath for Notify2<T>
//         where
//             Notify2<T>: ::core::any::Any + ::core::marker::Send + ::core::marker::Sync,
//             T: bevy_reflect::TypePath,
//         {
//             fn type_path() -> &'static str {
//                 static CELL: bevy_reflect::utility::GenericTypePathCell =
//                     bevy_reflect::utility::GenericTypePathCell::new();
//                 CELL.get_or_insert::<Self, _>(|| {
//                     ::core::ops::Add::<&str>::add(
//                         ::core::ops::Add::<&str>::add(
//                             bevy_reflect::__macro_exports::alloc_utils::ToString::to_string(
//                                 ::core::concat!(
//                                     ::core::concat!(
//                                         ::core::concat!(::core::module_path!(), "::"),
//                                         "Notify2"
//                                     ),
//                                     "<"
//                                 ),
//                             ),
//                             <T as bevy_reflect::TypePath>::type_path(),
//                         ),
//                         ">",
//                     )
//                 })
//             }
//             fn short_type_path() -> &'static str {
//                 static CELL: bevy_reflect::utility::GenericTypePathCell =
//                     bevy_reflect::utility::GenericTypePathCell::new();
//                 CELL.get_or_insert::<Self, _>(|| {
//                     ::core::ops::Add::<&str>::add(
//                         ::core::ops::Add::<&str>::add(
//                             bevy_reflect::__macro_exports::alloc_utils::ToString::to_string(
//                                 ::core::concat!("Notify2", "<"),
//                             ),
//                             <T as bevy_reflect::TypePath>::short_type_path(),
//                         ),
//                         ">",
//                     )
//                 })
//             }
//             fn type_ident() -> Option<&'static str> {
//                 ::core::option::Option::Some("Notify2")
//             }
//             fn crate_name() -> Option<&'static str> {
//                 ::core::option::Option::Some(::core::module_path!().split(':').next().unwrap())
//             }
//             fn module_path() -> Option<&'static str> {
//                 ::core::option::Option::Some(::core::module_path!())
//             }
//         }
//     };
//     impl<T> bevy_reflect::Reflect for Notify2<T>
//     where
//         Notify2<T>: ::core::any::Any + ::core::marker::Send + ::core::marker::Sync,
//         T: bevy_reflect::TypePath,
//         T: bevy_reflect::FromReflect
//             + bevy_reflect::TypePath
//             + bevy_reflect::MaybeTyped
//             + bevy_reflect::__macro_exports::RegisterForReflection,
//         u64: bevy_reflect::FromReflect
//             + bevy_reflect::TypePath
//             + bevy_reflect::MaybeTyped
//             + bevy_reflect::__macro_exports::RegisterForReflection,
//     {
//         #[inline]
//         fn into_any(
//             self: bevy_reflect::__macro_exports::alloc_utils::Box<Self>,
//         ) -> bevy_reflect::__macro_exports::alloc_utils::Box<dyn ::core::any::Any> {
//             self
//         }
//         #[inline]
//         fn as_any(&self) -> &dyn ::core::any::Any {
//             self
//         }
//         #[inline]
//         fn as_any_mut(&mut self) -> &mut dyn ::core::any::Any {
//             self
//         }
//         #[inline]
//         fn into_reflect(
//             self: bevy_reflect::__macro_exports::alloc_utils::Box<Self>,
//         ) -> bevy_reflect::__macro_exports::alloc_utils::Box<dyn bevy_reflect::Reflect> {
//             self
//         }
//         #[inline]
//         fn as_reflect(&self) -> &dyn bevy_reflect::Reflect {
//             self
//         }
//         #[inline]
//         fn as_reflect_mut(&mut self) -> &mut dyn bevy_reflect::Reflect {
//             self
//         }
//         #[inline]
//         fn set(
//             &mut self,
//             value: bevy_reflect::__macro_exports::alloc_utils::Box<dyn bevy_reflect::Reflect>,
//         ) -> ::core::result::Result<
//             (),
//             bevy_reflect::__macro_exports::alloc_utils::Box<dyn bevy_reflect::Reflect>,
//         > {
//             *self = <dyn bevy_reflect::Reflect>::take(value)?;
//             ::core::result::Result::Ok(())
//         }
//     }
//     impl<T> bevy_reflect::Struct for Notify2<T>
//     where
//         Notify2<T>: ::core::any::Any + ::core::marker::Send + ::core::marker::Sync,
//         T: bevy_reflect::FromReflect
//             + bevy_reflect::TypePath
//             + bevy_reflect::MaybeTyped
//             + bevy_reflect::__macro_exports::RegisterForReflection,
//     {
//         fn field(&self, name: &str) -> ::core::option::Option<&dyn bevy_reflect::PartialReflect> {
//             match name {
//                 "value" => ::core::option::Option::Some(&self.value),
//                 "counter" => ::core::option::Option::Some(&self.counter),
//                 _ => ::core::option::Option::None,
//             }
//         }
//         fn field_mut(
//             &mut self,
//             name: &str,
//         ) -> ::core::option::Option<&mut dyn bevy_reflect::PartialReflect> {
//             match name {
//                 "value" => ::core::option::Option::Some(&mut self.value),
//                 "counter" => ::core::option::Option::Some(&mut self.counter),
//                 _ => ::core::option::Option::None,
//             }
//         }
//         fn field_at(
//             &self,
//             index: usize,
//         ) -> ::core::option::Option<&dyn bevy_reflect::PartialReflect> {
//             match index {
//                 0usize => ::core::option::Option::Some(&self.value),
//                 1usize => ::core::option::Option::Some(&self.counter),
//                 _ => ::core::option::Option::None,
//             }
//         }
//         fn field_at_mut(
//             &mut self,
//             index: usize,
//         ) -> ::core::option::Option<&mut dyn bevy_reflect::PartialReflect> {
//             match index {
//                 0usize => ::core::option::Option::Some(&mut self.value),
//                 1usize => ::core::option::Option::Some(&mut self.counter),
//                 _ => ::core::option::Option::None,
//             }
//         }
//         fn name_at(&self, index: usize) -> ::core::option::Option<&str> {
//             match index {
//                 0usize => ::core::option::Option::Some("value"),
//                 1usize => ::core::option::Option::Some("counter"),
//                 _ => ::core::option::Option::None,
//             }
//         }
//         fn field_len(&self) -> usize {
//             2usize
//         }
//         fn iter_fields(&self) -> bevy_reflect::FieldIter {
//             bevy_reflect::FieldIter::new(self)
//         }
//         fn to_dynamic_struct(&self) -> bevy_reflect::DynamicStruct {
//             let mut dynamic: bevy_reflect::DynamicStruct = ::core::default::Default::default();
//             dynamic.set_represented_type(bevy_reflect::PartialReflect::get_represented_type_info(
//                 self,
//             ));
//             dynamic.insert_boxed(
//                 "value",
//                 bevy_reflect::PartialReflect::to_dynamic(&self.value),
//             );
//             dynamic.insert_boxed(
//                 "counter",
//                 bevy_reflect::PartialReflect::to_dynamic(&self.counter),
//             );
//             dynamic
//         }
//     }
//     impl<T> bevy_reflect::PartialReflect for Notify2<T>
//     where
//         Notify2<T>: ::core::any::Any + ::core::marker::Send + ::core::marker::Sync,
//         T: bevy_reflect::FromReflect
//             + bevy_reflect::TypePath
//             + bevy_reflect::MaybeTyped
//             + bevy_reflect::__macro_exports::RegisterForReflection,
//     {
//         #[inline]
//         fn get_represented_type_info(
//             &self,
//         ) -> ::core::option::Option<&'static bevy_reflect::TypeInfo> {
//             ::core::option::Option::Some(<Self as bevy_reflect::Typed>::type_info())
//         }
//         #[inline]
//         fn try_apply(
//             &mut self,
//             value: &dyn bevy_reflect::PartialReflect,
//         ) -> ::core::result::Result<(), bevy_reflect::ApplyError> {
//             if let bevy_reflect::ReflectRef::Struct(struct_value) =
//                 bevy_reflect::PartialReflect::reflect_ref(value)
//             {
//                 for (i, value) in ::core::iter::Iterator::enumerate(
//                     bevy_reflect::Struct::iter_fields(struct_value),
//                 ) {
//                     let name = bevy_reflect::Struct::name_at(struct_value, i).unwrap();
//                     if let ::core::option::Option::Some(v) =
//                         bevy_reflect::Struct::field_mut(self, name)
//                     {
//                         bevy_reflect::PartialReflect::try_apply(v, value)?;
//                     }
//                 }
//             } else {
//                 return ::core::result::Result::Err(bevy_reflect::ApplyError::MismatchedKinds {
//                     from_kind: bevy_reflect::PartialReflect::reflect_kind(value),
//                     to_kind: bevy_reflect::ReflectKind::Struct,
//                 });
//             }
//             ::core::result::Result::Ok(())
//         }
//         #[inline]
//         fn reflect_kind(&self) -> bevy_reflect::ReflectKind {
//             bevy_reflect::ReflectKind::Struct
//         }
//         #[inline]
//         fn reflect_ref(&self) -> bevy_reflect::ReflectRef {
//             bevy_reflect::ReflectRef::Struct(self)
//         }
//         #[inline]
//         fn reflect_mut(&mut self) -> bevy_reflect::ReflectMut {
//             bevy_reflect::ReflectMut::Struct(self)
//         }
//         #[inline]
//         fn reflect_owned(
//             self: bevy_reflect::__macro_exports::alloc_utils::Box<Self>,
//         ) -> bevy_reflect::ReflectOwned {
//             bevy_reflect::ReflectOwned::Struct(self)
//         }
//         #[inline]
//         fn try_into_reflect(
//             self: bevy_reflect::__macro_exports::alloc_utils::Box<Self>,
//         ) -> ::core::result::Result<
//             bevy_reflect::__macro_exports::alloc_utils::Box<dyn bevy_reflect::Reflect>,
//             bevy_reflect::__macro_exports::alloc_utils::Box<dyn bevy_reflect::PartialReflect>,
//         > {
//             ::core::result::Result::Ok(self)
//         }
//         #[inline]
//         fn try_as_reflect(&self) -> ::core::option::Option<&dyn bevy_reflect::Reflect> {
//             ::core::option::Option::Some(self)
//         }
//         #[inline]
//         fn try_as_reflect_mut(&mut self) -> ::core::option::Option<&mut dyn bevy_reflect::Reflect> {
//             ::core::option::Option::Some(self)
//         }
//         #[inline]
//         fn into_partial_reflect(
//             self: bevy_reflect::__macro_exports::alloc_utils::Box<Self>,
//         ) -> bevy_reflect::__macro_exports::alloc_utils::Box<dyn bevy_reflect::PartialReflect>
//         {
//             self
//         }
//         #[inline]
//         fn as_partial_reflect(&self) -> &dyn bevy_reflect::PartialReflect {
//             self
//         }
//         #[inline]
//         fn as_partial_reflect_mut(&mut self) -> &mut dyn bevy_reflect::PartialReflect {
//             self
//         }
//         fn reflect_partial_eq(
//             &self,
//             value: &dyn bevy_reflect::PartialReflect,
//         ) -> ::core::option::Option<bool> {
//             (bevy_reflect::struct_partial_eq)(self, value)
//         }
//         #[inline]
//         #[allow(
//             unreachable_code,
//             reason = "Ignored fields without a `clone` attribute will early-return with an error"
//         )]
//         fn reflect_clone(
//             &self,
//         ) -> ::core::result::Result<
//             bevy_reflect::__macro_exports::alloc_utils::Box<dyn bevy_reflect::Reflect>,
//             bevy_reflect::ReflectCloneError,
//         > {
//             ::core::result::Result::Ok(bevy_reflect::__macro_exports::alloc_utils::Box::new(Self {
//                 value: bevy_reflect::PartialReflect::reflect_clone(&self.value)?
//                     .take()
//                     .map_err(|value| bevy_reflect::ReflectCloneError::FailedDowncast {
//                         expected: bevy_reflect::__macro_exports::alloc_utils::Cow::Borrowed(
//                             <T as bevy_reflect::TypePath>::type_path(),
//                         ),
//                         received: bevy_reflect::__macro_exports::alloc_utils::Cow::Owned(
//                             bevy_reflect::__macro_exports::alloc_utils::ToString::to_string(
//                                 bevy_reflect::DynamicTypePath::reflect_type_path(&*value),
//                             ),
//                         ),
//                     })?,
//                 counter: bevy_reflect::PartialReflect::reflect_clone(&self.counter)?
//                     .take()
//                     .map_err(|value| bevy_reflect::ReflectCloneError::FailedDowncast {
//                         expected: bevy_reflect::__macro_exports::alloc_utils::Cow::Borrowed(
//                             <u64 as bevy_reflect::TypePath>::type_path(),
//                         ),
//                         received: bevy_reflect::__macro_exports::alloc_utils::Cow::Owned(
//                             bevy_reflect::__macro_exports::alloc_utils::ToString::to_string(
//                                 bevy_reflect::DynamicTypePath::reflect_type_path(&*value),
//                             ),
//                         ),
//                     })?,
//             }))
//         }
//     }
//     impl<T> bevy_reflect::FromReflect for Notify2<T>
//     where
//         Notify2<T>: ::core::any::Any + ::core::marker::Send + ::core::marker::Sync,
//         T: bevy_reflect::FromReflect
//             + bevy_reflect::TypePath
//             + bevy_reflect::MaybeTyped
//             + bevy_reflect::__macro_exports::RegisterForReflection,
//     {
//         fn from_reflect(
//             reflect: &dyn bevy_reflect::PartialReflect,
//         ) -> ::core::option::Option<Self> {
//             if let bevy_reflect::ReflectRef::Struct(__ref_struct) =
//                 bevy_reflect::PartialReflect::reflect_ref(reflect)
//             {
//                 let __this = Self {
//                     value: (|| {
//                         <T as bevy_reflect::FromReflect>::from_reflect(bevy_reflect::Struct::field(
//                             __ref_struct,
//                             "value",
//                         )?)
//                     })()?,
//                     counter: (|| {
//                         <u64 as bevy_reflect::FromReflect>::from_reflect(
//                             bevy_reflect::Struct::field(__ref_struct, "counter")?,
//                         )
//                     })()?,
//                 };
//                 ::core::option::Option::Some(__this)
//             } else {
//                 ::core::option::Option::None
//             }
//         }
//     }
// };

impl PlaybackSettings {
    /// Start or resume playback.
    ///
    /// ```
    /// # use bevy_seedling::prelude::*;
    /// # use bevy::prelude::*;
    /// fn resume_paused_samples(mut samples: Query<&mut PlaybackSettings>) {
    ///     for mut params in samples.iter_mut() {
    ///         if matches!(*params.playback, PlaybackState::Pause) {
    ///             params.play();
    ///         }
    ///     }
    /// }
    /// ```
    pub fn play(&mut self) {
        *self.playback = PlaybackState::Play {
            playhead: Some(Playhead::Seconds(0.0)),
        };
    }

    /// Pause playback.
    ///
    /// ```
    /// # use bevy_seedling::prelude::*;
    /// # use bevy::prelude::*;
    /// fn pause_all_samples(mut samples: Query<&mut PlaybackSettings>) {
    ///     for mut params in samples.iter_mut() {
    ///         params.pause();
    ///     }
    /// }
    /// ```
    pub fn pause(&mut self) {
        *self.playback = PlaybackState::Pause;
    }

    /// Stop playback, resetting the playhead to the start.
    ///
    /// ```
    /// # use bevy_seedling::prelude::*;
    /// # use bevy::prelude::*;
    /// fn stop_all_samples(mut samples: Query<&mut PlaybackSettings>) {
    ///     for mut params in samples.iter_mut() {
    ///         params.stop();
    ///     }
    /// }
    /// ```
    pub fn stop(&mut self) {
        *self.playback = PlaybackState::Stop;
    }
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            playback: Notify::new(PlaybackState::Play {
                playhead: Some(Playhead::Seconds(0.0)),
            }),
            speed: 1.0,
            on_complete: OnComplete::Despawn,
        }
    }
}

/// A marker struct for entities that are waiting
/// for asset loading and playback assignment.
#[derive(Debug, Component, Default)]
#[component(storage = "SparseSet")]
pub struct QueuedSample;

#[cfg(feature = "rand")]
pub use random::{PitchRngSource, RandomPitch};

#[cfg(feature = "rand")]
pub(crate) use random::RandomPlugin;

#[cfg(feature = "rand")]
mod random {
    use crate::SeedlingSystems;

    use super::PlaybackSettings;
    use bevy_app::prelude::*;
    use bevy_ecs::prelude::*;
    use rand::{SeedableRng, rngs::SmallRng};

    pub struct RandomPlugin;

    impl Plugin for RandomPlugin {
        fn build(&self, app: &mut App) {
            app.insert_resource(PitchRngSource::new(SmallRng::from_entropy()))
                .add_systems(Last, RandomPitch::apply.before(SeedlingSystems::Acquire));
        }
    }

    trait PitchRng {
        fn gen_pitch(&mut self, range: std::ops::Range<f64>) -> f64;
    }

    struct RandRng<T>(T);

    impl<T: rand::Rng> PitchRng for RandRng<T> {
        fn gen_pitch(&mut self, range: std::ops::Range<f64>) -> f64 {
            self.0.gen_range(range)
        }
    }

    #[derive(Resource)]
    pub struct PitchRngSource(Box<dyn PitchRng + Send + Sync>);

    impl PitchRngSource {
        pub fn new<T: rand::Rng + Send + Sync + 'static>(rng: T) -> Self {
            Self(Box::new(RandRng(rng)))
        }
    }

    /// A component that applies a random pitch
    /// to a sample player when spawned.
    #[derive(Debug, Component, Default, Clone)]
    #[require(PlaybackSettings)]
    #[component(immutable)]
    #[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
    pub struct RandomPitch(pub core::ops::Range<f64>);

    impl RandomPitch {
        /// Create a new [`PitchRange`] with deviation about 1.0.
        ///
        /// ```
        /// # use bevy::prelude::*;
        /// # use bevy_seedling::prelude::*;
        /// # fn deviation(mut commands: Commands, server: Res<AssetServer>) {
        /// commands.spawn((
        ///     SamplePlayer::new(server.load("my_sample.wav")),
        ///     PitchRange::new(0.05),
        /// ));
        /// # }
        /// ```
        pub fn new(deviation: f64) -> Self {
            let minimum = (1.0 - deviation).clamp(0.0, f64::MAX);
            let maximum = (1.0 + deviation).clamp(0.0, f64::MAX);

            Self(minimum..maximum)
        }

        fn apply(
            mut samples: Query<(Entity, &mut PlaybackSettings, &Self)>,
            mut commands: Commands,
            mut rng: ResMut<PitchRngSource>,
        ) {
            for (entity, mut settings, range) in samples.iter_mut() {
                settings.speed = rng.0.gen_pitch(range.0.clone());
                commands.entity(entity).remove::<Self>();
            }
        }
    }
}
