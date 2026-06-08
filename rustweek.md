# RustWeek

## Move away from sampler pools

A number of people don't like the abstraction. Let's explore alternatives!

- Sampler pools are a simple way to minimize graph recompilations and
  limit voice counts.

## Manage audio devices in the ECS

What if we could manage devices and streams in the vocabulary of ECS?

## On-the-fly decoding

`bevy_seedling` loads and decodes entire audio files before sending them
to the audio thread. This can increase initial latency and consume
a ton of memory.
