use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    Me,
    Them,
}

impl Speaker {
    pub fn label(&self) -> &'static str {
        match self {
            Speaker::Me => "Me",
            Speaker::Them => "Interviewer",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Line {
    pub id: Uuid,
    pub speaker: Speaker,
    pub text: String,
    pub start: DateTime<Utc>,
}

// Phrases that open an interviewer prompt, matched at the start of a clause.
const PROMPTS: [&str; 18] = [
    "tell me", "walk me", "talk me through", "take me through", "describe", "explain", "give me an example", "share an example",
    "how would you", "how do you", "how did you", "what would you", "why do you", "why did you", "can you", "could you", "what's your", "what is your",
];

impl Line {
    pub fn new(speaker: Speaker, text: impl Into<String>) -> Self {
        Line { id: Uuid::new_v4(), speaker, text: text.into(), start: Utc::now() }
    }

    // ponytail: keyword heuristic so answers start instantly; TypeSafe catches the questions it misses.
    pub fn sounds_like_question(&self) -> bool {
        let text = self.text.to_lowercase();
        if text.trim().ends_with('?') {
            return true;
        }
        // Check each clause so "So, tell me about…" counts but "let me explain" does not.
        text.split(|c| ".!,;".contains(c))
            .any(|clause| PROMPTS.iter().any(|p| clause.trim().starts_with(p)))
    }
}

pub fn transcript(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| format!("{}: {}", l.speaker.label(), l.text))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every captured line in the order it was spoken.
pub fn merged(a: &[Line], b: &[Line]) -> Vec<Line> {
    let mut all: Vec<Line> = a.iter().chain(b.iter()).cloned().collect();
    all.sort_by_key(|l| l.start);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots_questions_and_prompts() {
        for t in ["What drew you to this role?", "So, tell me about a time you disagreed with a manager.", "Great. Walk me through your last project",
                  "Okay. How would you design a rate limiter", "Can you share an example of leading without authority"] {
            assert!(Line::new(Speaker::Them, t).sounds_like_question(), "{t}");
        }
    }

    #[test]
    fn ignores_statements() {
        for t in ["Thanks, that makes sense.", "Let me explain how the team is structured.", "We use Swift and Kotlin here", ""] {
            assert!(!Line::new(Speaker::Them, t).sounds_like_question(), "{t}");
        }
    }

    #[test]
    fn transcript_labels_speakers() {
        assert_eq!(transcript(&[Line::new(Speaker::Them, "Why Cue?"), Line::new(Speaker::Me, "I love the product.")]),
                   "Interviewer: Why Cue?\nMe: I love the product.");
    }

    #[test]
    fn merged_orders_by_start() {
        let b = Line::new(Speaker::Them, "b");
        let a = Line { start: b.start + chrono::Duration::seconds(1), ..Line::new(Speaker::Me, "a") };
        assert_eq!(merged(&[a.clone()], &[b.clone()]), vec![b, a]);
    }
}
