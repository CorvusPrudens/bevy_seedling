use core::cell::RefCell;
use firewheel::{FirewheelConfig, FirewheelContext};

use super::{AudioThreadState, LocalStore};

thread_local! {
    static CONTEXT: RefCell<AudioThreadState> = panic!("audio context should be initialized");
}

/// A simple, single-threaded context wrapper.
#[derive(Debug)]
pub struct InnerContext(());

impl InnerContext {
    /// Spawn the audio process and control thread.
    #[inline(always)]
    pub fn new(settings: FirewheelConfig) -> Self {
        CONTEXT.set(AudioThreadState::new(settings));

        Self(())
    }

    /// Operate on the underlying context.
    #[inline(always)]
    pub fn with_store<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut FirewheelContext, &mut LocalStore) -> O + Send,
        O: Send + 'static,
    {
        CONTEXT.with(|state| {
            let mut state = state.borrow_mut();
            let AudioThreadState { context, store } = &mut *state;
            f(context, store)
        })
    }
}
