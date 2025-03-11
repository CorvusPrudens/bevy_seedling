use bevy_ecs::prelude::*;
use core::cell::RefCell;
use firewheel::{clock::ClockSeconds, FirewheelConfig, FirewheelContext};

#[cfg(target_arch = "wasm32")]
thread_local! {
    static CONTEXT: RefCell<FirewheelContext> = panic!("audio context should be initialized");
}

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
pub struct AudioContext(());

impl AudioContext {
    /// Spawn the audio process and control thread.
    pub fn new(settings: FirewheelConfig) -> Self {
        let mut context = FirewheelContext::new(settings);
        context
            .start_stream(Default::default())
            .expect("failed to activate the audio context");

        CONTEXT.set(context);

        Self(())
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
        CONTEXT.with(|c| f(&mut c.borrow_mut()))
    }
}
