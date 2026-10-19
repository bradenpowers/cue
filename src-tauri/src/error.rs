use serde::{Serialize, Serializer};
use std::fmt;

#[derive(Debug)]
pub struct CueError(pub String);

impl fmt::Display for CueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CueError {}

impl Serialize for CueError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl From<std::io::Error> for CueError {
    fn from(e: std::io::Error) -> Self {
        CueError(e.to_string())
    }
}

impl From<serde_json::Error> for CueError {
    fn from(e: serde_json::Error) -> Self {
        CueError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CueError>;

pub fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(CueError(msg.into()))
}
