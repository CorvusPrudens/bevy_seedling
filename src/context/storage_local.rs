use bevy_ecs::{
    prelude::World,
    system::{Commands, NonSendMut, SystemParam},
};
use firewheel::{FirewheelConfig, FirewheelContext};

use super::{AudioThreadState, LocalStore};

/// A simple, single-threaded context wrapper.
#[derive(Debug, SystemParam)]
pub struct InnerContext<'w>(NonSendMut<'w, ThreadLocalContext>);

struct ThreadLocalContext(AudioThreadState);

impl core::fmt::Debug for ThreadLocalContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ThreadLocalContext").finish()
    }
}

impl InnerContext<'_> {
    /// Spawn the audio process and control thread.
    #[inline(always)]
    pub fn insert(settings: FirewheelConfig, mut commands: Commands) {
        commands.queue(move |world: &mut World| {
            world.insert_non_send_resource(ThreadLocalContext(AudioThreadState::new(settings)));
        });
    }

    /// Operate on the underlying context.
    #[inline(always)]
    pub fn with_store<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut FirewheelContext, &mut LocalStore) -> O + Send,
        O: Send + 'static,
    {
        let AudioThreadState { context, store } = &mut self.0.0;
        f(context, store)
    }
}
