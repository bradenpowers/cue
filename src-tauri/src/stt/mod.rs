pub mod vad;

mod stub; // Tasks 12 (macOS) and 13 (Windows) swap in their real recognizers here.
pub use stub::Recognizer;

#[derive(Debug, Clone, PartialEq)]
pub enum SttEvent {
    Partial(String),
    Final(String),
    Error(String),
}
