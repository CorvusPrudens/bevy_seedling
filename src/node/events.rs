use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::FloatExt;
use bevy_time::{Time, TimeSystem};
use firewheel::{
    Volume,
    clock::{DurationSeconds, InstantSeconds},
    diff::{Diff, EventQueue, Patch, PathBuilder},
    event::NodeEventType,
    nodes::volume::VolumeNode,
};
use portable_atomic::AtomicU64;
use std::sync::Arc;

use crate::time::Audio;

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
    pub(super) queue: Vec<NodeEventType>,
    /// We keep a timeline like this because a simple queue of rendered events is not sufficient.
    ///
    /// Since we'll send out the scheduled events a little bit in advance, there will be some
    /// amount of time in the ECS where we don't have access to the patches -- which is exactly
    /// when we need them! Keep in mind that the events are not `Clone`.
    ///
    /// If we can instead render the events on-demand, we can fetch them whenever we need.
    /// It's also much easier to detect overlapping events.
    pub(super) timeline: Vec<TimelineEvent>,
    // TODO: This doesn't work great when manually constructing the events.
    now: InstantSeconds,
}

impl AudioEvents {
    /// Create a new instant of [`AudioEvents`], primed
    /// with the current audio context time.
    pub fn new(now: &Time<Audio>) -> Self {
        Self {
            queue: Default::default(),
            timeline: Default::default(),
            now: now.context().instant(),
        }
    }

    /// Clone any timeline events from `other` that aren't present in `self`.
    pub fn merge_timelines(&mut self, other: &Self) {
        for event in &other.timeline {
            if !self.timeline.iter().any(|ev| ev.id() == event.id()) {
                self.timeline.push(event.clone());
            }
        }
    }

    /// Like `merge_timelines`, but clear all the events in `other` that
    /// have elapsed.
    pub(crate) fn merge_timelines_and_clear(&mut self, other: &mut Self, now: InstantSeconds) {
        other.timeline.retain(|event| {
            if !self.timeline.iter().any(|ev| ev.id() == event.id()) {
                self.timeline.push(event.clone());
            }

            !event.completely_elapsed(now)
        });
    }

    /// Clear the timeline of any elapsed events.
    pub fn clear_elapsed_events(&mut self, now: InstantSeconds) {
        self.timeline
            .retain(|event| !event.completely_elapsed(now) || !event.render_progress.complete);
    }

    pub fn timeline(&self) -> &[TimelineEvent] {
        &self.timeline
    }

    /// Schedule an event at an absolute time in terms of the audio clock.
    ///
    /// This method will apply any patches to the value before passing it to the closure,
    /// ensuring any previous scheduled events are respected.
    pub fn schedule<T, F>(&mut self, time: InstantSeconds, value: &T, change: F)
    where
        T: Diff + Patch + Send + Sync + Clone + 'static,
        F: FnOnce(&mut T),
    {
        // let's make sure to apply any patches that may exist in this queue before the start time
        let initial_value = self.get_value_at(time, value);

        let mut new_value = initial_value.clone();
        change(&mut new_value);

        self.timeline.push(TimelineEvent::new(SingleEvent {
            before: initial_value,
            after: new_value,
            instant: time,
        }));
    }

    /// Schedule a tween with a custom interpolator.
    fn schedule_tween<T, F>(
        &mut self,
        start: InstantSeconds,
        end: InstantSeconds,
        start_value: T,
        end_value: T,
        total_events: usize,
        interpolate: F,
    ) where
        T: Diff + Patch + Send + Sync + Clone + 'static,
        F: Fn(&T, &T, f32) -> T + Send + Sync + 'static,
    {
        self.timeline.push(TimelineEvent::new(TweenEvent {
            start: (start, start_value),
            end: (end, end_value),
            total_events,
            interpolate: Box::new(interpolate),
        }))
    }

    /// Schedule an event at an absolute time without applying previous patches.
    pub fn schedule_immediate<T, F>(&mut self, time: InstantSeconds, value: &T, change: F)
    where
        T: Diff + Patch + Send + Sync + Clone + 'static,
        F: FnOnce(&mut T),
    {
        let mut new_value = value.clone();
        change(&mut new_value);

        self.timeline.push(TimelineEvent::new(SingleEvent {
            before: value.clone(),
            after: new_value,
            instant: time,
        }));
    }

