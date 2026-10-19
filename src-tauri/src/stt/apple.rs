//! Apple Speech recognizer: ports `AudioCapture.start/recognize/waitForPause/finishUtterance/stop`.
use super::SttEvent;
use crate::error::{err, Result};
use crate::store::Settings;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::AllocAnyThread;
use objc2_avf_audio::{AVAudioFormat, AVAudioPCMBuffer};
use objc2_foundation::{NSError, NSLocale, NSOperationQueue, NSString};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognitionTask, SFSpeechRecognizer,
    SFSpeechRecognizerAuthorizationStatus,
};
use std::path::Path;
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const PAUSE: Duration = Duration::from_millis(900); // calibration knob: silence that ends an utterance
const ROLLOVER: Duration = Duration::from_secs(50); // requests are limited to about a minute

type OnEvent = Arc<dyn Fn(SttEvent) + Send + Sync>;

pub struct Recognizer {
    shared: Arc<Shared>,
    timers: Option<JoinHandle<()>>,
}

struct Shared {
    on_event: OnEvent,
    recognizer: Retained<SFSpeechRecognizer>,
    format: Retained<AVAudioFormat>,
    state: Mutex<State>,
    wake: Condvar,
}

// The Speech objects are used only under `state`'s lock (or are immutable after `start`), as the Swift app did with `RecognitionFeed`.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

struct State {
    running: bool,
    generation: u64, // drops callbacks from replaced requests
    request: Option<Retained<SFSpeechAudioBufferRecognitionRequest>>,
    task: Option<Retained<SFSpeechRecognitionTask>>,
    partial: String,
    pause_at: Option<Instant>,
    rollover_at: Instant,
}

fn authorized() -> bool {
    let (tx, rx) = mpsc::channel();
    let done = RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
        let _ = tx.send(status == SFSpeechRecognizerAuthorizationStatus::Authorized);
    });
    unsafe { SFSpeechRecognizer::requestAuthorization(&done) };
    rx.recv().unwrap_or(false)
}

impl Recognizer {
    pub fn start(on_event: OnEvent, _settings: &Settings, _data_dir: &Path) -> Result<Recognizer> {
        if !authorized() {
            return err("Allow Speech Recognition for Cue in System Settings → Privacy & Security.");
        }
        let locale = NSLocale::initWithLocaleIdentifier(NSLocale::alloc(), &NSString::from_str("en-US"));
        let recognizer = unsafe { SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale) };
        let Some(recognizer) = recognizer.filter(|r| unsafe { r.isAvailable() }) else {
            return err("English speech recognition is unavailable. Check your connection and macOS speech settings.");
        };
        // Results otherwise arrive on the main queue, which Tauri's event loop owns.
        unsafe { recognizer.setQueue(&NSOperationQueue::new()) };
        let format = unsafe { AVAudioFormat::initStandardFormatWithSampleRate_channels(AVAudioFormat::alloc(), 16_000.0, 1) }
            .expect("16 kHz mono is a valid format");
        let shared = Arc::new(Shared {
            on_event,
            recognizer,
            format,
            state: Mutex::new(State {
                running: true,
                generation: 0,
                request: None,
                task: None,
                partial: String::new(),
                pause_at: None,
                rollover_at: Instant::now(),
            }),
            wake: Condvar::new(),
        });
        shared.recognize(&mut shared.state.lock().unwrap());
        let timers = shared.clone();
        let timers = std::thread::spawn(move || timers.run_timers());
        Ok(Recognizer { shared, timers: Some(timers) })
    }

    /// Appends 16 kHz mono audio to the current request.
    pub fn push(&self, pcm: &[f32]) {
        let Some(buffer) = (unsafe { AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(AVAudioPCMBuffer::alloc(), &self.shared.format, pcm.len() as u32) }) else {
            return;
        };
        unsafe {
            buffer.setFrameLength(pcm.len() as u32);
            std::ptr::copy_nonoverlapping(pcm.as_ptr(), (*buffer.floatChannelData()).as_ptr(), pcm.len());
        }
        if let Some(request) = &self.shared.state.lock().unwrap().request {
            unsafe { request.appendAudioPCMBuffer(&buffer) };
        }
    }

    pub fn stop(self) {} // Drop does the work
}

