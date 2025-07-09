//! Audio-rate parameter curves.

use crate::fixed_vec::FixedVec;
use bevy::math::{
    Curve,
    curve::{EaseFunction, EasingCurve},
};
use bevy_math::curve::{Ease, Interval};
use firewheel::{
    clock::ClockSeconds,
    diff::{Diff, EventQueue, Patch, PatchError, PathBuilder},
    event::ParamData,
};

/// A parameter expressed as a timeline of events.
///
/// This allows parameters to vary smoothly at audio-rate
/// with minimal cross-thread communication.
pub type Timeline<T> = TimelineGeneric<T, EasingCurve<T>>;

/// A discrete parameter expressed as a timeline of events.
///
/// This allows parameters to be changed at audio rate, although does not allow smoothing.
pub type DiscreteTimeline<T> = TimelineGeneric<T, Never>;

/// A parameter expressed as a timeline of events, either discrete or continuous.
///
/// This allows parameters to vary smoothly at audio-rate
/// with minimal cross-thread communication.
#[derive(Debug, Clone)]
pub struct TimelineGeneric<T, C> {
    value: T,
    events: FixedVec<TimelineEvent<T, C>>,
    /// The total number of events consumed.
    consumed: usize,
}

impl<T, C> Default for TimelineGeneric<T, C>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            value: Default::default(),
            events: Default::default(),
            consumed: 0,
        }
    }
}

impl<T, C> TimelineGeneric<T, C> {
    /// Create a new instance of [`Timeline`] with an initial value.
    pub fn new(value: T) -> Self {
        Self {
            value,
            events: Default::default(),
            consumed: 0,
        }
    }

    /// Returns whether the value is changing at `time`.
    pub fn is_active(&self, time: ClockSeconds) -> bool {
        self.events
            .iter()
            .any(|e| e.contains(time) && matches!(e, TimelineEvent::Curve { .. }))
    }

    /// Returns whether this node will change within the time range.
    pub fn active_within(&self, start: ClockSeconds, end: ClockSeconds) -> bool {
        self.events.iter().any(|e| {
            e.start_time()
                .is_some_and(|t| (start.0..end.0).contains(&t.0))
                || e.end_time()
                    .is_some_and(|t| (start.0..end.0).contains(&t.0))
        })
    }

    /// Remove all events from the timeline.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// [`Timeline`]'s error type.
#[derive(Debug, Clone)]
pub enum TimelineError {
    /// A value overlaps another range in the timeline.
    OverlappingRanges,
}

impl<T, C> TimelineGeneric<T, C>
where
    T: Clone,
    C: Curve<T>,
{
    /// Push an event to the timeline, popping off the oldest one if the
    /// queue is full.
    pub fn push(&mut self, event: TimelineEvent<T, C>) -> Result<(), TimelineError> {
        // scan the events to ensure the event doesn't overlap any ranges
        match &event {
            TimelineEvent::Deferred { time, .. } => {
                if self.events.iter().any(|e| e.overlaps(*time)) {
                    return Err(TimelineError::OverlappingRanges);
                }
            }
            TimelineEvent::Curve { start, end, .. } => {
                if self
                    .events
                    .iter()
                    .any(|e| e.overlaps(*start) || e.overlaps(*end))
                {
                    return Err(TimelineError::OverlappingRanges);
                }
            }
            TimelineEvent::Immediate(i) => {
                self.clear();
                self.value = i.clone();
            }
        }

        self.events.push(event);
        self.consumed += 1;

        Ok(())
    }

    /// Set the value immediately.
    pub fn set(&mut self, value: T) {
        // This push cannot fail.
        self.push(TimelineEvent::Immediate(value)).unwrap();
    }

    /// Get the value at a point in time.
    pub fn value_at(&self, time: ClockSeconds) -> T {
        if let Some(bounded) = self.events.iter().find(|e| e.contains(time)) {
            return bounded.get(time);
        }

        let mut recent_time = f64::MAX;
        let mut recent_value = None;

        for event in self.events.iter() {
            if let Some(end) = event.end_time() {
                let delta = time.0 - end.0;

                if delta >= 0. && delta < recent_time {
                    recent_time = delta;
                    recent_value = Some(event.end_value());
                }
            }
        }

        recent_value.unwrap_or(self.value.clone())
    }

    /// Get the current value without respect to time.
    ///
    /// This depends on regular calls to [`Timeline::tick`]
    /// for accuracy.
    pub fn get(&self) -> T {
        self.value.clone()
    }

    /// Update the inner value to the current timestamp.
    pub fn tick(&mut self, now: ClockSeconds) {
        self.value = self.value_at(now);
    }
}

impl<T> Timeline<T>
where
    T: Clone + Ease + 'static,
{
    /// Push a curve event with absolute timestamps.
    pub fn push_curve(
        &mut self,
        end_value: T,
        start: ClockSeconds,
        end: ClockSeconds,
        curve: EaseFunction,
    ) -> Result<(), TimelineError> {
        let start_value = self.value_at(start);
        let curve = EasingCurve::new(start_value, end_value, curve);

        self.push(TimelineEvent::Curve { curve, start, end })
    }
}

/// A single timeline event for a parameter with a continuous domain.
pub type ContinuousTimelineEvent<T> = TimelineEvent<T, EasingCurve<T>>;

/// A type that cannot be instantiated.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Never {}

impl<T> Curve<T> for Never {
    fn domain(&self) -> Interval {
        unreachable!()
    }

