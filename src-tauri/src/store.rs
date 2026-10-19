use crate::ai::{LLMConfiguration, LLMProvider};
use crate::error::{err, Result};
use chrono::{DateTime, Local, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub provider: LLMProvider,
    pub models: BTreeMap<String, String>,
    pub local_base_url: String,
    pub resume: String,
    pub role: String,
    pub welcomed: bool,
    pub whisper_model: String,
    pub redecode_interval: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: LLMProvider::Openai,
            models: BTreeMap::new(),
            local_base_url: "http://localhost:11434/v1".into(),
            resume: String::new(),
            role: String::new(),
            welcomed: false,
            whisper_model: "base.en".into(),
            redecode_interval: 1.0,
        }
    }
}

impl Settings {
    pub fn model(&self, p: LLMProvider) -> String {
        self.models.get(p.account()).cloned().unwrap_or_else(|| p.default_model().into())
    }
    pub fn set_model(&mut self, p: LLMProvider, m: &str) {
        self.models.insert(p.account().into(), m.into());
    }
    pub fn llm(&self) -> LLMConfiguration {
        LLMConfiguration { provider: self.provider, model: self.model(self.provider), local_base_url: self.local_base_url.clone() }
    }
}

fn ser_id<S: Serializer>(id: &Uuid, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&id.hyphenated().to_string().to_uppercase())
}
fn ser_date<S: Serializer>(d: &DateTime<Utc>, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&d.to_rfc3339_opts(SecondsFormat::Secs, true))
}
fn de_date<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<DateTime<Utc>, D::Error> {
    let s = String::deserialize(d)?;
    DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&Utc)).map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interview {
    #[serde(serialize_with = "ser_id")]
    pub id: Uuid,
    #[serde(serialize_with = "ser_date", deserialize_with = "de_date")]
    pub date: DateTime<Utc>,
    pub role: String,
    pub transcript: String,
    pub talking_points: Vec<String>,
    pub notes: String,
}

impl Interview {
    pub fn new() -> Self {
        // whole seconds, matching what is persisted
        let now = DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
        Self { id: Uuid::new_v4(), date: now, role: String::new(), transcript: String::new(), talking_points: vec![], notes: String::new() }
    }
    pub fn is_empty(&self) -> bool {
        self.transcript.trim().is_empty() && self.notes.trim().is_empty() && self.talking_points.is_empty()
    }
    pub fn title(&self) -> String {
        self.role.lines().next().map(str::to_string).unwrap_or_else(|| "Interview".into())
    }
    pub fn markdown(&self) -> String {
        let date = self.date.with_timezone(&Local).format("%b %-d, %Y at %-I:%M %p");
        format!(
            "# Cue session\n\n{date}\n\n## Transcript\n\n{}\n\n## Talking points\n\n{}\n\n## Notes\n\n{}",
            self.transcript,
            self.talking_points.join("\n\n"),
            self.notes
        )
    }
}

