use crate::ai::{self, LLMConfiguration, LLMProvider};
use crate::audio::{self, Capture, Sink};
use crate::error::{err, CueError, Result};
use crate::resume::{self, ImportedResume};
use crate::session::{experience_for, suffix, CaptureState, Heard, Session, Snapshot};
use crate::stt::{self, Recognizer, SttEvent};
use crate::store::{Interview, Settings, Store};
use crate::transcript::{transcript, Speaker};
use crate::typesafe::{self, Assessment, QuestionKind, QUESTION_THRESHOLD};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::async_runtime::{self, JoinHandle};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent};

pub struct AppState {
    session: Mutex<Session>,
    store: Store,
    client: reqwest::Client,
    pending: Mutex<Option<JoinHandle<()>>>,
    save_gen: AtomicU64,
    live: Mutex<[Option<Live>; 2]>, // indexed by `Speaker as usize`; only touched under the session lock
    generations: AtomicU64,
}

/// A running capture and the recognizer it feeds.
struct Live {
    capture: Capture,
    recognizer: Arc<Recognizer>,
}

impl Live {
    fn stop(self) {
        drop(self.capture); // joins the stream thread, which drops the sink's recognizer handle
        if let Ok(r) = Arc::try_unwrap(self.recognizer) {
            r.stop();
        }
    }
}

impl AppState {
    fn next_generation(&self) -> u64 {
        self.generations.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Stops listening on `c` and hands back its stream and recognizer to shut down outside the session lock.
    fn end(&self, c: &mut CaptureState, speaker: Speaker) -> Option<Live> {
        c.end(self.next_generation());
        self.live.lock().unwrap()[speaker as usize].take()
    }
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self { session: Mutex::new(Session::new()), store, client: ai::http_client(), pending: Mutex::new(None), save_gen: AtomicU64::new(0), live: Default::default(), generations: AtomicU64::new(0) }
    }
}

fn state(app: &AppHandle) -> tauri::State<'_, AppState> {
    app.state::<AppState>()
}

fn typesafe_key(s: &AppState) -> Result<String> {
    s.store.read_key(typesafe::ACCOUNT, "TypeSafe")
}

pub fn emit(app: &AppHandle) {
    let snapshot = state(app).session.lock().unwrap().snapshot();
    let _ = app.emit("state", snapshot);
}

/// Emits now and saves 2 s after the last touch.
pub fn touch(app: &AppHandle) {
    emit(app);
    let gen = state(app).save_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if state(&app).save_gen.load(Ordering::SeqCst) == gen {
            save_now(&app);
        }
    });
}

pub fn save_now(app: &AppHandle) {
    let s = state(app);
    let role = s.store.load_settings().role;
    let mut session = s.session.lock().unwrap();
    if let Err(e) = s.store.save_interview(&session.to_interview(&role)) {
        session.error = format!("Could not save this interview: {e}");
        drop(session);
        emit(app);
    }
}

fn capture(session: &mut Session, speaker: Speaker) -> &mut CaptureState {
    match speaker {
        Speaker::Them => &mut session.them,
        Speaker::Me => &mut session.me,
    }
}

/// Ports `AudioCapture.start`: `system` captures computer audio, otherwise the microphone.
fn start_capture(app: &AppHandle, speaker: Speaker, system: bool) -> Result<()> {
    let s = state(app);
    let generation = {
        let mut session = s.session.lock().unwrap();
        let c = capture(&mut session, speaker);
        if c.running || c.starting {
            return Ok(());
        }
        c.starting = true;
        c.error.clear();
        c.generation = s.next_generation();
        c.generation
    };
    emit(app);
    let opened = open_capture(app, speaker, system, generation);
    let mut session = s.session.lock().unwrap();
    let c = capture(&mut session, speaker);
    c.starting = false;
    let current = c.generation == generation;
    match opened {
        Ok(live) if current => {
            c.running = true;
            s.live.lock().unwrap()[speaker as usize] = Some(live);
            Ok(())
        }
        // Stopped, or its recognizer failed, while starting; that path already updated the state.
        Ok(live) => {
            drop(session);
            live.stop();
            Ok(())
        }
        Err(e) if current => Err(e),
        Err(_) => Ok(()),
    }
}

