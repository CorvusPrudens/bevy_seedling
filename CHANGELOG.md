# 0.5.0

## Features

### Reflection

`bevy_seedling` now features reflection for all public, ECS-facing
types. Since many `bevy_seedling` types are actually just Firewheel
types, we've extended reflection to many of Firewheel's types as well.

Reflection can be enabled with the `reflect` feature, now enabled
by default.

### New audio nodes

0.5 features a few new audio nodes:

- `HrtfNode`, for HRTF-based spatialization, a more advanced
  and convincing effect than `SpatialBasicNode` (at the cost of
  more expensive computation).
- `ItdNode`, a simple, cheap spatialization technique that can
  help make the basic panning spatialization of `SpatialBasicNode`
  a bit more convincing.
- `LimiterNode`, a dynamic limiter, which eliminates jarring distortion
  when things get too loud. This is automatically applied to the output
  of the new default graph configuration.
- `LufsNode`, a LUFS analyzer, which helps sound designers monitor the
  overall loudness and consistency of a Bevy app's sound.

### Configurable initial graph

The initial graph now features a few built-in configurations.
The default configuration creates a simple, common game setup,
facilitating straightforward SFX/Music/Master volume configuration.
The other two offer increasingly minimal setups for those who know
exactly what they want.

#### Migration guide

To ease into the new setup, you can start with the `Minimal` configuration,
which matches `0.4`'s setup:

```rs
// 0.4
app.insert_plugins(SeedlingPlugin::default());

// 0.5
app.insert_plugins(SeedlingPlugin {
    graph_config: GraphConfiguration::Minimal,
    ..Default::default()
});
```

The `SeedlingPlugin::spawn_default_pool` field has been absorbed by the
new `GraphConfiguration`. To achieve something similar, you'll probably
want the `Empty` configuration.

```rs
// 0.4
app.insert_plugins(SeedlingPlugin {
    spawn_default_pool: false,
    ..Default::default()
});

// 0.5
app.insert_plugins(SeedlingPlugin {
    graph_config: GraphConfiguration::Minimal,
    ..Default::default()
});

fn minimal_setup(mut commands: Commands) {
    commands
        .spawn((MainBus, VolumeNode::default()))
        .connect(AudioGraphOutput);

    commands.spawn((
        bevy_seedling::pool::dynamic::DynamicBus,
        VolumeNode::default(),
    ));
}
```

### Default pool size

The default pool size is now configured purely as a resource.

#### Migration guide

```rs
// 0.4
app.insert_plugins(SeedlingPlugin {
    pool_size: 4..=32,
    ..Default::default()
});

// 0.5
app
  .insert_plugins(SeedlingPlugin::default())
  .insert_resource(DefaultPoolSize(4..=32));
```

### Dynamic bus

Dynamic pools are now routed to the `DynamicBus`, giving you
a bit more control over where they go. To completely disabled
dynamic pool creation, simply despawn this bus, or use an initial
graph configuration that doesn't create it.

### Automatic node configuration updates

All Firewheel nodes have a configuration struct: the `Config` associated
type of the `AudioNode` trait. When you register a node, the configuration
is added as a required component. This configuration is used once
when the node is created and inserted into the graph. In 0.4, further
changes would do nothing. In 0.5, we now automatically recreate and
reinsert the node when its configuration changes.

### `Sample` -> `AudioSample`

The `Sample` type, the primary asset for playing sounds, has been renamed
to `AudioSample` and is now re-exported in the prelude.

#### Migration guide

```rs
// 0.4
use bevy_seedling::{prelude::*, sample::Sample};

fn play_sound(source: Handle<Sample>) -> impl Bundle {
    SamplePlayer::new(source)
}

// 0.5
use bevy_seedling::prelude::*;

fn play_sound(source: Handle<AudioSample>) -> impl Bundle {
    SamplePlayer::new(source)
}
```

# 0.4.4

## Fixes

- Account for 3D listener orientation
- Fixed system ordering issue for spatial positioning

# 0.4.3

## Fixes

- Updated `symphonium` to match `firewheel` due to a breaking change in the former

# 0.4.2

## Fixes

- The default `OnComplete` behavior in `PlaybackSettings` is now `OnComplete::Despawn`, matching the documentation
- The `Sampler` component now properly fetches the `SamplerNode` state
- The `head` and `tail` methods on `ConnectCommands` now return the correct entities following a `chain_node` call on `EntityCommands`

# 0.4.1

## Features

- Added an `AudioGraphInput` component, making input connections
  easier to manage

# 0.4.0

## Features

This version marks a significant API change in sample
players, sample effects, and pool mechanisms. Along the way, each has
received significant upgrades in functionality.

### Samples

- Sample playback (play, pause, stop) parameters
- Sample speed control
  - The new `PitchRange` component will automatically
    randomize the starting pitch of your samples.
- Clear separation between fixed and dynamic sample parameters
  - Fixed parameters exist on the `SamplePlayer`, while dynamic
    ones are kept in `PlaybackSettings`.

#### Migration guide

The `SamplePlayer` API is moving away from the `bevy_audio` style, where
the `SamplePlayer` (or `AudioPlayer`) is just a wrapper around a sample resource.

While this may lead to more difficult migrations, it results in a much
clearer API. `SamplePlayer` now includes `volume` and `repeat_mode` fields,
taken from `PlaybackSettings`. In Firewheel, these parameters can only be set
when providing the sample, so `bevy_seedling` enforces this with `SamplePlayer`'s
immutability. This should make it clear to users that a sample's volume isn't dynamic,
and that they should reach for other mechanisms (like routing to buses, adding effects, etc.)
if they want dynamic volume.