pub struct Store {
    pub dir: PathBuf,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    // Keys are owner-only from creation, never briefly world-readable.
    #[cfg(unix)]
    if path.file_name().is_some_and(|n| n == "keys.json") {
        use std::os::unix::fs::OpenOptionsExt;
        let _ = std::fs::remove_file(&tmp); // mode only applies when the file is created
        opts.mode(0o600);
    }
    std::io::Write::write_all(&mut opts.open(&tmp)?, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
    pub fn default_dir() -> PathBuf {
        dirs::data_dir().unwrap().join("Cue")
    }

    fn keys(&self) -> std::result::Result<BTreeMap<String, String>, String> {
        match std::fs::read(self.dir.join("keys.json")) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn read_key(&self, account: &str, label: &str) -> Result<String> {
        match self.keys() {
            Ok(k) => Ok(k.get(account).cloned().unwrap_or_default()),
            Err(e) => err(format!("Could not read the {label} key from Cue's settings ({e}).")),
        }
    }
    pub fn save_key(&self, value: &str, account: &str) -> Result<()> {
        let save = || -> std::result::Result<(), String> {
            let mut k = self.keys()?;
            if value.is_empty() { k.remove(account); } else { k.insert(account.into(), value.into()); }
            let bytes = serde_json::to_vec(&k).map_err(|e| e.to_string())?;
            write_atomic(&self.dir.join("keys.json"), &bytes).map_err(|e| e.0)
        };
        save().or_else(|e| err(format!("Could not save the API key ({e}).")))
    }

    pub fn load_settings(&self) -> Settings {
        std::fs::read(self.dir.join("settings.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        write_atomic(&self.dir.join("settings.json"), &serde_json::to_vec_pretty(s)?)
    }

    fn interview_path(&self, id: Uuid) -> PathBuf {
        self.dir.join("Interviews").join(format!("{}.json", id.hyphenated().to_string().to_uppercase()))
    }
    pub fn save_interview(&self, i: &Interview) -> Result<()> {
        if i.is_empty() {
            return self.delete_interview(i.id);
        }
        write_atomic(&self.interview_path(i.id), &serde_json::to_vec(i)?)
    }
    pub fn load_interviews(&self) -> Vec<Interview> {
        let Ok(rd) = std::fs::read_dir(self.dir.join("Interviews")) else { return vec![] };
        let mut v: Vec<Interview> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| serde_json::from_slice(&std::fs::read(p).ok()?).ok())
            .collect();
        v.sort_by(|a, b| b.date.cmp(&a.date));
        v
    }
    pub fn delete_interview(&self, id: Uuid) -> Result<()> {
        match std::fs::remove_file(self.interview_path(id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    /// Copies the old Swift app's UserDefaults into settings.json. Returns false (no-op) if settings.json already exists.
    #[cfg(target_os = "macos")]
    pub fn import_swift_prefs(&self, plist: &Path) -> Result<bool> {
        if self.dir.join("settings.json").exists() {
            return Ok(false);
        }
        let v = plist::Value::from_file(plist).map_err(|e| crate::error::CueError(e.to_string()))?;
        let Some(d) = v.as_dictionary() else { return err("Unexpected preferences format.") };
        let s = |k: &str| d.get(k).and_then(|v| v.as_string()).map(str::to_string);
        let mut st = Settings::default();
        if let Some(p) = s("llm.provider").and_then(|p| LLMProvider::ALL.into_iter().find(|x| x.account() == p)) {
            st.provider = p;
        }
        for p in LLMProvider::ALL {
            let key = if p == LLMProvider::Openai { "model".into() } else { format!("llm.{}.model", p.account()) };
            if let Some(m) = s(&key) { st.set_model(p, &m); }
        }
        if let Some(u) = s("llm.local.baseURL") { st.local_base_url = u; }
        st.resume = s("resume").unwrap_or_default();
        st.role = s("role").unwrap_or_default();
        st.welcomed = d.get("welcomed").and_then(|v| v.as_boolean()).unwrap_or(false);
        self.save_settings(&st)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::LLMProvider::*;
    use tempfile::TempDir;

    fn mk(older_id: Option<Uuid>, secs: i64) -> Interview {
        let mut i = Interview::new();
        i.date = DateTime::from_timestamp(secs, 0).unwrap();
        if let Some(id) = older_id { i.id = id }
        i
    }

    #[test]
    fn save_load_and_delete_interviews() {
        let d = TempDir::new().unwrap();
        let s = Store::new(d.path().to_path_buf());
        assert!(s.load_interviews().is_empty());
        let mut older = mk(None, 1000);
        older.role = "Staff Engineer\nAcme".into();
        older.transcript = "Tell me about yourself.".into();
        older.talking_points = vec!["## Q\n\nA".into()];
        older.notes = "Ask about team".into();
        let mut newer = mk(None, 2000);
        newer.notes = "Follow up".into();
        s.save_interview(&older).unwrap();
        s.save_interview(&newer).unwrap();
        assert_eq!(s.load_interviews(), vec![newer.clone(), older.clone()]);
        assert_eq!(s.load_interviews().last().unwrap().title(), "Staff Engineer");
        let mut blank = Interview::new();
        blank.id = newer.id;
        s.save_interview(&blank).unwrap();
        assert_eq!(s.load_interviews(), vec![older.clone()]);
        s.delete_interview(older.id).unwrap();
        s.delete_interview(older.id).unwrap();
        assert!(s.load_interviews().is_empty());
    }

    #[test]
    fn reads_swift_interview_json() {
        let d = TempDir::new().unwrap();
        let s = Store::new(d.path().to_path_buf());
        let dir = d.path().join("Interviews");
        std::fs::create_dir_all(&dir).unwrap();
        let id = "0F8B2C1E-5D6A-4B7C-8D9E-0A1B2C3D4E5F";
        let json = format!(r#"{{"id":"{id}","date":"1970-01-01T00:16:40Z","role":"","transcript":"t","talkingPoints":[],"notes":""}}"#);
        let path = dir.join(format!("{id}.json"));
        std::fs::write(&path, &json).unwrap();
        let loaded = s.load_interviews();
        assert_eq!(loaded.len(), 1);
        s.save_interview(&loaded[0]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#""date":"1970-01-01T00:16:40Z""#) && text.contains(id), "{text}");
        std::fs::write(&path, json.replace("40Z", "40.5Z")).unwrap();
        assert_eq!(s.load_interviews().len(), 1);
    }

    #[test]
    fn keys_stored_in_app_file() {
        let d = TempDir::new().unwrap();
        let s = Store::new(d.path().join("nested"));
        assert_eq!(s.read_key("openai", "ChatGPT / OpenAI").unwrap(), "");
        s.save_key("sk-openai", "openai").unwrap();
        s.save_key("ts-key", "typesafe").unwrap();
        assert_eq!(s.read_key("openai", "x").unwrap(), "sk-openai");
        assert_eq!(s.read_key("claude", "x").unwrap(), "");
        assert_eq!(s.read_key("typesafe", "TypeSafe").unwrap(), "ts-key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(s.dir.join("keys.json")).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        s.save_key("", "openai").unwrap();
        assert_eq!(s.read_key("openai", "x").unwrap(), "");
        assert_eq!(s.read_key("typesafe", "TypeSafe").unwrap(), "ts-key");
    }

    #[test]
    fn corrupt_keys_file_errors() {
        let d = TempDir::new().unwrap();
        let s = Store::new(d.path().to_path_buf());
        let p = d.path().join("keys.json");
        std::fs::write(&p, "not json").unwrap();
        let e = s.read_key("openai", "ChatGPT / OpenAI").unwrap_err();
        assert!(e.0.starts_with("Could not read the ChatGPT / OpenAI key from Cue's settings ("), "{}", e.0);
        assert!(s.save_key("x", "openai").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"not json");
    }

    #[test]
    fn settings_default_and_per_provider_models() {
        let d = TempDir::new().unwrap();
        let s = Store::new(d.path().to_path_buf());
        let mut st = s.load_settings();
        assert_eq!(st.provider, Openai);
        assert_eq!(st.local_base_url, "http://localhost:11434/v1");
        assert_eq!(st.whisper_model, "base.en");
        assert_eq!(st.redecode_interval, 1.0);
        assert_eq!(st.model(Openai), "gpt-4.1-mini");
        st.set_model(Claude, "x");
        s.save_settings(&st).unwrap();
        let back = s.load_settings();
        assert_eq!(back.model(Claude), "x");
        assert_eq!(back.model(Openai), "gpt-4.1-mini");
        assert_eq!(back.llm().model, "gpt-4.1-mini");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn imports_swift_prefs() {
        let d = TempDir::new().unwrap();
        let s = Store::new(d.path().to_path_buf());
        let plist = d.path().join("prefs.plist");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/swift-prefs.plist"), &plist).unwrap();
        assert!(s.import_swift_prefs(&plist).unwrap());
        let st = s.load_settings();
        assert_eq!(st.provider, Claude);
        assert_eq!(st.model(Openai), "my-existing-model");
        assert_eq!(st.model(Claude), "my-claude-model");
        assert_eq!((st.resume.as_str(), st.role.as_str()), ("R", "Ro"));
        assert!(st.welcomed);
        assert_eq!(st.local_base_url, "http://127.0.0.1:1234/v1");
        assert!(!s.import_swift_prefs(&plist).unwrap());
    }
}
