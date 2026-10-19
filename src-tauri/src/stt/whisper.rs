//! Whisper recognizer: VAD splits utterances, and the open one is re-decoded every `redecode_interval` for partials.
use super::model::model_path;
use super::vad::{Segmenter, MAX_MS, PAUSE_MS, VAD_THRESHOLD};
use super::SttEvent;
use crate::error::{err, CueError, Result};
use crate::store::Settings;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

type OnEvent = Arc<dyn Fn(SttEvent) + Send + Sync>;

pub struct Recognizer {
    audio: Sender<Vec<f32>>,
    worker: JoinHandle<()>,
}

impl Recognizer {
    pub fn start(on_event: OnEvent, settings: &Settings, data_dir: &Path) -> Result<Recognizer> {
        let path = model_path(data_dir, &settings.whisper_model);
        if !path.exists() {
            return err("Download the speech model in Setup first.");
        }
        let load = |e: whisper_rs::WhisperError| CueError(format!("Could not load the speech model: {e}"));
        let ctx = WhisperContext::new_with_params(&path, WhisperContextParameters::default()).map_err(load)?;
        let state = ctx.create_state().map_err(load)?; // keeps the model alive
        let interval = Duration::try_from_secs_f32(settings.redecode_interval).unwrap_or(Duration::from_secs(1));
        let (audio, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || run(state, rx, interval, on_event));
        Ok(Recognizer { audio, worker })
    }

    pub fn push(&self, pcm: &[f32]) {
        let _ = self.audio.send(pcm.to_vec());
    }

    pub fn stop(self) {
        drop(self.audio);
        let _ = self.worker.join();
    }
}

fn run(mut state: WhisperState, rx: Receiver<Vec<f32>>, interval: Duration, on_event: OnEvent) {
    let mut seg = Segmenter::new(VAD_THRESHOLD, PAUSE_MS, MAX_MS);
    let mut last_partial = Instant::now();
    let mut last_audio = Instant::now();
    let pause = Duration::from_millis(PAUSE_MS as u64);
    loop {
        let mut done = vec![];
        loop {
            match rx.try_recv() {
                Ok(pcm) => {
                    last_audio = Instant::now();
                    done.extend(seg.push(&pcm));
                }
                Err(TryRecvError::Empty) => break,
                // Stopped: the app has already committed the last partial, so anything decoded now would be discarded.
                Err(TryRecvError::Disconnected) => return,
            }
        }
        // WASAPI loopback delivers nothing while no app is playing, so stand in silence to end the utterance.
        if seg.speaking() && last_audio.elapsed() >= pause {
            done.extend(seg.push(&vec![0.0; PAUSE_MS as usize * 16]));
        }
        for utterance in done {
            match decode(&mut state, &utterance) {
                Ok(Some(text)) => on_event(SttEvent::Final(text)),
                Ok(None) => {}
                Err(e) => return on_event(SttEvent::Error(e)),
            }
        }
        if !seg.speaking() {
            last_partial = Instant::now();
        } else if last_partial.elapsed() >= interval {
            match decode(&mut state, seg.current()) {
                Ok(Some(text)) => on_event(SttEvent::Partial(text)),
                Ok(None) => {}
                Err(e) => return on_event(SttEvent::Error(e)),
            }
            last_partial = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn decode(state: &mut WhisperState, pcm: &[f32]) -> std::result::Result<Option<String>, String> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_no_context(true);
    params.set_single_segment(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    state.full(params, pcm).map_err(|e| e.to_string())?;
    let text: String = state.as_iter().filter_map(|s| s.to_str_lossy().ok().map(|t| t.into_owned())).collect();
    Ok(clean(&text))
}

/// Drops empty results and non-speech markers such as `[BLANK_AUDIO]`.
fn clean(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty() && !(text.starts_with('[') && text.ends_with(']'))).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn clean_drops_markers_and_blanks() {
        assert_eq!(clean("  Hello there. "), Some("Hello there.".into()));
        assert_eq!(clean(" [BLANK_AUDIO]"), None);
        assert_eq!(clean("  "), None);
    }
}
