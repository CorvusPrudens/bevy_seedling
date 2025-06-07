//! Audio sample components.

use crate::prelude::Volume;
use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};
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
#[require(PlaybackSettings, SamplePriority, SampleQueueLifetime)]
#[component(on_insert = on_insert_sample, immutable)]
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

fn on_insert_sample(mut world: DeferredWorld, context: HookContext) {
    world.commands().entity(context.entity).insert(QueuedSample);
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
pub struct PlaybackSettings {
    /// Sets the playback state, allowing you to play, pause or stop samples.
    ///
    /// This field provides only one-way communication with the
    /// audio processor. To get whether the sample is playing,
    /// see [`Sampler::is_playing`][crate::pool::Sampler::is_playing].
    pub playback: Notify<PlaybackState>,

    /// Sets the playhead.
    ///
    /// This field provides only one-way communication with the
    /// audio processor. To get the current value of the playhead,
    /// see [`Sampler::playhead_frames`][crate::pool::Sampler::playhead_frames].
    pub playhead: Notify<Playhead>,

    /// Sets the playback speed.
    pub speed: f64,

    /// Determines this sample's behavior on playback completion.
    pub on_complete: OnComplete,
}

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
        *self.playback = PlaybackState::Play { delay: None };
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
        *self.playhead = Playhead::default();
    }
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            playback: Notify::new(PlaybackState::Play { delay: None }),
            playhead: Notify::default(),
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
pub use random::PitchRange;

#[cfg(feature = "rand")]
pub(crate) use random::RandomPlugin;

#[cfg(feature = "rand")]
mod random {
    use super::PlaybackSettings;
    use bevy::{
        ecs::{component::HookContext, world::DeferredWorld},
        prelude::*,
    };
    use rand::{Rng, SeedableRng, rngs::SmallRng};

    pub struct RandomPlugin;

    impl Plugin for RandomPlugin {
        fn build(&self, app: &mut App) {
            app.insert_resource(PitchRng(SmallRng::from_entropy()));
        }
    }

    #[derive(Resource)]
    struct PitchRng(SmallRng);

    /// A component that applies a random pitch
    /// to a sample player when spawned.
    #[derive(Debug, Component, Default, Clone)]
    #[require(PlaybackSettings)]
    #[component(immutable, on_add = Self::on_add_hook)]
    pub struct PitchRange(pub core::ops::Range<f64>);

    impl PitchRange {
        fn on_add_hook(mut world: DeferredWorld, context: HookContext) {
            let range = world
                .get::<PitchRange>(context.entity)
                .expect("Entity should have a `PitchRange` component")
                .0
                .clone();

            let mut rng = world.resource_mut::<PitchRng>();
            let value = rng.0.gen_range(range);

            world
                .commands()
                .entity(context.entity)
                .entry::<PlaybackSettings>()
                .or_default()
                .and_modify(move |mut params| params.speed = value);
        }
    }
}
