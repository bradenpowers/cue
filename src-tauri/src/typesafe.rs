use crate::ai::{redact, send, HttpRequest};
use crate::error::{err, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    Behavioral,
    Technical,
    Experience,
    Logistics,
    SmallTalk,
    Other,
}

impl QuestionKind {
    pub const ALL: [QuestionKind; 6] = [Self::Behavioral, Self::Technical, Self::Experience, Self::Logistics, Self::SmallTalk, Self::Other];

    pub fn raw(self) -> &'static str {
        match self {
            Self::Behavioral => "behavioral",
            Self::Technical => "technical",
            Self::Experience => "experience",
            Self::Logistics => "logistics",
            Self::SmallTalk => "small_talk",
            Self::Other => "other",
        }
    }

    pub fn rubric(self) -> &'static str {
        match self {
            Self::Behavioral => "Asks how the candidate handled a past situation, such as 'Tell me about a time…', a conflict, a failure, or leading others",
            Self::Technical => "Asks the candidate to solve, design, explain, or reason through a technical or domain problem",
            Self::Experience => "Asks about the candidate's background, a specific job or project, their skills, or why they want this role",
            Self::Logistics => "Asks about salary, availability, start date, location, work authorization, or the interview process",
            Self::SmallTalk => "Greetings, pleasantries, or casual chat that needs no prepared answer",
            Self::Other => "Any other question, or no question is being asked",
        }
    }

    pub fn guidance(self) -> Option<&'static str> {
        match self {
            Self::Behavioral => Some("This is a behavioral question: outline one real example as situation, task, action, and result."),
            Self::Technical => Some("This is a technical question: lead with the approach, then name the key trade-offs."),
            Self::Experience => Some("This is a background question: connect the most relevant experience directly to the role."),
            Self::Logistics => Some("This is a logistics question: suggest a plain one- or two-sentence reply and never invent numbers or dates."),
            Self::SmallTalk => Some("This is small talk: suggest a short, friendly reply."),
            Self::Other => None,
        }
    }

    fn from_raw(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.raw() == s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assessment {
    pub asks_question: f64,
    pub kind: QuestionKind,
    /// The most relevant résumé lines, in résumé order. Empty means none cleared the bar.
    pub evidence: Vec<String>,
}

pub const ACCOUNT: &str = "typesafe";
pub const MODEL: &str = "jev-latest";
// Calibration knobs: tune against real interviews.
pub const QUESTION_THRESHOLD: f64 = 0.7; // P(interviewer just asked something) before auto-suggesting
pub const KIND_THRESHOLD: f64 = 0.5; // below this, send no question-type guidance
pub const EVIDENCE_THRESHOLD: f64 = 1.5; // 0 unrelated … 3 direct example
pub const MAX_EVIDENCE: usize = 8;
pub const RELEVANCE_LEVELS: [&str; 4] = ["Unrelated to the question", "Loosely related background", "Relevant supporting evidence", "A direct, strong example for this answer"];

// ponytail: single lines lose their employer heading; score whole sections if answers need that context.
pub fn resume_lines(resume: &str) -> Vec<String> {
    resume
        .split(['\n', '\r', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}'])
        .map(|l| l.trim_matches([' ', '\t']).to_string())
        .filter(|l| l.chars().count() >= 25)
        .take(80)
        .collect()
}

pub fn make_request(key: &str, role: &str, earlier: &str, latest: &str, lines: &[String], detect_question: bool) -> Result<HttpRequest> {
    let key = key.trim();
    if key.is_empty() {
        return err("Add your TypeSafe API key in Setup.");
    }
    let criteria: Map<String, Value> = QuestionKind::ALL.iter().map(|k| (k.raw().to_string(), json!(k.rubric()))).collect();
    let mut questions = Map::new();
    questions.insert(
        "question_type".into(),
        json!({"type": "choice", "instructions": "What kind of interview question is the most recent question in `latest_speech`?", "criteria": criteria}),
    );
    if detect_question {
        questions.insert(
            "asks_question".into(),
            json!({"type": "noul", "instructions": "Does `latest_speech` end with the interviewer asking the candidate a question, or making a request, that the candidate should answer now?",
                   "criteria": {"true": "The final sentences pose a complete question or request to the candidate, such as 'Tell me about a time…' or 'How would you design…'",
                                "false": "It ends with a statement, an unfinished question, the candidate's own answer, or a quick check that needs no prepared answer, such as 'Can you hear me?'"}}),
        );
    }
    for i in 0..lines.len() {
        questions.insert(
            format!("line_{i}"),
            json!({"type": "score", "instructions": format!("How useful is `resume_lines[{i}]` as evidence for the candidate's answer to the most recent question in `latest_speech`?"), "criteria": RELEVANCE_LEVELS}),
        );
    }
    let state = json!({"role": role.chars().take(4000).collect::<String>(), "earlier_conversation": earlier, "latest_speech": latest, "resume_lines": lines});
    Ok(HttpRequest {
        url: "https://api.typesafe.ai/v1/systemone".into(),
        headers: vec![("Authorization".into(), format!("Bearer {key}")), ("Content-Type".into(), "application/json".into())],
        body: json!({"model": MODEL, "state": state, "questions": questions}),
        timeout: Duration::from_secs(30),
    })
}

pub fn parse(body: &[u8], status: u16, lines: &[String], key: &str) -> Result<Assessment> {
    let object: Option<Value> = serde_json::from_slice(body).ok();
    let answers = object.as_ref().and_then(|o| o["answers"].as_object()).filter(|_| (200..300).contains(&status));
    let Some(answers) = answers else {
        let e = object.as_ref().map(|o| &o["error"]);
        let detail = e
            .and_then(|e| e["message"].as_str().or_else(|| e.as_str()))
            .or_else(|| object.as_ref().and_then(|o| o["detail"].as_str()))
            .map(String::from)
            .unwrap_or_else(|| {
                if status == 429 || status == 529 { "Busy right now; try again shortly.".into() } else { format!("Request failed (HTTP {status}). Check your key.") }
            });
        return err(format!("TypeSafe: {}", redact(&detail, key)));
    };
    let kind_answer = answers.get("question_type");
    let top = kind_answer.and_then(|t| t["choice"].as_str()).and_then(QuestionKind::from_raw).unwrap_or(QuestionKind::Other);
    let top_p = kind_answer.and_then(|t| t["probabilities"][top.raw()].as_f64()).unwrap_or(0.0);
    let mut scored: Vec<(usize, f64)> =
        (0..lines.len()).filter_map(|i| answers.get(&format!("line_{i}")).and_then(|a| a["score"].as_f64()).map(|s| (i, s))).filter(|&(_, s)| s >= EVIDENCE_THRESHOLD).collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut idx: Vec<usize> = scored.into_iter().take(MAX_EVIDENCE).map(|(i, _)| i).collect();
    idx.sort_unstable();
    Ok(Assessment {
        asks_question: answers.get("asks_question").and_then(|a| a["noul"].as_f64()).unwrap_or(0.0),
        kind: if top_p >= KIND_THRESHOLD { top } else { QuestionKind::Other },
        evidence: idx.into_iter().map(|i| lines[i].clone()).collect(),
    })
}

pub async fn assess(client: &reqwest::Client, key: &str, role: &str, earlier: &str, latest: &str, lines: &[String], detect_question: bool) -> Result<Assessment> {
    let req = make_request(key, role, earlier, latest, lines, detect_question)?;
    match send(client, &req).await {
        Ok((status, body)) => parse(&body, status, lines, key),
        Err(_) => err("Could not reach TypeSafe. Check your internet connection."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lines() -> Vec<String> {
        ["Led the payments migration to Kubernetes across 40 services", "Mentored four junior engineers through promotion", "Built the company softball league schedule app"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn request_asks_one_score_per_line_and_optional_detection() {
        let l = lines();
        let req = make_request(" ts-key ", "Staff engineer", "", "Tell me about a migration you led.", &l, true).unwrap();
        assert_eq!(req.url, "https://api.typesafe.ai/v1/systemone");
        let auth = req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("authorization")).map(|(_, v)| v.as_str());
        assert_eq!(auth, Some("Bearer ts-key"));
        let mut keys: Vec<_> = req.body["questions"].as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["asks_question", "line_0", "line_1", "line_2", "question_type"]);
        assert_eq!(req.body["questions"]["line_2"]["type"], "score");
        assert_eq!(req.body["state"]["resume_lines"], json!(l));
        let manual = make_request("k", "", "", "q", &[], false).unwrap();
        let keys: Vec<_> = manual.body["questions"].as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["question_type"]);
        assert!(make_request("  ", "", "", "q", &[], false).is_err());
    }

    #[test]
    fn parse_keeps_relevant_lines_in_resume_order_and_gates_kind() {
        let l = lines();
        let json = r#"{"model":"jev-1.13.0","answers":{"asks_question":{"type":"noul","noul":0.93},"question_type":{"type":"choice","choice":"behavioral","probabilities":{"behavioral":0.8,"experience":0.2},"confidence":0.7},"line_0":{"type":"score","score":2.9},"line_1":{"type":"score","score":1.6},"line_2":{"type":"score","score":0.1}}}"#;
        let a = parse(json.as_bytes(), 200, &l, "").unwrap();
        assert_eq!(a, Assessment { asks_question: 0.93, kind: QuestionKind::Behavioral, evidence: vec![l[0].clone(), l[1].clone()] });

        let unsure = r#"{"answers":{"question_type":{"type":"choice","choice":"technical","probabilities":{"technical":0.4,"experience":0.35,"other":0.25}}}}"#;
        assert_eq!(parse(unsure.as_bytes(), 200, &l, "").unwrap(), Assessment { asks_question: 0.0, kind: QuestionKind::Other, evidence: vec![] });
    }

    #[test]
    fn errors_redact_key() {
        let e = parse(br#"{"detail":"bad key sk-123"}"#, 401, &[], "sk-123").unwrap_err();
        assert_eq!(e.to_string(), "TypeSafe: bad key [redacted]");
    }

    #[test]
    fn resume_lines_drop_short_fragments() {
        assert_eq!(resume_lines("ACME\n  Led the payments migration to Kubernetes  \n\n2020–2023"), ["Led the payments migration to Kubernetes"]);
    }
}
