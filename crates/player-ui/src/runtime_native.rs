use super::*;
use session_engine::{audio_queue::AudioSink, realtime::Runtime};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
pub struct Worker {
    input: Arc<Mutex<Input>>,
    audio: Arc<Mutex<Option<AudioSink>>>,
    latest: Arc<Mutex<Option<Output>>>,
    stop: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<Stopped, String>>,
    pub lifecycle: Lifecycle,
    thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn new(
        rom: Vec<u8>,
        state: Vec<u8>,
        setup: Setup,
        audio: Option<AudioSink>,
        ctx: egui::Context,
    ) -> Result<Self, String> {
        let audio = Arc::new(Mutex::new(audio));
        let audio_worker = audio.clone();
        let input = Arc::new(Mutex::new(Input {
            aim: [-1, -1],
            ..Default::default()
        }));
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (i, l, s) = (input.clone(), latest.clone(), stop.clone());
        let handle = thread::Builder::new()
            .name("nes-session".into())
            .spawn(move || {
                let result = (|| {
                    let mut runtime = Runtime::new(rom, &state, setup)?;
                    let clock = Instant::now();
                    while !s.load(Ordering::Acquire) {
                        if let Ok(mut input) = i.lock() {
                            runtime.input(*input);
                            input.pulse = [0; 2];
                            input.fire_frames = 0;
                        }
                        let audio = audio_worker.lock().ok().and_then(|a| a.clone());
                        if let Some(mut frame) = runtime.update(
                            clock.elapsed().as_secs_f64(),
                            audio.as_ref().map(|a| a.queued()),
                        ) {
                            if let Some(a) = &audio {
                                a.push(&frame.audio);
                            }
                            frame.audio.clear();
                            let finished = frame.view.finished;
                            if let Ok(mut latest) = l.lock() {
                                *latest = Some(frame);
                            }
                            ctx.request_repaint();
                            if finished {
                                break;
                            }
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                    runtime.pause();
                    Ok(Stopped {
                        state: runtime.snapshot(),
                        view: runtime.view(),
                        pixels: runtime.session.machine.ppu().framebuffer().to_vec(),
                        battery: runtime.session.machine.battery_ram().map(|b| b.to_vec()),
                    })
                })();
                let _ = tx.send(result);
                ctx.request_repaint();
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            input,
            audio,
            latest,
            stop,
            rx,
            lifecycle: Lifecycle::Starting,
            thread: Some(handle),
        })
    }
    pub fn refresh_audio(&self, audio: Option<AudioSink>) {
        if let Ok(mut a) = self.audio.lock() {
            *a = audio;
        }
    }
    pub fn input(&self, next: Input) {
        if let Ok(mut i) = self.input.lock() {
            let pulse = [i.pulse[0] | next.pulse[0], i.pulse[1] | next.pulse[1]];
            let fire = i.fire_frames.max(next.fire_frames);
            *i = next;
            i.pulse = pulse;
            i.fire_frames = fire;
        }
    }
    pub fn take_frame(&mut self) -> Option<Output> {
        let frame = self.latest.lock().ok()?.take();
        if frame.is_some() && self.lifecycle == Lifecycle::Starting {
            self.lifecycle = Lifecycle::Running;
        }
        frame
    }
    pub fn stop(&mut self) {
        self.lifecycle = Lifecycle::Pausing;
        self.stop.store(true, Ordering::Release);
    }
    pub fn finish(&mut self) -> Option<Result<Stopped, String>> {
        self.stop();
        if let Some(handle) = self.thread.take()
            && handle.join().is_err()
        {
            return Some(Err("Emulation worker panicked".into()));
        }
        self.result()
    }
    pub fn result(&mut self) -> Option<Result<Stopped, String>> {
        match self.rx.try_recv() {
            Ok(v) => Some(v),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err("Emulation worker disconnected".into())),
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}