- Looping playback

```rs
// 0.3
commands.spawn((
    SamplePlayer::new(server.load("my_sample.wav")),
    PlaybackSettings::LOOP,
));

// 0.4
commands.spawn(SamplePlayer::new(server.load("my_sample.wav")).looping());
// or
commands.spawn(SamplePlayer {
    sample: server.load("my_sample.wav"),
    repeat_mode: RepeatMode::RepeatEndlessly,
    ..Default::default()
});
```

- Sample volume

```rs
// 0.3
commands.spawn((
    SamplePlayer::new(server.load("my_sample.wav")),
    PlaybackSettings {
        volume: Volume::Decibels(-6.0),
        ..Default::default()
    },
));

// 0.4
commands.spawn(
    SamplePlayer::new(server.load("my_sample.wav"))
        .with_volume(Volume::Decibels(-6.0))
);
// or
commands.spawn(SamplePlayer {
    sample: server.load("my_sample.wav"),
    volume: Volume::Decibels(-6.0),
    ..Default::default()
});
```

- Sample life cycle

```rs
// 0.3
commands.spawn((
    SamplePlayer::new(server.load("my_sample.wav")),
    PlaybackSettings::REMOVE,
));

// 0.4
commands.spawn(
    SamplePlayer::new(server.load("my_sample.wav")),
    PlaybackSettings {
        on_complete: OnComplete::Remove,
        ..Default::default()
    }
);
```

- Sample playback information

```rs
// 0.3
fn get_playhead(players: Query<&SamplePlayer>) {
    for player in &players {
        if let Some(playhead) = player.playhead_frames() {
            // ...
        }
    }
}

// 0.4
fn get_playhead(players: Query<&Sampler>) {
    for sampler in &players {
        let playhead = sampler.playhead_frames();
        // ...
    }
}
```

### Sampler pools

- Sampler pools now use declarative, spawn-based construction
- Sample queuing is much more robust
  - Samples can declare a priority with `SamplePriority`
  - Samples can declare how long they're allowed to wait before
    giving up with `SampleQueueLifetime`
  - Sample pools grow eagerly, minimizing any latency from recompiling
    the audio graph

#### Migration guide

```rs
// 0.3
Pool::new(DefaultPool, 24).spawn(&mut commands);

// 0.4
// The size does not need to be specified.
// It defaults to the `DefaultPoolSize` resource.
commands.spawn(SamplerPool(DefaultPool));
// or
commands.spawn((
  SamplerPool(DefaultPool),
  PoolSize(24..=24),
));
```

### Sample effects

In `0.3`, sample effects were placed directly on the `SamplePlayer`
entity. This was very convenient for querying, but it had some
persistent drawbacks.

1. It was impossible to express an effects chain with duplicated processors.
   For example, you couldn't throw together a two-pole filter by placing two
   one-pole filters back-to-back.
2. It would not be possible to distinguish audio node configuration structs
   for two different nodes that happen to share the same configuration type.
3. The imperative trait-extension approach won't work well with the upcoming
   BSN data format.
4. Effect ordering is a little ambiguous and difficult to rearrange after spawning.

`0.4` takes advantage of Bevy's new relationships feature and moves effects into
separate, related entities. This removes all clashing problems and makes it easy to
spawn effects declaratively and in order. The main drawback is that effects are
now more cumbersome to query for in terms of the sample they're applied to. The new
`EffectsQuery` should help alleviate the degraded UX, and eventually Bevy should
gain more sophisticated relations queries.

#### Migration guide

- Dynamic pools

```rs
// 0.3
commands
    .spawn(SamplePlayer::new(server.load("my_sample.wav")))
    .effect(VolumeNode::default())
    .effect(SpatialBasicNode::default());

// 0.4
commands.spawn((
    SamplePlayer::new(server.load("my_sample.wav")),
    sample_effects![
        VolumeNode::default(),
        SpatialBasicNode::default(),
    ],
));
```

- Static pools

```rs
// 0.3
Pool::new(DefaultPool, 24)
    .effect(VolumeNode::default())
    .effect(SpatialBasicNode::default())
    .spawn(&mut commands);

// 0.4
commands.spawn((
    SamplerPool(DefaultPool),
    sample_effects![
        VolumeNode::default(),
        SpatialBasicNode::default(),
    ],
));
```

- Effects queries

```rs
// 0.3
commands
    .spawn((Marker, SamplePlayer::new(server.load("my_sample.wav"))))
    .effect(VolumeNode::default())
    .effect(SpatialBasicNode::default());

fn get_volume(mut players: Query<&mut VolumeNode, With<Marker>>) {
    for mut node in &mut players {
        node.volume = Volume::Decibels(-6.0);
    }
}

// 0.4
commands.spawn((
    Marker,
    SamplePlayer::new(server.load("my_sample.wav")),
    sample_effects![
        VolumeNode::default(),
        SpatialBasicNode::default(),
    ],
));

fn get_volume(
    players: Query<&SampleEffects, With<Marker>>,
    mut nodes: Query<&mut VolumeNode>,
) -> Result {
    for effects in &players {
        nodes.get_effect_mut(effects)?.volume = Volume::Decibels(-6.0);
    }
}
```

# 0.3.1

## Fixes

- Fix web compilation by [@heydocode](https://github.com/heydocode)

# 0.3.0

This version is the first published to [crates.io](https://crates.io), and
includes a number of major improvements.

## Features

- Spatial audio is fully integrated
- Sample pools can define per-player effects chains
- _Dynamic pools_ can be constructed on-the-fly directly on sample players
- Nodes can be disconnected
- `SendNode` can be used to easily route to sends
