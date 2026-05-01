//! Audio graph and I/O initialization.
//!
//! `bevy_seedling` initializes audio in two stages.
//!
//! 1. In [`PreStartup`], the selected [`AudioGraphTemplate`] is populated.
//! 2. In [`PostUpdate`], the [`AudioStreamConfig`] resource is used to
//!    start the audio stream.
//!
//! This two-stage initialization allows systems in [`Startup`] to
//! configure the audio stream before it's
//! initialized. Following this initialization in [`PostStartup`], any
//! further changes to [`AudioStreamConfig`] will cause the stream to
//! stop and restart with the new configuration.
//!
//! [`AudioStreamConfig`]: crate::prelude::AudioStreamConfig

use crate::{
    context::{AudioContext, StreamRestartEvent, StreamStartEvent},
    edge::{AudioGraphInput, AudioGraphOutput, PendingConnections},
    node::{FirewheelNode, FirewheelNodeInfo},
};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_seedling_macros::{NodeLabel, PoolLabel};
use bevy_transform::prelude::Transform;
use core::fmt::Debug;

pub(super) struct GraphPlugin;

impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioGraphTemplate>()
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

/// In [`AudioGraphTemplate::Game`], a sampler pool with spatial audio
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
/// In [`AudioGraphTemplate::Game`], a sampler pool specifically
/// for music is spawned. This pool is unused in all other configurations,
/// so you can freely reuse it.
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct MusicPool;

/// The default bus for sound effects.
///
/// In [`AudioGraphTemplate::Game`], all audio besides the [`MusicPool`] is
/// routed through this bus. This label is unused in all other configurations,
/// so you can freely reuse it.
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct SoundEffectsBus;

/// Provides a template for the initial audio graph configuration.
///
/// If you're not familiar with routing audio or are unsure what you need,
/// the [`Game`] template should provide a great starting point.
/// For those who want more control, [`Minimal`] and [`Empty`] will get
/// out of your way.
///
/// [`Game`]: AudioGraphTemplate::Game
/// [`Minimal`]: AudioGraphTemplate::Minimal
/// [`Empty`]: AudioGraphTemplate::Empty
#[derive(Debug, Default, Clone, Copy, Resource)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub enum AudioGraphTemplate {
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
    /// [`Game`]: AudioGraphTemplate::Game
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
    mut context: ResMut<AudioContext>,
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
fn set_up_graph(mut commands: Commands, config: Res<AudioGraphTemplate>) {
    use crate::prelude::*;

    match *config {
        AudioGraphTemplate::Game => {
            // Buses

            #[allow(unused_mut)]
            let mut main_bus =
                commands.spawn((MainBus, VolumeNode::default(), Name::new("Main Bus")));

            #[cfg(feature = "limiter")]
            {
                main_bus
                    .chain_node(LimiterNode::new(0.003, 0.15))
                    .connect(AudioGraphOutput);
            }
            #[cfg(not(feature = "limiter"))]
            {
                main_bus.connect(AudioGraphOutput);
            }

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
        AudioGraphTemplate::Minimal => {
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
        AudioGraphTemplate::Empty => {}
    }
}
