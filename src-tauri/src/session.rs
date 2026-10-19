use crate::error::Result;
use crate::store::Interview;
use crate::transcript::{merged, transcript, Line, Speaker};
use crate::typesafe::Assessment;
use serde::Serialize;

#[derive(Default, Clone)]
pub struct CaptureState {
    pub running: bool,
    pub starting: bool,
    pub error: String,
    pub segments: Vec<Line>,
    pub partial: Option<Line>,
}

impl CaptureState {
    pub fn lines(&self) -> Vec<Line> {
        self.segments.iter().chain(self.partial.iter()).cloned().collect()
    }

    pub fn set_partial(&mut self, speaker: Speaker, text: &str) {
        match &mut self.partial {
            Some(p) => p.text = text.into(),
            None => self.partial = Some(Line::new(speaker, text)),
        }
    }

    pub fn commit(&mut self) -> Option<Line> {
        let mut line = self.partial.take()?;
        line.text = line.text.trim().into();
        if line.text.is_empty() {
            return None;
        }
        self.segments.push(line.clone());
        Some(line)
    }
}

#[derive(Debug, PartialEq)]
pub enum Heard {
    Ignore,
    Answer(String),
    Check { latest: String, earlier: String, count: usize },
}

#[derive(Serialize, Clone)]
pub struct CaptureView {
    running: bool,
    starting: bool,
    error: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    lines: Vec<Line>,
    them: CaptureView,
    me: CaptureView,
    answer: String,
    busy: bool,
    error: String,
    download: Option<f32>,
}

pub struct Session {
    pub them: CaptureState,
    pub me: CaptureState,
    pub answer: String,
    pub busy: bool,
    pub error: String,
    pub history: Vec<String>,
    pub notes: String,
    pub download: Option<f32>,
    pub run: u64,
    answered: usize,
    interview: Interview,
}

impl Session {
    pub fn new() -> Self {
        Session {
            them: Default::default(),
            me: Default::default(),
            answer: String::new(),
            busy: false,
            error: String::new(),
            history: vec![],
            notes: String::new(),
            download: None,
            run: 0,
            answered: 0,
            interview: Interview::new(),
        }
    }

    pub fn lines(&self) -> Vec<Line> {
        merged(&self.them.lines(), &self.me.lines())
    }

    pub fn answered(&self) -> usize {
        self.answered
    }

    pub fn mark_answered(&mut self, count: usize) {
        self.answered = count;
    }

    pub fn snapshot(&self) -> Snapshot {
        let view = |c: &CaptureState| CaptureView { running: c.running, starting: c.starting, error: c.error.clone() };
        Snapshot {
            lines: self.lines(),
            them: view(&self.them),
            me: view(&self.me),
            answer: self.answer.clone(),
            busy: self.busy,
            error: self.error.clone(),
            download: self.download,
        }
    }

    /// The interviewer finished speaking: answer now if it was a question, else maybe ask TypeSafe.
    pub fn heard(&mut self, has_typesafe_key: bool) -> Heard {
        let unanswered = &self.them.segments[self.answered.min(self.them.segments.len())..];
        let (Some(first), Some(last)) = (unanswered.first(), unanswered.last()) else { return Heard::Ignore };
        let latest = suffix(&unanswered.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" "), 800);
        if last.sounds_like_question() {
            self.answered = self.them.segments.len();
            return Heard::Answer(latest);
        }
        if !has_typesafe_key || latest.split_whitespace().count() < 3 {
            return Heard::Ignore;
        }
        let before: Vec<Line> = self.lines().into_iter().filter(|l| l.start < first.start).collect();
        Heard::Check { latest, earlier: suffix(&transcript(&before), 1500), count: self.them.segments.len() }
    }

    pub fn begin_run(&mut self) -> u64 {
        self.run += 1;
        self.busy = true;
        self.error.clear();
        self.run
    }

    /// `result` is (question, answer, note); a superseded run is ignored.
    pub fn finish_run(&mut self, id: u64, result: Result<(String, String, String)>) {
        if id != self.run {
            return;
        }
        match result {
            Ok((q, answer, note)) => {
                self.history.push(format!("## {q}\n\n{answer}"));
                self.answer = answer;
                self.error = note;
            }
            Err(e) => self.error = e.0,
        }
        self.busy = false;
    }

    pub fn to_interview(&self, role: &str) -> Interview {
        Interview {
            role: role.into(),
            transcript: transcript(&self.lines()),
            talking_points: self.history.clone(),
            notes: self.notes.clone(),
            ..self.interview.clone()
        }
    }

    pub fn reset(&mut self) {
        let run = self.run + 1; // invalidates any in-flight run
        *self = Session::new();
        self.run = run;
    }
}

