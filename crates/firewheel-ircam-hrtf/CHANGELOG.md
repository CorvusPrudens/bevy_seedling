# Unreleased

# Fixes

- Fix `HrtfNode` leaving part of its output unwritten when the block size isn't a multiple of the FFT size
- Fix a reused `HrtfNode` starting at the previous sound's gain and direction
- Let the HRIR ring out after the input goes silent

# 0.5.0

# Changes

- Update Firewheel to 0.12.0 and Bevy to 0.19

# 0.4.0

# Changes

- Update Firewheel to 0.10.0 and Bevy to 0.18

# 0.2.0

# Features

- Introduced `HrtfNode::coeff_update_factor`, which provides a bit
  more granular control over parameter smoothing.

# 0.1.1

## Fixes

- Ensure docs build with all features

# 0.1.0

This version is the first published to crates.io.