    fn sample(&self, _: f32) -> Option<T> {
        unreachable!()
    }

    fn sample_clamped(&self, _: f32) -> T {
        unreachable!()
    }

    fn sample_unchecked(&self, _: f32) -> T {
        unreachable!()
    }
}

/// A single timeline event for a parameter with a discrete domain.
pub type DiscreteTimelineEvent<T> = TimelineEvent<T, Never>;

/// A single timeline event.
#[derive(Debug, Clone)]
pub enum TimelineEvent<T, Curve> {
    /// An immediate event, which also clears the timeline buffer.
    Immediate(T),
    /// A deferred event.
    Deferred {
        /// The target value.
        value: T,
        /// The time at which this value should be written.
        time: ClockSeconds,
    },
    /// An animation curve.
    Curve {
        /// The easing curve for this animation.
        curve: Curve,
        /// The animation's start time.
        start: ClockSeconds,
        /// The animation's end time.
        end: ClockSeconds,
    },
}

impl<T, Curve> TimelineEvent<T, Curve> {
    /// This event's start time, if any.
    pub fn start_time(&self) -> Option<ClockSeconds> {
        match self {
            Self::Deferred { time, .. } => Some(*time),
            Self::Curve { start, .. } => Some(*start),
            _ => None,
        }
    }

    /// This event's end time, if any.
    ///
    /// A [`TimelineEvent::Deferred`] variant will
    /// provide its start time.
    pub fn end_time(&self) -> Option<ClockSeconds> {
        match self {
            Self::Deferred { time, .. } => Some(*time),
            Self::Curve { end, .. } => Some(*end),
            _ => None,
        }
    }

    /// Returns true if the event contains the given time.
    pub fn contains(&self, time: ClockSeconds) -> bool {
        match self {
            Self::Deferred { time: t, .. } => *t == time,
            Self::Curve { start, end, .. } => (*start..=*end).contains(&time),
            _ => false,
        }
    }