pub fn experience_for(assessment: Option<&Assessment>, resume: &str) -> String {
    match assessment {
        Some(a) if !a.evidence.is_empty() => a.evidence.join("\n"),
        _ => resume.into(),
    }
}

pub fn suffix(s: &str, n: usize) -> String {
    let skip = s.chars().count().saturating_sub(n);
    s.chars().skip(skip).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hear(s: &mut Session, text: &str) {
        s.them.set_partial(Speaker::Them, text);
        s.them.commit();
    }

    #[test]
    fn heard_answers_question_heuristically() {
        let mut s = Session::new();
        let q = "So, tell me about a time you led a migration.";
        hear(&mut s, q);
        assert_eq!(s.heard(false), Heard::Answer(q.into()));
        assert_eq!(s.heard(false), Heard::Ignore);
    }

    #[test]
    fn heard_checks_statements_with_typesafe() {
        let mut s = Session::new();
        hear(&mut s, "I'd love to hear about your last project");
        assert!(matches!(s.heard(true), Heard::Check { count: 1, .. }));
        let mut s = Session::new();
        hear(&mut s, "I'd love to hear about your last project");
        assert_eq!(s.heard(false), Heard::Ignore);
    }

    #[test]
    fn heard_ignores_short_fragments() {
        let mut s = Session::new();
        hear(&mut s, "Okay great");
        assert_eq!(s.heard(true), Heard::Ignore);
    }

    #[test]
    fn heard_latest_joins_unanswered_and_caps_800() {
        let mut s = Session::new();
        // multi-word so the 3-word minimum holds (500 solid chars would be one word)
        let (a, b) = ("a ".repeat(250), "b ".repeat(250));
        let (a, b) = (a.trim(), b.trim());
        hear(&mut s, &a);
        hear(&mut s, &b);
        match s.heard(true) {
            Heard::Check { latest, .. } => assert_eq!(latest, suffix(&format!("{a} {b}"), 800)),
            h => panic!("{h:?}"),
        }
    }

    #[test]
    fn stale_run_is_ignored() {
        let mut s = Session::new();
        let a = s.begin_run();
        let _b = s.begin_run();
        s.finish_run(a, Ok(("q".into(), "ans".into(), "".into())));
        assert!(s.answer.is_empty());
        assert!(s.busy);
    }

    #[test]
    fn run_started_before_reset_is_ignored() {
        let mut s = Session::new();
        let old = s.begin_run();
        s.reset();
        s.finish_run(old, Ok(("q".into(), "ans".into(), "".into())));
        assert!(s.answer.is_empty() && s.history.is_empty() && !s.busy);
    }

    #[test]
    fn experience_falls_back_to_resume() {
        use crate::typesafe::QuestionKind;
        let with = |evidence: Vec<String>| Assessment { asks_question: 1.0, kind: QuestionKind::Other, evidence };
        assert_eq!(experience_for(None, "resume"), "resume");
        assert_eq!(experience_for(Some(&with(vec![])), "resume"), "resume");
        assert_eq!(experience_for(Some(&with(vec!["x".into(), "y".into()])), "resume"), "x\ny");
    }

    #[test]
    fn interview_snapshot_includes_unsaved_state() {
        let mut s = Session::new();
        s.them.set_partial(Speaker::Them, "half a sentence");
        s.history = vec!["## q\n\na".into()];
        s.notes = "n".into();
        let i = s.to_interview("Role");
        assert!(i.transcript.contains("half a sentence"));
        assert_eq!(i.talking_points, s.history);
        assert_eq!((i.notes.as_str(), i.role.as_str()), ("n", "Role"));
    }
}
