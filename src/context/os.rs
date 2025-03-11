use bevy_ecs::prelude::*;
use firewheel::{FirewheelConfig, FirewheelContext};
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

type ThreadLocalCall = Box<dyn FnOnce(&mut FirewheelContext) + Send + 'static>;

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
