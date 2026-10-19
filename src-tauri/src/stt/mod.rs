pub mod model;
pub mod vad;

#[cfg(target_os = "macos")]
mod apple;
#[cfg(target_os = "macos")]
pub use apple::Recognizer;

#[cfg(windows)]
mod whisper;
#[cfg(windows)]
pub use whisper::Recognizer;

#[cfg(not(any(target_os = "macos", windows)))]
mod stub;
#[cfg(not(any(target_os = "macos", windows)))]
pub use stub::Recognizer;

#[derive(Debug, Clone, PartialEq)]
pub enum SttEvent {
    Partial(String),
    Final(String),
    Error(String),
}
