use crate::error::{err, CueError, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc;
use std::thread::JoinHandle;

/// Receives 16 kHz mono f32 frames.
pub type Sink = Box<dyn FnMut(&[f32]) + Send>;

/// Keeps a stream alive on its own thread (cpal streams aren't always `Send`); dropping it stops the stream.
pub struct Capture {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    /// Runs `open` on a new thread and holds what it returns there until the `Capture` is dropped.
    pub fn hold<S>(open: impl FnOnce() -> Result<S> + Send + 'static) -> Result<Capture> {
        let (ready_tx, ready) = mpsc::channel();
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = std::thread::spawn(move || match open() {
            Ok(stream) => {
                let _ = ready_tx.send(Ok(()));
                let _ = stopped.recv(); // returns once the sender is dropped
                drop(stream);
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        });
        let result = ready.recv().unwrap_or_else(|_| err("Audio capture stopped unexpectedly."));
        let capture = Capture { stop: Some(stop), thread: Some(thread) };
        result.map(|()| capture)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn start_mic(mut sink: Sink) -> Result<Capture> {
    Capture::hold(move || {
        let device = cpal::default_host().default_input_device().ok_or_else(|| mic_error(MicError::NoDevice))?;
        let config = device.default_input_config().map_err(|e| mic_error(classify(e.to_string())))?;
        let (channels, rate) = (config.channels(), config.sample_rate().0);
        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| sink(&to_mono_16k(data, channels, rate)),
                |e| eprintln!("microphone stream error: {e}"),
                None,
            )
            .map_err(|e| mic_error(classify(e.to_string())))?;
        stream.play().map_err(|e| mic_error(classify(e.to_string())))?;
        Ok(stream)
    })
}

// Task 12 (macOS) and Task 13 (Windows) replace this with real system-audio capture.
pub fn start_system(_sink: Sink) -> Result<Capture> {
    err("System audio capture isn't available on this platform yet.")
}

// ponytail: linear resample, fine for speech; use rubato if accuracy suffers
pub fn to_mono_16k(input: &[f32], channels: u16, rate: u32) -> Vec<f32> {
    let channels = channels.max(1) as usize;
    let mono: Vec<f32> = input.chunks_exact(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect();
    if rate == 16_000 || mono.is_empty() {
        return mono;
    }
    let step = rate as f64 / 16_000.0;
    let len = (mono.len() as f64 / step) as usize;
    (0..len)
        .map(|i| {
            let pos = i as f64 * step;
            let (j, t) = (pos as usize, pos.fract() as f32);
            let next = mono.get(j + 1).copied().unwrap_or(mono[j]);
            mono[j] + (next - mono[j]) * t
        })
        .collect()
}

pub enum MicError {
    NoDevice,
    Denied,
    Other(String),
}

fn classify(message: String) -> MicError {
    // WASAPI reports a privacy-settings block as E_ACCESSDENIED.
    if cfg!(windows) && (message.contains("0x80070005") || message.contains("Access is denied")) {
        return MicError::Denied;
    }
    MicError::Other(message)
}

pub fn mic_error(e: MicError) -> CueError {
    CueError(match e {
        MicError::NoDevice => "No working microphone found.".into(),
        MicError::Denied if cfg!(windows) => "Allow microphone access for Cue in Settings → Privacy & security → Microphone.".into(),
        MicError::Denied => "Allow Microphone access for Cue in System Settings → Privacy & Security.".into(),
        MicError::Other(s) => s,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_mono_16k_downsamples_48k_stereo() {
        let out = to_mono_16k(&vec![0.5; 4800 * 2], 2, 48_000);
        assert!(out.len().abs_diff(1600) <= 1, "{}", out.len());
        assert!(out.iter().all(|&s| s == 0.5));
    }

    #[test]
    fn no_device_error_text() {
        assert_eq!(mic_error(MicError::NoDevice).0, "No working microphone found.");
    }
}
