[![crates.io](https://img.shields.io/crates/v/bevy_seedling)](https://crates.io/crates/bevy_seedling)
[![docs.rs](https://docs.rs/bevy_seedling/badge.svg)](https://docs.rs/bevy_seedling)

A sprouting integration of the [Firewheel](https://github.com/BillyDM/firewheel)
audio engine for [Bevy](https://bevyengine.org/).

## Getting started

First, you'll need to add the dependency to your `Cargo.toml`.

```toml
[dependencies]
bevy_seedling = "0.3"
```

Then, you'll need to add the [`SeedlingPlugin`] to your app.

```rs
use bevy::prelude::*;
use bevy_seedling::SeedlingPlugin;

fn main() {
    App::default()
        .add_plugins((DefaultPlugins, SeedlingPlugin::default()))
        .run();
}
```

[The repository's examples](https://github.com/CorvusPrudens/bevy_seedling/tree/master/examples)
should help you get up to speed on common usage patterns.

## Overview

`bevy_seedling` provides a thin ECS wrapper over `Firewheel`.

A `Firewheel` audio node is typically represented in the ECS as
an entity with a [`Node`] and a component that can generate
`Firewheel` events, such as [`VolumeNode`].

Interactions with the audio engine are buffered.
This includes inserting nodes into the audio graph,
removing nodes from the graph, making connections
between nodes, and sending node events. This provides
a few advantages:

1. Audio entities do not need to wait until
   they have Firewheel IDs before they can
   make connections or generate events.
2. Systems that spawn or interact with
   audio entities can be trivially parallelized.
3. Graph-mutating interactions are properly deferred
   while the audio graph isn't ready, for example
   if it's been temporarily deactiviated.

The main disadvantage is potentially increased latency,
since several milliseconds may pass between
event generation and actually propagating those events
to the audio engine. However, since
`bevy_seedling` provides direct access to the audio context,
you can always immediately queue and flush events
if necessary.

## Bevy version compatibility

| `bevy` | `bevy_seedling` |
| ------ | --------------- |
| 0.15   | 0.3             |

## Future work

- Graph operations

  Audio entities currently support only a subset of the underlying
  Firewheel graph API. In particular, the ability to disconnect nodes will
  need to be added.

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
