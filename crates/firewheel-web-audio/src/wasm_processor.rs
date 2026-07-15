use audioadapter::{Adapter, AdapterMut};
use bevy_platform::time::Instant;
use firewheel::{
    backend::BackendProcessInfo, collector::ArcGc, node::StreamStatus,
    processor::FirewheelProcessor,
};
use js_sys::{Array, Float32Array};
use std::sync::{atomic::AtomicBool, mpsc};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};

#[wasm_bindgen]
pub(crate) struct ProcessorHost {
    pub(crate) processor: FirewheelProcessor,
    pub(crate) timestamps: mpsc::Receiver<Timestamp>,
    pub(crate) latest_timestamp: Timestamp,
    pub(crate) alive: ArcGc<AtomicBool>,
    pub(crate) inputs: usize,
    pub(crate) outputs: usize,
}

impl ProcessorHost {
    fn process_fallible(
        &mut self,
        inputs: js_sys::Array,
        outputs: js_sys::Array,
        current_time: f64,
    ) -> Result<bool, wasm_bindgen::JsValue> {
        if !self.alive.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(false);
        }

        struct WorkletBuffer {
            channels: usize,
            array: js_sys::Array,
        }

        unsafe impl Adapter<f32> for WorkletBuffer {
            // # Safety
            //
            // We don't actually dance with unsafe here.
            // I figure it's not worth it -- `copy_from_channel_to_slice`
            // should be preferred anyway.
            unsafe fn read_sample_unchecked(&self, channel: usize, frame: usize) -> f32 {
                self.array
                    .get(0)
                    .unchecked_into::<Array>()
                    .get(channel as u32)
                    .unchecked_into::<Float32Array>()
                    .at(frame as i32)
                    .unwrap()
            }

            fn channels(&self) -> usize {
                self.channels
            }

            fn frames(&self) -> usize {
                crate::BLOCK_FRAMES
            }

            fn read_sample(&self, channel: usize, frame: usize) -> Option<f32> {
                self.array
                    .get(0)
                    .dyn_into::<Array>()
                    .ok()?
                    .get(channel as u32)
                    .dyn_into::<Float32Array>()
                    .ok()?
                    .at(frame as i32)
            }

            fn copy_from_channel_to_slice(
                &self,
                channel: usize,
                skip: usize,
                slice: &mut [f32],
            ) -> usize {
                let bytes_to_copy = crate::BLOCK_FRAMES.saturating_sub(skip);
                if bytes_to_copy == 0 {
                    return 0;
                }

                let Ok(array) = self.array.get(0).dyn_into::<Array>() else {
                    return 0;
                };
                let Ok(channel_array) = array.get(channel as u32).dyn_into::<Float32Array>() else {
                    return 0;
                };
                if channel_array.is_undefined() {
                    return 0;
                }

                if bytes_to_copy != crate::BLOCK_FRAMES {
                    let channel_array =
                        channel_array.subarray(skip as u32, crate::BLOCK_FRAMES as u32);
                    channel_array.copy_to(slice);
                } else {
                    channel_array.copy_to(slice);
                }

                bytes_to_copy
            }
        }

        unsafe impl AdapterMut<f32> for WorkletBuffer {
            unsafe fn write_sample_unchecked(
                &mut self,
                channel: usize,
                frame: usize,
                value: &f32,
            ) -> bool {
                self.array
                    .get(0)
                    .unchecked_into::<Array>()
                    .get(channel as u32)
                    .unchecked_into::<Float32Array>()
                    .set_index(frame as u32, *value);

                false
            }

            fn copy_from_slice_to_channel(
                &mut self,
                channel: usize,
                skip: usize,
                slice: &[f32],
            ) -> (usize, usize) {
                let bytes_to_copy = crate::BLOCK_FRAMES.saturating_sub(skip);
                if bytes_to_copy == 0 {
                    return (0, 0);
                }

                let Ok(array) = self.array.get(0).dyn_into::<Array>() else {
                    return (0, 0);
                };
                let Ok(channel_array) = array.get(channel as u32).dyn_into::<Float32Array>() else {
                    return (0, 0);
                };
                if channel_array.is_undefined() {
                    return (0, 0);
                }

                if bytes_to_copy != crate::BLOCK_FRAMES {
                    let channel_array =
                        channel_array.subarray(skip as u32, crate::BLOCK_FRAMES as u32);

                    channel_array.copy_from(slice);
                } else {
                    channel_array.copy_from(slice);
                }

                (bytes_to_copy, 0)
            }

            fn fill_channel_with(&mut self, channel: usize, value: &f32) -> Option<()> {
                if channel >= self.channels {
                    return None;
                }

                let Ok(array) = self.array.get(0).dyn_into::<Array>() else {
                    return Some(());
                };
                let Ok(channel_array) = array.get(channel as u32).dyn_into::<Float32Array>() else {
                    return Some(());
                };
                if channel_array.is_undefined() {
                    return Some(());
                }

                channel_array.fill(*value, 0, crate::BLOCK_FRAMES as u32);

                Some(())
            }
        }

        if let Some(timestamp) = self.timestamps.try_iter().last() {
            self.latest_timestamp = timestamp;
        }

        let time_since_stamp = current_time - self.latest_timestamp.audio_thread;
        let process_timestamp = self.latest_timestamp.main_thread
            + std::time::Duration::from_secs_f64(time_since_stamp / 1000.0);

        self.processor.process(
            &WorkletBuffer {
                array: inputs,
                channels: self.inputs,
            },
            &mut WorkletBuffer {
                array: outputs,
                channels: self.outputs,
            },
            BackendProcessInfo {
                process_to_playback_delay: None,
                frames: crate::BLOCK_FRAMES,
                process_timestamp: Some(process_timestamp),
                duration_since_stream_start: std::time::Duration::from_secs_f64(current_time),
                input_stream_status: StreamStatus::empty(),
                output_stream_status: StreamStatus::empty(),
                dropped_frames: 0,
            },
        );

        Ok(true)
    }
}

#[wasm_bindgen]
#[allow(dead_code)]
impl ProcessorHost {
    /// Pack the object to send through the web audio worklet constructor
    pub fn pack(self) -> usize {
        Box::into_raw(Box::new(self)) as usize
    }

    /// Unpack the object from the worklet constructor
    /// # Safety
    /// This should only be called within the worklet constructor from a known
    /// good pointer
    pub unsafe fn unpack(ptr: usize) -> Self {
        unsafe { *Box::from_raw(ptr as *mut Self) }
    }

    pub fn process(
        &mut self,
        inputs: js_sys::Array,
        outputs: js_sys::Array,
        current_time: f64,
    ) -> bool {
        // since we're in the audio context, it's difficult to
        // do anything but ignore the error
        self.process_fallible(inputs, outputs, current_time)
            .unwrap_or(true)
    }
}

pub struct Timestamp {
    pub main_thread: Instant,
    pub audio_thread: f64,
}
