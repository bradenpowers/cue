pub const VAD_THRESHOLD: f32 = 0.01;
pub const PAUSE_MS: u32 = 900;
pub const MAX_MS: u32 = 28_000;

const FRAME: usize = 480; // 30 ms at 16 kHz

/// Splits 16 kHz speech into utterances by RMS energy: an utterance ends after `pause_ms` of quiet or at `max_ms`.
pub struct Segmenter {
    threshold: f32,
    pause: usize,
    max: usize,
    pending: Vec<f32>,
    utterance: Vec<f32>,
    quiet: usize,
}

impl Segmenter {
    pub fn new(threshold: f32, pause_ms: u32, max_ms: u32) -> Self {
        let samples = |ms: u32| ms as usize * 16;
        Segmenter { threshold, pause: samples(pause_ms), max: samples(max_ms), pending: vec![], utterance: vec![], quiet: 0 }
    }

    /// Returns the finished utterance, if this audio ended one. Audio after it is kept for the next push.
    pub fn push(&mut self, pcm: &[f32]) -> Option<Vec<f32>> {
        self.pending.extend_from_slice(pcm);
        let mut used = 0;
        let mut done = None;
        while done.is_none() && self.pending.len() - used >= FRAME {
            let frame = &self.pending[used..used + FRAME];
            used += FRAME;
            let rms = (frame.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt();
            if rms >= self.threshold {
                self.quiet = 0;
            } else if self.speaking() {
                self.quiet += FRAME;
            } else {
                continue;
            }
            self.utterance.extend_from_slice(frame);
            if self.quiet >= self.pause || self.utterance.len() >= self.max {
                self.quiet = 0;
                done = Some(std::mem::take(&mut self.utterance));
            }
        }
        self.pending.drain(..used);
        done
    }

    pub fn current(&self) -> &[f32] {
        &self.utterance
    }

    pub fn speaking(&self) -> bool {
        !self.utterance.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: usize = 16_000;

    fn sine(n: usize) -> Vec<f32> {
        (0..n).map(|i| 0.3 * (i as f32 * 440.0 * std::f32::consts::TAU / S as f32).sin()).collect()
    }

    fn feed(seg: &mut Segmenter, pcm: &[f32]) -> Vec<Vec<f32>> {
        pcm.chunks(480).filter_map(|c| seg.push(c)).collect()
    }

    fn segmenter() -> Segmenter {
        Segmenter::new(VAD_THRESHOLD, PAUSE_MS, MAX_MS)
    }

    #[test]
    fn segmenter_splits_at_pause() {
        let pcm = [sine(S), vec![0.0; S], sine(S), vec![0.0; S]].concat();
        let out = feed(&mut segmenter(), &pcm);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|u| (16_000..=32_000).contains(&u.len())), "{:?}", out.iter().map(Vec::len).collect::<Vec<_>>());
    }

    #[test]
    fn segmenter_force_splits_at_max() {
        let out = feed(&mut segmenter(), &sine(30 * S));
        assert_eq!(out.len(), 1);
        assert!(out[0].len().abs_diff(448_000) <= 480, "{}", out[0].len());
    }

    #[test]
    fn segmenter_ignores_silence() {
        let mut seg = segmenter();
        assert!(feed(&mut seg, &vec![0.0; 5 * S]).is_empty());
        assert!(!seg.speaking());
    }
}
