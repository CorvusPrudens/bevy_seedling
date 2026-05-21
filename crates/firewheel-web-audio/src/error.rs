use core::fmt::Display;
use wasm_bindgen::JsValue;

pub trait JsContext<T> {
    fn context(self, context: impl Display) -> Result<T, String>;
}

impl<T> JsContext<T> for Result<T, JsValue> {
    fn context(self, context: impl Display) -> Result<T, String> {
        self.map_err(|e| format!("Error: {context}: {e:?}"))
    }
}
