//! System audio via ScreenCaptureKit, and the microphone permission prompt.
use super::{Capture, OnError, Sink};
use crate::error::{CueError, Result};
use block2::RcBlock;
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeAudio};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{
    SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamDelegate, SCStreamOutput, SCStreamOutputType,
};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;

/// Moves an Objective-C object across a channel; the frameworks hand these out on their own queues.
struct Handoff<T>(T);
unsafe impl<T> Send for Handoff<T> {}

fn describe(e: &NSError) -> String {
    e.localizedDescription().to_string()
}

/// Blocks until the user answers the Microphone prompt (or immediately, once answered).
pub fn microphone_allowed() -> bool {
    let (tx, rx) = mpsc::channel();
    let done = RcBlock::new(move |granted: Bool| {
        let _ = tx.send(granted.as_bool());
    });
    let Some(audio) = (unsafe { AVMediaTypeAudio }) else { return false };
    unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &done) };
    rx.recv().unwrap_or(false)
}

pub struct Ivars {
    sink: Mutex<Sink>,
    on_error: OnError,
    stopped: AtomicBool,
}

define_class!(
    // Receives the stream's audio and its unexpected stop.
    #[unsafe(super(NSObject))]
    #[name = "CueSystemAudioOutput"]
    #[ivars = Ivars]
    struct Output;

    unsafe impl NSObjectProtocol for Output {}

    unsafe impl SCStreamOutput for Output {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn did_output(&self, _stream: &SCStream, sample: &CMSampleBuffer, kind: SCStreamOutputType) {
            if kind != SCStreamOutputType::Audio || !unsafe { sample.is_valid() } {
                return;
            }
            let Some(data) = (unsafe { sample.data_buffer() }) else { return };
            // Non-interleaved f32: the first `frames` floats are channel 0 (the stream asks for mono anyway).
            let frames = (unsafe { sample.num_samples() }).max(0) as usize;
            let bytes = (frames * 4).min(unsafe { data.data_length() });
            let mut pcm = vec![0f32; bytes / 4];
            if pcm.is_empty() || unsafe { data.copy_data_bytes(0, pcm.len() * 4, NonNull::new(pcm.as_mut_ptr().cast()).unwrap()) } != 0 {
                return;
            }
            (self.ivars().sink.lock().unwrap())(&pcm);
        }
    }

    unsafe impl SCStreamDelegate for Output {
        #[unsafe(method(stream:didStopWithError:))]
        fn did_stop(&self, _stream: &SCStream, error: &NSError) {
            if !self.ivars().stopped.swap(true, Ordering::SeqCst) {
                (self.ivars().on_error)(format!("Mac audio capture stopped: {}", describe(error)));
            }
        }
    }
);

/// A started stream; dropping it stops capture.
struct Running {
    stream: Retained<SCStream>,
    output: Retained<Output>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.output.ivars().stopped.store(true, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move |_: *mut NSError| {
            let _ = tx.send(());
        });
        unsafe { self.stream.stopCaptureWithCompletionHandler(Some(&done)) };
        let _ = rx.recv_timeout(Duration::from_secs(2)); // don't hang shutdown on a wedged stream
    }
}

/// Ports `AudioCapture.begin(system: true)`: captures all computer audio except Cue's own.
pub fn start(sink: Sink, on_error: OnError) -> Result<Capture> {
    Capture::hold(move || {
        let display = first_display()?;
        let config = unsafe {
            let c = SCStreamConfiguration::new();
            c.setCapturesAudio(true);
            c.setExcludesCurrentProcessAudio(true);
            c.setSampleRate(16_000);
            c.setChannelCount(1);
            c.setWidth(2);
            c.setHeight(2);
            c.setMinimumFrameInterval(CMTime::new(1, 1));
            c
        };
        let filter = unsafe { SCContentFilter::initWithDisplay_excludingWindows(SCContentFilter::alloc(), &display.0, &NSArray::new()) };
        let output = Output::alloc().set_ivars(Ivars { sink: Mutex::new(sink), on_error, stopped: AtomicBool::new(false) });
        let output: Retained<Output> = unsafe { msg_send![super(output), init] };
        let stream = unsafe {
            SCStream::initWithFilter_configuration_delegate(SCStream::alloc(), &filter, &config, Some(ProtocolObject::from_ref(&*output)))
        };
        let queue = DispatchQueue::new("cue.audio.them", None);
        unsafe { stream.addStreamOutput_type_sampleHandlerQueue_error(ProtocolObject::from_ref(&*output), SCStreamOutputType::Audio, Some(&queue)) }
            .map_err(|e| CueError(describe(&e)))?;
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move |e: *mut NSError| {
            let _ = tx.send(unsafe { e.as_ref() }.map(describe));
        });
        unsafe { stream.startCaptureWithCompletionHandler(Some(&done)) };
        if let Some(e) = rx.recv().unwrap_or_else(|_| Some("Mac audio capture didn't start.".into())) {
            return Err(CueError(e));
        }
        Ok(Running { stream, output })
    })
}

fn first_display() -> Result<Handoff<Retained<objc2_screen_capture_kit::SCDisplay>>> {
    let (tx, rx) = mpsc::channel();
    let done = RcBlock::new(move |content: *mut SCShareableContent, e: *mut NSError| {
        let found = match (unsafe { content.as_ref() }, unsafe { e.as_ref() }) {
            (Some(c), _) => unsafe { c.displays() }.firstObject().map(Handoff).ok_or_else(|| "No display is available for Mac audio capture.".to_string()),
            (None, Some(e)) => Err(describe(e)),
            (None, None) => Err("No display is available for Mac audio capture.".to_string()),
        };
        let _ = tx.send(found);
    });
    unsafe { SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(false, true, &done) };
    rx.recv().unwrap_or_else(|_| Err("No display is available for Mac audio capture.".into())).map_err(CueError)
}
