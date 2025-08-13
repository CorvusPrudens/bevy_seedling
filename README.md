[![crates.io](https://img.shields.io/crates/v/bevy_seedling)](https://crates.io/crates/bevy_seedling)
[![docs.rs](https://docs.rs/bevy_seedling/badge.svg)](https://docs.rs/bevy_seedling)

A sprouting integration of the [Firewheel](https://github.com/BillyDM/firewheel)
audio engine for [Bevy](https://bevyengine.org/).

`bevy_seedling` is powerful, flexible, and
[fast](https://github.com/CorvusPrudens/rust-audio-demo?tab=readme-ov-file#performance).
You can play sounds, apply effects,
and route audio anywhere. Creating
and integrating custom audio processors
is simple.

## Getting started

First, you'll need to add the dependency to your `Cargo.toml`.
Note that you'll need to disable Bevy's `bevy_audio` feature,
meaning you'll need to specify quite a few features
manually!

```toml
[dependencies]
bevy_seedling = "0.4"
bevy = { version = "0.16", default-features = false, features = [
  "animation",
  "bevy_asset",
  "bevy_color",
  "bevy_core_pipeline",
  "bevy_gilrs",
  "bevy_gizmos",
  "bevy_gltf",
  "bevy_mesh_picking_backend",
  "bevy_pbr",
  "bevy_picking",
  "bevy_render",
  "bevy_scene",
  "bevy_sprite",
  "bevy_sprite_picking_backend",
  "bevy_state",
  "bevy_text",
  "bevy_ui",
  "bevy_ui_picking_backend",
  "bevy_window",
  "bevy_winit",
  "custom_cursor",
  "default_font",
  "hdr",
  "multi_threaded",
  "png",
  "smaa_luts",
  "sysinfo_plugin",
  "tonemapping_luts",
  "webgl2",
  "x11",
] }
```

Then, you'll need to add the `SeedlingPlugin` to your app.

```rs
use bevy::prelude::*;
use bevy_seedling::prelude::*;

fn main() {
    App::default()
        .add_plugins((DefaultPlugins, SeedlingPlugin::default()))
        .run();
}
```

Once you've set it all up, playing sounds is easy!

```rs
fn play_sound(mut commands: Commands, server: Res<AssetServer>) {
    // Play a sound!
    commands.spawn(SamplePlayer::new(server.load("my_sample.wav")));

    // Play a sound... with effects :O
    commands.spawn((
        SamplePlayer::new(server.load("my_ambience.wav")).looping(),
        sample_effects![LowPassNode { frequency: 500.0 }],
    ));
}
```

[The crate docs](https://docs.rs/bevy_seedling/latest) provide an overview of
`bevy_seedling`'s features, and
[the repository's examples](https://github.com/CorvusPrudens/bevy_seedling/tree/master/examples)
should help you get up to speed on common usage patterns.

## Feature flags

| Flag            | Description                                | Default |
| --------------- | ------------------------------------------ | ------- |
| `reflect`       | Enable `bevy_reflect` derive macros.       | Yes     |
| `rand`          | Enable the `RandomPitch` component.        | Yes     |
| `wav`           | Enable WAV format and PCM encoding.        | Yes     |
| `ogg`           | Enable Ogg format and Vorbis encoding.     | Yes     |
| `mp3`           | Enable mp3 format and encoding.            | No      |
| `mkv`           | Enable mkv format.                         | No      |
| `adpcm`         | Enable adpcm encoding.                     | No      |
| `flac`          | Enable FLAC format and encoding.           | No      |
| `web_audio`     | Enable the multi-threading web backend.    | No      |
| `hrtf`          | Enable HRTF Spatialization.                | No      |
| `hrtf_subjects` | Enable all HRTF embedded data.             | No      |
| `loudness`      | Enable LUFS analyzer node.                 | Yes     |
| `stream`        | Enable CPAL input and output stream nodes. | Yes     |

## Bevy version compatibility

| `bevy` | `bevy_seedling` |
| ------ | --------------- |
| 0.16   | 0.4, 0.5        |
| 0.15   | 0.3             |

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