    /// Returns true if the event fully overlaps the given time.
    pub fn overlaps(&self, time: ClockSeconds) -> bool {
        match self {
            Self::Curve { start, end, .. } => time > *start && time < *end,
            _ => false,
        }
    }
}

impl<T, C> TimelineEvent<T, C>
where
    C: Curve<T>,
    T: Clone,
{
    /// Calculates the value at `time`.
    pub fn get(&self, time: ClockSeconds) -> T {
        match self {
            Self::Immediate(i) => i.clone(),
            Self::Deferred { value, .. } => value.clone(),
            Self::Curve { curve, start, end } => {
                let range = end.0 - start.0;
                let progress = time.0 - start.0;

                curve.sample((progress / range) as f32).unwrap()
            }
        }
    }

    /// Gets the starting value.
    pub fn start_value(&self) -> T {
        match self {
            Self::Immediate(i) => i.clone(),
            Self::Deferred { value, .. } => value.clone(),
            Self::Curve { curve, .. } => curve.sample(0.).unwrap(),
        }
    }

    /// Gets the ending value.
    ///
    /// [`TimelineEvent::Immediate`] and [`TimelineEvent::Deferred`]
    /// will simply return their single value.
    pub fn end_value(&self) -> T {
        match self {
            Self::Immediate(i) => i.clone(),
            Self::Deferred { value, .. } => value.clone(),
            Self::Curve { curve, .. } => curve.sample(1.).unwrap(),
        }
    }
}

impl<T, C> Diff for TimelineGeneric<T, C>
where
    T: Clone + Send + Sync + 'static,
    C: Curve<T> + Clone + Send + Sync + 'static,
{
    fn diff<E: EventQueue>(&self, baseline: &Self, path: PathBuilder, event_queue: &mut E) {
        let newly_consumed = self.consumed.saturating_sub(baseline.consumed);
        if newly_consumed == 0 {
            return;
        }

        // If more items were added than the buffer can hold, we only have the most recent self.events.len() items.
        let clamped_newly_consumed = newly_consumed.min(self.events.len());

        // Start index for the new items. They are the last 'clamped_newly_consumed' items in the buffer.
        let start = self.events.len() - clamped_newly_consumed;
        let new_items = &self.events[start..];

        for event in new_items.iter() {
            event_queue.push_param(ParamData::any(event.clone()), path.clone());
        }
    }
}

impl<T, C> Patch for TimelineGeneric<T, C>
where
    T: Clone + 'static,
    C: Curve<T> + Clone + 'static,
{
    type Patch = TimelineEvent<T, C>;

    fn patch(data: &ParamData, _: &[u32]) -> Result<Self::Patch, PatchError> {
        let value: &TimelineEvent<T, C> = data.downcast_ref().ok_or(PatchError::InvalidData)?;

        Ok(value.clone())
    }

    fn apply(&mut self, patch: Self::Patch) {
        let _ = self.push(patch);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use firewheel::event::NodeEventType;

    #[test]
    fn test_continuous_diff() {
        let a = Timeline::new(0f32);
        let mut b = a.clone();

        b.push_curve(
            2f32,
            ClockSeconds(1.),
            ClockSeconds(2.),
            EaseFunction::Linear,
        )
        .unwrap();

        let mut events = Vec::new();
        b.diff(&a, Default::default(), &mut events);

        assert!(
            matches!(&events.as_slice(), &[NodeEventType::Param { data, .. }] if matches!(data, ParamData::Any(_)))
        )
    }

    // #[test]
    // fn test_full_diff() {
    //     let mut a = Timeline::new(0f32);
    //
    //     for _ in 0..8 {
    //         a.push_curve(
    //             2f32,
    //             ClockSeconds(1.),
    //             ClockSeconds(2.),
    //             EaseFunction::Linear,
    //         )
    //         .unwrap();
    //     }
    //
    //     let mut b = a.clone();
    //
    //     b.push_curve(
    //         1f32,
    //         ClockSeconds(1.),
    //         ClockSeconds(2.),
    //         EaseFunction::Linear,
    //     )
    //     .unwrap();
    //
    //     let mut events = Vec::new();
    //     b.diff(&a, events.push(event), Default::default(), &mut events);
    //
    //     assert!(
    //         matches!(&events.as_slice(), &[NodeEventType::Param { data, .. }] if matches!(data, ParamData::F32(d) if d.end_value() == 1.))
    //     )
    // }

    #[test]
    fn test_linear_curve() {
        let mut value = Timeline::new(0f32);

        value
            .push_curve(
                1f32,
                ClockSeconds(0.),
                ClockSeconds(1.),
                EaseFunction::Linear,
            )
            .unwrap();

        value
            .push_curve(
                2f32,
                ClockSeconds(1.),
                ClockSeconds(2.),
                EaseFunction::Linear,
            )
            .unwrap();

        value
            .push(TimelineEvent::Deferred {
                value: 3.0,
                time: ClockSeconds(2.5),
            })
            .unwrap();

        assert_eq!(value.value_at(ClockSeconds(0.)), 0.);
        assert_eq!(value.value_at(ClockSeconds(0.5)), 0.5);
        assert_eq!(value.value_at(ClockSeconds(1.0)), 1.0);

        assert_eq!(value.value_at(ClockSeconds(1.)), 1.);
        assert_eq!(value.value_at(ClockSeconds(1.5)), 1.5);
        assert_eq!(value.value_at(ClockSeconds(2.0)), 2.0);

        assert_eq!(value.value_at(ClockSeconds(2.25)), 2.0);

        assert_eq!(value.value_at(ClockSeconds(2.5)), 3.0);
    }
}
