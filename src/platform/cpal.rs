//! Platform abstractions for `cpal`.

use std::sync;

use bevy_app::prelude::*;
use bevy_asset::AssetServer;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashSet;
use firewheel::{
    FirewheelCtx,
    backend::AudioBackend,
    cpal::{
        CpalBackend,
        cpal::{self},
    },
};

use crate::{
    context::{AudioContext, AudioStreamConfig, StreamStartEvent},
    platform::*,
    prelude::SeedlingStartupSystems,
};

pub struct CpalPlatformPlugin;

impl Plugin for CpalPlatformPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, spawn_context)
            .add_systems(
                PostStartup,
                start_stream.in_set(SeedlingStartupSystems::StreamInitialization),
            )
            .add_observer(observe_fetch_devices);
    }
}

#[derive(Component)]
struct CpalBackendMarker;

fn spawn_context(config: Res<AudioContextConfig>, mut commands: Commands) {
    let context = AudioContext::new::<CpalBackend>(config.0);
    commands.spawn((
        context,
        CpalBackendMarker,
        HostId(CpalBackend::enumerator().default_host().host_id()),
    ));
}

fn observe_fetch_devices(
    trigger: On<FetchDevices>,
    backend: Query<
        (
            &HostId<cpal::HostId>,
            Option<&InputDevices>,
            Option<&OutputDevices>,
        ),
        With<CpalBackendMarker>,
    >,
    input_query: Query<(
        Entity,
        &DeviceId<cpal::DeviceId>,
        // &InputChannels,
        // &SampleRates,
        Has<DefaultInputDevice>,
    )>,
    output_query: Query<(
        Entity,
        &DeviceId<cpal::DeviceId>,
        // &OutputChannels,
        // &SampleRates,
        Has<DefaultOutputDevice>,
    )>,
    mut commands: Commands,
) -> Result {
    let Ok((host, input_devices, output_devices)) = backend.get(trigger.entity) else {
        return Ok(());
    };

    let enumerator = CpalBackend::enumerator();
    let host = enumerator.get_host(host.0)?;
    let inputs = host.input_devices();

    let old_inputs: Vec<_> = input_devices.iter().flat_map(|d| d.iter()).collect();

    let mut new_set = HashSet::new();
    for input in inputs {
        let matching = input_query
            .iter_many(old_inputs.iter())
            .find(|(_, id, ..)| id.0 == input.id);

        new_set.insert(input.id.clone());

        // let extra = host.get_device(&input.id).ok_or("Failed to acquire extra information")?;
        // let input = extra.supported_input_configs()?;
        // for config in input {

        // }

        match matching {
            Some((entity, _id, is_default)) => {
                if is_default != input.is_default {
                    if is_default {
                        commands.entity(entity).insert(DefaultInputDevice);
                    } else {
                        commands.entity(entity).remove::<DefaultInputDevice>();
                    }
                }
            }
            None => {
                let mut entity = commands.spawn(DeviceId(input.id));
                if input.is_default {
                    entity.insert(DefaultInputDevice);
                }

                if let Some(name) = input.name {
                    entity.insert(Name::new(name));
                }
            }
        }
    }

    for (entity, id, ..) in input_query.iter_many(old_inputs.iter()) {
        if !new_set.contains(&id.0) {
            commands.entity(entity).despawn();
        }
    }

    let outputs = host.input_devices();

    let old_outputs: Vec<_> = output_devices.iter().flat_map(|d| d.iter()).collect();

    let mut new_set = HashSet::new();
    for output in outputs {
        let matching = output_query
            .iter_many(old_outputs.iter())
            .find(|(_, id, ..)| id.0 == output.id);

        new_set.insert(output.id.clone());

        // let extra = host.get_device(&input.id).ok_or("Failed to acquire extra information")?;
        // let input = extra.supported_input_configs()?;
        // for config in input {

        // }

        match matching {
            Some((entity, _id, is_default)) => {
                if is_default != output.is_default {
                    if is_default {
                        commands.entity(entity).insert(DefaultOutputDevice);
                    } else {
                        commands.entity(entity).remove::<DefaultOutputDevice>();
                    }
                }
            }
            None => {
                let mut entity = commands.spawn(DeviceId(output.id));
                if output.is_default {
                    entity.insert(DefaultOutputDevice);
                }

                if let Some(name) = output.name {
                    entity.insert(Name::new(name));
                }
            }
        }
    }

    for (entity, id, ..) in output_query.iter_many(old_outputs.iter()) {
        if !new_set.contains(&id.0) {
            commands.entity(entity).despawn();
        }
    }

    Ok(())
}

fn start_stream(
    contexts: Query<(&mut AudioContext, &AudioStreamConfig<CpalBackend>)>,
    server: Res<AssetServer>,
    mut commands: Commands,
) -> Result {
    for (mut context, config) in contexts {
        context.with(|context| -> Result {
            let context = context
                .downcast_mut::<FirewheelCtx<CpalBackend>>()
                .expect("Attempted to initialize audio context with unexpected backend type.");
            context
                .start_stream(config.0.clone())
                .map_err(|e| format!("failed to start audio stream: {e:?}"))?;

            let raw_sample_rate = context.stream_info().unwrap().sample_rate;
            let sample_rate = crate::context::SampleRate(sync::Arc::new(
                sync::atomic::AtomicU32::new(raw_sample_rate.get()),
            ));

            commands.insert_resource(sample_rate.clone());
            server.register_loader(crate::sample::SampleLoader { sample_rate });

            commands.trigger(StreamStartEvent {
                sample_rate: raw_sample_rate,
            });

            Ok(())
        })?;
    }

    Ok(())
}

// pub(crate) fn restart_context<B>(
//     stream_config: Res<AudioStreamConfig<B>>,
//     mut commands: Commands,
//     mut audio_context: ResMut<AudioContext>,
//     sample_rate: Res<SampleRate>,
// ) -> Result
// where
//     B: AudioBackend + 'static,
//     B::Config: Clone + Send + Sync + 'static,
//     B::StreamError: Send + Sync + 'static,
// {
//     audio_context.with(|context| {
//         let context: &mut FirewheelCtx<B> = context
//             .downcast_mut()
//             .ok_or("only one audio context should be active at a time")?;

//         context.stop_stream();
//         context
//             .start_stream(stream_config.0.clone())
//             .map_err(|e| format!("failed to restart audio stream: {e:?}"))?;

//         let previous_rate = sample_rate.get();

//         let current_rate = context.stream_info().unwrap().sample_rate;
//         sample_rate
//             .0
//             .store(current_rate.get(), sync::atomic::Ordering::Relaxed);

//         commands.trigger(StreamRestartEvent {
//             previous_rate,
//             current_rate,
//         });

//         Ok(())
//     })
// }
