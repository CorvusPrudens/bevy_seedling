//! Limiter with configurable lookahead, attack and release.

use core::f32;
use std::num::NonZeroU32;

use bevy_ecs::component::Component;
use firewheel::{
    Volume,
    channel_config::{ChannelConfig, NonZeroChannelCount},
    diff::{Diff, Patch},
    dsp::filter::smoothing_filter::{SmoothingFilter, SmoothingFilterCoeff},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, ProcBuffers,
        ProcExtra, ProcInfo, ProcessStatus,
    },
};

/// The configuration for an [`AsymmetricalSmoothedParam`]
#[derive(Debug, Clone, Copy, PartialEq)]
struct AsymmetricalSmootherConfig {
    /// The amount of smoothing in seconds when the target is higher than the current value
    pub smooth_secs_up: f32,
    /// The amount of smoothing in seconds when the target is lower than the current value
    pub smooth_secs_down: f32,
}

/// A helper struct to smooth an f32 parameter, allowing different rates for up and down.
#[derive(Debug, Clone)]
struct AsymmetricalSmoothedParam {
    target_value: f32,
    target_times_a_up: f32,
    target_times_a_down: f32,
    filter: SmoothingFilter,
    coeff_up: SmoothingFilterCoeff,
    coeff_down: SmoothingFilterCoeff,
    smooth_secs_up: f32,
    smooth_secs_down: f32,
}

impl AsymmetricalSmoothedParam {
    /// Construct a new smoothed f32 parameter with the given configuration.
    pub fn new(value: f32, config: AsymmetricalSmootherConfig, sample_rate: NonZeroU32) -> Self {
        assert!(config.smooth_secs_up > 0.0);
        assert!(config.smooth_secs_down > 0.0);

        let coeff_up = SmoothingFilterCoeff::new(sample_rate, config.smooth_secs_up);
        let coeff_down = SmoothingFilterCoeff::new(sample_rate, config.smooth_secs_down);

        Self {
            target_value: value,
            target_times_a_up: value * coeff_up.a0,
            target_times_a_down: value * coeff_down.a0,
            filter: SmoothingFilter::new(value),
            coeff_up,
            coeff_down,
            smooth_secs_up: config.smooth_secs_up,
            smooth_secs_down: config.smooth_secs_down,
        }
    }

    /// The target value of the parameter.
    pub fn target_value(&self) -> f32 {
        self.target_value
    }

    /// Set the target value of the parameter.
    pub fn set_value(&mut self, value: f32) {
        self.target_value = value;
        self.target_times_a_up = value * self.coeff_up.a0;
        self.target_times_a_down = value * self.coeff_down.a0;
    }

    /// Set the smooth rate when the target value is higher than the current value.
    pub fn set_smooth_secs_up(&mut self, sample_rate: NonZeroU32, smooth_secs_up: f32) {
        let coeff_up = SmoothingFilterCoeff::new(sample_rate, smooth_secs_up);
        self.smooth_secs_up = smooth_secs_up;
        self.coeff_up = coeff_up;
    }

    /// Set the smooth rate when the target value is lower than the current value.
    pub fn set_smooth_secs_down(&mut self, sample_rate: NonZeroU32, smooth_secs_down: f32) {
        let coeff_down = SmoothingFilterCoeff::new(sample_rate, smooth_secs_down);
        self.smooth_secs_down = smooth_secs_down;
        self.coeff_down = coeff_down;
    }

    /// Return the next smoothed value.
    #[inline(always)]
    pub fn next_smoothed(&mut self) -> f32 {
        // Branchless alternation between up and down.
        let signum = (self.target_value() - self.filter.z1).signum();
        let less_factor = signum.max(0.);
        let more_factor = (-signum).max(0.);

        debug_assert!(less_factor == 1. || more_factor == 1.);

        let target_times_a =
            less_factor * self.target_times_a_up + more_factor * self.target_times_a_down;
        let coeff_b1 = less_factor * self.coeff_up.b1 + more_factor * self.coeff_down.b1;
        self.filter.process_sample_a(target_times_a, coeff_b1)
    }

