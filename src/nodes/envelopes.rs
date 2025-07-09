//! AHDSR (attack, hold, decay, sustain, release) volume envelope.

use bevy::ecs::component::Component;
use firewheel::{
    StreamInfo,
    channel_config::{ChannelConfig, ChannelCount},
    clock::{ClockSamples, ClockSeconds},
    diff::{Diff, Patch},
    dsp::volume::Volume,
    event::NodeEventList,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, EmptyConfig,
        ProcBuffers, ProcInfo, ProcessStatus,
    },
    param::smoother::SmoothedParam,
};
use std::{num::NonZeroU32, ops::Range};

use crate::timeline::{DiscreteTimeline, TimelineEvent};

/// How an envelope responds to receiving [`TriggerEvent::On`] if it is not at rest.
#[derive(Diff, Patch, Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum RetriggerMode {
    /// Restart the cycle of the envelope from the `attack` section, but use the current value
    /// as the starting point.
    #[default]
    Normal,
    /// Restart the envelope from `lo`.
    Restart,
    /// Ignore any new [`TriggerEvent::On`] events unless the envelope is at rest.
    Ignore,
}

impl RetriggerMode {
    fn state_transition(&self) -> fn(f32, Option<AhdsrState>) -> (f32, AhdsrState) {
        match self {
            RetriggerMode::Ignore => |cur, state| (cur, state.unwrap_or(AhdsrState::Attack)),
            RetriggerMode::Normal => |cur, _| (cur, AhdsrState::Attack),
            RetriggerMode::Restart => |_, _| (0., AhdsrState::Attack),
        }
    }
}

/// Configuration for how an envelope reacts to "on" messages.
#[derive(Diff, Patch, Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum TriggerMode {
    /// Start the envelope on [`TriggerEvent::On`], holding at `sustain` until [`TriggerEvent::Off`] is received.
    #[default]
    Normal,
    /// Ignore [`TriggerEvent::Off`] events, simply go through the full cycle without further input.
    Once,
}

impl TriggerMode {
    fn state_transition(&self) -> fn(AhdsrState) -> AhdsrState {
        match self {
            TriggerMode::Normal => |_| AhdsrState::Release,
            TriggerMode::Once => |state| state,
        }
    }

    fn sustain_state(&self) -> AhdsrState {
        match self {
            Self::Normal => AhdsrState::Sustain,
            Self::Once => AhdsrState::Release,
        }
    }
}

/// Whether the envelope is triggered
#[derive(Default, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum TriggerState {
    /// The envelope is in the triggered state.
    On,
    /// The envelope is returning to its resting state.
    #[default]
    Off,
}

/// An envelope over the volume of a node.
#[derive(Component, Diff, Patch, Debug, Clone)]
pub struct AhdsrVolumeNode {
    /// The low value, used when the envelope is not triggered.
    pub lo: Volume,
    /// The high value, used when the envelope is at its peak.
    pub hi: Volume,
    /// The amount of time to transition between `hi` and `lo`, in seconds.
    pub attack: f64,
    /// The amount of time to hold the peak before progressing to decay, in seconds.
    pub hold: f64,
    /// The amount of time to transition between `hi` and `sustain`, in seconds.
    pub decay: f64,
    /// The ratio between `lo` and `hi` to decay to.
    pub sustain_proportion: f32,
    /// The amount of time to transition between `sustain` and `lo`, in seconds.
    pub release: f64,
    /// How to respond to [`TriggerEvent`]s when the envelope is at rest.
    pub trigger_mode: TriggerMode,
    /// How to respond to [`TriggerEvent`]s when the envelope is already triggered.
    pub retrigger_mode: RetriggerMode,
    /// Whether the envelope is triggered.
    pub triggered: DiscreteTimeline<TriggerState>,
}

