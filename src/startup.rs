//! Audio stream initialization.

use crate::{
    context::AudioStreamConfig,
    edge::{AudioGraphInput, AudioGraphOutput, PendingConnections},
    node::FirewheelNode,
    nodes::limiter::LimiterNode,
};
use bevy::prelude::*;
use bevy_seedling_macros::{NodeLabel, PoolLabel};
use core::marker::PhantomData;
use firewheel::backend::AudioBackend;

pub(crate) struct SeedlingStartup<B: AudioBackend> {
    firewheel_config: crate::prelude::FirewheelConfig,
    _backend: PhantomData<fn() -> B>,
}

impl<B: AudioBackend> SeedlingStartup<B> {
    pub fn new(firewheel_config: crate::prelude::FirewheelConfig) -> Self {
        Self {
            firewheel_config,
            _backend: PhantomData,
        }
    }
}

impl<B: AudioBackend> Plugin for SeedlingStartup<B>
where
    B: 'static,
    B::Config: Clone + Send + Sync + 'static,
    B::StreamError: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        let initialize_stream = {
            let config = self.firewheel_config;

            move |mut commands: Commands,
                  server: Res<AssetServer>,
                  stream_config: Res<AudioStreamConfig<B>>| {
                crate::context::initialize_context::<B>(
                    config,
                    stream_config.0.clone(),
                    &mut commands,
                    &server,
                );
            }
        };

        app.preregister_asset_loader::<crate::sample::SampleLoader>(
            crate::sample::SampleLoader::extensions(),
        )
        .add_systems(
            PreStartup,
            (insert_io::<B>, set_up_graph)
                .chain()
                .in_set(SeedlingStartupSystems::GraphSetup),
        )
        .add_systems(
            PostStartup,
            (initialize_stream, connect_io)
                .chain()
                .in_set(SeedlingStartupSystems::StreamInitialization),
        );
    }
}

/// System sets for audio initialization.
#[derive(Debug, SystemSet, PartialEq, Eq, Hash, Clone)]
pub enum SeedlingStartupSystems {
    /// I/O devices are fetched and the graoh configuration is initialized.
    ///
    /// This is run in the [`PreStartup`] schedule.
    GraphSetup,

    /// The audio stream is initialized with the selected I/O.
    ///
    /// This is run in the [`PostStartup`] schedule.
    StreamInitialization,
}

/// Information about an audio input device.
#[derive(Component, Debug)]
#[component(immutable)]
pub struct InputDeviceInfo(firewheel::backend::DeviceInfo);

impl InputDeviceInfo {
    /// The device's name.
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// The number of channels the device expects.
    pub fn num_channels(&self) -> u16 {
        self.0.num_channels
    }

    /// Whether this device is the default selection.
    pub fn is_default(&self) -> bool {
        self.0.is_default
    }
}

/// Information about an audio input device.
#[derive(Component, Debug)]
#[component(immutable)]
pub struct OutputDeviceInfo(firewheel::backend::DeviceInfo);

impl OutputDeviceInfo {
    /// The device's name.
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// The number of channels the device expects.
    pub fn num_channels(&self) -> u16 {
        self.0.num_channels
    }

    /// Whether this device is the default selection.
    pub fn is_default(&self) -> bool {
        self.0.is_default
    }
}

/// In [`GraphConfiguration::Game`], a sampler pool with spatial audio
/// processing is spawned.
///
/// This pool is unused in all other configurations,
/// so you can freely reuse it.
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpatialPool;

/// In [`GraphConfiguration::Game`], a sampler pool specifically
/// for music is spawned.
///
/// This pool is unused in all other configurations,
/// so you can freely reuse it.
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MusicPool;

