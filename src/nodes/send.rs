//! A convenient node for routing to sends.

use crate::{
    edge::{Disconnect, EdgeTarget, PendingConnections, PendingEdge},
    node::follower::FollowerOf,
    prelude::MainBus,
};
use bevy_ecs::prelude::*;
use firewheel::{
    SilenceMask, Volume,
    channel_config::{ChannelConfig, ChannelCount, NonZeroChannelCount},
    diff::{Diff, Patch},
    dsp::volume::DEFAULT_AMP_EPSILON,
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, ProcBuffers,
        ProcExtra, ProcInfo, ProcessStatus,
    },
    param::smoother::{SmoothedParamBuffer, SmootherConfig},
};

/// A convenient node for routing to sends.
///
/// [`SendNode`] has two outputs: one for passing audio along
/// untouched, and another to route to arbitrary sends.
/// This is especially useful for dynamic pools, where you
/// otherwise have no routing control.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::prelude::*;
/// // Assuming this points to some expensive effects chain.
/// #[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
/// struct ExpensiveChain;
///
/// fn dynamic_send(mut commands: Commands, server: Res<AssetServer>) {
///     commands.spawn((
///         SamplePlayer::new(server.load("my_sample.wav")),
///         sample_effects![SendNode::new(Volume::UNITY_GAIN, ExpensiveChain)],
///     ));
/// }
/// ```
///
/// The signal simply passing through [`SendNode`] is untouched, while the
/// send output has [`SendNode::send_volume`] applied.
#[derive(Diff, Patch, Debug, Clone, Component)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "reflect", reflect(from_reflect = false))]
pub struct SendNode {
    /// The send volume.
    ///
    /// This affects only the send outputs.
    pub send_volume: Volume,

    #[diff(skip)]
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    pub(crate) target: EdgeTarget,
}

pub(crate) fn connect_sends(
    mut sends: Query<
        (
            Entity,
            &SendNode,
            &SendConfig,
            Option<&mut PendingConnections>,
        ),
        Added<SendNode>,
    >,
    mut commands: Commands,
) {
    for (entity, send_node, send_config, pending) in sends.iter_mut() {
        let target = send_node.target.clone();

        let total_channels = send_config.channels.get().get();
        let ports = (0..total_channels)
            .map(|c| (c + total_channels, c))
            .collect();

        let pending_connection = PendingEdge::new(target, Some(ports));

        match pending {
            Some(mut pending) => {
                pending.push(pending_connection);
            }

            None => {
                let mut pending = PendingConnections::default();
                pending.push(pending_connection);

                let default_ports = (0..total_channels).map(|c| (c, c)).collect();

                pending.push(PendingEdge::new(MainBus, Some(default_ports)));
                commands.entity(entity).insert(pending);
            }
        }
    }
}

// TODO: make this more reactive
pub(crate) fn update_remote_sends(
    remote: Query<&SendNode>,
    mut sends: Query<
        (
            Entity,
            &SendNode,
            &FollowerOf,
            &SendConfig,
            &mut PendingConnections,
        ),
        Changed<SendNode>,
    >,
    mut commands: Commands,
) {
    for (send_entity, send_params, follower, send_config, mut pending) in sends.iter_mut() {
        let Ok(remote_node) = remote.get(follower.0) else {
            continue;
        };

        let old_target = send_params.target.clone();
        let new_target = remote_node.target.clone();

        if old_target == new_target {
            continue;
        }

        let total_channels = send_config.channels.get().get();
        let ports = (0..total_channels)
            .map(|c| (c + total_channels, c))
            .collect();

        let pending_connection = PendingEdge::new(new_target, Some(ports));
        pending.push(pending_connection);

        commands.entity(send_entity).disconnect(old_target);
    }
}

impl SendNode {
    /// Construct a new [`Send`] that taps out to `send_target`.
    pub fn new(send_volume: Volume, send_target: impl Into<EdgeTarget>) -> Self {
        Self {
            send_volume,
            target: send_target.into(),
        }
    }
}

/// [`SendNode`]'s configuration.
#[derive(Debug, Component, Clone, PartialEq)]
pub struct SendConfig {
    /// The number of channels in this node's direct output and send output.
    pub channels: NonZeroChannelCount,

    /// The amount of smoothing to apply to the send volume.
    ///
    /// This defaults to 5 milliseconds.
    pub smooth_config: SmootherConfig,
}

impl Default for SendConfig {
    fn default() -> Self {
        Self {
            channels: NonZeroChannelCount::STEREO,
            smooth_config: Default::default(),
        }
    }
}

impl AudioNode for SendNode {
    type Configuration = SendConfig;

    fn info(&self, config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("low-pass filter")
            .channel_config(ChannelConfig {
                num_inputs: config.channels.get(),
                // TODO: remove panic
                num_outputs: ChannelCount::new(config.channels.get().get() * 2)
                    .expect("send channel count must not exceed 32"),
            })
    }

    fn construct_processor(
        &self,
        config: &Self::Configuration,
        ctx: ConstructProcessorContext,
    ) -> impl AudioNodeProcessor {
        // We pre-calculate the silence mask since it's kind of annoying.
        let mut silence_mask = 0;
        for i in 0..config.channels.get().get() {
            silence_mask |= 1 << i;
        }

        SendProcessor {
            gain: SmoothedParamBuffer::new(
                self.send_volume.amp(),
                config.smooth_config,
                ctx.stream_info,
            ),
            silence_mask: !silence_mask,
        }
    }
}

struct SendProcessor {
    gain: SmoothedParamBuffer,
    silence_mask: u64,
}

impl AudioNodeProcessor for SendProcessor {
    fn process(
        &mut self,
        proc_info: &ProcInfo,
        ProcBuffers { inputs, outputs }: ProcBuffers,
        events: &mut ProcEvents,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        for SendNodePatch::SendVolume(v) in events.drain_patches::<SendNode>() {
            self.gain.set_value(v.amp_clamped(DEFAULT_AMP_EPSILON));
        }

        if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
            return ProcessStatus::ClearAllOutputs;
        }

        let gain_is_silent = !self.gain.is_smoothing() && self.gain.target_value() < 0.00001;

        if gain_is_silent {
            for frame in 0..proc_info.frames {
                for (i, input) in inputs.iter().enumerate() {
                    outputs[i][frame] = input[frame];
                }
            }

            ProcessStatus::OutputsModified {
                out_silence_mask: SilenceMask(self.silence_mask),
            }
        } else {
            let gain_buffer = self.gain.get_buffer(proc_info.frames).0;
            for frame in 0..proc_info.frames {
                for (i, input) in inputs.iter().enumerate() {
                    outputs[i][frame] = input[frame];
                    outputs[i + inputs.len()][frame] = input[frame] * gain_buffer[frame];
                }
            }

            ProcessStatus::outputs_not_silent()
        }
    }
}
