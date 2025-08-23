//! EBU R128 loudness measurement.

use bevy_ecs::component::Component;
use core::sync::atomic::Ordering;
use ebur128::{Channel, EbuR128, Mode};
use firewheel::{
    channel_config::{ChannelConfig, ChannelCount},
    collector::ArcGc,
    diff::{Diff, Notify, Patch},
    event::ProcEvents,
    node::{AudioNode, AudioNodeProcessor, ProcBuffers, ProcExtra, ProcInfo},
};
use portable_atomic::AtomicF64;

/// A node that analyzes the loudness of an incoming signal.
#[derive(Debug, Default, Clone, Component, Diff, Patch)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct LoudnessNode {
    /// Reset the measurement.
    ///
    /// Touching the field is sufficient to trigger a reset.
    /// ```
    /// # use bevy_seedling::prelude::*;
    /// # let mut loudness = LoudnessNode::default();
    /// loudness.reset.notify();
    /// ```
    pub reset: Notify<bool>,
}

/// Configuration for [`LoudnessNode`].
#[derive(Debug, Default, Clone, Component, PartialEq)]
pub struct LoudnessConfig {
    /// The EBU R128 channel map.
    ///
    /// If no map is explicitly provided,
    /// this defaults to a simple stereo mapping.
    pub channel_map: Option<Vec<Channel>>,

    /// Whether to ignore processing when the input is silent.
    ///
    /// If you only care about the loudness of sounds or music while
    /// they're playing, and you want to ignore any silence, set this
    /// to `true`.
    ///
    /// Defaults to `false`.
    pub ignore_silence: bool,
}

#[derive(Debug, Default)]
struct InnerState {
    /// The global integrated loudness in LUFs.
    integrated: AtomicF64,

    /// The momentary (last 400ms) loudness in LUFs.
    momentary: AtomicF64,

    /// The short-term (last 3s) loudness in LUFs.
    short_term: AtomicF64,

    /// The loudness range (LRA) in LU.
    loudness_range: AtomicF64,

    /// The maximum sample peak from all frames that have been processed.
    sample_peak: Box<[AtomicF64]>,

    /// The maximum true peak from all frames that have been processed.
    true_peak: Box<[AtomicF64]>,
}

/// The shared atomics used by [`LoudnessNode`] to communicate
/// its current state.
///
/// Because audio is processed in chunks, this will typically
/// update at a rate of 40-80 hertz. As a result, you may not
/// observe changes on every frame.
#[derive(Debug, Clone)]
pub struct LoudnessState(ArcGc<InnerState>);

impl LoudnessState {
    /// The global integrated loudness in LUFs.
    pub fn integrated(&self) -> f64 {
        self.0.integrated.load(Ordering::Relaxed)
    }

    /// The momentary (last 400ms) loudness in LUFs.
    pub fn momentary(&self) -> f64 {
        self.0.momentary.load(Ordering::Relaxed)
    }

    /// The short-term (last 3s) loudness in LUFs.
    pub fn short_term(&self) -> f64 {
        self.0.short_term.load(Ordering::Relaxed)
    }

    /// The loudness range (LRA) in LU.
    pub fn loudness_range(&self) -> f64 {
        self.0.loudness_range.load(Ordering::Relaxed)
    }

    /// The maximum sample peak from all frames that have been processed,
    /// measured in dBFS.
    ///
    /// # Panics
    ///
    /// Panics if the channel index is out of bounds.
    pub fn sample_peak(&self, channel: usize) -> f64 {
        let max = self.0.sample_peak[channel].load(Ordering::Relaxed);

        20.0 * max.log10()
    }

    /// The maximum true peak from all frames that have been processed,
    /// measured in dBFS.
    ///
    /// # Panics
    ///
    /// Panics if the channel index is out of bounds.
    pub fn true_peak(&self, channel: usize) -> f64 {
        let max = self.0.true_peak[channel].load(Ordering::Relaxed);

        20.0 * max.log10()
    }
}

