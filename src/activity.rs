//! Manage the activity of audio nodes.

use crate::node::Events;
use bevy_ecs::{component::ComponentId, prelude::*, world::DeferredWorld};
use firewheel::node::NodeEventType;

/// Pause an audio node and its queued events.
///
/// This produces `Pause` event when inserted
/// into an entity. It will also resume the
/// node when removed.
///
/// ```
/// # use bevy_seedling::*;
/// # use bevy::prelude::*;
/// fn pause_all(q: Query<Entity, With<Node>>, mut commands: Commands) {
///     for entity in q.iter() {
///         commands.entity(entity).insert(Pause);
///     }
/// }
/// ```
#[derive(Debug, Component)]
#[component(on_add = on_add_pause, on_remove = on_remove_pause)]
pub struct Pause;

fn on_add_pause(mut world: DeferredWorld, entity: Entity, _: ComponentId) {
    let already_paused = world.get::<Stop>(entity).is_some();

    if already_paused {
        return;
    }

    world
        .commands()
        .entity(entity)
        .entry::<Events>()
        .or_default()
        .and_modify(|mut events| {
            events.push(NodeEventType::Pause);
        });
}

fn on_remove_pause(mut world: DeferredWorld, entity: Entity, _: ComponentId) {
    let stopped = world.get::<Stop>(entity).is_some();

    if stopped {
        return;
    }

    world
        .commands()
        .entity(entity)
        .entry::<Events>()
        .or_default()
        .and_modify(|mut events| {
            events.push(NodeEventType::Resume);
        });
}

/// Stops an audio node and discards its queued events.
///
/// This produces `Stop` event when inserted
/// into an entity. It will also resume the
/// node when removed.
///
/// ```
/// # use bevy_seedling::*;
/// # use bevy::prelude::*;
/// fn stop_all(q: Query<Entity, With<Node>>, mut commands: Commands) {
///     for entity in q.iter() {
///         commands.entity(entity).insert(Stop);
///     }
/// }
#[derive(Debug, Component)]
#[component(on_add = on_add_stop, on_remove = on_remove_stop)]
pub struct Stop;

fn on_add_stop(mut world: DeferredWorld, entity: Entity, _: ComponentId) {
    world
        .commands()
        .entity(entity)
        .entry::<Events>()
        .or_default()
        .and_modify(|mut events| {
            events.push(NodeEventType::Stop);
        });
}

fn on_remove_stop(mut world: DeferredWorld, entity: Entity, _: ComponentId) {
    let paused = world.get::<Pause>(entity).is_some();

    if paused {
        return;
    }

    world
        .commands()
        .entity(entity)
        .entry::<Events>()
        .or_default()
        .and_modify(|mut events| {
            events.push(NodeEventType::Resume);
        });
}
