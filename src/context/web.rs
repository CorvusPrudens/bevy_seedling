use crate::context::SeedlingContext;
use core::cell::RefCell;
use firewheel::{FirewheelConfig, FirewheelCtx, backend::AudioBackend};

#[cfg(target_arch = "wasm32")]
thread_local! {
    static CONTEXT: RefCell<SeedlingContext> = panic!("audio context should be initialized");
}

/// A simple, single-threaded context wrapper.
#[derive(Debug)]
pub struct InnerContext(());

impl InnerContext {
    /// Spawn the audio process and control thread.
    #[inline(always)]
    pub fn new<B>(settings: FirewheelConfig) -> Self
    where
        B: AudioBackend + 'static,
        B::StreamError: Send + Sync + 'static,
    {
        let mut context = FirewheelCtx::<B>::new(settings);
        CONTEXT.set(SeedlingContext::new(context));

        Self(())
    }

    /// Operate on the underlying context.
    #[inline(always)]
    pub fn with<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut SeedlingContext) -> O + Send,
        O: Send + 'static,
    {
        CONTEXT.with(|c| f(&mut c.borrow_mut()))
    }
}
