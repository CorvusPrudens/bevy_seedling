use core::cell::RefCell;
use firewheel::{FirewheelConfig, FirewheelContext};

#[cfg(target_arch = "wasm32")]
thread_local! {
    static CONTEXT: RefCell<FirewheelContext> = panic!("audio context should be initialized");
}

/// A simple, single-threaded context wrapper.
#[derive(Debug)]
pub struct InnerContext(());

impl InnerContext {
    /// Spawn the audio process and control thread.
    #[inline(always)]
    pub fn new(settings: FirewheelConfig) -> Self {
        let mut context = FirewheelContext::new(settings);
        context
            .start_stream(Default::default())
            .expect("failed to activate the audio context");

        CONTEXT.set(context);

        Self(())
    }

    /// Operate on the underlying context.
    #[inline(always)]
    pub fn with<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut FirewheelContext) -> O + Send,
        O: Send + 'static,
    {
        CONTEXT.with(|c| f(&mut c.borrow_mut()))
    }
}
