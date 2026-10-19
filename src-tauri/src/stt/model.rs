use crate::error::{err, CueError, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub struct ModelInfo {
    pub name: &'static str,
    pub sha256: &'static str,
}

pub const MODELS: [ModelInfo; 3] = [
    ModelInfo { name: "tiny.en", sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f" },
    ModelInfo { name: "base.en", sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002" },
    ModelInfo { name: "small.en", sha256: "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d" },
];

pub fn model_path(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("models").join(format!("ggml-{name}.bin"))
}

/// Verifies `part` against the checksum and moves it to `dest`; a bad file is deleted.
fn finish(part: &Path, dest: &Path, expected_sha256: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut std::fs::File::open(part)?, &mut hasher)?;
    if format!("{:x}", hasher.finalize()) != expected_sha256 {
        let _ = std::fs::remove_file(part);
        return err("The speech model download was corrupted (checksum mismatch). Try again.");
    }
    std::fs::rename(part, dest)?;
    Ok(())
}

/// Downloads a known Whisper model into `<data>/models`, reporting a 0.0-1.0 fraction.
pub async fn download(data_dir: &Path, name: &str, progress: impl Fn(f32)) -> Result<PathBuf> {
    let Some(info) = MODELS.iter().find(|m| m.name == name) else {
        return err(format!("Unknown speech model \"{name}\"."));
    };
    let dest = model_path(data_dir, name);
    let part = dest.with_extension("bin.part");
    let result = fetch(name, &part, &progress).await.and_then(|()| finish(&part, &dest, info.sha256));
    match result {
        Ok(()) => Ok(dest),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            // The checksum message is already user-facing.
            if e.0.contains("checksum") { Err(e) } else { err(format!("Could not download the speech model ({e}). Check your connection and try again.")) }
        }
    }
}

async fn fetch(name: &str, part: &Path, progress: &impl Fn(f32)) -> Result<()> {
    let fail = |e: reqwest::Error| CueError(e.to_string());
    std::fs::create_dir_all(part.parent().unwrap())?;
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{name}.bin");
    let mut resp = reqwest::Client::new().get(url).send().await.and_then(|r| r.error_for_status()).map_err(fail)?;
    let total = resp.content_length().unwrap_or(0) as f32;
    let mut file = tokio::fs::File::create(part).await?;
    let mut received = 0f32;
    while let Some(chunk) = resp.chunk().await.map_err(fail)? {
        file.write_all(&chunk).await?;
        received += chunk.len() as f32;
        if total > 0.0 {
            progress((received / total).min(1.0));
        }
    }
    file.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_mismatch_removes_part() {
        let dir = tempfile::tempdir().unwrap();
        let (part, dest) = (dir.path().join("x.part"), dir.path().join("x"));
        std::fs::write(&part, "abc").unwrap();
        let e = finish(&part, &dest, &"0".repeat(64)).unwrap_err();
        assert!(e.0.contains("checksum"), "{e}");
        assert!(!part.exists() && !dest.exists());
    }

    #[test]
    fn checksum_match_moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let (part, dest) = (dir.path().join("x.part"), dir.path().join("x"));
        std::fs::write(&part, "abc").unwrap();
        finish(&part, &dest, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").unwrap();
        assert!(dest.exists() && !part.exists());
    }

    #[tokio::test]
    async fn unknown_name_rejected_without_touching_disk() {
        let dir = tempfile::tempdir().unwrap();
        let e = download(dir.path(), "nope", |_| {}).await.unwrap_err();
        assert!(e.0.contains("Unknown"), "{e}");
        assert!(!dir.path().join("models").exists());
    }
}
