use crate::{
    input::{Device, Preferences, TouchLayout, TouchPad},
    metadata,
    platform::{self, Event, FileKind, Message},
    session::{Mode, Model, Replay, Session},
    theme::{self, *},
};
use egui::*;
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};
#[path = "dialogs.rs"]
mod dialogs;
#[path = "gallery.rs"]
mod gallery;
#[path = "saved_games.rs"]
mod saved_games;
#[path = "view.rs"]
mod view;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Dialog {
    Menu,
    GallerySetup,
    Player(usize),
    Agents(usize),
    Controls(usize),
    Settings,
    Saves,
    Touch,
    Help,
    Workshop,
    Confirm,
    Error,
}
enum Replacement {
    SaveSlot(usize),
    Game(Box<Session>),
    State(Vec<u8>),
    Replay(Box<Replay>, Box<nes_core::machine::Machine>),
    Restart,
    Counter(bool),
}
struct Playback {
    replay: Replay,
    index: usize,
}
#[derive(Clone, Copy)]
enum Operation {
    File,
    Download,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Recovery {
    InputDevice(usize),
}

#[derive(Default)]
struct HeldInputs {
    blocked_keys: HashSet<Key>,
    blocked_pads: HashMap<usize, u8>,
    connected: HashSet<usize>,
}

type PausedAction = Box<dyn FnOnce(&mut PlayerApp, &Context)>;
pub struct PlayerApp {
    session: Option<Session>,
    accessibility: crate::accessibility::SharedBridge,
    ui_context: Context,
    gallery: gallery::Gallery,
    prefs: Preferences,
    host: platform::Platform,
    texture: Option<TextureHandle>,
    worker: Option<crate::runtime::Worker>,
    presentation: session_engine::realtime::View,
    runtime_battery: Option<Vec<u8>>,
    after_pause: Option<PausedAction>,
    dialog: Option<Dialog>,
    replacement: Option<Replacement>,
    notice: String,
    error: String,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    requests: crate::operation::RequestGate<Operation>,
    touch: TouchPad,
    touch_layout: Option<TouchLayout>,
    touch_mask: u8,
    touch_port: usize,
    menu_port: usize,
    extra_port: bool,
    last_size: Vec2,
    focused: bool,
    held_inputs: HeldInputs,
    game_rect: Rect,
    game_focus: Option<Id>,
    mouse_touch: bool,
    zapper_aim: [i16; 2],
    zapper_frames: u8,
    quick_save: Option<Vec<u8>>,
    saves: [Option<crate::saves::LocalSave>; 3],
    save_textures: HashMap<usize, TextureHandle>,
    back_stack: Vec<Dialog>,
    error_file: Option<FileKind>,
    recovery: Option<Recovery>,
    show_trace: bool,
    playback: Option<Playback>,
    replay_status: String,
    desired_mode: [Mode; 2],
    queued_model: Option<(usize, Model)>,
    search: String,
    report: Option<serde_json::Value>,
    remap: Option<(usize, usize)>,
    last_battery: f64,
    advanced: bool,
    exit_frames: Option<u64>,
    exit_start: u64,
    #[cfg(not(target_arch = "wasm32"))]
    screenshot: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    screenshot_requested: bool,
    ui_frames: u64,
}
impl PlayerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rom: Option<(String, Vec<u8>)>) -> Self {
        theme::setup(&cc.egui_ctx);
        let accessibility = std::sync::Arc::new(std::sync::Mutex::new(
            crate::accessibility::Bridge::default(),
        ));
        cc.egui_ctx
            .add_plugin(crate::accessibility::Plugin(accessibility.clone()));
        let host = platform::Platform::default();
        let mut prefs: Preferences = cc
            .storage
            .and_then(|s| eframe::get_value(s, "nes-rust-ui-v1"))
            .unwrap_or_default();
        if cc
            .storage
            .and_then(|s| s.get_string("nes-rust-ui-v1"))
            .is_none()
        {
            platform::migrate_keys(&mut prefs);
        }
        if host.touch_device()
            && cc
                .storage
                .and_then(|s| s.get_string("nes-rust-ui-v1"))
                .is_none()
        {
            prefs.touch = true;
            prefs.devices[0] = Device::Touch;
        }
        cc.egui_ctx.set_zoom_factor(prefs.ui_scale);
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            session: None,
            accessibility,
            ui_context: cc.egui_ctx.clone(),
            gallery: gallery::Gallery::default(),
            prefs,
            host,
            texture: None,
            worker: None,
            presentation: Default::default(),
            runtime_battery: None,
            after_pause: None,
            dialog: None,
            replacement: None,
            notice: "Game files stay on this device.".into(),
            error: String::new(),
            tx,
            rx,
            requests: Default::default(),
            touch: TouchPad::default(),
            touch_layout: None,
            touch_mask: 0,
            touch_port: 0,
            menu_port: 0,
            extra_port: false,
            last_size: Vec2::ZERO,
            focused: false,
            held_inputs: HeldInputs::default(),
            game_rect: Rect::NOTHING,
            game_focus: None,
            mouse_touch: false,
            zapper_aim: [-1, -1],
            zapper_frames: 0,
            quick_save: None,
            saves: Default::default(),
            save_textures: Default::default(),
            back_stack: Vec::new(),
            error_file: None,
            recovery: None,
            show_trace: false,
            playback: None,
            replay_status: String::new(),
            desired_mode: [Mode::Ai; 2],
            queued_model: None,
            search: String::new(),
            report: None,
            remap: None,
            last_battery: 0.,
            advanced: false,
            exit_frames: None,
            exit_start: 0,
            #[cfg(not(target_arch = "wasm32"))]
            screenshot: None,
            #[cfg(not(target_arch = "wasm32"))]
            screenshot_requested: false,
            ui_frames: 0,
        };
        app.touch_port = app
            .prefs
            .devices
            .iter()
            .position(|d| *d == Device::Touch)
            .unwrap_or(0);
        let (view, slot) = platform::launch_query();
        app.gallery.open = view == "ai-games";
        app.dialog = match view.as_str() {
            "agents" => Some(Dialog::Agents(slot)),
            "workshop" => Some(Dialog::Workshop),
            _ => None,
        };
        if let Some((name, bytes)) = rom {
            match app.prepare_game(&name, bytes) {
                Ok(s) => app.install(s, &cc.egui_ctx),
                Err(e) => app.fail(e, &cc.egui_ctx),
            }
        }
        app
    }
    fn has_bundled_agent(&self) -> bool {
        self.session.as_ref().is_some_and(|s| {
            metadata::builtins().iter().any(|b| {
                serde_json::from_str::<serde_json::Value>(b.manifest)
                    .ok()
                    .is_some_and(|m| m["rom_sha256"].as_str() == Some(&s.machine.cartridge_hash()))
            })
        })
    }
    fn error_title(&self) -> &'static str {
        match self.error_file {
            Some(FileKind::Rom) => "Couldn’t open ROM",
            Some(_) => "Couldn’t load file",
            None if self
                .session
                .as_ref()
                .is_some_and(|s| s.modes.iter().any(|m| matches!(m, Mode::Ai | Mode::Shared))) =>
            {
                "Your control is needed"
            }
            None => "Game paused",
        }
    }
    fn visible_players(&self) -> usize {
        if self.extra_port {
            2
        } else {
            self.session.as_ref().map_or(1, |s| {
                if s.modes[1] == Mode::Off {
                    1
                } else {
                    s.info.players
                }
            })
        }
    }
    fn transitioning(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|w| w.lifecycle != crate::runtime::Lifecycle::Running)
    }
    fn defer_pause(
        &mut self,
        ctx: &Context,
        action: impl FnOnce(&mut Self, &Context) + 'static,
    ) -> bool {
        if self.worker.is_none() {
            return false;
        }
        self.after_pause = Some(Box::new(action));
        self.pause(ctx);
        true
    }
    fn current_frame(&self) -> u64 {
        self.session.as_ref().map_or(0, |s| {
            if self.worker.is_some() {
                self.presentation.frame
            } else {
                s.machine.ppu().completed_frames()
            }
        })
    }
    fn quick_capture(&mut self, ctx: &Context) {
        if self.defer_pause(ctx, |app, ctx| app.quick_capture(ctx)) {
            return;
        }
        self.pause(ctx);
        self.store_save(0, ctx);
    }
    fn poll_runtime(&mut self, ctx: &Context) {
        let (frame, stopped) = if let Some(worker) = &mut self.worker {
            (worker.take_frame(), worker.result())
        } else {
            return;
        };
        if let Some(frame) = frame {
            self.presentation = frame.view;
            if let Some(s) = &mut self.session {
                s.decisions = self.presentation.decisions.clone();
            }
            if let Some(p) = &mut self.playback
                && let Some(index) = self.presentation.replay_index
            {
                p.index = index;
            }
            self.runtime_battery = frame.battery;
            self.upload_pixels(ctx, &frame.pixels);
        }
        if let Some(result) = stopped {
            self.worker = None;
            self.host.audio_stop();
            if let Some(s) = &mut self.session {
                s.pause();
            }
            match result {
                Ok(stopped) => {
                    let mut error = stopped.view.error.clone();
                    if !stopped.state.is_empty() {
                        if let Some(s) = &mut self.session {
                            if let Err(e) = s.adopt_transport(&stopped.state, error.as_deref()) {
                                error = Some(e);
                            }
                            s.decisions = stopped.view.decisions.clone();
                        }
                        self.presentation = stopped.view;
                        self.runtime_battery = stopped.battery;
                        self.upload_pixels(ctx, &stopped.pixels);
                        if let Some(p) = &mut self.playback {
                            if let Some(index) = self.presentation.replay_index {
                                p.index = index;
                            }
                            if p.index == p.replay.inputs.len() && error.is_none() {
                                self.replay_status =
                                    "Replay complete · final state verified".into();
                                self.dialog = Some(Dialog::Saves);
                            }
                        }
                    }
                    self.clear(ctx);
                    if let Some(e) = error {
                        self.after_pause = None;
                        self.fail(e, ctx);
                    } else if let Some(action) = self.after_pause.take() {
                        action(self, ctx);
                    }
                    if let Err(e) = self.battery() {
                        self.notice = e;
                    }
                }
                Err(e) => {
                    self.after_pause = None;
                    self.fail(e, ctx);
                }
            }
        }
    }

    fn running(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.running())
    }
    fn prepare_game(&self, name: &str, bytes: Vec<u8>) -> Result<Session, String> {
        let mut s = Session::new(name, bytes)?;
        self.gallery.validate_file(&s.machine.cartridge_hash())?;
        if s.machine.battery_ram().is_some()
            && let Some(bytes) = platform::battery_read(&s.machine.cartridge_hash())?
        {
            s.machine
                .restore_battery_ram(&bytes)
                .map_err(|e| format!("Battery save could not be restored: {e}"))?;
        }
        if let Some((slot, model)) = &self.queued_model {
            s.load_model(*slot, model.clone(), self.desired_mode[*slot])?;
        }
        Ok(s)
    }
    fn battery(&self) -> Result<(), String> {
        if let Some(s) = &self.session
            && let Some(b) = if self.worker.is_some() {
                self.runtime_battery
                    .as_deref()
                    .or_else(|| s.machine.battery_ram())
            } else {
                s.machine.battery_ram()
            }
        {
            platform::battery_write(&s.machine.cartridge_hash(), b)?;
        }
        Ok(())
    }
    fn install(&mut self, s: Session, ctx: &Context) {
        if let Err(e) = self.battery() {
            self.fail(e, ctx);
            return;
        }
        self.clear(ctx);
        self.presentation = Default::default();
        self.runtime_battery = None;
        self.session = Some(s);
        self.error.clear();
        self.extra_port = false;
        self.queued_model = None;
        self.quick_save = None;
        self.back_stack.clear();
        self.playback = None;
        self.replay_status.clear();
        self.dialog = if self.gallery.pending_rom.take().is_some() {
            Some(Dialog::GallerySetup)
        } else {
            self.gallery.open = false;
            None
        };
        self.notice = if self.session.as_ref().is_some_and(|s| s.has_run) {
            String::new()
        } else {
            "Game ready. Press Play, then use NES Start to begin.".into()
        };
        self.load_saves(ctx);
        self.upload_frame(ctx);
        if self.dialog == Some(Dialog::GallerySetup) {
            self.finish_gallery_setup(ctx);
        }
    }
    fn clear(&mut self, ctx: &Context) {
        self.touch.clear();
        self.touch_mask = 0;
        self.mouse_touch = false;
        self.zapper_frames = 0;
        self.zapper_aim = [-1, -1];
        self.host.audio_clear();
        self.held_inputs
            .blocked_keys
            .extend(ctx.input(|i| i.keys_down.clone()));
        for (id, _) in self.host.gamepads() {
            self.held_inputs
                .blocked_pads
                .insert(id, self.host.pad_buttons(id));
        }
        if let Some(s) = &mut self.session {
            s.release_inputs();
            s.machine.set_zapper(-1, -1, false);
        }
    }
    fn pause(&mut self, ctx: &Context) {
        if let Some(worker) = &mut self.worker {
            worker.stop();
            self.host.audio_stop();
        }
        if let Some(s) = &mut self.session {
            s.pause()
        }
        self.clear(ctx);
        if let Err(e) = self.battery() {
            self.notice = e;
        }
    }
    fn open(&mut self, dialog: Dialog, ctx: &Context) {
        if self.defer_pause(ctx, move |app, ctx| app.open(dialog, ctx)) {
            return;
        }
        self.pause(ctx);
        self.remap = None;
        if let Some(previous) = self.dialog
            && previous != dialog
        {
            self.back_stack.push(previous);
        }
        self.notice.clear();
        self.dialog = Some(dialog);
    }
    fn back(&mut self, ctx: &Context) {
        self.clear(ctx);
        self.remap = None;
        self.notice.clear();
        self.dialog = self.back_stack.pop().or(Some(Dialog::Menu));
    }
    fn close_dialog(&mut self, ctx: &Context) {
        self.clear(ctx);
        self.remap = None;
        self.dialog = None;
        self.back_stack.clear();
        self.gallery.pending_rom = None;
        self.replacement = None;
        self.notice.clear();
    }
    fn fail(&mut self, e: impl Into<String>, ctx: &Context) {
        self.pause(ctx);
        self.error = e.into();
        self.error_file = None;
        self.recovery = None;
        self.gallery.pending_rom = None;
        self.dialog = Some(Dialog::Error);
    }
    fn fail_file(&mut self, e: String, kind: FileKind, ctx: &Context) {
        let expected = if matches!(kind, FileKind::Rom) {
            self.gallery.pending_rom.clone()
        } else {
            None
        };
        self.fail(e, ctx);
        self.error_file = Some(kind);
        // Retrying the file picker remains part of this gallery setup. Explicit
        // cancellation or leaving the error dialog clears the restriction.
        self.gallery.pending_rom = expected;
    }
    fn gamepad_disconnected(&mut self, slot: usize, ctx: &Context) {
        self.fail(
            format!(
                "Player {}'s gamepad disconnected. Reconnect it or choose another input device.",
                slot + 1
            ),
            ctx,
        );
        self.recovery = Some(Recovery::InputDevice(slot));
    }
    fn recover_input_device(&mut self, ctx: &Context) {
        if let Some(Recovery::InputDevice(slot)) = self.recovery.take() {
            self.open(Dialog::Player(slot), ctx);
        }
    }
    fn toggle(&mut self, ctx: &Context) {
        if self
            .worker
            .as_ref()
            .is_some_and(|w| w.lifecycle == crate::runtime::Lifecycle::Pausing)
        {
            return;
        }
        if self.running() {
            self.pause(ctx);
        } else if let Some(s) = &mut self.session {
            match if self.playback.is_some() {
                s.resume_playback(true)
            } else {
                s.resume()
            } {
                Ok(()) => {
                    self.error.clear();
                    self.notice.clear();
                    self.dialog = None;
                    self.gallery.open = false;
                    self.gallery.pending_rom = None;
                    ctx.memory_mut(|m| {
                        if let Some(id) = m.focused() {
                            m.surrender_focus(id);
                        }
                    });
                    self.clear(ctx);
                    self.host.start_audio(if self.prefs.muted {
                        0.
                    } else {
                        self.prefs.volume
                    });
                    let s = self.session.as_ref().unwrap();
                    self.presentation.frame = s.machine.ppu().completed_frames();
                    let setup = session_engine::realtime::Setup::from_session(
                        s,
                        self.playback.as_ref().map(|p| (p.replay.clone(), p.index)),
                    );
                    match crate::runtime::Worker::new(
                        s.rom.clone(),
                        s.machine.save_state(),
                        setup,
                        self.host.audio_sink(),
                        ctx.clone(),
                    ) {
                        Ok(worker) => self.worker = Some(worker),
                        Err(e) => self.fail(e, ctx),
                    }
                }
                Err(e) => self.fail(e, ctx),
            }
        }
    }
    fn operation(&mut self, operation: Operation, label: &str) -> u64 {
        self.requests.begin(operation, label)
    }
    fn pick(&mut self, kind: FileKind, ctx: &Context) {
        if self.defer_pause(ctx, move |app, ctx| app.pick(kind, ctx)) {
            return;
        }
        self.pause(ctx);
        self.error_file = Some(kind);
        let id = self.operation(Operation::File, "Opening file…");
        platform::pick(id, kind, self.tx.clone(), ctx.clone());
    }
    fn download(&mut self, name: String, bytes: Vec<u8>, ctx: &Context) {
        self.pause(ctx);
        let id = self.operation(Operation::Download, "Saving file…");
        platform::download(id, name, bytes, self.tx.clone(), ctx.clone());
    }
    fn save_file(&mut self, ctx: &Context) {
        if self.defer_pause(ctx, |app, ctx| app.save_file(ctx)) {
            return;
        }
        if let Some(s) = &self.session {
            match nes_core::state_file::export(&s.machine) {
                Ok(bytes) => self.download(format!("{}.state", s.info.title), bytes, ctx),
                Err(e) => self.fail(e.to_string(), ctx),
            }
        }
    }
    fn cancel_pending(&mut self) {
        if self
            .requests
            .pending()
            .is_some_and(|p| matches!(p.operation, Operation::File))
        {
            self.gallery.pending_rom = None;
        }
        self.requests.cancel();
        self.notice = "Canceled. Your current session is unchanged.".into();
    }
    fn results(&mut self, ctx: &Context) {
        while let Ok((id, event)) = self.rx.try_recv() {
            let Some(_op) = self.requests.complete(id) else {
                continue;
            };
            let result: Result<(), String> = match event {
                Event::Error(e) => Err(e),
                Event::Written(r) => r.map(|()| {
                    self.notice =
                        "Save dialog closed. Keep your downloaded save with the matching game."
                            .into()
                }),
                Event::File(FileKind::Rom, None) => {
                    self.gallery.pending_rom = None;
                    Ok(())
                }
                Event::File(_, None) => Ok(()),
                Event::File(kind, Some((name, bytes))) => self.file(kind, &name, bytes, ctx),
            };
            if let Err(e) = result {
                if let Some(kind) = self.error_file {
                    self.fail_file(e, kind, ctx);
                } else {
                    self.fail(e, ctx);
                }
            }
        }
    }
    fn file(
        &mut self,
        kind: FileKind,
        name: &str,
        bytes: Vec<u8>,
        ctx: &Context,
    ) -> Result<(), String> {
        if self.worker.is_some() {
            let name = name.to_owned();
            self.defer_pause(ctx, move |app, ctx| {
                if let Err(e) = app.file(kind, &name, bytes, ctx) {
                    app.fail_file(e, kind, ctx);
                }
            });
            return Ok(());
        }
        match kind {
            FileKind::Rom => {
                let next = self.prepare_game(name, bytes)?;
                if self.session.is_none() {
                    self.install(next, ctx)
                } else {
                    self.replacement = Some(Replacement::Game(Box::new(next)));
                    self.dialog = Some(Dialog::Confirm)
                }
            }
            FileKind::State => {
                let s = self.session.as_ref().ok_or("Open a game first")?;
                let mut probe = Session::new(&s.info.title, s.rom.clone())?;
                probe.restore(&bytes)?;
                self.replacement = Some(Replacement::State(bytes));
                self.dialog = Some(Dialog::Confirm)
            }
            FileKind::Replay => {
                let s = self.session.as_ref().ok_or("Open a game first")?;
                let (replay, machine) = Replay::prepare(&bytes, s)?;
                self.replacement = Some(Replacement::Replay(Box::new(replay), Box::new(machine)));
                self.dialog = Some(Dialog::Confirm)
            }
            FileKind::Report => {
                self.report = Some(serde_json::from_slice(&bytes).map_err(|e| e.to_string())?);
                self.dialog = Some(Dialog::Workshop)
            }
        }
        Ok(())
    }
    fn apply_model(&mut self, slot: usize, model: Model, ctx: &Context) -> Result<(), String> {
        self.clear(ctx);
        if self.session.is_none() {
            self.notice = format!(
                "{} is ready. Choose the matching game file to play.",
                model.name
            );
            self.queued_model = Some((slot, model));
            self.dialog = None;
            return Ok(());
        }
        self.session
            .as_mut()
            .unwrap()
            .load_model(slot, model, self.desired_mode[slot])?;
        self.dialog = Some(Dialog::Player(slot));
        if self.back_stack.last() == Some(&Dialog::Player(slot)) {
            self.back_stack.pop();
        }
        Ok(())
    }
    fn mode(&mut self, slot: usize, mode: Mode, ctx: &Context) {
        if self.defer_pause(ctx, move |app, ctx| app.mode(slot, mode, ctx)) {
            return;
        }
        self.pause(ctx);
        if matches!(mode, Mode::Ai | Mode::Shared)
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.models[slot].is_none())
        {
            self.desired_mode[slot] = mode;
            self.open(Dialog::Agents(slot), ctx);
            return;
        }
        if let Some(s) = &mut self.session
            && let Err(e) = s.set_mode(slot, mode)
        {
            self.fail(e, ctx)
        }
    }
    fn confirm(&mut self, ctx: &Context) {
        if self.defer_pause(ctx, |app, ctx| app.confirm(ctx)) {
            return;
        }
        let Some(replacement) = self.replacement.take() else {
            return;
        };
        self.clear(ctx);
        let result = match replacement {
            Replacement::SaveSlot(slot) => {
                self.dialog = Some(Dialog::Saves);
                self.store_save(slot, ctx);
                return;
            }
            Replacement::Game(s) => {
                self.install(*s, ctx);
                Ok(())
            }
            Replacement::State(bytes) => {
                let result = self
                    .session
                    .as_mut()
                    .ok_or("No game".into())
                    .and_then(|s| s.restore(&bytes));
                if result.is_ok() {
                    self.playback = None;
                    self.replay_status.clear();
                }
                result
            }
            Replacement::Replay(r, m) => {
                if let Some(s) = &mut self.session {
                    s.machine = *m;
                    s.pause();
                    s.has_run = true;
                }
                self.playback = Some(Playback {
                    replay: *r,
                    index: 0,
                });
                self.replay_status = "Replay ready. Resume to play recorded inputs.".into();
                Ok(())
            }
            Replacement::Counter(alternate) => {
                self.playback = None;
                self.session.as_mut().ok_or("No game".into()).and_then(|s| {
                    s.alternate = alternate;
                    s.reset()
                })
            }
            Replacement::Restart => {
                self.playback = None;
                self.session
                    .as_mut()
                    .ok_or("No game".into())
                    .and_then(|s| s.reset())
            }
        };
        match result {
            Ok(()) => {
                self.error.clear();
                if self.dialog != Some(Dialog::GallerySetup) {
                    self.dialog = None;
                }
                self.upload_frame(ctx);
                if self.playback.is_none()
                    && let Some(s) = &self.session
                {
                    match s.unsupported() {
                        Ok(true) => self.fail("This save is outside the agent's supported stage. Take full control or choose a compatible agent before resuming.",ctx),
                        Err(e) => self.fail(e,ctx),
                        _ => {},
                    }
                }
            }
            Err(e) => self.fail(e, ctx),
        }
    }
    fn upload_frame(&mut self, ctx: &Context) {
        if let Some(s) = &self.session {
            let pixels = s.machine.ppu().framebuffer().to_vec();
            self.upload_pixels(ctx, &pixels);
        }
    }
    fn upload_pixels(&mut self, ctx: &Context, pixels: &[u16]) {
        if pixels.len() != 256 * 240 {
            return;
        }
        let rgba = nes_video::display::rgba(pixels);
        let image = ColorImage::from_rgba_unmultiplied([256, 240], &rgba);
        if let Some(t) = &mut self.texture {
            t.set(image, TextureOptions::NEAREST);
        } else {
            self.texture = Some(ctx.load_texture("NES pixels", image, TextureOptions::NEAREST));
        }
    }
    fn human(&self, ctx: &Context) -> [u8; 2] {
        std::array::from_fn(|slot| match self.prefs.devices[slot] {
            Device::Keyboard
                if ctx.wants_keyboard_input() && ctx.memory(|m| m.focused()) != self.game_focus =>
            {
                0
            }
            Device::Keyboard => ctx.input(|i| {
                self.prefs.keys[slot]
                    .iter()
                    .enumerate()
                    .fold(0, |mask, (n, key)| {
                        if i.key_down(*key) && !self.held_inputs.blocked_keys.contains(key) {
                            mask | (1 << n)
                        } else {
                            mask
                        }
                    })
            }),
            Device::Gamepad(id) => {
                self.host.pad_buttons(id)
                    & !self.held_inputs.blocked_pads.get(&id).copied().unwrap_or(0)
            }
            Device::Touch => {
                if self.touch_port == slot {
                    self.touch_mask
                } else {
                    0
                }
            }
        })
    }
    fn advance(&mut self, ctx: &Context) {
        self.host
            .audio_settings(self.prefs.muted, self.prefs.volume);
        let human = self.human(ctx);
        if let Some(worker) = &self.worker {
            #[cfg(not(target_arch = "wasm32"))]
            worker.refresh_audio(self.host.audio_sink());
            if worker.lifecycle != crate::runtime::Lifecycle::Pausing {
                let pulse = self
                    .session
                    .as_mut()
                    .map_or([0; 2], |s| std::mem::take(&mut s.pulse));
                worker.input(session_engine::realtime::Input {
                    human,
                    pulse,
                    aim: self.zapper_aim,
                    fire_frames: std::mem::take(&mut self.zapper_frames),
                });
            }
        }
        let now = ctx.input(|i| i.time);
        if now - self.last_battery > 10. {
            self.last_battery = now;
            if let Err(e) = self.battery() {
                self.notice = e;
            }
        }
    }
    fn events(&mut self, ctx: &Context) {
        let (focused, size, events) =
            ctx.input(|i| (i.focused, i.content_rect().size(), i.events.clone()));
        if self.focused && !focused && self.exit_frames.is_none() {
            self.pause(ctx)
        }
        self.focused = focused;
        if self.last_size != Vec2::ZERO
            && (size - self.last_size).length() > 80.
            && self.prefs.touch
        {
            self.pause(ctx)
        }
        self.last_size = size;
        self.held_inputs
            .blocked_keys
            .retain(|key| ctx.input(|i| i.key_down(*key)));
        self.host.poll();
        self.held_inputs.blocked_pads.retain(|id, mask| {
            *mask &= self.host.pad_buttons(*id);
            *mask != 0
        });
        let connected: HashSet<usize> = self.host.gamepads().into_iter().map(|(n, _)| n).collect();
        for slot in 0..2 {
            if let Device::Gamepad(id) = self.prefs.devices[slot]
                && self.held_inputs.connected.contains(&id)
                && !connected.contains(&id)
            {
                self.gamepad_disconnected(slot, ctx);
            }
        }
        self.held_inputs.connected = connected;
        if ctx.input(|i| i.key_pressed(Key::Escape)) && self.remap.take().is_none() {
            if self.requests.pending().is_some() {
                self.cancel_pending();
            } else if self.dialog.is_some() {
                self.close_dialog(ctx);
            } else if self.gallery.open {
                self.gallery.open = false;
                self.gallery.pending_rom = None;
            } else {
                self.open(Dialog::Menu, ctx)
            }
        }
        if !self.gallery.open
            && self.remap.is_none()
            && self.dialog.is_none()
            && !ctx.wants_keyboard_input()
        {
            if ctx.input(|i| i.key_pressed(Key::F5)) {
                self.quick_capture(ctx);
            }
            if ctx.input(|i| i.key_pressed(Key::F8))
                && let Some(bytes) = self.quick_save.clone()
            {
                self.pause(ctx);
                self.replacement = Some(Replacement::State(bytes));
                self.dialog = Some(Dialog::Confirm);
            }
            if ctx.input(|i| i.key_pressed(Key::F11)) {
                platform::fullscreen(ctx)
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::O)) {
                self.pick(FileKind::Rom, ctx)
            }
        }
        for event in events {
            match event {
                egui::Event::Touch { id, phase, pos, .. } => {
                    let enabled = self.running() && !self.transitioning() && self.dialog.is_none();
                    if let Some(l) = self.touch_layout {
                        match phase {
                            TouchPhase::Start if enabled => self.touch.begin(id.0, pos, &l),
                            TouchPhase::Move => self.touch.move_to(id.0, pos),
                            TouchPhase::End | TouchPhase::Cancel => self.touch.end(id.0),
                            _ => {}
                        }
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } => {
                    if let Some((slot, button)) = self.remap
                        && key != Key::Escape
                    {
                        if matches!(key, Key::F5 | Key::F8 | Key::F11) {
                            self.notice = "That key is reserved for application controls.".into();
                        } else if self.prefs.keys.iter().enumerate().any(|(p, keys)| {
                            keys.iter()
                                .enumerate()
                                .any(|(b, k)| *k == key && (p, b) != (slot, button))
                        }) {
                            self.notice =
                                "That key already controls another button. Choose a different key."
                                    .into()
                        } else {
                            self.prefs.keys[slot][button] = key;
                            self.remap = None;
                            self.notice = "Keyboard mapping saved.".into();
                            self.clear(ctx);
                        }
                    }
                }
                _ => {}
            }
        }
        self.touch_mask = self.touch_layout.map_or(0, |l| self.touch.buttons(&l));
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            self.pause(ctx);
            if dropped.len() != 1 {
                self.fail("Open one game file at a time.", ctx)
            } else {
                let f = &dropped[0];
                let bytes = f.bytes.as_ref().map(|b| b.to_vec());
                #[cfg(not(target_arch = "wasm32"))]
                let bytes = bytes.or_else(|| f.path.as_ref().and_then(|p| std::fs::read(p).ok()));
                if let Some(bytes) = bytes {
                    let name = if f.name.is_empty() {
                        f.path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Game".into())
                    } else {
                        f.name.clone()
                    };
                    if let Err(e) = self.file(FileKind::Rom, &name, bytes, ctx) {
                        self.fail(e, ctx)
                    }
                }
            }
        }
    }
}
impl eframe::App for PlayerApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_runtime(ctx);
        self.results(ctx);
        self.events(ctx);
        self.advance(ctx);
        self.workspace(ctx);
        // Pointer-operated NES/menu buttons must not steal gameplay keyboard input.
        if self.running() && self.dialog.is_none() && ctx.input(|i| i.pointer.any_released()) {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        self.dialogs(ctx);
        self.capture(ctx);
        if self.running() || self.worker.is_some() {
            ctx.request_repaint_after(Duration::from_millis(4));
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn on_exit(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(mut worker) = self.worker.take()
            && let Some(Ok(stopped)) = worker.finish()
            && let Some(s) = &mut self.session
        {
            let _ = s.adopt_transport(&stopped.state, stopped.view.error.as_deref());
        }
        let _ = self.battery();
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "nes-rust-ui-v1", &self.prefs);
        let _ = self.battery();
    }
    fn auto_save_interval(&self) -> Duration {
        Duration::from_secs(10)
    }
}

