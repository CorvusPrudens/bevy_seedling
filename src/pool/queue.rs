use super::{
    PlaybackCompletionEvent, PoolSamplerOf, PoolSamplers, PoolShape, PoolSize, SamplerOf,
    SamplerStateWrapper,
    sample_effects::{EffectOf, SampleEffects},
};
use crate::{
    node::{EffectId, follower::FollowerOf},
    pool::label::PoolLabelContainer,
    prelude::DefaultPool,
    sample::{AudioSample, QueuedSample, SamplePlayer, SamplePriority, SampleQueueLifetime},
};
use bevy_asset::prelude::*;
use bevy_ecs::{entity::EntityCloner, prelude::*, relationship::Relationship};
use bevy_log::prelude::*;
use bevy_platform::collections::HashMap;
use bevy_time::{Stopwatch, Time};
use firewheel::nodes::sampler::{RepeatMode, SamplerConfig, SamplerNode};
use std::ops::Deref;

#[derive(PartialEq, Debug, Eq, PartialOrd, Ord, Copy, Clone)]
struct SamplerScore {
    priority: SamplePriority,
    is_looping: bool,
    has_assignment: bool,
    raw_score: u64,
}

impl Default for SamplerScore {
    fn default() -> Self {
        SamplerScore {
            priority: Default::default(),
            is_looping: false,
            has_assignment: false,
            raw_score: u64::MAX,
        }
    }
}

/// Eagerly grow pools to handle over-allocation when possible.
pub(super) fn grow_pools(
    queued_samples: Query<(&SamplePlayer, &PoolLabelContainer), With<QueuedSample>>,
    pools: Query<(
        Entity,
        &PoolLabelContainer,
        &PoolSamplers,
        &PoolSize,
        Option<&SampleEffects>,
        &SamplerConfig,
    )>,
    nodes: Query<Option<&SamplerOf>, With<PoolSamplerOf>>,
    assets: Res<Assets<AudioSample>>,
    mut commands: Commands,
) -> Result {
    let queued_samples: HashMap<_, usize> = queued_samples
        .iter()
        .filter_map(|(player, label)| assets.get(&player.sample).map(|_| label))
        .fold(HashMap::new(), |mut acc, label| {
            *acc.entry(label.label).or_default() += 1;
            acc
        });

    if queued_samples.is_empty() {
        return Ok(());
    }

    for (pool_entity, label, samplers, size, pool_effects, pool_config) in pools {
        let Some(queued_samples) = queued_samples.get(&label.label).copied() else {
            continue;
        };

        let inactive_samplers = nodes
            .iter_many(samplers.iter())
            .filter(|n| n.is_none())
            .count();

        if inactive_samplers >= queued_samples {
            continue;
        }

        let difference = queued_samples - inactive_samplers;

        // attempt to grow pool if possible
        if samplers.len() < *size.0.end() {
            let growth_size = difference.max(samplers.len().min(16));
            let new_size = (samplers.len() + growth_size).min(*size.0.end());

            #[cfg(debug_assertions)]
            commands.queue({
                let id = label.label_id;
                let num_samplers = samplers.len();
                move |world: &mut World| {
                    let component = world.components().get_descriptor(id);

                    if let Some(component) = component {
                        let s = if new_size != 1 { "s" } else { "" };
                        debug!(
                            "growing {} from {} to {} sampler{s} ({} over-allocated)",
                            component.name(),
                            num_samplers,
                            new_size,
                            difference,
                        );
                    }
                }
            });

            for _ in samplers.len()..new_size {
                super::spawn_chain(
                    pool_entity,
                    Some(pool_config.clone()),
                    pool_effects.map(|e| e.deref()).unwrap_or(&[]),
                    &mut commands,
                );
            }
        }
    }

    Ok(())
}

