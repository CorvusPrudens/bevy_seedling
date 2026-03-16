//! Audio graph and I/O initialization.
//!
//! `bevy_seedling` initializes audio in two stages.
//!
//! 1. In [`PreStartup`], the selected [`GraphConfiguration`] is
//!    established and [`InputDeviceInfo`] and [`OutputDeviceInfo`] entities
//!    are spawned.
//! 2. In [`PostUpdate`], the [`AudioStreamConfig`] resource is used to
//!    start the audio stream.
//!
//! This two-stage initialization allows systems in [`Startup`] to query
//! for device information and configure the audio stream before it's
//! initialized. Following this initialization in [`PostStartup`], any
//! further changes to [`AudioStreamConfig`] will cause the stream to
//! stop and restart with the new configuration.

use crate::{
    context::{AudioGraph, StreamRestartEvent, StreamStartEvent},
    edge::{AudioGraphInput, AudioGraphOutput, PendingConnections},
    node::{FirewheelNode, FirewheelNodeInfo},
};
use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_seedling_macros::{NodeLabel, PoolLabel};
use bevy_transform::prelude::Transform;
use std::fmt::Debug;

pub struct SeedlingStartup;

impl Plugin for SeedlingStartup {
    fn build(&self, app: &mut App) {
        app.preregister_asset_loader::<crate::sample::SampleLoader>(
            crate::sample::SampleLoader::extensions(),
        )
        .add_systems(
            PreStartup,
            (crate::context::initialize_context, insert_io, set_up_graph)
                .chain()
                .in_set(SeedlingStartupSystems::GraphSetup),
        )
        .add_systems(
            Last,
            add_default_transforms.before(crate::SeedlingSystems::Acquire),
        )
        .add_observer(connect_io::<StreamStartEvent>)
        .add_observer(connect_io::<StreamRestartEvent>);
    }
}

/// System sets for audio initialization.
#[derive(Debug, SystemSet, PartialEq, Eq, Hash, Clone)]
pub enum SeedlingStartupSystems {
    /// The graph configuration is initialized.
    ///
    /// This is run in the [`PreStartup`] schedule.
    GraphSetup,

    /// The audio stream is initialized with the selected I/O.
    ///
    /// This is run in the [`PostStartup`] schedule.
    StreamInitialization,
}

// fn restart_audio(
//     _: On<RestartAudioEvent>,
//     inputs: Query<&InputDeviceInfo>,
//     outputs: Query<&OutputDeviceInfo>,
//     mut config: ResMut<AudioStreamConfig>,
// ) -> Result {
//     // Since people often won't have any input
//     // at all, we'll be careful about selecting
//     // a new device.
//     if let Some(input) = &mut config.0.input {
//         // If the current input device no longer exists, attempt to
//         // fetch the default input, otherwise leaving the choice up
//         // to `cpal`.
//         if let Some(input_id) = &input.device_id {
//             if !inputs.iter().any(|i| i.id == input_id.to_string()) {
//                 // try to find the default input, or just pass `None`
//                 let new_input_id = inputs
//                     .iter()
//                     .find(|i| i.is_default)
//                     .map(|input| input.id.clone());
//                 input.device_id = new_input_id.and_then(|id| id.parse().ok());
//             }
//         }
//     }

//     if let Some(output_id) = &config.0.output.device_id {
//         // If the current output device no longer exists, attempt to
//         // fetch the default output, otherwise leaving the choice up
//         // to `cpal`.
//         if !outputs.iter().any(|i| i.id == output_id.to_string()) {
//             let new_output_name = outputs
//                 .iter()
//                 .find(|o| o.is_default)
//                 .map(|output| output.id.clone());
//             config.0.output.device_id = new_output_name.and_then(|id| id.parse().ok());
//         }
//     }

//     // set it changed in case the above made
//     // no modifications
//     config.set_changed();

//     Ok(())
// }

/// In [`GraphConfiguration::Game`], a sampler pool with spatial audio
/// processing is spawned.
///
/// This pool is unused in all other configurations,
/// so you can freely reuse it.
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SpatialPool;

/// For convenience, we automatically insert `Transform` components
/// on sample players with `SpatialPool`.
fn add_default_transforms(
    q: Query<
        Entity,
        (
            With<crate::prelude::SamplePlayer>,
            With<SpatialPool>,
            Without<Transform>,
        ),
    >,
    mut commands: Commands,
) {
    for entity in &q {
        commands.entity(entity).insert(Transform::default());
    }
}