impl PlayerApp {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn launch(&mut self, options: crate::LaunchOptions, ctx: &Context) {
        self.gallery.open = options.ai_games;
        self.exit_frames = options.exit_frames;
        self.screenshot = options.screenshot;
        if options.muted {
            self.prefs.muted = true;
        }
        let result = (|| -> Result<(), String> {
            if let Some(s) = &mut self.session {
                if options.alternate {
                    s.machine
                        .set_mmc3_alternate(true)
                        .map_err(|e| e.to_string())?;
                    s.alternate = true;
                }
                if let Some(task) = options.start_task {
                    if task == "contra-jungle" {
                        s.contra_two_player()?;
                    } else {
                        let task =
                            contra_agent::tasks::Task::parse(&task).map_err(|e| e.to_string())?;
                        s.machine = contra_agent::tasks::Episode::new(&s.rom, task, 108000, None)
                            .map_err(|e| e.to_string())?
                            .machine;
                        s.has_run = true;
                    }
                }
                if let Some(bytes) = options.state {
                    s.restore(&bytes)?;
                    s.has_run = true;
                }
                for (slot, bundle) in options.agents.into_iter().enumerate() {
                    if let Some((m, w)) = bundle {
                        s.load_model(slot, Model::new(m, w)?, Mode::Ai)?;
                    }
                }
                self.exit_start = s.machine.ppu().completed_frames();
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.fail(e, ctx)
        } else if options.autoplay || options.exit_frames.is_some() {
            self.toggle(ctx)
        }
        self.upload_frame(ctx);
    }
    pub fn accessibility_json(&self) -> String {
        self.accessibility
            .lock()
            .map(|b| b.json.clone())
            .unwrap_or_default()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn accessibility_focus(&mut self, focused: bool) {
        if let Ok(mut bridge) = self.accessibility.lock() {
            bridge.browser_focus = Some(focused);
        }
        self.ui_context.request_repaint();
    }
    pub fn accessibility_action(&mut self, id: &str, action: &str, value: &str) -> bool {
        let accepted = self
            .accessibility
            .lock()
            .is_ok_and(|mut b| b.action(id, action, value));
        if accepted {
            self.ui_context.request_repaint();
        }
        accepted
    }
    pub fn diagnostics(&self) -> String {
        serde_json::json!({"touch_layout":self.touch_layout.map(|l|[l.dpad,l.a,l.b,l.ab].map(|r|[r.left(),r.top(),r.width(),r.height()])),"save_slots":self.saves.iter().map(|s|s.as_ref().map(|s|s.frame)).collect::<Vec<_>>(),"ui":"rust-egui","gallery":self.gallery.open,"ui_frames":self.ui_frames,"running":self.running(),"dialog":self.dialog.map(|d|match d{Dialog::Menu=>"menu",Dialog::GallerySetup=>"gallery-setup",Dialog::Player(_)=>"player",Dialog::Agents(_)=>"agents",Dialog::Controls(_)=>"controls",Dialog::Settings=>"settings",Dialog::Saves=>"saves",Dialog::Touch=>"touch",Dialog::Help=>"help",Dialog::Workshop=>"workshop",Dialog::Confirm=>"confirm",Dialog::Error=>"error"}),"error":self.error,"touch":self.touch_mask,"audio_queued":self.host.audio_queued(),"audio_samples":self.host.audio_samples(),"frame":self.session.as_ref().map(|_|self.current_frame()),"runtime":self.worker.as_ref().map(|w|format!("{:?}",w.lifecycle)),"session_state":self.session.as_ref().map(|s|&s.state),"title":self.session.as_ref().map(|s|&s.info.title),"players":self.session.as_ref().map(|s|s.info.players),"modes":self.session.as_ref().map(|s|s.modes.map(|m|m.label())),"decisions":self.session.as_ref().and_then(|s|s.decisions.as_ref()),"frame_rate":self.session.as_ref().map(|s|s.machine.frame_rate()),"game_rect":[self.game_rect.left(),self.game_rect.top(),self.game_rect.width(),self.game_rect.height()]}).to_string()
    }
    fn capture(&mut self, ctx: &Context) {
        self.ui_frames += 1;
        #[cfg(not(target_arch = "wasm32"))]
        if self.screenshot.is_some() && !self.screenshot_requested && self.ui_frames > 8 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.screenshot_requested = true;
        }
        #[cfg(not(target_arch = "wasm32"))]
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event
                && let Some(path) = self.screenshot.take()
            {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Err(e) = image::save_buffer(
                    path,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("Screenshot failed: {e}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if let Some(limit) = self.exit_frames
            && self.current_frame().saturating_sub(self.exit_start) >= limit
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    fn render(app: &mut PlayerApp, ctx: &Context, size: Vec2) -> Vec<egui::accesskit::Node> {
        let mut nodes = Vec::new();
        // egui resolves window placement across initial passes.
        for _ in 0..3 {
            let output = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ctx| {
                    app.workspace(ctx);
                    app.dialogs(ctx);
                },
            );
            nodes = output
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
                .into_iter()
                .map(|(_, n)| n)
                .collect();
        }
        nodes
    }
    fn labels(nodes: &[egui::accesskit::Node]) -> Vec<String> {
        nodes
            .iter()
            .filter_map(|n| n.label().map(str::to_owned))
            .collect()
    }
    #[test]
    fn disconnected_gamepad_recovers_the_affected_player() {
        let ctx = Context::default();
        let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        for slot in 0..2 {
            app.gamepad_disconnected(slot, &ctx);
            app.error = "Localized message without device keywords".into();
            app.recover_input_device(&ctx);
            assert!(app.dialog == Some(Dialog::Player(slot)));
            assert_eq!(app.recovery, None);
        }
        app.gamepad_disconnected(1, &ctx);
        app.fail("Unrelated failure", &ctx);
        assert_eq!(app.recovery, None);
    }
    #[test]
    fn gallery_has_accessible_navigation_and_first_card_action_on_small_phones() {
        for size in [vec2(1440., 900.), vec2(390., 844.), vec2(320., 568.)] {
            let ctx = Context::default();
            ctx.enable_accesskit();
            let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
            app.gallery.open = true;
            let nodes = render(&mut app, &ctx, size);
            let text = labels(&nodes);
            for label in ["Player", "AI games", "Open ROM", "Set up Contra"] {
                assert!(
                    text.iter().any(|s| s == label),
                    "{size:?}: missing {label}: {text:?}"
                );
            }
            assert!(
                nodes
                    .iter()
                    .filter(|n| n.label() == Some("Set up Contra"))
                    .any(|n| {
                        n.bounds().is_some_and(|bounds| {
                            bounds.x0 >= 0.
                                && bounds.x1 <= size.x as f64
                                && bounds.y0 >= 0.
                                && bounds.y1 <= size.y as f64
                        })
                    }),
                "No visible gallery action at {size:?}"
            );
        }
    }
    #[test]
    fn desktop_and_phone_expose_player_and_transport_actions_semantically() {
        for size in [vec2(1440., 900.), vec2(390., 844.), vec2(844., 390.)] {
            let ctx = Context::default();
            ctx.enable_accesskit();
            let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
            app.session =
                Some(Session::new("Playground", nes_core::diagnostic::controller_rom()).unwrap());
            app.prefs.touch = size.x < 900.;
            let nodes = render(&mut app, &ctx, size);
            let text = labels(&nodes);
            assert!(
                text.iter().any(|s| s.contains("Player 1")),
                "{size:?}: {text:?}"
            );
            assert!(
                text.iter().any(|s| s.contains("Player 2")),
                "{size:?}: {text:?}"
            );
            assert!(text.iter().any(|s| s.ends_with("Start")));
            assert!(text.iter().any(|s| s.ends_with("Select")));
            app.session.as_mut().unwrap().info.players = 1;
            let solo = labels(&render(&mut app, &ctx, size));
            assert!(solo.iter().any(|s| s.contains("Player 1")));
            assert!(!solo.iter().any(|s| s.contains("Player 2")), "{solo:?}");
            app.dialog = Some(Dialog::Player(0));
            let settings = labels(&render(&mut app, &ctx, size));
            for mode in ["Human", "AI", "Shared", "No input"] {
                assert!(settings.iter().any(|s| s == mode), "{mode}: {settings:?}");
            }
        }
    }
}
