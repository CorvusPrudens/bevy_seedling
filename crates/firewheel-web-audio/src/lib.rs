//! [![crates.io](https://img.shields.io/crates/v/firewheel-web-audio)](https://crates.io/crates/firewheel-web-audio)
//! [![docs.rs](https://docs.rs/firewheel-web-audio/badge.svg)](https://docs.rs/firewheel-web-audio)
//!
//! A multi-threaded `wasm32-unknown-unknown` Web Audio
//! backend for [Firewheel](https://github.com/BillyDM/firewheel).
//!
//! Currently, this backend only supports stereo inputs and outputs.
//!
//! ## Requirements
//!
//! Because this crate relies on Wasm multi-threading, it has
//! some additional requirements.
//!
//! 1. A nightly compiler is required along with the Rust standard library source code
//!    (with `rustup`, you can add it with `rustup component add rust-src`).
//! 2. You'll need the `atomics` target feature and additional linker settings.
//!    These can be enabled with a `.cargo/config.toml`:
//!
//! ```toml
//! [target.wasm32-unknown-unknown]
//! rustflags = [
//!   "-Ctarget-feature=+atomics",
//!   "-Clink-arg=--shared-memory",
//!   "-Clink-arg=--max-memory=1073741824",
//!   "-Clink-arg=--import-memory",
//!   "-Clink-arg=--export=__wasm_init_tls",
//!   "-Clink-arg=--export=__tls_size",
//!   "-Clink-arg=--export=__tls_align",
//!   "-Clink-arg=--export=__tls_base",
//! ]
//!
//! [unstable]
//! build-std = ["std", "panic_abort"]
//! ```
//! 3. Wherever your project is served, the protocol must be secure (usually `https`)
//!    and the response must include two security headers:
//!
//! ```text
//! Cross-Origin-Opener-Policy: same-origin
//! Cross-Origin-Embedder-Policy: require-corp
//! # or
//! Cross-Origin-Embedder-Policy: credentialless
//! ```
//!
//! Note that `credentialless` may not work on Safari: the browser
//! may throw an error in the audio worklet upon receiving shared Wasm memory.

mod auto_resume;
mod backend;
mod dynamic_module;
mod error;
mod wasm_processor;

pub use backend::{WebAudioBackend, WebAudioConfig, WebAudioStartError, WebAudioStreamError};

const BLOCK_FRAMES: usize = 128;