/// Scan through the set of pending sample players
/// and assign work to the most appropriate sampler node.
pub(super) fn assign_work(
    mut queued_samples: Query<
        (
            Entity,
            &SamplePlayer,
            &PoolLabelContainer,
            Option<&SampleEffects>,
            &SamplePriority,
        ),
        With<QueuedSample>,
    >,
    pools: Query<(
        &PoolLabelContainer,
        &PoolSamplers,
        &PoolSize,
        &PoolShape,
        Option<&SampleEffects>,
    )>,
    mut nodes: Query<
        (
            Entity,
            &mut SamplerNode,
            &SamplerStateWrapper,
            Option<&SamplerOf>,
        ),
        With<PoolSamplerOf>,
    >,
    active_samples: Query<(&SamplePlayer, &SamplePriority)>,
    mut effects: Query<&EffectId, With<EffectOf>>,
    assets: Res<Assets<AudioSample>>,
    mut commands: Commands,
) -> Result {
    let mut queued_samples: HashMap<_, Vec<_>> = queued_samples
        .iter_mut()
        .filter_map(|(entity, player, label, effects, priority)| {
            let asset = assets.get(&player.sample)?;

            Some((label.label, (entity, player, asset, effects, priority)))
        })
        .fold(HashMap::new(), |mut acc, (key, value)| {
            acc.entry(key).or_default().push(value);
            acc
        });

    if queued_samples.is_empty() {
        return Ok(());
    }

    for (label, samplers, size, pool_shape, pool_effects) in pools {
        // To suppress warnings when debug assertions are disabled, as `size` is only used in the debug-only `commands.queue` call below.
        #[cfg(not(debug_assertions))]
        let _size = size;

        let Some(mut queued_samples) = queued_samples.remove(&label.label) else {
            continue;
        };

        // if there is enough sampler availability in the pool,
        // don't bother sorting samples by priority

        let inactive_samplers: Vec<_> = samplers
            .iter()
            .filter(|s| nodes.get(*s).is_ok_and(|n| n.3.is_none()))
            .collect();

        #[cfg(debug_assertions)]
        commands.queue({
            let inactive = inactive_samplers.len();
            let queued_len = queued_samples.len();
            let total_samplers = samplers.len();
            let size = size.clone();
            let id = label.label_id;
            move |world: &mut World| {
                let component = world.components().get_descriptor(id);

                if let Some(component) = component {
                    let s = if queued_len != 1 { "s" } else { "" };
                    debug!(
                        "queued {queued_len} sample{s} in {} ({} total, {inactive} inactive, {:?})",
                        component.name(),
                        total_samplers,
                        size.0
                    );
                }
            }
        });

        if inactive_samplers.len() >= queued_samples.len() {
            let mut inactive = inactive_samplers.iter();

            for (sample_entity, player, asset, sample_effects, _priority) in queued_samples {
                let (sampler_entity, mut params, state, _) =
                    nodes.get_mut(*inactive.next().unwrap())?;

                params.sample = Some(asset.get());
                params.volume = player.volume;
                params.repeat_mode = player.repeat_mode;
                // params.playback = sett

                // commands
                //     .entity(sample_entity)
                //     .insert(crate::prelude::SampleState(state.0.clone()));
                state.0.clear_finished();

                // normalize sample effects
                if sample_effects.is_some() && pool_effects.is_none() {
                    match player.sample.path() {
                        Some(path) => warn!(
                            "Queued sample \"{}\" with effects in an effect-less pool.",
                            path
                        ),
                        None => warn!("Queued sample with effects in an effect-less pool."),
                    }
                }

                if let Some(pool_effects) = pool_effects {
                    match sample_effects {
                        Some(sample_effects) => {
                            let component_ids = match super::fetch_effect_ids(
                                sample_effects,
                                &mut effects.as_query_lens(),
                            ) {
                                Ok(ids) => ids,
                                Err(e) => {
                                    error!("{e}");

                                    continue;
                                }
                            };

                            if component_ids != pool_shape.0 {
                                // N will never be large enough for this to be a concern
                                if component_ids.iter().any(|id| !pool_shape.0.contains(id)) {
                                    match player.sample.path() {
                                        Some(path) => warn!(
                                            "Queued sample \"{}\" contains one or more effects that the pool does not.",
                                            path
                                        ),
                                        None => warn!(
                                            "Queued sample contains one or more effects that the pool does not."
                                        ),
                                    }
                                }

                                let mut new_effects = Vec::new();
                                new_effects.reserve_exact(pool_shape.0.len());
                                let mut clone_into = Vec::new();

                                for (effect, id) in pool_effects.iter().zip(&pool_shape.0) {
                                    match component_ids.iter().position(|c| c == id) {
                                        Some(index) => {
                                            new_effects.push(sample_effects[index]);
                                        }
                                        None => {
                                            let empty = commands.spawn_empty().id();

                                            clone_into.push((empty, effect));
                                            new_effects.push(empty);
                                        }
                                    }
                                }

                                commands
                                    .entity(sample_entity)
                                    .remove_related::<EffectOf>(sample_effects)
                                    .add_related::<EffectOf>(&new_effects);

                                commands.queue(move |world: &mut World| {
                                    let mut cloner = EntityCloner::build(world);
                                    cloner.deny::<EffectOf>();
                                    let mut cloner = cloner.finish();

                                    for (dest, src) in clone_into {
                                        cloner.clone_entity(world, src, dest);
                                    }
                                });
                            }
                        }
                        None => {
                            let pool_effects: Vec<_> = pool_effects.iter().collect();
                            commands.queue(move |world: &mut World| {
                                let mut cloner = EntityCloner::build(world);
                                cloner.deny::<EffectOf>();
                                let mut cloner = cloner.finish();

                                let mut sample_effects = Vec::new();
                                sample_effects.reserve_exact(pool_effects.len());
                                for effect in pool_effects {
                                    let sample_effect = cloner.spawn_clone(world, effect);
                                    sample_effects.push(sample_effect);
                                }

                                world
                                    .entity_mut(sample_entity)
                                    .add_related::<EffectOf>(&sample_effects);
                            });
                        }
                    }
                }

                commands
                    .entity(sample_entity)
                    .remove::<QueuedSample>()
                    .add_one_related::<SamplerOf>(sampler_entity);
            }

            continue;
        }

        // otherwise, sort the available samplers
        let mut sampler_scores = Vec::new();
        for (sampler_entity, params, state, assignment) in nodes.iter_many(samplers.iter()) {
            let raw_score = state.0.worker_score(params);
            let has_assignment = assignment.is_some();

            let active_data = assignment.and_then(|a| {
                active_samples
                    .get(a.0)
                    .map(|s| (s.0.repeat_mode, *s.1))
                    .ok()
            });

            let (is_looping, priority) = match active_data {
                Some((repeat, priority)) => (repeat != RepeatMode::PlayOnce, priority),
                None => (false, SamplePriority(0)),
            };

            sampler_scores.push((
                sampler_entity,
                SamplerScore {
                    priority,
                    raw_score,
                    has_assignment,
                    is_looping,
                },
            ));
        }

        sampler_scores.sort_by_key(|pair| pair.1);

        // then sort the queued samples
        queued_samples.sort_by_key(|s| {
            (
                core::cmp::Reverse(s.4),
                s.1.repeat_mode == RepeatMode::PlayOnce,
            )
        });

        for ((sampler_entity, sampler_score), queued) in
            sampler_scores.into_iter().zip(queued_samples.into_iter())
        {
            let (sample_entity, player, asset, sample_effects, priority) = queued;

            // Due to the sorting, if any queued sample has a lower priority then a currently playing sample,
            // then every subsequent sample must also have a lower priority than its corresponding player.
            if &sampler_score.priority > priority {
                break;
            }

            // We'll also skip over samples that won't loop
            // when the occupied sampler is currently looping.
            if sampler_score.is_looping && player.repeat_mode == RepeatMode::PlayOnce {
                continue;
            }

            let (sampler_entity, mut params, state, _) = nodes.get_mut(sampler_entity)?;

            params.sample = Some(asset.get());
            params.volume = player.volume;
            params.repeat_mode = player.repeat_mode;
            // commands
            //     .entity(sample_entity)
            //     .insert(crate::prelude::SampleState(state.0.clone()));
            state.0.clear_finished();

            // normalize sample effects
            if sample_effects.is_some() && pool_effects.is_none() {
                match player.sample.path() {
                    Some(path) => warn!(
                        "Queued sample \"{}\" with effects in an effect-less pool.",
                        path
                    ),
                    None => warn!("Queued sample with effects in an effect-less pool."),
                }
            }

            if let Some(pool_effects) = pool_effects {
                match sample_effects {
                    Some(sample_effects) => {
                        let component_ids = match super::fetch_effect_ids(
                            sample_effects,
                            &mut effects.as_query_lens(),
                        ) {
                            Ok(ids) => ids,
                            Err(e) => {
                                error!("{e}");

                                continue;
                            }
                        };

                        if component_ids != pool_shape.0 {
                            // N will never be large enough for this to be a concern
                            if component_ids.iter().any(|id| !pool_shape.0.contains(id)) {
                                match player.sample.path() {
                                    Some(path) => warn!(
                                        "Queued sample \"{}\" contains one or more effects that the pool does not.",
                                        path
                                    ),
                                    None => warn!(
                                        "Queued sample contains one or more effects that the pool does not."
                                    ),
                                }
                            }

                            let mut new_effects = Vec::new();
                            new_effects.reserve_exact(pool_shape.0.len());
                            let mut clone_into = Vec::new();

                            for (effect, id) in pool_effects.iter().zip(&pool_shape.0) {
                                match component_ids.iter().position(|c| c == id) {
                                    Some(index) => {
                                        new_effects.push(sample_effects[index]);
                                    }
                                    None => {
                                        let empty = commands.spawn_empty().id();

                                        clone_into.push((empty, effect));
                                        new_effects.push(empty);
                                    }
                                }
                            }

                            commands
                                .entity(sample_entity)
                                .remove_related::<EffectOf>(sample_effects)
                                .add_related::<EffectOf>(&new_effects);

                            commands.queue(move |world: &mut World| {
                                let mut cloner = EntityCloner::build(world);
                                cloner.deny::<EffectOf>();
                                let mut cloner = cloner.finish();

                                for (dest, src) in clone_into {
                                    cloner.clone_entity(world, src, dest);
                                }
                            });
                        }
                    }
                    None => {
                        let pool_effects: Vec<_> = pool_effects.iter().collect();
                        commands.queue(move |world: &mut World| {
                            let mut cloner = EntityCloner::build(world);
                            cloner.deny::<EffectOf>();
                            let mut cloner = cloner.finish();

                            let mut sample_effects = Vec::new();
                            sample_effects.reserve_exact(pool_effects.len());
                            for effect in pool_effects {
                                let sample_effect = cloner.spawn_clone(world, effect);
                                sample_effects.push(sample_effect);
                            }

                            world
                                .entity_mut(sample_entity)
                                .add_related::<EffectOf>(&sample_effects);
                        });
                    }
                }
            }

            commands
                .entity(sample_entity)
                .remove::<QueuedSample>()
                .add_one_related::<SamplerOf>(sampler_entity);
        }
    }

    Ok(())
}

