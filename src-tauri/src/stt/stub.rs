use super::SttEvent;
use crate::error::{err, Result};
use crate::store::Settings;
use std::path::Path;
use std::sync::Arc;

/// Stands in on platforms whose speech recognizer isn't written yet.
pub struct Recognizer;

impl Recognizer {
    pub fn start(_on_event: Arc<dyn Fn(SttEvent) + Send + Sync>, _settings: &Settings, _data_dir: &Path) -> Result<Recognizer> {
        err("Live transcription isn't available on this platform yet.")
    }
    pub fn push(&self, _pcm: &[f32]) {}
    pub fn stop(self) {}
}