    /// Apply all scheduled events before `Instant` in this event queue to `value`.
    pub fn value_at<T>(&self, start: InstantSeconds, end: InstantSeconds, value: &mut T)
    where
        T: Diff + Patch + Clone,
    {
        // Since we're rendering these on-the-fly, there's no need to
        // push them to a temporary queue. Just apply them directly!
        let mut func = |event: NodeEventType, _| {
            if let Some(patch) = T::patch_event(&event) {
                value.apply(patch);
            }
        };
        for event in &self.timeline {
            let queue = TimelineQueue::new(start, &mut func);
            event.tween.render(start, end, queue);
        }
    }

    /// Apply all scheduled events before `Instant` in this event queue to `value`.
    pub fn get_value_at<T>(&self, instant: InstantSeconds, value: &T) -> T
    where
        T: Diff + Patch + Clone,
    {
        let mut new_value = value.clone();
        self.value_at(InstantSeconds(0.0), instant, &mut new_value);
        new_value
    }
}

impl EventQueue for AudioEvents {
    fn push(&mut self, data: firewheel::event::NodeEventType) {
        self.queue.push(data);
    }
}

impl core::fmt::Debug for AudioEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEvents")
            .field("queue", &())
            .field("timeline", &self.timeline)
            .field("now", &self.now)
            .finish()
    }
}

pub struct TimelineQueue<'a> {
    queue: &'a mut dyn FnMut(NodeEventType, InstantSeconds),
    pub instant: InstantSeconds,
}

impl<'a> TimelineQueue<'a> {
    pub fn new(
        initial_instant: InstantSeconds,
        f: &'a mut impl FnMut(NodeEventType, InstantSeconds),
    ) -> Self {
        Self {
            queue: f,
            instant: initial_instant,
        }
    }
}

impl EventQueue for TimelineQueue<'_> {
    fn push(&mut self, data: NodeEventType) {
        (self.queue)(data, self.instant);
    }
}

pub trait TimelineTween: Send + Sync + 'static {
    fn render(&self, start: InstantSeconds, end: InstantSeconds, queue: TimelineQueue);

    fn time_range(&self) -> core::ops::Range<InstantSeconds>;
}

static TIMELINE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct TimelineEvent {
    pub tween: Arc<dyn TimelineTween>,
    pub render_progress: RenderProgress,
    id: u64,
}

#[derive(Clone, Debug)]
pub struct RenderProgress {
    pub range: core::ops::Range<InstantSeconds>,
    pub complete: bool,
}

impl RenderProgress {
    pub fn new(range: core::ops::Range<InstantSeconds>) -> Self {
        Self {
            range,
            complete: false,
        }
    }
}

impl TimelineEvent {
    pub fn new<T>(event: T) -> Self
    where
        T: TimelineTween,
    {
        fn new(event: Arc<dyn TimelineTween>) -> TimelineEvent {
            let render_start = event.time_range().start;
            let render_progress = RenderProgress::new(render_start..render_start);

            TimelineEvent {
                tween: event,
                render_progress,
                id: TIMELINE_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            }
        }

        new(Arc::new(event))
    }

    pub fn completely_elapsed(&self, now: InstantSeconds) -> bool {
        self.tween.time_range().end < now
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn render_range(
        &self,
        full_range: core::ops::Range<InstantSeconds>,
    ) -> Option<core::ops::Range<InstantSeconds>> {
        if self.render_progress.complete {
            return None;
        }

        let range = self.tween.time_range();
        let new_start = self.render_progress.range.end.0.max(full_range.start.0);
        let new_end = range.end.0.min(full_range.end.0);

        Some(InstantSeconds(new_start)..InstantSeconds(new_end))
    }
}

impl core::fmt::Debug for TimelineEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimelineEvent")
            .field("tween", &())
            .field("render_progress", &self.render_progress)
            .field("id", &self.id)
            .finish()
    }
}

