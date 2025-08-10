//! Audio graph and I/O initialization.

use crate::{
    context::AudioStreamConfig,
    edge::{AudioGraphInput, AudioGraphOutput, PendingConnections},
    node::FirewheelNode,
};
use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::prelude::*;
use bevy_seedling_macros::{NodeLabel, PoolLabel};
use bevy_transform::prelude::Transform;
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
                )
            }
        };

        app.preregister_asset_loader::<crate::sample::SampleLoader>(
            crate::sample::SampleLoader::extensions(),
        )
        .add_systems(
            PreStartup,
            (insert_io, set_up_graph)
                .chain()
                .in_set(SeedlingStartupSystems::GraphSetup),
        )
        .add_systems(
            PostStartup,
            (initialize_stream, connect_io)
                .chain()
                .in_set(SeedlingStartupSystems::StreamInitialization),
        )
        .add_systems(
            Last,
            add_default_transforms.before(crate::SeedlingSystems::Acquire),
        )
        .add_observer(fetch_io::<B>)
        .add_observer(restart_audio);
    }
}

/// System sets for audio initialization.
#[derive(Debug, SystemSet, PartialEq, Eq, Hash, Clone)]
pub enum SeedlingStartupSystems {
    /// I/O devices are fetched and the graph configuration is initialized.
    ///
    /// This is run in the [`PreStartup`] schedule.
    GraphSetup,

    /// The audio stream is initialized with the selected I/O.
    ///
    /// This is run in the [`PostStartup`] schedule.
    StreamInitialization,
}

/// When triggered globally, this event refreshes the audio
/// I/O device entities.
///
/// Any devices that are no longer available
/// are despawned.
#[derive(Event, Debug)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct FetchAudioIoEvent;

fn fetch_io<B: AudioBackend>(
    _: Trigger<FetchAudioIoEvent>,
    existing_inputs: Query<(Entity, &InputDeviceInfo)>,
    existing_outputs: Query<(Entity, &OutputDeviceInfo)>,
    mut commands: Commands,
) {
    let new_inputs = B::available_input_devices()
        .into_iter()
        .map(|input| InputDeviceInfo {
            name: input.name,
            num_channels: input.num_channels,
            is_default: input.is_default,
        })
        .collect::<Vec<_>>();
    let old_inputs: Vec<_> = existing_inputs.iter().collect();

    // check for new or changed inputs
    for new_input in &new_inputs {
        let matching = old_inputs.iter().find(|e| e.1.name == new_input.name);
        match matching {
            Some((entity, old_value)) => {
                if old_value != &new_input {
                    commands.entity(*entity).insert(new_input.clone());
                }
            }
            None => {
                debug!("Found audio input \"{}\"", new_input.name);
                commands.spawn((new_input.clone(), Name::new("Audio Input Device")));
            }
        }
    }

    // check for unavailable inputs
    for (entity, old_input) in old_inputs {
        if !new_inputs.iter().any(|i| i.name == old_input.name) {
            debug!("Audio input \"{}\" no longer available.", old_input.name);
            commands.entity(entity).despawn();
        }
    }

    let new_outputs = B::available_output_devices()
        .into_iter()
        .map(|output| OutputDeviceInfo {
            name: output.name,
            num_channels: output.num_channels,
            is_default: output.is_default,
        })
        .collect::<Vec<_>>();
    let old_outputs: Vec<_> = existing_outputs.iter().collect();

    // check for new or changed outputs
    for new_output in &new_outputs {
        let matching = old_outputs.iter().find(|e| e.1.name == new_output.name);
        match matching {
            Some((entity, old_value)) => {
                if old_value != &new_output {
                    commands.entity(*entity).insert(new_output.clone());
                }
            }
            None => {
                debug!("Found audio output \"{}\"", new_output.name);
                commands.spawn((new_output.clone(), Name::new("Audio Output Device")));
            }
        }
    }

    // check for unavailable outputs
    for (entity, old_output) in old_outputs {
        if !new_outputs.iter().any(|i| i.name == old_output.name) {
            debug!("Audio output \"{}\" no longer available.", old_output.name);
            commands.entity(entity).despawn();
        }
    }
}

/// When triggered globally, this attempts to automatically
/// restart the audio stream.
///
/// If the current devices are no longer available, this will
/// attempt to select the default input and output.
///
/// This only works with the default `cpal` backend.
#[derive(Event, Debug)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct RestartAudioEvent;