pub(super) fn update_followers(
    samplers: Query<(&Children, &SamplerOf), Changed<SamplerOf>>,
    samples: Query<&SampleEffects>,
    mut commands: Commands,
) {
    for (children, assignment) in &samplers {
        let Ok(effects) = samples.get(assignment.get()) else {
            continue;
        };

        for (effect, follower) in effects.iter().zip(children.iter()) {
            commands.entity(follower).insert(FollowerOf(effect));
        }
    }
}

#[derive(Component)]
pub(super) struct SkipTimer(Stopwatch);

pub(super) fn mark_skipped(
    samples: Query<(Entity, &SamplePlayer), (With<QueuedSample>, Without<SkipTimer>)>,
    assets: Res<Assets<AudioSample>>,
    mut commands: Commands,
) {
    for (sample, player) in &samples {
        if assets.get(&player.sample).is_some() {
            commands.entity(sample).insert(SkipTimer(Stopwatch::new()));
        }
    }
}

pub(super) fn tick_skipped(
    mut samples: Query<
        (Entity, &mut SkipTimer, &SampleQueueLifetime),
        (With<SamplePlayer>, With<QueuedSample>),
    >,
    time: Res<Time>,
    mut commands: Commands,
) {
    let delta = time.delta();

    for (sample_entity, mut timer, lifetime) in &mut samples {
        if timer.0.tick(delta).elapsed() >= lifetime.0 {
            debug!("skipping sample {:?} after {:?}", sample_entity, lifetime.0,);

            commands
                .entity(sample_entity)
                .trigger(PlaybackCompletionEvent);
        }
    }
}