fn open_capture(app: &AppHandle, speaker: Speaker, system: bool, generation: u64) -> Result<Live> {
    let s = state(app);
    let events = app.clone();
    let on_event = Arc::new(move |e| on_stt(&events, speaker, generation, e));
    let recognizer = Arc::new(Recognizer::start(on_event, &s.store.load_settings(), &s.store.dir)?);
    let feed = recognizer.clone();
    let sink: Sink = Box::new(move |pcm| feed.push(pcm));
    match if system { audio::start_system(sink) } else { audio::start_mic(sink) } {
        Ok(capture) => Ok(Live { capture, recognizer }),
        Err(e) => {
            if let Ok(r) = Arc::try_unwrap(recognizer) {
                r.stop();
            }
            Err(e)
        }
    }
}

/// Ports `AudioCapture.stop`: ends the stream and recognizer and keeps any partial line.
fn stop_capture(app: &AppHandle, speaker: Speaker) {
    let s = state(app);
    let live = s.end(capture(&mut s.session.lock().unwrap(), speaker), speaker);
    if let Some(live) = live {
        live.stop();
    }
}

/// Recognizer events arrive on its own threads; ones from a stopped or replaced recognizer are dropped.
fn on_stt(app: &AppHandle, speaker: Speaker, generation: u64, event: SttEvent) {
    let s = state(app);
    let mut session = s.session.lock().unwrap();
    let c = capture(&mut session, speaker);
    if c.generation != generation {
        return;
    }
    match event {
        SttEvent::Partial(t) => {
            c.set_partial(speaker, &t);
            drop(session);
            emit(app);
        }
        SttEvent::Final(t) => {
            c.set_partial(speaker, &t);
            let line = c.commit();
            drop(session);
            touch(app);
            if line.is_some() && speaker == Speaker::Them {
                on_utterance(app);
            }
        }
        SttEvent::Error(m) => {
            let live = s.end(c, speaker);
            c.error = format!("Transcription stopped: {m}. Start listening to reconnect.");
            drop(session);
            // Stopping joins the recognizer, so do it off the recognizer's thread.
            if let Some(live) = live {
                std::thread::spawn(move || live.stop());
            }
            touch(app);
        }
    }
}

/// Starts a suggestion; the newest one wins.
fn start_suggestion(app: &AppHandle, typed: Option<String>, assessment: Option<Assessment>) {
    let task = async_runtime::spawn(run_suggestion(app.clone(), typed, assessment));
    if let Some(old) = state(app).pending.lock().unwrap().replace(task) {
        old.abort();
    }
}

async fn run_suggestion(app: AppHandle, typed: Option<String>, assessment: Option<Assessment>) {
    let s = state(&app);
    let settings = s.store.load_settings();
    let (id, transcript) = {
        let mut session = s.session.lock().unwrap();
        (session.begin_run(), transcript(&session.lines()))
    };
    emit(&app);
    let q = typed.clone().unwrap_or_else(|| "Help me respond to the latest interview question.".into());
    let result = async {
        let mut assessment = assessment;
        let mut note = String::new();
        let typesafe_key = typesafe_key(&s)?;
        if assessment.is_none() && !typesafe_key.is_empty() {
            let latest = typed.unwrap_or_else(|| suffix(&transcript, 800));
            let lines = typesafe::resume_lines(&settings.resume);
            match typesafe::assess(&s.client, &typesafe_key, &settings.role, &suffix(&transcript, 1500), &latest, &lines, false).await {
                Ok(a) => assessment = Some(a),
                Err(e) => note = format!("{e} Used your full résumé instead."),
            }
        }
        let experience = experience_for(assessment.as_ref(), &settings.resume);
        let cfg = settings.llm();
        let key = s.store.read_key(cfg.provider.account(), cfg.provider.label())?;
        let guidance = assessment.and_then(|a| a.kind.guidance());
        let answer = ai::suggest(&s.client, &cfg, &key, &experience, &settings.role, &transcript, &q, guidance).await?;
        Ok((q, answer, note))
    }
    .await;
    s.session.lock().unwrap().finish_run(id, result);
    touch(&app);
}