/// In [`GraphConfiguration::Game`], all audio besides the [`MusicPool`] is
/// routed through this bus.
///
/// This label is unused in all other configurations,
/// so you can freely reuse it.
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SfxBus;

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
#[derive(Debug, Default, Clone, Copy)]
pub enum GraphConfiguration {
    /// The default game template, suitable for smaller projects.
    ///
    /// After [`SeedlingInitSystems::Devices`] in [`PreStartup`], the graph will
    /// have the following shape:
    ///
    /// ```text
    /// ┌───────────┐┌───────────┐┌──────────┐┌─────────┐
    /// │DefaultPool││SpatialPool││DynamicBus││MusicPool│
    /// └┬──────────┘└┬──────────┘└┬─────────┘└┬────────┘
    /// ┌▽────────────▽────────────▽┐          │
    /// │SfxBus                     │          │
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
    /// These [`VolumeNode`]s are _not_ included in the other configurations,
    /// so you'll need to insert them manually if you migrate.
    ///
    /// Here's how you can create this configuration yourself:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn game_setup(mut commands: Commands) {
    ///    // Buses
    ///    commands
    ///        .spawn((MainBus, VolumeNode::default()))
    ///        .chain_node(LimiterNode::new(0.003, 0.15))
    ///        .connect(AudioGraphOutput);
    ///
    ///    commands.spawn((SfxBus, VolumeNode::default()));
    ///
    ///    commands
    ///        .spawn((crate::pool::dynamic::DynamicBus, VolumeNode::default()))
    ///        .connect(SfxBus);
    ///
    ///    // Pools
    ///    commands
    ///        .spawn((
    ///            SamplerPool(DefaultPool),
    ///            sample_effects![VolumeNode::default()],
    ///        ))
    ///        .connect(SfxBus);
    ///    commands
    ///        .spawn((
    ///            SamplerPool(SpatialPool),
    ///            sample_effects![VolumeNode::default(), SpatialBasicNode::default()],
    ///        ))
    ///        .connect(SfxBus);
    ///
    ///    commands.spawn((
    ///        SamplerPool(MusicPool),
    ///        sample_effects![VolumeNode::default()],
    ///    ));
    /// }
    /// ```
    ///
    /// [`VolumeNode`]: crate::prelude::VolumeNode
    #[default]
    Game,

    /// A minimal graph.
    ///
    /// After [`SeedlingInitSystems::Devices`] in [`PreStartup`], the graph will
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
    /// Unlike the [`Game`] configuration, [`VolumeNode`]s are not inserted
    /// into the [`DefaultPool`].
    ///
    /// Here's how you can create this configuration yourself:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::prelude::*;
    /// fn minimal_setup(mut commands: Commands) {
    ///    // Buses
    ///    commands
    ///        .spawn((MainBus, VolumeNode::default()))
    ///        .connect(AudioGraphOutput);
    ///
    ///    commands.spawn((crate::pool::dynamic::DynamicBus, VolumeNode::default()));
    ///
    ///    // Pools
    ///    commands.spawn(SamplerPool(DefaultPool));
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

#[derive(Resource)]
pub(crate) struct ConfigResource(pub GraphConfiguration);

/// Insert the I/O markers and devices, facilitating the graph setup.
///
/// We have to defer adding [`FirewheelNode`] because the audio context
/// isn't yet available.
fn insert_io<B: AudioBackend>(mut commands: Commands) {
    // I/O
    commands.spawn((AudioGraphInput, PendingConnections::default()));
    commands.spawn((AudioGraphOutput, PendingConnections::default()));

    // Devices
    commands.spawn_batch(
        B::available_input_devices()
            .into_iter()
            .map(InputDeviceInfo),
    );

    commands.spawn_batch(
        B::available_output_devices()
            .into_iter()
            .map(OutputDeviceInfo),
    );
}

fn connect_io(
    input: Query<Entity, With<AudioGraphInput>>,
    output: Query<Entity, With<AudioGraphOutput>>,
    mut commands: Commands,
    mut context: ResMut<crate::prelude::AudioContext>,
) -> Result {
    context.with(|ctx| {
        commands
            .entity(input.single()?)
            .insert(FirewheelNode(ctx.graph_in_node_id()));

        commands
            .entity(output.single()?)
            .insert(FirewheelNode(ctx.graph_out_node_id()));

        Ok(())
    })
}

/// Set up the graph according to the initial configuration.
fn set_up_graph(mut commands: Commands, config: Res<ConfigResource>) {
    use crate::prelude::*;

    match config.0 {
        GraphConfiguration::Game => {
            // Buses
            commands
                .spawn((MainBus, VolumeNode::default()))
                .chain_node(LimiterNode::new(0.003, 0.15))
                .connect(AudioGraphOutput);

            commands.spawn((SfxBus, VolumeNode::default()));

            commands
                .spawn((crate::pool::dynamic::DynamicBus, VolumeNode::default()))
                .connect(SfxBus);

            // Pools
            commands
                .spawn((
                    SamplerPool(DefaultPool),
                    sample_effects![VolumeNode::default()],
                ))
                .connect(SfxBus);

            commands
                .spawn((
                    SamplerPool(SpatialPool),
                    sample_effects![VolumeNode::default(), SpatialBasicNode::default()],
                ))
                .connect(SfxBus);

            commands.spawn((
                SamplerPool(MusicPool),
                sample_effects![VolumeNode::default()],
            ));
        }
        GraphConfiguration::Minimal => {
            // Buses
            commands
                .spawn((MainBus, VolumeNode::default()))
                .connect(AudioGraphOutput);

            commands.spawn((crate::pool::dynamic::DynamicBus, VolumeNode::default()));

            // Pools
            commands.spawn(SamplerPool(DefaultPool));
        }
        GraphConfiguration::Empty => {}
    }

    commands.remove_resource::<ConfigResource>();
}