/// Assign the default pool label to a sample player that has no label.
pub(super) fn assign_default(
    samples: Query<
        Entity,
        (
            With<SamplePlayer>,
            Without<PoolLabelContainer>,
            Without<SampleEffects>,
        ),
    >,
    mut commands: Commands,
) {
    for sample in samples.iter() {
        commands.entity(sample).insert(DefaultPool);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sorting() {
        fn test_order<const LEN: usize>(candidates: [SamplerScore; LEN], expected: &[usize]) {
            let mut candidates = candidates.into_iter().enumerate().collect::<Vec<_>>();
            candidates.sort_by_key(|c| c.1);

            let ordering = candidates.into_iter().map(|c| c.0).collect::<Vec<_>>();
            assert_eq!(ordering.as_slice(), expected);
        }

        let candidates = [
            SamplerScore::default(),
            SamplerScore {
                priority: SamplePriority(1),
                ..Default::default()
            },
        ];

        test_order(candidates, &[0, 1]);

        let candidates = [
            SamplerScore {
                is_looping: true,
                ..Default::default()
            },
            SamplerScore::default(),
        ];

        test_order(candidates, &[1, 0]);

        let candidates = [
            SamplerScore {
                priority: SamplePriority(1),
                ..Default::default()
            },
            SamplerScore {
                priority: SamplePriority(0),
                is_looping: true,
                has_assignment: true,
                raw_score: 0,
            },
        ];

        test_order(candidates, &[1, 0]);
    }
}
