use super::SeedlingContext;
use firewheel::{FirewheelConfig, FirewheelCtx, backend::AudioBackend};
use std::sync::mpsc;

/// A thread-safe wrapper around the underlying Firewheel audio context.
#[derive(Debug)]
pub struct InnerContext(mpsc::Sender<ThreadLocalCall>);

type ThreadLocalCall = Box<dyn FnOnce(&mut SeedlingContext) + Send + 'static>;

impl InnerContext {
    // Spawn the audio process and control thread.
    #[inline(always)]
    pub fn new<B>(
        settings: FirewheelConfig,
        stream_settings: B::Config,
    ) -> bevy_ecs::prelude::Result<Self>
    where
        B: AudioBackend + 'static,
        B::Config: Send + 'static,
        B::StreamError: Send + Sync + 'static,
    {
        let (audio_to_bev_tx, audio_to_bev_rx) = mpsc::channel::<Result<(), String>>();
        let (bev_to_audio_tx, bev_to_audio_rx) = mpsc::channel::<ThreadLocalCall>();

        std::thread::spawn(move || {
            let mut context = FirewheelCtx::<B>::new(settings);
            let result = context.start_stream(stream_settings);

            if let Err(e) = result {
                audio_to_bev_tx
                    .send(Err(format!("failed to start audio stream: {e:?}")))
                    .unwrap();
                return;
            } else {
                audio_to_bev_tx.send(Ok(())).unwrap();
            }

            let mut context = SeedlingContext::new(context);

            while let Ok(func) = bev_to_audio_rx.recv() {
                (func)(&mut context);
            }
        });

        audio_to_bev_rx.recv()??;
        Ok(InnerContext(bev_to_audio_tx))
    }

    // Send `f` to the underlying control thread to operate on the audio context.
    //
    // This call will block until `f` returns.
    //
    // This method takes a mutable reference to `self` to prevent trivial deadlocks.
    // This API can't completely prevent them in the general case: calling
    // [AudioContext::with] within itself will deadlock.
    //
    // This API is based on [this PR](https://github.com/bevyengine/bevy/pull/9122).
    #[inline(always)]
    pub fn with<F, O>(&mut self, f: F) -> O
    where
        F: FnOnce(&mut SeedlingContext) -> O + Send,
        O: Send + 'static,
    {
        let (send, receive) = mpsc::sync_channel(1);
        let func: Box<dyn FnOnce(&mut SeedlingContext) + Send> = Box::new(move |ctx| {
            let result = f(ctx);
            send.send(result).unwrap();
        });

        // # SAFETY
        //
        // This thread will block until the function returns,
        // so we can pretend it has a static lifetime.
        let func = unsafe {
            core::mem::transmute::<
                Box<dyn FnOnce(&mut SeedlingContext) + Send>,
                Box<dyn FnOnce(&mut SeedlingContext) + Send + 'static>,
            >(func)
        };

        // If the audio communication thread fails to send or receive
        // messages, like in the event of a panic, a panic will be
        // propagated to the calling thread .
        self.0.send(func).unwrap();
        receive.recv().unwrap()
    }
}
