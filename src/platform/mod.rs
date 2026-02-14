//! Components that abstract over different backends.

use bevy_ecs::prelude::*;

pub mod cpal;

#[derive(EntityEvent)]
pub struct FetchDevices {
    pub entity: Entity,
    pub detailed: bool,
}

#[derive(Component)]
#[component(immutable)]
pub struct HostId<T>(pub T);

#[derive(Component)]
#[component(immutable)]
pub struct DeviceId<T>(pub T);

#[derive(Component)]
#[component(immutable)]
pub struct SampleRate(pub u32);

#[derive(Component)]
#[component(immutable)]
pub struct BufferFrames(pub u32);

#[derive(Component)]
pub struct DefaultInputDevice;

#[derive(Component)]
pub struct DefaultOutputDevice;

#[derive(Component)]
#[relationship_target(relationship = InputDeviceOf)]
pub struct InputDevices(Vec<Entity>);

#[derive(Component)]
#[relationship(relationship_target = InputDevices)]
pub struct InputDeviceOf(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = OutputDeviceOf)]
pub struct OutputDevices(Vec<Entity>);

#[derive(Component)]
#[relationship(relationship_target = OutputDevices)]
pub struct OutputDeviceOf(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = SelectedInputDeviceOf)]
pub struct SelectedInputDevice(Entity);

#[derive(Component)]
#[relationship(relationship_target = SelectedInputDevice)]
pub struct SelectedInputDeviceOf(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = SelectedOutputDeviceOf)]
pub struct SelectedOutputDevice(Entity);

#[derive(Component)]
#[relationship(relationship_target = SelectedOutputDevice)]
pub struct SelectedOutputDeviceOf(pub Entity);

#[derive(Component)]
#[component(immutable)]
pub struct SampleRates(Vec<u32>);

#[derive(Component)]
#[component(immutable)]
pub struct InputChannels(pub u32);

#[derive(Component)]
#[component(immutable)]
pub struct OutputChannels(pub u32);

#[derive(Component)]
#[component(immutable)]
pub struct DuplexChannels(pub u32);

#[derive(Resource, Default)]
pub struct AudioContextConfig(pub crate::prelude::FirewheelConfig);

fn spawn_context<B>(config: Res<AudioContextConfig>, mut commands: Commands)
where
    B: firewheel::backend::AudioBackend + 'static,
    B::StreamError: Send + Sync + 'static,
{
    let context = crate::context::AudioContext::new::<B>(config.0);
    commands.spawn(context);
}
