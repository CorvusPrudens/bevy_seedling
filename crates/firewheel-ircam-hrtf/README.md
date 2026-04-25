[![crates.io](https://img.shields.io/crates/v/firewheel-ircam-hrtf)](https://crates.io/crates/firewheel-ircam-hrtf)
[![docs.rs](https://docs.rs/firewheel-ircam-hrtf/badge.svg)](https://docs.rs/firewheel-ircam-hrtf)

# Firewheel IRCAM HRTF

A head-related transfer function (HRTF) node for
[Firewheel](https://github.com/BillyDM/Firewheel),
powered by [Fyrox](https://docs.rs/hrtf/latest/hrtf/)'s
[IRCAM](http://recherche.ircam.fr/equipes/salles/listen/download.html)-based HRIR.

HRTFs can provide far more convincing spatialization compared to
simpler techniques. They simulate the way our bodies filter sounds
based on where they're coming from, allowing you to distinguish up/down,
front/back, and the more typical left/right.

This simulation is moderately expensive. You'll generally want to avoid more
than 32-64 HRTF emitters, especially on less powerful devices.

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
