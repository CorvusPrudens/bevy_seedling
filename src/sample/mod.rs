//! Audio sample components.

use crate::{
    node::DiffTimestamp,
    prelude::{AudioEvents, Volume},
    time::Audio,
};
use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_math::FloatExt;
use firewheel::{
    clock::{DurationSeconds, InstantSeconds},
    diff::Notify,
    nodes::sampler::{PlaybackState, Playhead, RepeatMode},
};
use std::time::Duration;

mod assets;

pub use assets::{AudioSample, SampleLoader, SampleLoaderError};

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
///         PlaybackSettings::default().remove(),
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
///         playback: Notify::new(PlaybackState::Play {
///             playhead: Some(Playhead::Seconds(0.0)),
///         }),
///         speed: 1.0,
///         on_complete: OnComplete::Despawn,
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
    pub sample: Handle<AudioSample>,

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
    pub fn new(handle: Handle<AudioSample>) -> Self {
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

/// We use this to, by default, ensure samples play "when they should."
///
/// In practice, this means that samples which take a long time to load
/// will skip the time spent loading. If you notice this in your game,
/// consider preloading your sound assets, or simply disable all
/// automatic scheduling with [`ScheduleDiffing`][crate::node::ScheduleDiffing].
pub(super) fn observe_player_insert(
    player: Trigger<OnInsert, SamplePlayer>,
    time: Res<bevy_time::Time<Audio>>,
    mut commands: Commands,
) {
    commands
        .entity(player.target())
        .insert(DiffTimestamp::new(&time))
        .insert_if_new(AudioEvents::new(&time));
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
///         PlaybackSettings::default().with_playback(PlaybackState::Play {
///             playhead: Some(Playhead::Seconds(1.0)),
///         }),
///     ));
///
///     commands.spawn((
///         SamplePlayer::new(server.load("my_sample.wav")),
///         // Or even spawn with paused playback
///         PlaybackSettings::default().with_playback(PlaybackState::Pause),
///     ));
/// }
/// ```
#[derive(Component, Debug, Clone)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct PlaybackSettings {
    /// Sets the playback state, allowing you to play, pause or stop samples.
    ///
    /// This field provides only one-way communication with the
    /// audio processor. To get whether the sample is playing,
    /// see [`Sampler::is_playing`][crate::pool::Sampler::is_playing].
    pub playback: Notify<PlaybackState>,

    /// Sets the playback speed.
    ///
    /// This is a factor, meaning `1.0` is normal speed, `2.0` is twice
    /// as fast, and `0.5` is half as fast.
    ///
    /// The speed of a sample is also inherently linked to its pitch. A
    /// sample played twice as fast will sound an octave higher
    /// (i.e. a fair bit higher-pitched). This can be a relatively cheap way
    /// to break up the monotony of repeated sounds. The [`RandomPitch`]
    /// component is an easy way to get started with this technique.
    pub speed: f64,

    /// Determines this sample's behavior on playback completion.
    pub on_complete: OnComplete,
}

impl PlaybackSettings {
    /// Set the [`PlaybackState`].
    pub fn with_playback(self, playback: PlaybackState) -> Self {
        Self {
            playback: Notify::new(playback),
            ..self
        }
    }

    /// Set the sample speed.
    pub fn with_speed(self, speed: f64) -> Self {
        Self { speed, ..self }
    }

    /// Set the [`OnComplete`] behavior.
    pub fn with_on_complete(self, on_complete: OnComplete) -> Self {
        Self {
            on_complete,
            ..self
        }
    }

    /// Set [`PlaybackSettings::on_complete`] to [`OnComplete::Preserve`].
    pub fn preserve(self) -> Self {
        Self {
            on_complete: OnComplete::Preserve,
            ..self
        }
    }

    /// Set [`PlaybackSettings::on_complete`] to [`OnComplete::Remove`].
    pub fn remove(self) -> Self {
        Self {
            on_complete: OnComplete::Remove,
            ..self
        }
    }

    /// Set [`PlaybackSettings::on_complete`] to [`OnComplete::Despawn`].
    ///
    /// Note that this is the default value.
    pub fn despawn(self) -> Self {
        Self {
            on_complete: OnComplete::Despawn,
            ..self
        }
    }

    /// Begin playing a sample at `time`.
    ///
    /// This can also be used to seek within a playing
    /// sample by providing a [`Playhead`].
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn play_at(time: Res<Time<Audio>>, server: Res<AssetServer>, mut commands: Commands) {
    ///     let mut events = AudioEvents::new(&time);
    ///     let settings = PlaybackSettings::default().with_playback(PlaybackState::Pause);
    ///
    ///     // Start playing exactly one second from now.
    ///     settings.play_at(None, time.delay(DurationSeconds(1.0)), &mut events);
    ///
    ///     commands.spawn((
    ///         events,
    ///         settings,
    ///         SamplePlayer::new(server.load("my_sample.wav")),
    ///     ));
    /// }
    /// ```
    pub fn play_at(
        &self,
        playhead: Option<Playhead>,
        time: InstantSeconds,
        events: &mut AudioEvents,
    ) {
        events.schedule(time, self, |settings| {
            *settings.playback = PlaybackState::Play { playhead };
        });
    }

    /// Pause a sample at `time`.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn pause(time: Res<Time<Audio>>, server: Res<AssetServer>, mut commands: Commands) {
    ///     let mut events = AudioEvents::new(&time);
    ///     let settings = PlaybackSettings::default();
    ///
    ///     // Allow the sample to start playing, but pause at exactly
    ///     // one second from now.
    ///     settings.pause_at(time.delay(DurationSeconds(1.0)), &mut events);
    ///
    ///     commands.spawn((
    ///         events,
    ///         settings,
    ///         SamplePlayer::new(server.load("my_sample.wav")),
    ///     ));
    /// }
    /// ```
    pub fn pause_at(&self, time: InstantSeconds, events: &mut AudioEvents) {
        events.schedule(time, self, |settings| {
            *settings.playback = PlaybackState::Pause;
        });
    }

    /// Stop a sample at `time`.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn stop_at(time: Res<Time<Audio>>, server: Res<AssetServer>, mut commands: Commands) {
    ///     let mut events = AudioEvents::new(&time);
    ///     let settings = PlaybackSettings::default();
    ///
    ///     // Allow the sample to start playing, but stop at exactly
    ///     // one second from now.
    ///     settings.stop_at(time.delay(DurationSeconds(1.0)), &mut events);
    ///
    ///     commands.spawn((
    ///         events,
    ///         settings,
    ///         SamplePlayer::new(server.load("my_sample.wav")),
    ///     ));
    /// }
    /// ```
    pub fn stop_at(&self, time: InstantSeconds, events: &mut AudioEvents) {
        events.schedule(time, self, |settings| {
            *settings.playback = PlaybackState::Stop;
        });
    }

    /// Linearly interpolate a sample's speed from its current value to `speed`.
    ///
    /// The interpolation uses an approximation of the average just noticeable
    /// different (JND) for pitch to calculate how many events are required to
    /// sound perfectly smooth. Since we are sensitive to changes in pitch,
    /// this will usually generate many more events than volume animation.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn speed_to(time: Res<Time<Audio>>, server: Res<AssetServer>, mut commands: Commands) {
    ///     let mut events = AudioEvents::new(&time);
    ///     let settings = PlaybackSettings::default();
    ///
    ///     // As soon as the sample starts playing, slow it down to half its
    ///     // speed over one second.
    ///     settings.speed_to(0.5, DurationSeconds(1.0), &mut events);
    ///
    ///     commands.spawn((
    ///         events,
    ///         settings,
    ///         SamplePlayer::new(server.load("my_sample.wav")),
    ///     ));
    /// }
    /// ```
    pub fn speed_to(&self, speed: f64, duration: DurationSeconds, events: &mut AudioEvents) {
        self.speed_at(speed, events.now(), events.now() + duration, events)
    }

    /// Linearly interpolate a sample's speed from its value at `start` to `speed`.
    ///
    /// The interpolation uses an approximation of the average just noticeable
    /// different (JND) for pitch to calculate how many events are required to
    /// sound perfectly smooth. Since we are sensitive to changes in pitch,
    /// this will usually generate many more events than volume animation.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn speed_at(time: Res<Time<Audio>>, server: Res<AssetServer>, mut commands: Commands) {
    ///     let mut events = AudioEvents::new(&time);
    ///     let settings = PlaybackSettings::default();
    ///
    ///     // A second after the sample starts playing, slow it down to half its
    ///     // speed over another second.
    ///     settings.speed_at(
    ///         0.5,
    ///         time.now() + DurationSeconds(1.0),
    ///         time.now() + DurationSeconds(2.0),
    ///         &mut events,
    ///     );
    ///
    ///     commands.spawn((
    ///         events,
    ///         settings,
    ///         SamplePlayer::new(server.load("my_sample.wav")),
    ///     ));
    /// }
    /// ```
    pub fn speed_at(
        &self,
        speed: f64,
        start: InstantSeconds,
        end: InstantSeconds,
        events: &mut AudioEvents,
    ) {
        let start_value = events.get_value_at(start, self);
        let mut end_value = start_value.clone();
        end_value.speed = speed;

        // This, too, is a very rough JND estimate.
        let pitch_span = (end_value.speed - start_value.speed).abs();
        let total_events = (pitch_span / 0.001).max(1.0) as usize;
        let total_events =
            crate::node::events::max_event_rate(end.0 - start.0, 0.001).min(total_events);

        events.schedule_tween(
            start,
            end,
            start_value,
            end_value,
            total_events,
            |a, b, t| {
                let mut output = a.clone();
                output.speed = a.speed.lerp(b.speed, t as f64);
                output
            },
        );
    }

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

// NOTE: this is specifically designed to produce Firewheel's
// `SamplerNodePatch` value. This is so we can leverage the event
// scheduling system as if this were a real node.
impl firewheel::diff::Diff for PlaybackSettings {
    fn diff<E: firewheel::diff::EventQueue>(
        &self,
        baseline: &Self,
        path: firewheel::diff::PathBuilder,
        event_queue: &mut E,
    ) {
        self.playback
            .diff(&baseline.playback, path.with(2), event_queue);
        self.speed.diff(&baseline.speed, path.with(4), event_queue);
    }
}

impl firewheel::diff::Patch for PlaybackSettings {
    type Patch = firewheel::nodes::sampler::SamplerNodePatch;

    fn patch(
        data: &firewheel::event::ParamData,
        path: &[u32],
    ) -> std::result::Result<Self::Patch, firewheel::diff::PatchError> {
        firewheel::nodes::sampler::SamplerNode::patch(data, path)
    }

    fn apply(&mut self, patch: Self::Patch) {
        match patch {
            firewheel::nodes::sampler::SamplerNodePatch::Playback(p) => self.playback = p,
            firewheel::nodes::sampler::SamplerNodePatch::Speed(s) => self.speed = s,
            _ => {}
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

    /// Provides the RNG source for the [`RandomPitch`] component.
    ///
    /// By default, this uses [`rand::rngs::SmallRng`]. To provide
    /// your own RNG source, simply insert this resource after
    /// adding the [`SeedlingPlugin`][crate::prelude::SeedlingPlugin].
    #[derive(Resource)]
    pub struct PitchRngSource(Box<dyn PitchRng + Send + Sync>);

    impl core::fmt::Debug for PitchRngSource {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_tuple("PitchRngSource").finish_non_exhaustive()
        }
    }

    impl PitchRngSource {
        /// Construct a new [`PitchRngSource`].
        pub fn new<T: rand::Rng + Send + Sync + 'static>(rng: T) -> Self {
            Self(Box::new(RandRng(rng)))
        }
    }

    /// A component that applies a random pitch to [`PlaybackSettings`] when spawned.
    ///
    /// This can be used for subtle sound variations, breaking up
    /// the monotony of repeated sounds like footsteps.
    ///
    /// To control the RNG source, you can provide a custom [`PitchRngSource`] resource.
    #[derive(Debug, Component, Default, Clone)]
    #[require(PlaybackSettings)]
    #[component(immutable)]
    #[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
    pub struct RandomPitch(pub core::ops::Range<f64>);

    impl RandomPitch {
        /// Create a new [`RandomPitch`] with deviation about 1.0.
        ///
        /// ```
        /// # use bevy::prelude::*;
        /// # use bevy_seedling::prelude::*;
        /// # fn deviation(mut commands: Commands, server: Res<AssetServer>) {
        /// commands.spawn((
        ///     SamplePlayer::new(server.load("my_sample.wav")),
        ///     RandomPitch::new(0.05),
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
