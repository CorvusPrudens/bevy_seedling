//! Perceptual volume conversion.

use crate::prelude::*;
use bevy_ecs::component::Component;
use bevy_math::prelude::*;

/// Helper for converting a simple \[0.0, 1.0\] range to [`Volume`].
///
/// Since we perceive volume logarithmically, a simple linear mapping
/// between a slider and a sound's amplitude will produce awkward results;
/// near the bottom of the range, the slider would produce huge changes in perceived
/// volume, whereas near the top it would do almost nothing.
///
/// This struct corrects for this and handles some edge-cases near zero.
/// It can convert both ways, facilitating easy two-way bindings for
/// your settings.
#[derive(Debug, Component, Clone, Copy)]
pub struct PerceptualVolume {
    /// When the perceptual control value is below this value, the mapping will be linear between:
    /// - 0 perceptual = 0 volume
    /// - [`Self::pivot_pos`] perceptual = [`Self::pivot_volume`] volume
    ///
    /// When above this value, the mapping will be exponential between:
    /// - [`Self::pivot_pos`] perceptual = [`Self::pivot_volume`] volume
    /// - 1.0 perceptual = 0 dB
    pub pivot_pos: f32,
    /// The volume to use at [`Self::pivot_pos`]
    pub pivot_volume: Volume,
}

impl PerceptualVolume {
    /// Construct a new, default [`PerceptualVolume`].
    pub const fn new() -> Self {
        Self {
            pivot_volume: Volume::Decibels(-50.0),
            pivot_pos: 0.01,
        }
    }
}

impl Default for PerceptualVolume {
    fn default() -> Self {
        Self::new()
    }
}

impl PerceptualVolume {
    /// Converts a simple, linear \[0.0, 1.0\] range to an intuitive [`Volume`].
    pub fn perceptual_to_volume(&self, perceptual: f32) -> Volume {
        let perceptual = perceptual.max(0f32);

        if perceptual < self.pivot_pos {
            let min = 0.0_f32;
            let max = self.pivot_volume.linear();
            let t = perceptual / self.pivot_pos;
            Volume::Linear(min.lerp(max, t))
        } else {
            let min = self.pivot_volume.decibels();
            let max = 0.0;
            let t = (perceptual - self.pivot_pos) / (1.0 - self.pivot_pos);
            Volume::Decibels(min.lerp(max, t))
        }
    }

    /// Converts [`Volume`] into a simple, linear [0.0, 1.0] range.
    pub fn volume_to_perceptual(&self, volume: Volume) -> f32 {
        if volume.linear() <= self.pivot_volume.linear() {
            let vol = volume.linear();
            let pivot = self.pivot_volume.linear();
            let t = vol / pivot;
            t * self.pivot_pos
        } else {
            let vol = volume.decibels();
            let pivot = self.pivot_volume.decibels();
            let t = (vol - pivot) / (0.0 - pivot);
            self.pivot_pos + t * (1.0 - self.pivot_pos)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let converter = PerceptualVolume::default();
        for i in 0..100 {
            // We'll test a little outside the normal 0-1 range too
            let percent = -0.1 + i as f32 / 90.0;
            let volume = converter.perceptual_to_volume(percent);
            let perceptual = converter.volume_to_perceptual(volume);

            if percent < 0.0 {
                assert_eq!(perceptual, 0.0);
            } else {
                assert!(
                    (perceptual - percent).abs() < 0.0001,
                    "perceptual: {perceptual}, percent: {percent}"
                );
            }
        }
    }
}
