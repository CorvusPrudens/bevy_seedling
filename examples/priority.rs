//! This example demonstrates sample playback priority.

use bevy::{log::LogPlugin, prelude::*};
use bevy_seedling::{pool::PoolSize, prelude::*};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin::default(),
            SeedlingPlugin::default(),
        ))
        .add_systems(Startup, priority)
        .run();
}

fn priority(server: Res<AssetServer>, mut commands: Commands) {
    #[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
    struct SmallPool;

    #[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
    struct LargePool;

    commands.spawn((SamplerPool(SmallPool), PoolSize(3..=3)));
    commands.spawn((SamplerPool(LargePool), PoolSize(5..=5)));

    // Let's prioritize the most musically significant notes in this chord.
    let samples = [
        ("a6.ogg", SamplePriority(0)),
        ("cs6.ogg", SamplePriority(1)),
        ("d6.ogg", SamplePriority(1)),
        ("e6.ogg", SamplePriority(0)),
        ("fs6.ogg", SamplePriority(1)),
    ];

    // In the larger pool, there's no contention and all samples can play.
    info!("Playing all samples!");

    for (sample, priority) in samples.iter().take(4) {
        let path = format!("notes/{sample}");
        commands.spawn((LargePool, SamplePlayer::new(server.load(&path)), *priority));
    }
    let path = format!("notes/{}", samples[4].0);
    let mut last_sample = commands.spawn((
        LargePool,
        SamplePlayer::new(server.load(&path)),
        samples[4].1,
    ));

    // We'll play the next batch after this one's done.
    last_sample.observe(
        move |_trigger: Trigger<PlaybackCompletionEvent>,
              server: Res<AssetServer>,
              mut commands: Commands| {
            // In the smaller pool, with only three samplers, only the
            // samples with the highest priority will be played.
            info!("Playing prioritized samples!");

            for (sample, priority) in samples {
                let path = format!("notes/{sample}");
                commands.spawn((SmallPool, SamplePlayer::new(server.load(&path)), priority));
            }
        },
    );
}