/// The interviewer finished speaking (called by the `them` capture after each committed line).
pub fn on_utterance(app: &AppHandle) {
    let s = state(app);
    let has_key = !typesafe_key(&s).unwrap_or_default().is_empty();
    let heard = s.session.lock().unwrap().heard(has_key);
    match heard {
        Heard::Ignore => {}
        Heard::Answer(q) => start_suggestion(app, Some(q), None),
        Heard::Check { latest, earlier, count } => {
            let app = app.clone();
            async_runtime::spawn(async move {
                let s = state(&app);
                let settings = s.store.load_settings();
                let key = typesafe_key(&s).unwrap_or_default();
                let lines = typesafe::resume_lines(&settings.resume);
                match typesafe::assess(&s.client, &key, &settings.role, &earlier, &latest, &lines, true).await {
                    Ok(a) => {
                        let go = {
                            let mut session = s.session.lock().unwrap();
                            // A newer line is already being checked; let that one decide.
                            let go = session.them.segments.len() == count && session.answered() < count && a.asks_question >= QUESTION_THRESHOLD && a.kind != QuestionKind::SmallTalk;
                            if go {
                                session.mark_answered(count);
                            }
                            go
                        };
                        if go {
                            start_suggestion(&app, Some(latest), Some(a));
                        }
                    }
                    Err(e) => {
                        s.session.lock().unwrap().error = e.0;
                        emit(&app);
                    }
                }
            });
        }
    }
}

#[tauri::command]
pub fn snapshot(app: AppHandle) -> Snapshot {
    state(&app).session.lock().unwrap().snapshot()
}

#[tauri::command(async)]
pub fn start_listening(app: AppHandle, call: bool) {
    let them = start_capture(&app, Speaker::Them, call);
    let me = if call { start_capture(&app, Speaker::Me, false) } else { Ok(()) };
    {
        let s = state(&app);
        let mut session = s.session.lock().unwrap();
        if let Err(e) = them {
            session.them.error = e.0;
        }
        if let Err(e) = me {
            session.me.error = e.0;
        }
    }
    emit(&app);
}

#[tauri::command(async)]
pub fn stop_listening(app: AppHandle) {
    stop_capture(&app, Speaker::Them);
    stop_capture(&app, Speaker::Me);
    touch(&app); // stopping commits any partial line
}

#[tauri::command]
pub fn suggest(app: AppHandle, question: Option<String>) {
    let q = question.unwrap_or_default();
    {
        let s = state(&app);
        let mut session = s.session.lock().unwrap();
        if session.busy {
            return;
        }
        if q.trim().is_empty() && session.lines().is_empty() {
            session.error = "Start listening or type a question first.".into();
            drop(session);
            return emit(&app);
        }
    }
    start_suggestion(&app, (!q.is_empty()).then_some(q), None);
}

#[tauri::command]
pub fn cancel(app: AppHandle) {
    let s = state(&app);
    if let Some(task) = s.pending.lock().unwrap().take() {
        task.abort();
    }
    {
        let mut session = s.session.lock().unwrap();
        session.run += 1; // drops a result that lands between abort and its next await
        session.busy = false;
    }
    emit(&app);
}

#[tauri::command(async)]
pub fn clear(app: AppHandle) {
    let s = state(&app);
    if let Some(task) = s.pending.lock().unwrap().take() {
        task.abort();
    }
    stop_capture(&app, Speaker::Them);
    stop_capture(&app, Speaker::Me);
    save_now(&app);
    s.session.lock().unwrap().reset();
    emit(&app);
}