fn restart_audio(
    _: Trigger<RestartAudioEvent>,
    inputs: Query<&InputDeviceInfo>,
    outputs: Query<&OutputDeviceInfo>,
    mut config: ResMut<AudioStreamConfig>,
) {
    // Since people often won't have any input
    // at all, we'll be careful about selecting
    // a new device.
    if let Some(input) = &mut config.0.input {
        // If the current input device no longer exists, attempt to
        // fetch the default input, otherwise leaving the choice up
        // to `cpal`.
        if let Some(input_name) = &input.device_name {
            if !inputs.iter().any(|i| &i.name == input_name) {
                // try to find the default input, or just pass `None`
                let new_input_name = inputs
                    .iter()
                    .find(|i| i.is_default)
                    .map(|input| input.name.clone());
                input.device_name = new_input_name;
            }
        }
    }

    if let Some(output_name) = &config.0.output.device_name {
        // If the current output device no longer exists, attempt to
        // fetch the default output, otherwise leaving the choice up
        // to `cpal`.
        if !outputs.iter().any(|i| &i.name == output_name) {
            let new_output_name = outputs
                .iter()
                .find(|o| o.is_default)
                .map(|output| output.name.clone());
            config.0.output.device_name = new_output_name;
        }
    }

    // set it changed in case the above made
    // no modifications
    config.set_changed();
}

/// Information about an audio input device.
#[derive(Component, Debug, PartialEq, Clone)]
#[component(immutable)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct InputDeviceInfo {
    /// The device's name.
    pub name: String,
    /// The number of channels the device expects.
    pub num_channels: u16,
    /// Whether this device is the default selection.
    pub is_default: bool,
}

/// Information about an audio input device.
#[derive(Component, Debug, PartialEq, Clone)]
#[component(immutable)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct OutputDeviceInfo {
    /// The device's name.
    pub name: String,
    /// The number of channels the device expects.
    pub num_channels: u16,
    /// Whether this device is the default selection.
    pub is_default: bool,
}

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

/// In [`GraphConfiguration::Game`], a sampler pool specifically
/// for music is spawned.
///
/// This pool is unused in all other configurations,
/// so you can freely reuse it.
#[derive(PoolLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
pub struct MusicPool;

/// In [`GraphConfiguration::Game`], all audio besides the [`MusicPool`] is
/// routed through this bus.
///
/// This label is unused in all other configurations,
/// so you can freely reuse it.
#[derive(NodeLabel, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
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
    ///     commands.spawn((SfxBus, VolumeNode::default()));
    ///
    ///     commands
    ///         .spawn((
    ///             bevy_seedling::pool::dynamic::DynamicBus,
    ///             VolumeNode::default(),
    ///         ))
    ///         .connect(SfxBus);
    ///
    ///     // Pools
    ///     commands
    ///         .spawn((
    ///             SamplerPool(DefaultPool),
    ///             sample_effects![VolumeNode::default()],
    ///         ))
    ///         .connect(SfxBus);
    ///     commands
    ///         .spawn((
    ///             SamplerPool(SpatialPool),
    ///             sample_effects![VolumeNode::default(), SpatialBasicNode::default()],
    ///         ))
    ///         .connect(SfxBus);
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

#[derive(Resource)]
pub(crate) struct ConfigResource(pub GraphConfiguration);

/// Insert the I/O markers and devices, facilitating the graph setup.
///
/// We have to defer adding [`FirewheelNode`] because the audio context
/// isn't yet available.
fn insert_io(mut commands: Commands) {
    commands.spawn((AudioGraphInput, PendingConnections::default()));
    commands.spawn((AudioGraphOutput, PendingConnections::default()));
    commands.trigger(FetchAudioIoEvent);
}

fn connect_io(
    input: Query<Entity, With<AudioGraphInput>>,
    output: Query<Entity, With<AudioGraphOutput>>,
    mut commands: Commands,
    mut context: ResMut<crate::prelude::AudioContext>,
) -> Result {
    context.with(|ctx| {
        commands.entity(input.single()?).insert((
            FirewheelNode(ctx.graph_in_node_id()),
            Name::new("Audio Input Node"),
        ));

        commands.entity(output.single()?).insert((
            FirewheelNode(ctx.graph_out_node_id()),
            Name::new("Audio Output Node"),
        ));

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
                .spawn((MainBus, VolumeNode::default(), Name::new("Main Bus")))
                .chain_node(LimiterNode::new(0.003, 0.15))
                .connect(AudioGraphOutput);

            commands.spawn((SfxBus, VolumeNode::default(), Name::new("SFX Bus")));

            commands
                .spawn((
                    crate::pool::dynamic::DynamicBus,
                    VolumeNode::default(),
                    Name::new("Dynamic Bus"),
                ))
                .connect(SfxBus);

            // Pools
            commands
                .spawn((
                    SamplerPool(DefaultPool),
                    Name::new("Default Sampler Pool"),
                    sample_effects![VolumeNode::default()],
                ))
                .connect(SfxBus);

            commands
                .spawn((
                    SamplerPool(SpatialPool),
                    Name::new("Spatial Sampler Pool"),
                    sample_effects![VolumeNode::default(), SpatialBasicNode::default()],
                ))
                .connect(SfxBus);

            commands.spawn((
                SamplerPool(MusicPool),
                Name::new("Music Sampler Pool"),
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
            commands.spawn((
                SamplerPool(DefaultPool),
                sample_effects![VolumeNode::default()],
            ));
        }
        GraphConfiguration::Empty => {}
    }

    commands.remove_resource::<ConfigResource>();
}