    /// Update the sample rate.
    pub fn update_sample_rate(&mut self, sample_rate: NonZeroU32) {
        self.coeff_up = SmoothingFilterCoeff::new(sample_rate, self.smooth_secs_up);
        self.coeff_down = SmoothingFilterCoeff::new(sample_rate, self.smooth_secs_down);
        self.target_times_a_up = self.target_value() * self.coeff_up.a0;
        self.target_times_a_down = self.target_value() * self.coeff_down.a0;
    }
}

/// Buffer to incrementally calculate a maximum value of a buffer with the minimum number of comparisons.
#[derive(Debug, Clone)]
struct IncrementalMax {
    // First item is unused for convenience. Buffer length is rounded up to an even number.
    buffer: Box<[f32]>,
    length: usize,
    leaf_offset: usize,
}

impl IncrementalMax {
    #[inline]
    fn get_index(&self, i: usize) -> usize {
        self.leaf_offset + i
    }

    /// Create a new [`IncrementalMax`].
    pub fn new(length: usize) -> Self {
        let leaf_offset = length.next_power_of_two();
        Self {
            buffer: vec![0.; leaf_offset + length + (length & 1)].into(),
            length,
            leaf_offset,
        }
    }

    /// The length of the internal buffer.
    #[inline]
    // `is_empty` doesn't make sense for this type, the length should always be >0
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Get the maximum of the values in the buffer.
    #[inline]
    pub fn max(&self) -> f32 {
        self.buffer[1]
    }

    /// Set a value at the given index.
    pub fn set(&mut self, index: usize, value: f32) {
        let mut i = self.get_index(index);

        self.buffer[i] = value;

        while i > 1 {
            let max = self.buffer[i].max(self.buffer[i ^ 1]);
            i >>= 1;
            self.buffer[i] = max;
        }
    }
}

/// Configuration for a [`LimiterNode`].
#[derive(Debug, Clone, Component, PartialEq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct LimiterConfig {
    /// The limiter lookahead.
    ///
    /// This is how much latency will be introduced in order to ensure that the
    /// limiter will reduce volume in time for high peaks to be reduced.
    ///
    /// By default, it will set the lookahead to the same as the `attack` of the limiter.
    pub lookahead: Option<f32>,
    /// How much extra headroom to add.
    ///
    /// The intended target volume will be unity gain minus this.
    ///
    /// By default, no headroom is added.
    pub headroom: Volume,
    /// How many channels to take as input/return as output.
    ///
    /// By default, this is stereo.
    pub channels: NonZeroChannelCount,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            lookahead: None,
            headroom: Volume::Decibels(0.),
            channels: NonZeroChannelCount::STEREO,
        }
    }
}

/// A limiter node with lookahead.
///
/// By default the lookahead will be set to `attack`, see [`LimiterConfig`] to see how to
/// set lookahead to something else.
#[derive(Diff, Patch, Debug, Clone, Component)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct LimiterNode {
    /// How long it takes to react to increases in volume, in seconds.
    ///
    /// By default, this is 0.05s.
    pub attack: f32,
    /// How long it takes to react to decreases in volume, in seconds.
    ///
    /// By default, this is 0.2s.
    pub release: f32,
}

impl LimiterNode {
    /// Create a new [`LimiterNode`].
    pub fn new(attack: f32, release: f32) -> Self {
        Self { attack, release }
    }
}

impl Default for LimiterNode {
    fn default() -> Self {
        Self::new(0.05, 0.2)
    }
}

/// Look-ahead limiter.
struct Limiter {
    lookahead: f32,
    headroom: Volume,
    sample_rate: NonZeroU32,
    reducer: IncrementalMax,
    follower: AsymmetricalSmoothedParam,
    buffer: Box<[f32]>,
    num_channels: u32,
    max_buffer_length: NonZeroU32,
    index: usize,
}

impl AudioNode for LimiterNode {
    type Configuration = LimiterConfig;

