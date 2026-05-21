use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioContext, AudioContextState, Document, Event, Window};

#[allow(clippy::type_complexity)]
struct AudioResumerState {
    audio_context: AudioContext,
    document: Document,
    listeners: Vec<(String, Closure<dyn FnMut(Event)>)>,
    on_resume: Rc<RefCell<Option<Box<dyn FnOnce()>>>>,
}

impl AudioResumerState {
    /// This function is called directly by the event listener closures.
    ///
    /// It processes the user interaction to potentially resume the audio context
    /// and clean up listeners according to the specified logic.
    fn process_interaction_event(state: &RefCell<Option<Self>>) {
        let mut state_borrow = state.borrow_mut();
        let Some(inner_state) = state_borrow.as_mut() else {
            return;
        };

        if inner_state.audio_context.state() != AudioContextState::Running {
            // AudioContext is not running, attempt to resume it.
            match inner_state.audio_context.resume() {
                Ok(promise) => {
                    let on_resume = inner_state.on_resume.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        match JsFuture::from(promise).await {
                            Ok(_) => {
                                if let Some(on_resume) = on_resume.borrow_mut().take() {
                                    on_resume();
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "Audio autoresume: Promise to resume context failed: {e:?}"
                                );
                            }
                        }
                    });
                }
                Err(e) => {
                    log::error!("Audio autoresume: Error calling resume on AudioContext: {e:?}");
                }
            }
        } else {
            for (event_name, closure) in inner_state.listeners.drain(..) {
                if let Err(e) = inner_state.document.remove_event_listener_with_callback(
                    &event_name,
                    closure.as_ref().unchecked_ref(),
                ) {
                    log::error!(
                        "Audio autoresume: Failed to remove event listener for '{event_name}': {e:?}",
                    );
                }
            }

            *state_borrow = None;
        }
    }
}

thread_local! {
    static GLOBAL_AUDIO_RESUMER_STATE: RefCell<Option<AudioResumerState>> = const { RefCell::new(None) };
}

/// Sets up automatic resumption for a given `AudioContext` upon user interaction.
///
/// If the `AudioContext` is already running, this function does nothing.
/// Otherwise, it attaches event listeners to the document for common user interaction events.
///
/// When an interaction occurs:
///   - If the context is already running, all listeners are removed.
///   - If the context is not running, `resume()` is called on it. Listeners remain.
pub fn setup_autoresume(
    audio_context: AudioContext,
    on_resume: impl FnOnce() + 'static,
) -> Result<(), JsValue> {
    // Check if autoresume is already set up to prevent duplication.
    if GLOBAL_AUDIO_RESUMER_STATE.with(|s| s.borrow().is_some()) {
        return Ok(());
    }

    // If the provided AudioContext is already in a running state, no listeners are needed.
    if audio_context.state() == AudioContextState::Running {
        return Ok(());
    }

    let window: Window =
        web_sys::window().ok_or_else(|| JsValue::from_str("Failed to get window object"))?;
    let document: Document = window
        .document()
        .ok_or_else(|| JsValue::from_str("Failed to get document object"))?;

    let user_input_event_names = [
        "click",
        "contextmenu",
        "auxclick",
        "dblclick",
        "mousedown",
        "mouseup",
        "pointerup",
        "touchend",
        "keydown",
        "keyup",
    ];

    let mut listeners = Vec::new();
    for event_name_str in user_input_event_names.iter() {
        let event_name = event_name_str.to_string();

        let closure = Closure::wrap(Box::new(move |_event: Event| {
            GLOBAL_AUDIO_RESUMER_STATE.with(AudioResumerState::process_interaction_event);
        }) as Box<dyn FnMut(Event)>);

        document.add_event_listener_with_callback(&event_name, closure.as_ref().unchecked_ref())?;

        listeners.push((event_name.clone(), closure));
    }

    // Create the shared state for the resumer.
    GLOBAL_AUDIO_RESUMER_STATE.with(|s| {
        *s.borrow_mut() = Some(AudioResumerState {
            audio_context: audio_context.clone(),
            document: document.clone(),
            listeners,
            on_resume: Rc::new(RefCell::new(Some(Box::new(on_resume)))),
        });
    });

    Ok(())
}

/// Tears down the audio context autoresume feature by removing all active event listeners
/// and cleaning up associated state.
#[expect(dead_code)]
pub fn teardown_autoresume() {
    GLOBAL_AUDIO_RESUMER_STATE.with(|s| {
        if let Some(mut state) = s.borrow_mut().take() {
            let listeners_to_remove = std::mem::take(&mut state.listeners);
            for (event_name, closure) in listeners_to_remove {
                if let Err(e) = state.document.remove_event_listener_with_callback(
                    &event_name,
                    closure.as_ref().unchecked_ref(),
                ) {
                    log::error!(
                        "Audio autoresume: Failed to remove event listener for '{event_name}' during teardown: {e:?}",
                    );
                }
            }
        }
    });
}