impl Drop for Recognizer {
    fn drop(&mut self) {
        {
            let mut st = self.shared.state.lock().unwrap();
            st.running = false;
            st.generation += 1;
            if let Some(r) = st.request.take() {
                unsafe { r.endAudio() };
            }
            if let Some(t) = st.task.take() {
                unsafe { t.cancel() };
            }
        }
        self.shared.wake.notify_all();
        if let Some(t) = self.timers.take() {
            // An event handler could drop us from the timer thread itself; joining there would deadlock.
            if t.thread().id() != std::thread::current().id() {
                let _ = t.join();
            }
        }
    }
}

impl Shared {
    /// Starts a fresh request; audio keeps flowing into whichever request is current.
    fn recognize(self: &Arc<Self>, st: &mut State) {
        st.generation += 1;
        let generation = st.generation;
        let request = unsafe { SFSpeechAudioBufferRecognitionRequest::new() };
        unsafe {
            request.setShouldReportPartialResults(true);
            request.setAddsPunctuation(true);
            if self.recognizer.supportsOnDeviceRecognition() {
                request.setRequiresOnDeviceRecognition(true);
            }
        }
        if let Some(old) = st.request.replace(request.clone()) {
            unsafe { old.endAudio() };
        }
        if let Some(old) = st.task.take() {
            unsafe { old.cancel() };
        }
        let weak = Arc::downgrade(self);
        let handler = RcBlock::new(move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
            on_result(&weak, generation, unsafe { result.as_ref() }, unsafe { error.as_ref() });
        });
        st.task = Some(unsafe { self.recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler) });
        st.pause_at = None;
        st.rollover_at = Instant::now() + ROLLOVER;
    }

    /// Commits what was heard as one utterance and starts listening for the next.
    fn finish_utterance(self: &Arc<Self>, st: &mut State) -> Option<SttEvent> {
        let text = std::mem::take(&mut st.partial).trim().to_string();
        self.recognize(st);
        (!text.is_empty()).then_some(SttEvent::Final(text))
    }

    /// Fires the pause and rollover timers.
    fn run_timers(self: Arc<Self>) {
        let mut st = self.state.lock().unwrap();
        while st.running {
            let due = st.pause_at.map_or(st.rollover_at, |p| p.min(st.rollover_at));
            let now = Instant::now();
            if now < due {
                st = self.wake.wait_timeout(st, due - now).unwrap().0;
                continue;
            }
            let event = self.finish_utterance(&mut st);
            st = self.emit(st, event);
        }
    }

    /// Sends `event` without holding the lock, since the handler takes the app's session lock.
    fn emit<'a>(&'a self, st: MutexGuard<'a, State>, event: Option<SttEvent>) -> MutexGuard<'a, State> {
        let Some(event) = event else { return st };
        drop(st);
        (self.on_event)(event);
        self.state.lock().unwrap()
    }
}

fn on_result(shared: &Weak<Shared>, generation: u64, result: Option<&SFSpeechRecognitionResult>, error: Option<&NSError>) {
    let Some(shared) = shared.upgrade() else { return };
    let mut st = shared.state.lock().unwrap();
    if !st.running || st.generation != generation {
        return;
    }
    if let Some(result) = result {
        let text = unsafe { result.bestTranscription().formattedString() }.to_string();
        let event = if !text.is_empty() {
            st.partial = text.clone();
            st.pause_at = Some(Instant::now() + PAUSE);
            shared.wake.notify_all();
            Some(SttEvent::Partial(text))
        } else {
            None
        };
        st = shared.emit(st, event);
        if unsafe { result.isFinal() } && st.running && st.generation == generation {
            let event = shared.finish_utterance(&mut st);
            drop(shared.emit(st, event));
        }
    } else if let Some(error) = error {
        st.generation += 1; // only one error per request
        drop(st);
        (shared.on_event)(SttEvent::Error(error.localizedDescription().to_string()));
    }
}