/// The default bus for music.
///
/// In [`GraphConfiguration::Game`], a sampler pool specifically
/// for music is spawned. This pool is unused in all other configurations,
/// so you can freely reuse it.
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct MusicPool;

/// The default bus for sound effects.
///
/// In [`GraphConfiguration::Game`], all audio besides the [`MusicPool`] is
/// routed through this bus. This label is unused in all other configurations,
/// so you can freely reuse it.
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SoundEffectsBus;

/// Describes the initial audio graph configuration.
///
/// If you're not familiar with routing audio or are unsure what you need,
/// the [`Game`] configuration should provide a great starting point.
/// For those who want more control, [`Minimal`] and [`Empty`] will get
/// out of your way.
///
/// [`Game`]: GraphConfiguration::Game
/// [`Minimal`]: GraphConfiguration::Minimal
/// [`Empty`]: GraphConfiguration::Empty
#[derive(Debug, Default, Clone, Copy, Resource)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub enum GraphConfiguration {
    /// The default game template, suitable for smaller projects.
    ///
    /// After [`SeedlingStartupSystems::GraphSetup`] in [`PreStartup`], the graph will
    /// have the following shape:
    ///
    /// ```text
    /// ┌───────────┐┌───────────┐┌──────────┐┌─────────┐
    /// │DefaultPool││SpatialPool││DynamicBus││MusicPool│
    /// └┬──────────┘└┬──────────┘└┬─────────┘└┬────────┘
    /// ┌▽────────────▽────────────▽┐          │
    /// │SoundEffectsBus            │          │
    /// └┬──────────────────────────┘          │
    /// ┌▽─────────────────────────────────────▽┐
    /// │MainBus                                │
    /// └┬──────────────────────────────────────┘
    /// ┌▽──────┐
    /// │Limiter│
    /// └───────┘
    /// ```
    ///
    /// Additionally, each sampler pool includes a [`VolumeNode`] effect
    /// for each sample player, allowing you to dynamically modulate volume
    /// on a per-sample basis.
    ///
    /// Here's how you can create this configuration yourself:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn game_setup(mut commands: Commands) {
    ///     // Buses
    ///     commands
    ///         .spawn((MainBus, VolumeNode::default()))
    ///         .chain_node(LimiterNode::new(0.003, 0.15))
    ///         .connect(AudioGraphOutput);
    ///
    ///     commands.spawn((SoundEffectsBus, VolumeNode::default()));
    ///
    ///     commands
    ///         .spawn((
    ///             bevy_seedling::pool::dynamic::DynamicBus,
    ///             VolumeNode::default(),
    ///         ))
    ///         .connect(SoundEffectsBus);
    ///
    ///     // Pools
    ///     commands
    ///         .spawn((
    ///             SamplerPool(DefaultPool),
    ///             sample_effects![VolumeNode::default()],
    ///         ))
    ///         .connect(SoundEffectsBus);
    ///     commands
    ///         .spawn((
    ///             SamplerPool(SpatialPool),
    ///             sample_effects![VolumeNode::default(), SpatialBasicNode::default()],
    ///         ))
    ///         .connect(SoundEffectsBus);
    ///
    ///     commands.spawn((
    ///         SamplerPool(MusicPool),
    ///         sample_effects![VolumeNode::default()],
    ///     ));
    /// }
    /// ```
    ///
    /// [`VolumeNode`]: crate::prelude::VolumeNode
    #[default]
    Game,

    /// A minimal graph.
    ///
    /// After [`SeedlingStartupSystems::GraphSetup`] in [`PreStartup`], the graph will
    /// have the following shape:
    ///
    /// ```text
    /// ┌───────────┐┌──────────┐
    /// │DefaultPool││DynamicBus│
    /// └┬──────────┘└┬─────────┘
    /// ┌▽────────────▽┐
    /// │MainBus       │
    /// └──────────────┘
    /// ```
    ///
    /// As with the [`Game`] configuration, [`VolumeNode`]s are included
    /// in the [`DefaultPool`].
    ///
    /// Here's how you can create this configuration yourself:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn minimal_setup(mut commands: Commands) {
    ///     // Buses
    ///     commands
    ///         .spawn((MainBus, VolumeNode::default()))
    ///         .connect(AudioGraphOutput);
    ///
    ///     commands.spawn((
    ///         bevy_seedling::pool::dynamic::DynamicBus,
    ///         VolumeNode::default(),
    ///     ));
    ///
    ///     // Pools
    ///     commands.spawn((
    ///         SamplerPool(DefaultPool),
    ///         sample_effects![VolumeNode::default()],
    ///     ));
    /// }
    /// ```
    ///
    /// [`VolumeNode`]: crate::prelude::VolumeNode
    /// [`DefaultPool`]: crate::prelude::DefaultPool
    /// [`Game`]: GraphConfiguration::Game
    Minimal,

    /// A completely empty graph.
    ///
    /// You'll likely want to set up the [`DefaultPool`] and [`MainBus`],
    /// and possibly the [`DynamicBus`] if you want to support [dynamic pools][crate::pool::dynamic].
    ///
    /// [`DefaultPool`]: crate::prelude::DefaultPool
    /// [`MainBus`]: crate::prelude::MainBus
    /// [`DynamicBus`]: crate::pool::dynamic::DynamicBus
    Empty,
}

