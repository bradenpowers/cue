pub mod model;
pub mod vad;

#[cfg(target_os = "macos")]
mod apple;
#[cfg(target_os = "macos")]
pub use apple::Recognizer;

#[cfg(not(target_os = "macos"))]
mod stub; // Task 13 (Windows) swaps in its real recognizer here.
#[cfg(not(target_os = "macos"))]
pub use stub::Recognizer;

#[derive(Debug, Clone, PartialEq)]
pub enum SttEvent {
    Partial(String),
    Final(String),
    Error(String),
}
