//! Glue code for interfacing with the underlying audio context.

use bevy_ecs::prelude::*;
use bevy_log::error;
use firewheel::{clock::ClockSeconds, FirewheelConfig, FirewheelContext};
use std::sync::mpsc;

/// A thread-safe wrapper around the underlying Firewheel audio context.
///
/// When the seedling plugin is initialized, this can be accessed as a resource.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_seedling::AudioContext;
/// fn system(mut context: ResMut<AudioContext>) {
///     context.with(|c| {
///         // ...
///     });
/// }
/// ```
#[derive(Debug, Resource)]
pub struct AudioContext(mpsc::Sender<ThreadLocalCall>);

impl AudioContext {
    /// Spawn the audio process and control thread.
    pub fn new(settings: FirewheelConfig) -> Self {
        let (bev_to_audio_tx, bev_to_audio_rx) = mpsc::channel::<ThreadLocalCall>();
        std::thread::spawn(move || {
            let mut context = FirewheelContext::new(settings);
            context
                .start_stream(Default::default())
                .expect("failed to activate the audio context");

            while let Ok(func) = bev_to_audio_rx.recv() {
                (func)(&mut context);
            }
        });

        AudioContext(bev_to_audio_tx)
    }
}

type ThreadLocalCall = Box<dyn FnOnce(&mut FirewheelContext) + Send + 'static>;

impl AudioContext {
    /// Get an absolute timestamp from the audio thread of the current time.
    ///
    /// This can be used to generate precisely-timed events.
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::{AudioContext, lpf::LowPassNode};
    /// # use firewheel::clock::ClockSeconds;
    /// fn mute_all(mut q: Query<&mut LowPassNode>, mut context: ResMut<AudioContext>) {
    ///     let now = context.now();
    ///     for mut filter in q.iter_mut() {
    ///         filter.
    ///             frequency
    ///             .push_curve(
    ///                 0.,
    ///                 now,
    ///                 now + ClockSeconds(1.),
    ///                 EaseFunction::ExponentialOut,
    ///             )
    ///             .unwrap();
    ///     }
    /// }
    /// ```
    pub fn now(&mut self) -> ClockSeconds {
        self.with(|c| c.clock_now())
    }

    /// Send `f` to the underlying control thread to operate on the audio context.
    ///
    /// This call will block until `f` returns.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_seedling::AudioContext;
    /// fn system(mut context: ResMut<AudioContext>) {
    ///     let input_devices = context.with(|context| {
    ///         context.available_input_devices()
    ///     });
    /// }
    /// ```
    ///
    /// This method takes a mutable reference to `self` to prevent trivial deadlocks.
    /// This API can't completely prevent them in the general case: calling
    /// [AudioContext::with] within itself will deadlock.
    ///
    /// This API is based on [this PR](https://github.com/bevyengine/bevy/pull/9122).
    pub fn with<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut FirewheelContext) -> O + Send,
        O: Send + 'static,
    {
        let (send, receive) = mpsc::sync_channel(1);
        let func: Box<dyn FnOnce(&mut FirewheelContext) + Send> = Box::new(move |ctx| {
            let result = f(ctx);
            send.send(result).unwrap();
        });

        // # SAFETY
        //
        // This thread will block until the function returns,
        // so we can pretend it has a static lifetime.
        let func = unsafe {
            core::mem::transmute::<
                Box<dyn FnOnce(&mut FirewheelContext) + Send>,
                Box<dyn FnOnce(&mut FirewheelContext) + Send + 'static>,
            >(func)
        };

        // If the audio communication thread fails to send or receive
        // messages, like in the event of a panic, a panic will be
        // propagated to the calling thread .
        self.0.send(func).unwrap();
        receive.recv().unwrap()
    }
}

pub(crate) fn update_context(mut context: ResMut<AudioContext>) {
    context.with(|context| {
        if let Err(e) = context.update() {
            error!("graph error: {:?}", e);
        }
    });
}