/// Insert the I/O markers, facilitating the graph setup.
///
/// We have to defer adding [`FirewheelNode`] because the audio context
/// isn't yet available.
fn insert_io(mut commands: Commands) {
    commands.spawn((AudioGraphInput, PendingConnections::default()));
    commands.spawn((AudioGraphOutput, PendingConnections::default()));
}

fn connect_io<E: Event>(
    _: On<E>,
    input: Query<Entity, With<AudioGraphInput>>,
    output: Query<Entity, With<AudioGraphOutput>>,
    mut commands: Commands,
    mut context: ResMut<crate::prelude::AudioGraph>,
) -> Result {
    context.with(|ctx| {
        let node_id = ctx.graph_in_node_id();
        let info = FirewheelNodeInfo::new(ctx.node_info(node_id).unwrap());
        commands
            .entity(input.single()?)
            .insert((info, Name::new("Audio Input Node")))
            .insert_if_new(FirewheelNode(node_id));

        let node_id = ctx.graph_out_node_id();
        let info = FirewheelNodeInfo::new(ctx.node_info(node_id).unwrap());
        commands
            .entity(output.single()?)
            .insert((info, Name::new("Audio Output Node")))
            .insert_if_new(FirewheelNode(node_id));

        Ok(())
    })
}

/// Set up the graph according to the initial configuration.
fn set_up_graph(mut commands: Commands, config: Res<GraphConfiguration>) {
    use crate::prelude::*;

    match *config {
        GraphConfiguration::Game => {
            // Buses
            commands
                .spawn((MainBus, VolumeNode::default(), Name::new("Main Bus")))
                .chain_node(LimiterNode::new(0.003, 0.15))
                .connect(AudioGraphOutput);

            commands.spawn((
                SoundEffectsBus,
                VolumeNode::default(),
                Name::new("Sound Effects Bus"),
            ));

            commands
                .spawn((
                    crate::pool::dynamic::DynamicBus,
                    VolumeNode::default(),
                    Name::new("Dynamic Bus"),
                ))
                .connect(SoundEffectsBus);

            // Pools
            commands
                .spawn((
                    SamplerPool(DefaultPool),
                    Name::new("Default Sampler Pool"),
                    sample_effects![VolumeNode::default()],
                ))
                .connect(SoundEffectsBus);

            commands
                .spawn((
                    SamplerPool(SpatialPool),
                    Name::new("Spatial Sampler Pool"),
                    sample_effects![VolumeNode::default(), SpatialBasicNode::default()],
                ))
                .connect(SoundEffectsBus);

            commands.spawn((
                SamplerPool(MusicPool),
                Name::new("Music Sampler Pool"),
                sample_effects![VolumeNode::default()],
            ));
        }
        GraphConfiguration::Minimal => {
            // Buses
            commands
                .spawn((MainBus, VolumeNode::default(), Name::new("Main Bus")))
                .connect(AudioGraphOutput);

            commands.spawn((
                crate::pool::dynamic::DynamicBus,
                VolumeNode::default(),
                Name::new("Dynamic Bus"),
            ));

            // Pools
            commands.spawn((
                SamplerPool(DefaultPool),
                Name::new("Default Sampler Pool"),
                sample_effects![VolumeNode::default()],
            ));
        }
        GraphConfiguration::Empty => {}
    }
}
