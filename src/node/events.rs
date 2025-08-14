use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::FloatExt;
use bevy_time::{Time, TimeSystem};
use firewheel::{
    Volume,
    clock::{DurationSeconds, EventInstant, InstantSeconds},
    diff::{Diff, EventQueue, Patch, PathBuilder},
    event::NodeEventType,
    nodes::volume::VolumeNode,
};

pub(crate) struct EventsPlugin;

impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(First, update_events_instant.after(TimeSystem));
    }
}

/// An audio event queue.
///
/// When inserted into an entity that contains a [FirewheelNode],
/// these events will automatically be drained and sent
/// to the audio context in the [SeedlingSystems::Flush] set.
#[derive(Component, Default)]
pub struct AudioEvents {
    pub(super) queue: Vec<(Option<EventInstant>, NodeEventType)>,
    instant: InstantSeconds,
}

fn update_events_instant(mut q: Query<&mut AudioEvents>, time: Res<Time<crate::time::Audio>>) {
    for mut event in &mut q {
        event.instant = time.context().instant();
    }
}

impl EventQueue for AudioEvents {
    fn push(&mut self, data: firewheel::event::NodeEventType) {
        self.queue.push((None, data));
    }
}

impl core::fmt::Debug for AudioEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEvents").finish_non_exhaustive()
    }
}

struct ScheduledEventQueue<'a> {
    queue: &'a mut Vec<(Option<EventInstant>, NodeEventType)>,
    timing: EventInstant,
}

impl EventQueue for ScheduledEventQueue<'_> {
    fn push(&mut self, data: firewheel::event::NodeEventType) {
        self.queue.push((Some(self.timing), data));
    }
}

impl AudioEvents {
    /// Schedule an event at a time relative to the end of the frame.
    ///
    /// This method will apply any patches to the value before passing it to the closure,
    /// ensuring any previous scheduled events are respected.
    pub fn schedule<T, F>(&mut self, delay: DurationSeconds, value: &T, change: F)
    where
        T: Diff + Patch + Clone,
        F: FnOnce(&mut T),
    {
        self.schedule_absolute(self.instant + delay, value, change)
    }

    /// Schedule an event at an absolute time in terms of the audio clock.
    ///
    /// This method will apply any patches to the value before passing it to the closure,
    /// ensuring any previous scheduled events are respected.
    pub fn schedule_absolute<T, F>(&mut self, time: InstantSeconds, value: &T, change: F)
    where
        T: Diff + Patch + Clone,
        F: FnOnce(&mut T),
    {
        let mut initial_value = value.clone();

        // let's make sure to apply any patches that may exist in this queue before the start time
        self.value_at(time, &mut initial_value);

        let mut new_value = initial_value.clone();
        change(&mut new_value);

        let mut queue = ScheduledEventQueue {
            queue: &mut self.queue,
            timing: EventInstant::Seconds(time),
        };

        new_value.diff(&initial_value, PathBuilder::default(), &mut queue);
    }

    /// Schedule an event at an absolute time without applying previous patches.
    pub fn schedule_immediate<T, F>(&mut self, time: InstantSeconds, value: &T, change: F)
    where
        T: Diff + Patch + Clone,
        F: FnOnce(&mut T),
    {
        let mut new_value = value.clone();
        change(&mut new_value);

        let mut queue = ScheduledEventQueue {
            queue: &mut self.queue,
            timing: EventInstant::Seconds(time),
        };

        new_value.diff(value, PathBuilder::default(), &mut queue);
    }

    /// Apply all scheduled events before `Instant` in this event queue to `value`.
    pub fn value_at<T>(&self, instant: InstantSeconds, value: &mut T)
    where
        T: Diff + Patch + Clone,
    {
        for (t, patch) in &self.queue {
            let Some(t) = t else {
                continue;
            };

            if matches!(t, EventInstant::Seconds(s) if *s < instant) {
                if let NodeEventType::Param { data, path } = patch {
                    let Ok(patch) = T::patch(data, path) else {
                        continue;
                    };
                    value.apply(patch);
                }
            }
        }
    }
}

trait AudioLerp: Default + Clone + Send + Sync + 'static {
    fn audio_lerp(&self, other: Self, amount: f32) -> Self;
}

fn clamp(db: f32) -> f32 {
    if db < -60.0 { -60.0 } else { db }
}

impl AudioLerp for Volume {
    fn audio_lerp(&self, other: Self, amount: f32) -> Self {
        match (self, other) {
            (Self::Linear(a), Self::Linear(b)) => Self::Linear(a.lerp(b, amount)),
            (Self::Decibels(a), Self::Decibels(b)) => Self::Decibels(a.lerp(b, amount)),
            (Self::Decibels(a), b) => Self::Decibels(a.lerp(clamp(b.decibels()), amount)),
            (a, Self::Decibels(b)) => Self::Decibels(clamp(a.decibels()).lerp(b, amount)),
        }
    }
}

pub trait VolumeFade {
    fn fade_to(&self, target: Volume, duration: DurationSeconds, events: &mut AudioEvents);
    fn fade_at(
        &self,
        target: Volume,
        start: InstantSeconds,
        end: InstantSeconds,
        events: &mut AudioEvents,
    );
}

impl VolumeFade for VolumeNode {
    fn fade_to(&self, target: Volume, duration: DurationSeconds, events: &mut AudioEvents) {
        let mut initial_value = *self;
        events.value_at(events.instant, &mut initial_value);

        // Here, we use the just noticeable difference, around 1 dB, to roughly calculate
        // how many total steps we need. We give a bit of margin just in case.
        let db_span = (clamp(initial_value.volume.decibels()) - clamp(target.decibels())).abs();
        let total_events = (db_span * 1.25).max(1.0) as usize;

        for i in 1..=total_events {
            let t = i as f32 / total_events as f32;
            let delay = t as f64 * duration.0;

            // TODO: the borrow of self.volume isn't quite right here
            events.schedule_immediate(events.instant + DurationSeconds(delay), self, |v| {
                v.volume = self.volume.audio_lerp(target, t);
            });
        }
    }

    fn fade_at(
        &self,
        target: Volume,
        start: InstantSeconds,
        end: InstantSeconds,
        events: &mut AudioEvents,
    ) {
        let duration = end.0 - start.0;

        let mut initial_value = *self;
        events.value_at(start, &mut initial_value);

        let db_span = (clamp(initial_value.volume.decibels()) - clamp(target.decibels())).abs();
        let total_events = (db_span * 1.25).max(1.0) as usize;

        for i in 1..=total_events {
            let t = i as f32 / total_events as f32;
            let delay = t as f64 * duration;

            // TODO: the borrow of self.volume isn't quite right here
            events.schedule_immediate(InstantSeconds(start.0 + delay), &initial_value, |v| {
                v.volume = initial_value.volume.audio_lerp(target, t);
            });
        }
    }
}