struct SingleEvent<T> {
    before: T,
    after: T,
    instant: InstantSeconds,
}

impl<T> TimelineTween for SingleEvent<T>
where
    T: Diff + Send + Sync + 'static,
{
    fn render(&self, start: InstantSeconds, end: InstantSeconds, mut queue: TimelineQueue) {
        if (start..=end).contains(&self.instant) {
            queue.instant = self.instant;
            self.after
                .diff(&self.before, Default::default(), &mut queue);
        }
    }

    fn time_range(&self) -> core::ops::Range<InstantSeconds> {
        self.instant..self.instant
    }
}

struct TweenEvent<T> {
    start: (InstantSeconds, T),
    end: (InstantSeconds, T),
    total_events: usize,
    interpolate: Box<dyn Fn(&T, &T, f32) -> T + Send + Sync>,
}

impl<T> TimelineTween for TweenEvent<T>
where
    T: Diff + Send + Sync + 'static,
{
    fn render(&self, start: InstantSeconds, end: InstantSeconds, mut queue: TimelineQueue) {
        let range = self.start.0.0..self.end.0.0;
        if range.is_empty() {
            return;
        }

        let start = range.start.max(start.0);
        let end = range.end.min(end.0);

        if range.contains(&start) && end <= range.end && start < end {
            let duration = range.end - range.start;
            for i in 1..=self.total_events {
                let proportion = i as f64 / self.total_events as f64;
                let instant = self.start.0.0 + proportion * duration;
                if !(start..=end).contains(&instant) {
                    continue;
                }

                queue.instant = InstantSeconds(instant);
                let new_value = (self.interpolate)(&self.start.1, &self.end.1, proportion as f32);
                new_value.diff(&self.start.1, PathBuilder::default(), &mut queue);
            }
        }
    }

    fn time_range(&self) -> core::ops::Range<InstantSeconds> {
        self.start.0..self.end.0
    }
}

fn update_events_instant(mut q: Query<&mut AudioEvents>, time: Res<Time<crate::time::Audio>>) {
    for mut event in &mut q {
        event.now = time.context().instant();
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

// Limit fades to one event per millisecond.
fn max_rate(duration: f64) -> usize {
    let max_rate = 0.001;
    (duration / max_rate).ceil() as usize
}

impl VolumeFade for VolumeNode {
    fn fade_to(&self, target: Volume, duration: DurationSeconds, events: &mut AudioEvents) {
        let start = events.now;
        let end = events.now + duration;
        let start_value = events.get_value_at(events.now, self);
        let mut end_value = start_value;
        end_value.volume = target;

        // Here, we use the just noticeable difference, around 1 dB, to roughly calculate
        // how many total steps we need. We give a bit of margin just in case.
        let db_span = (clamp(start_value.volume.decibels()) - clamp(target.decibels())).abs();
        let total_events = (db_span * 1.25).max(1.0) as usize;
        let total_events = max_rate(duration.0).min(total_events);

        events.schedule_tween(
            start,
            end,
            start_value,
            end_value,
            total_events,
            |a, b, t| {
                let mut output = *a;
                output.volume = a.volume.audio_lerp(b.volume, t);
                output
            },
        );
    }

    fn fade_at(
        &self,
        target: Volume,
        start: InstantSeconds,
        end: InstantSeconds,
        events: &mut AudioEvents,
    ) {
        let start_value = events.get_value_at(start, self);
        let mut end_value = start_value;
        end_value.volume = target;

        // Here, we use the just noticeable difference, around 1 dB, to roughly calculate
        // how many total steps we need. We give a bit of margin just in case.
        let db_span = (clamp(start_value.volume.decibels()) - clamp(target.decibels())).abs();
        let total_events = (db_span * 1.25).max(1.0) as usize;
        let total_events = max_rate(end.0 - start.0).min(total_events);

        events.schedule_tween(
            start,
            end,
            start_value,
            end_value,
            total_events,
            |a, b, t| {
                let mut output = *a;
                output.volume = a.volume.audio_lerp(b.volume, t);
                output
            },
        );
    }
}
