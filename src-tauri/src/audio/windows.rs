//! Computer audio on Windows: a WASAPI loopback stream (an input stream built on the default output device).
use super::{to_mono_16k, Capture, OnError, Sink};
use crate::error::{CueError, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub fn start(mut sink: Sink, on_error: OnError) -> Result<Capture> {
    Capture::hold(move || {
        let fail = |e: String| CueError(format!("Could not capture computer audio: {e}"));
        let device = cpal::default_host().default_output_device().ok_or_else(|| fail("no output device".into()))?;
        let config = device.default_output_config().map_err(|e| fail(e.to_string()))?;
        let (channels, rate) = (config.channels(), config.sample_rate().0);
        let mut reported = false;
        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| sink(&to_mono_16k(data, channels, rate)),
                // cpal's WASAPI stream thread ends after an error, so report the first one only.
                move |e| {
                    if !std::mem::replace(&mut reported, true) {
                        on_error(format!("Computer audio capture stopped: {e}"));
                    }
                },
                None,
            )
            .map_err(|e| fail(e.to_string()))?;
        stream.play().map_err(|e| fail(e.to_string()))?;
        Ok(stream)
    })
}