    fn info(&self, config: &Self::Configuration) -> firewheel::node::AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("limiter")
            .channel_config(ChannelConfig {
                num_inputs: config.channels.get(),
                num_outputs: config.channels.get(),
            })
    }

    fn construct_processor(
        &self,
        config: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> impl AudioNodeProcessor {
        Limiter::new(
            cx.stream_info.sample_rate,
            config.lookahead.unwrap_or(self.attack),
            self.attack,
            self.release,
            config.headroom,
            config.channels.get().get(),
            cx.stream_info.max_block_frames,
        )
    }
}

fn reducer_buf_size(sample_rate: NonZeroU32, lookahead: f32) -> usize {
    (sample_rate.get() as f32 * lookahead).round().max(1.) as usize
}

impl Limiter {
    fn advance(&mut self) {
        self.index = (self.index + 1) % self.reducer.len();
    }

    fn new(
        sample_rate: NonZeroU32,
        lookahead: f32,
        attack: f32,
        release: f32,
        headroom: Volume,
        num_channels: u32,
        max_buffer_length: NonZeroU32,
    ) -> Self {
        let follower = AsymmetricalSmoothedParam::new(
            1.,
            AsymmetricalSmootherConfig {
                smooth_secs_up: attack,
                smooth_secs_down: release,
            },
            sample_rate,
        );
        let reducer = IncrementalMax::new(reducer_buf_size(sample_rate, lookahead));
        let buffer = vec![0.; reducer.len() * num_channels as usize].into();

        Limiter {
            // Updated when given a new stream
            sample_rate,
            buffer,
            num_channels,
            max_buffer_length,
            reducer,
            index: 0,

            // Static
            lookahead,
            headroom,
            follower,
        }
    }
}

impl AudioNodeProcessor for Limiter {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        buffers: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        for patch in events.drain_patches::<LimiterNode>() {
            match patch {
                LimiterNodePatch::Attack(atk) => {
                    self.follower.set_smooth_secs_up(self.sample_rate, atk);
                }
                LimiterNodePatch::Release(rel) => {
                    self.follower.set_smooth_secs_down(self.sample_rate, rel);
                }
            }
        }

        if proc_info
            .in_silence_mask
            .all_channels_silent(buffers.inputs.len())
            && self.buffer.iter().all(|s| *s == 0.)
        {
            return ProcessStatus::ClearAllOutputs;
        }

        let frame_size = proc_info.frames;

        for i in 0..frame_size {
            let amplitude = buffers
                .inputs
                .iter()
                .map(|input| input[i])
                .filter(|x| x.is_finite())
                .fold(0f32, |amp, x| amp.max(x.abs()));

            self.reducer.set(self.index, amplitude);
            let max = self.reducer.max();

            self.follower.set_value(max * self.headroom.amp());

            let limit = self.follower.next_smoothed().max(1.);

            for ((current_chan, out_chan), input_chan) in self
                .buffer
                .chunks_exact_mut(self.num_channels as usize)
                .nth(self.index)
                .unwrap()
                .iter_mut()
                .zip(&mut *buffers.outputs)
                .zip(buffers.inputs)
            {
                out_chan[i] = *current_chan / limit;
                *current_chan = input_chan[i];
            }

            self.advance();
        }

        ProcessStatus::outputs_not_silent()
    }

    fn new_stream(&mut self, stream_info: &firewheel::StreamInfo) {
        self.index = 0;
        self.sample_rate = stream_info.sample_rate;
        self.max_buffer_length = stream_info.max_block_frames;

        self.reducer =
            IncrementalMax::new(reducer_buf_size(stream_info.sample_rate, self.lookahead));

        self.follower.update_sample_rate(stream_info.sample_rate);

        let new_buffer_size = self.reducer.len() * self.num_channels as usize;

        if self.buffer.len() == new_buffer_size {
            self.buffer.fill(0.);
        } else {
            self.buffer = vec![0.; new_buffer_size].into();
        }
    }
}