impl AudioNode for LoudnessNode {
    type Configuration = LoudnessConfig;

    fn info(&self, configuration: &Self::Configuration) -> firewheel::node::AudioNodeInfo {
        let channel_count = channel_count(configuration.channel_map.as_deref());

        let sample_peak = (0..channel_count).map(|_| Default::default()).collect();
        let true_peak = (0..channel_count).map(|_| Default::default()).collect();

        firewheel::node::AudioNodeInfo::new()
            .debug_name("loudness meter")
            .channel_config(ChannelConfig {
                num_inputs: channel_count.into(),
                num_outputs: ChannelCount::ZERO,
            })
            .custom_state(LoudnessState(ArcGc::new(InnerState {
                integrated: Default::default(),
                momentary: Default::default(),
                short_term: Default::default(),
                loudness_range: Default::default(),
                sample_peak,
                true_peak,
            })))
    }

    fn construct_processor(
        &self,
        configuration: &Self::Configuration,
        cx: firewheel::node::ConstructProcessorContext,
    ) -> impl firewheel::node::AudioNodeProcessor {
        LoudnessProcessor {
            analyzer: construct_analyzer(
                cx.stream_info.sample_rate.get(),
                configuration.channel_map.as_deref(),
            ),
            ignore_silence: configuration.ignore_silence,
            channel_map: configuration.channel_map.clone(),
            state: cx.custom_state().cloned().unwrap(),
        }
    }
}

struct LoudnessProcessor {
    analyzer: EbuR128,
    ignore_silence: bool,
    channel_map: Option<Vec<Channel>>,
    state: LoudnessState,
}

fn channel_count(channel_map: Option<&[Channel]>) -> usize {
    channel_map.map(|cm| cm.len()).unwrap_or(2)
}

fn construct_analyzer(sample_rate: u32, map: Option<&[Channel]>) -> EbuR128 {
    let channel_count = channel_count(map);
    let mut analyzer = EbuR128::new(channel_count as u32, sample_rate, Mode::all())
        .expect("failed to construct EBU R128 analyzer");

    if let Some(map) = map {
        analyzer
            .set_channel_map(map)
            .expect("failed to set EBU R128 channel map");
    }

    analyzer
}

impl AudioNodeProcessor for LoudnessProcessor {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        buffers: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> firewheel::node::ProcessStatus {
        for LoudnessNodePatch::Reset(_) in events.drain_patches::<LoudnessNode>() {
            self.analyzer.reset();
        }

        if self.ignore_silence
            && proc_info
                .in_silence_mask
                .all_channels_silent(buffers.inputs.len())
        {
            return firewheel::node::ProcessStatus::Bypass;
        }

        self.analyzer
            .add_frames_planar_f32(buffers.inputs)
            .expect("input channels should match configuration");

        let state = &self.state.0;
        state
            .integrated
            .store(self.analyzer.loudness_global().unwrap(), Ordering::Relaxed);
        state.momentary.store(
            self.analyzer.loudness_momentary().unwrap(),
            Ordering::Relaxed,
        );
        state.short_term.store(
            self.analyzer.loudness_shortterm().unwrap(),
            Ordering::Relaxed,
        );
        state
            .loudness_range
            .store(self.analyzer.loudness_range().unwrap(), Ordering::Relaxed);

        for i in 0..buffers.inputs.len() {
            state.sample_peak[i].store(
                self.analyzer.sample_peak(i as u32).unwrap(),
                Ordering::Relaxed,
            );

            state.true_peak[i].store(
                self.analyzer.true_peak(i as u32).unwrap(),
                Ordering::Relaxed,
            );
        }

        firewheel::node::ProcessStatus::Bypass
    }

    fn new_stream(&mut self, stream_info: &firewheel::StreamInfo) {
        if stream_info.sample_rate != stream_info.prev_sample_rate {
            // unfortunately, we have to re-construct here
            self.analyzer =
                construct_analyzer(stream_info.sample_rate.get(), self.channel_map.as_deref());
        }
    }
}