impl Default for AhdsrVolumeNode {
    fn default() -> Self {
        Self {
            lo: Volume::SILENT,
            hi: Volume::UNITY_GAIN,
            attack: Default::default(),
            hold: Default::default(),
            decay: Default::default(),
            sustain_proportion: 1.,
            release: Default::default(),

            trigger_mode: Default::default(),
            retrigger_mode: Default::default(),
            triggered: Default::default(),
        }
    }
}

impl AudioNode for AhdsrVolumeNode {
    type Configuration = EmptyConfig;

    fn info(&self, _config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("ahdsr_volume")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::STEREO,
                num_outputs: ChannelCount::STEREO,
            })
            .uses_events(true)
    }

    fn construct_processor(
        &self,
        _config: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> impl AudioNodeProcessor {
        let lo = self.lo.linear();
        let hi = self.hi.linear();
        let sustain_proportion = self.sustain_proportion.clamp(0., 1.);

        let attack = ClockSeconds(self.attack);
        let hold = ClockSeconds(self.hold);
        let decay = ClockSeconds(self.decay);
        let release = ClockSeconds(self.release);

        AhdsrVolumeProcessor {
            state: None,
            current: SmoothedParam::new(lo, Default::default(), cx.stream_info.sample_rate),

            lo,
            hi,

            attack_rate: (attack.0 * cx.stream_info.sample_rate_recip) as _,
            hold_samples: ClockSamples::from_secs_f64(hold.0, cx.stream_info.sample_rate.get()),
            decay_rate: (1. - sustain_proportion)
                * (decay.0 * cx.stream_info.sample_rate_recip) as f32,
            sustain_proportion,
            release_rate: sustain_proportion
                * (release.0 * cx.stream_info.sample_rate_recip) as f32,

            attack,
            hold,
            decay,
            release,

            off_state_transition: self.trigger_mode.state_transition(),
            on_state_transition: self.retrigger_mode.state_transition(),
            sustain_state: self.trigger_mode.sustain_state(),

            sample_rate: cx.stream_info.sample_rate,
            sample_rate_recip: cx.stream_info.sample_rate_recip,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AhdsrState {
    Attack,
    /// The inner value is the number of samples that the value has been held for.
    Hold(i64),
    Decay,
    Sustain,
    Release,
}

struct AhdsrVolumeProcessor {
    state: Option<AhdsrState>,

    /// Normalized 0..1, with 0 = lo, 1 = hi. This allows us to change `hi`/`lo`/`sustain` while the envelope
    /// is activated.
    current: SmoothedParam,

    lo: f32,
    hi: f32,

    attack_rate: f32,
    hold_samples: ClockSamples,
    decay_rate: f32,
    sustain_proportion: f32,
    release_rate: f32,

    // -- Information to reconstruct the above fields when the node params change --
    attack: ClockSeconds,
    hold: ClockSeconds,
    decay: ClockSeconds,
    release: ClockSeconds,

    // -- State transitions --
    off_state_transition: fn(AhdsrState) -> AhdsrState,
    sustain_state: AhdsrState,
    on_state_transition: fn(f32, Option<AhdsrState>) -> (f32, AhdsrState),

    // These are stored on `proc_info` in the latest, unreleased version of Firewheel,
    // when that is released these fields should be removed.
    sample_rate: NonZeroU32,
    sample_rate_recip: f64,
}

#[inline(always)]
fn lerp(a: f32, b: f32, ratio: f32) -> f32 {
    let ratio = ratio.clamp(0., 1.);
    a * (1. - ratio) + b * ratio
}

fn close_enough(a: f32, b: f32) -> bool {
    (a - b).abs() <= f32::EPSILON
}

impl AhdsrVolumeProcessor {
    // TODO: Write to a scratch buffer for better vectorisation
    fn tick(&mut self, max_frames: ClockSamples) -> (ClockSamples, f32) {
        match &mut self.state {
            Some(AhdsrState::Attack) => {
                self.current
                    .set_value((self.current.target_value() + self.attack_rate).min(1.));
                let current_smoothed = self.current.next_smoothed();
                if close_enough(current_smoothed, 1.) {
                    self.state = Some(AhdsrState::Hold(0));
                }

                (ClockSamples(1), current_smoothed)
            }
            Some(AhdsrState::Hold(val)) => {
                if self.current.settle() {
                    let current_smoothed = self.current.next_smoothed();
                    let new_hold_time = *val + max_frames.0;

                    let count = if new_hold_time >= self.hold_samples.0 {
                        self.state = Some(AhdsrState::Decay);

                        self.hold_samples.0 - new_hold_time
                    } else {
                        *val = new_hold_time;
                        max_frames.0
                    };

                    (ClockSamples(count), current_smoothed)
                } else {
                    let current_smoothed = self.current.next_smoothed();
                    if *val >= self.hold_samples.0 {
                        self.state = Some(AhdsrState::Decay);
                    } else {
                        *val += 1;
                    }

                    (ClockSamples(1), current_smoothed)
                }
            }
            Some(AhdsrState::Decay) => {
                self.current.set_value(
                    (self.current.target_value() - self.decay_rate).max(self.sustain_proportion),
                );
                let current_smoothed = self.current.next_smoothed();

                if close_enough(current_smoothed, self.sustain_proportion) {
                    self.state = Some(self.sustain_state);
                }

                (ClockSamples(1), current_smoothed)
            }
            Some(AhdsrState::Release) => {
                self.current
                    .set_value((self.current.target_value() - self.release_rate).max(0.));
                let current_smoothed = self.current.next_smoothed();

                if close_enough(current_smoothed, 0.) {
                    self.state = None;
                }

                (ClockSamples(1), current_smoothed)
            }
            None | Some(AhdsrState::Sustain) => {
                if self.current.settle() {
                    (max_frames, self.current.next_smoothed())
                } else {
                    (ClockSamples(1), self.current.next_smoothed())
                }
            }
        }
    }

    /// Transition based on a trigger edge, expressed as a `f32`. Currently the actual value of the trigger is
    /// ignored, only whether it is 0. or non-zero.
    fn transition(&mut self, trigger: TriggerState) {
        match trigger {
            TriggerState::On => {
                let (new_target, new_state) =
                    (self.on_state_transition)(self.current.target_value(), self.state);
                self.current.set_value(new_target);
                self.state = Some(new_state);
            }
            TriggerState::Off => {
                if let Some(state) = &mut self.state {
                    *state = (self.off_state_transition)(*state);
                }
            }
        }
    }

    /// Process a range of the given buffers. If the buffers are `None`, just tick and do not actually write
    /// (used when the inputs are known to all be silent).
    fn process_range(&mut self, range: Range<ClockSamples>, mut buffers: Option<&mut ProcBuffers>) {
        let mut cur_index = range.start;
        while cur_index < range.end {
            let (count, current) = self.tick(range.end - cur_index);

            if let Some(buffers) = buffers.as_deref_mut() {
                debug_assert!(count > ClockSamples(0));

                let range = cur_index.0 as usize..(cur_index + count).0 as usize;

                let amp = lerp(self.lo, self.hi, current);
                for (input, output) in buffers.inputs.iter().zip(buffers.outputs.iter_mut()) {
                    // It's more ergonomic to put this `if` inside the loop, with the hope that LLVM will hoist the condition
                    // during optimization. The number of buffers is small though, so even without hoisting this will be
                    // reasonable efficient.
                    if close_enough(amp, Volume::SILENT.linear()) {
                        output[range.clone()].fill(0.);
                    } else if close_enough(amp, Volume::UNITY_GAIN.linear()) {
                        output[range.clone()].copy_from_slice(&input[range.clone()]);
                    } else {
                        for (in_sample, out_sample) in
                            input[range.clone()].iter().zip(&mut output[range.clone()])
                        {
                            *out_sample = *in_sample * amp;
                        }
                    }
                }
            }

            cur_index += count;
        }
    }
}

impl AudioNodeProcessor for AhdsrVolumeProcessor {
    fn process(
        &mut self,
        mut buffers: ProcBuffers,
        proc_info: &ProcInfo,
        mut events: NodeEventList,
    ) -> ProcessStatus {
        let mut cur_index = ClockSamples(0);

        // We check whether the inputs are silent in order to prevent unncessary processing.
        let all_silent = proc_info
            .in_silence_mask
            .all_channels_silent(buffers.inputs.len());

        events.for_each_patch::<AhdsrVolumeNode>(|patch| match patch {
            AhdsrVolumeNodePatch::Lo(lo) => {
                self.lo = lo.linear();
            }
            AhdsrVolumeNodePatch::Hi(hi) => {
                self.hi = hi.linear();
            }
            AhdsrVolumeNodePatch::Attack(val) => {
                self.attack = ClockSeconds(val);
                self.attack_rate = (val * self.sample_rate_recip) as _;
            }
            AhdsrVolumeNodePatch::Hold(val) => {
                self.hold = ClockSeconds(val);
                self.hold_samples =
                    ClockSamples::from_secs_f64(self.hold.0, self.sample_rate.get());
            }
            AhdsrVolumeNodePatch::Decay(val) => {
                self.decay = ClockSeconds(val);
                self.decay_rate =
                    (1. - self.sustain_proportion) * (val * self.sample_rate_recip) as f32;
            }
            AhdsrVolumeNodePatch::SustainProportion(sustain_proportion) => {
                self.sustain_proportion = sustain_proportion;
                self.decay_rate =
                    (1. - self.sustain_proportion) * (self.decay.0 * self.sample_rate_recip) as f32;
                self.release_rate = (1. - self.sustain_proportion)
                    * (self.release.0 * self.sample_rate_recip) as f32;
            }
            AhdsrVolumeNodePatch::Release(val) => {
                self.release = ClockSeconds(val);
                self.release_rate = self.sustain_proportion * (val * self.sample_rate_recip) as f32;
            }
            AhdsrVolumeNodePatch::TriggerMode(val) => {
                self.off_state_transition = val.state_transition();
            }
            AhdsrVolumeNodePatch::RetriggerMode(val) => {
                self.on_state_transition = val.state_transition();
            }
            AhdsrVolumeNodePatch::Triggered(timeline_event) => match timeline_event {
                TimelineEvent::Immediate(new_val) => self.transition(new_val),
                TimelineEvent::Deferred {
                    value: new_val,
                    time,
                } => {
                    let time_samples = ClockSamples::from_secs_f64(time.0, self.sample_rate.get());
                    let end = time_samples - proc_info.clock_samples;

                    self.process_range(cur_index..end, Some(&mut buffers).filter(|_| !all_silent));
                    cur_index = end;

                    self.transition(new_val);
                }
            },
        });

        self.process_range(
            cur_index..ClockSamples(proc_info.frames as i64),
            Some(&mut buffers).filter(|_| !all_silent),
        );

        if all_silent {
            ProcessStatus::ClearAllOutputs
        } else {
            ProcessStatus::outputs_not_silent()
        }
    }

    fn new_stream(&mut self, stream_info: &StreamInfo) {
        self.attack_rate = (self.attack.0 * stream_info.sample_rate_recip) as _;
        self.hold_samples = ClockSamples::from_secs_f64(self.hold.0, stream_info.sample_rate.get());
        self.decay_rate =
            (1. - self.sustain_proportion) * (self.decay.0 * stream_info.sample_rate_recip) as f32;
        self.release_rate =
            self.sustain_proportion * (self.release.0 * stream_info.sample_rate_recip) as f32;
        self.sample_rate = stream_info.sample_rate;
        self.sample_rate_recip = stream_info.sample_rate_recip;
    }
}