#[tauri::command]
pub fn export(app: AppHandle, path: String) -> Result<()> {
    save_now(&app);
    let s = state(&app);
    let role = s.store.load_settings().role;
    let markdown = s.session.lock().unwrap().to_interview(&role).markdown();
    std::fs::write(path, markdown)?;
    Ok(())
}

#[tauri::command]
pub fn set_notes(app: AppHandle, text: String) {
    state(&app).session.lock().unwrap().notes = text;
    touch(&app);
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    state(&app).store.load_settings()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<()> {
    state(&app).store.save_settings(&settings)
}

#[tauri::command]
pub fn read_key(app: AppHandle, account: String) -> Result<String> {
    let label = LLMProvider::ALL.into_iter().find(|p| p.account() == account).map_or("TypeSafe", |p| p.label());
    state(&app).store.read_key(&account, label)
}

#[tauri::command]
pub fn save_key(app: AppHandle, account: String, value: String) -> Result<()> {
    state(&app).store.save_key(&value, &account)
}

#[tauri::command]
pub async fn test_provider(app: AppHandle, provider: LLMProvider, model: String, local_base_url: String, key: String) -> Result<String> {
    let cfg = LLMConfiguration { provider, model, local_base_url };
    ai::generate(&state(&app).client, &cfg, &key, "Reply briefly.", "Reply with OK to confirm this connection works.").await?;
    Ok(format!("Connected to {} using {}.", provider.label(), cfg.model))
}

#[tauri::command]
pub async fn test_typesafe(app: AppHandle, key: String) -> Result<()> {
    typesafe::assess(&state(&app).client, &key, "", "", "Tell me about yourself.", &[], false).await.map(|_| ())
}

#[tauri::command]
pub async fn import_resume(path: String) -> Result<ImportedResume> {
    resume::extract(Path::new(&path))
}

#[tauri::command]
pub fn list_interviews(app: AppHandle) -> Vec<Interview> {
    state(&app).store.load_interviews()
}

#[tauri::command]
pub fn delete_interview(app: AppHandle, id: String) -> Result<()> {
    let id = uuid::Uuid::parse_str(&id).map_err(|e| CueError(e.to_string()))?;
    state(&app).store.delete_interview(id)
}

#[tauri::command]
pub async fn download_whisper_model(app: AppHandle, name: String) -> Result<()> {
    if !stt::model::MODELS.iter().any(|m| m.name == name) {
        return err(format!("Unknown speech model \"{name}\"."));
    }
    let s = state(&app);
    {
        // One download at a time; a repeat call is a no-op.
        let mut session = s.session.lock().unwrap();
        if session.download.is_some() {
            return Ok(());
        }
        session.download = Some(0.0);
    }
    emit(&app);
    let set = |download: Option<f32>, error: Option<String>| {
        let mut session = s.session.lock().unwrap();
        session.download = download;
        if let Some(e) = error {
            session.error = e;
        }
        drop(session);
        emit(&app);
    };
    let result = stt::model::download(&s.store.dir, &name, |p| set(Some(p), None)).await;
    match result {
        Ok(_) => {
            set(None, None);
            Ok(())
        }
        Err(e) => {
            set(None, Some(e.0.clone()));
            Err(e)
        }
    }
}

#[tauri::command]
pub fn toggle_floating(app: AppHandle) {
    let Some(w) = app.get_webview_window("floating") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
    } else {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
pub fn show_main(app: AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_focus();
}

pub fn tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "floating", "Floating assistant", true, None::<&str>)?,
            &MenuItem::with_id(app, "main", "Show workspace", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit Cue", true, None::<&str>)?,
        ],
    )?;
    let mut tray = TrayIconBuilder::new().menu(&menu).on_menu_event(|app, e| match e.id().as_ref() {
        "floating" => toggle_floating(app.clone()),
        "main" => show_main(app.clone()),
        "quit" => app.exit(0),
        _ => {}
    });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Closing a window hides it; the app keeps running in the tray.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if matches!(window.label(), "main" | "floating") {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}
