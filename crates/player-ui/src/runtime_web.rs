use super::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{MessageEvent, WorkerOptions, WorkerType};
#[derive(Default)]
struct InputTransport {
    pending: Option<Input>,
    busy: bool,
}
struct Mailbox {
    frame: Option<Output>,
    result: Option<Result<Stopped, String>>,
}
pub struct Worker {
    worker: web_sys::Worker,
    mail: Rc<RefCell<Mailbox>>,
    input: Rc<RefCell<InputTransport>>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _error: Closure<dyn FnMut(web_sys::Event)>,
    _visibility: Closure<dyn FnMut(web_sys::Event)>,
    pub lifecycle: Lifecycle,
}
fn object(kind: &str) -> js_sys::Object {
    let o = js_sys::Object::new();
    set(&o, "kind", &kind.into());
    o
}
fn set(o: &JsValue, key: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(o, &key.into(), value);
}
fn get(o: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(o, &key.into()).unwrap_or(JsValue::UNDEFINED)
}
fn text(o: &JsValue, key: &str) -> String {
    get(o, key).as_string().unwrap_or_default()
}
fn pixels(o: &JsValue) -> Vec<u16> {
    js_sys::Uint16Array::new(&get(o, "pixels")).to_vec()
}
fn battery(o: &JsValue) -> Option<Vec<u8>> {
    let b = js_sys::Uint8Array::new(&get(o, "battery")).to_vec();
    (!b.is_empty()).then_some(b)
}
fn flush_input(worker: &web_sys::Worker, input: &RefCell<InputTransport>) -> Result<(), JsValue> {
    let mut input = input.borrow_mut();
    if input.busy {
        return Ok(());
    }
    if let Some(next) = input.pending.take() {
        let m = object("input");
        set(&m, "input", &serde_json::to_string(&next).unwrap().into());
        worker.post_message(&m)?;
        input.busy = true;
    }
    Ok(())
}
impl Worker {
    pub fn new(
        rom: Vec<u8>,
        state: Vec<u8>,
        setup: Setup,
        audio: Option<crate::platform::AudioLink>,
        ctx: egui::Context,
    ) -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker = web_sys::Worker::new_with_options("./session-worker.mjs", &options)
            .map_err(|e| format!("Could not start emulation worker: {e:?}"))?;
        let mail = Rc::new(RefCell::new(Mailbox {
            frame: None,
            result: None,
        }));
        let input = Rc::new(RefCell::new(InputTransport::default()));
        let transport = input.clone();
        let (m, w, c) = (mail.clone(), worker.clone(), ctx.clone());
        let message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let value = event.data();
            let kind = text(&value, "kind");
            if kind == "input-ack" {
                transport.borrow_mut().busy = false;
                if let Err(e) = flush_input(&w, &transport) {
                    m.borrow_mut().result = Some(Err(format!("Input transport stopped: {e:?}")));
                    c.request_repaint();
                }
                return;
            }
            let mut m = m.borrow_mut();
            if kind == "error" {
                m.result = Some(Err(text(&value, "error")));
            } else if kind == "canceled" {
                m.result = Some(Ok(Stopped {
                    state: Vec::new(),
                    view: View::default(),
                    pixels: Vec::new(),
                    battery: None,
                }));
            } else {
                match serde_json::from_str::<View>(&text(&value, "view")) {
                    Ok(view) => {
                        if kind == "stopped" {
                            m.result = Some(Ok(Stopped {
                                state: js_sys::Uint8Array::new(&get(&value, "state")).to_vec(),
                                view,
                                pixels: pixels(&value),
                                battery: battery(&value),
                            }));
                        } else {
                            m.frame = Some(Output {
                                view,
                                pixels: pixels(&value),
                                audio: Vec::new(),
                                battery: battery(&value),
                            });
                            let _ = w.post_message(&object("ack"));
                        }
                    }
                    Err(e) => m.result = Some(Err(format!("Invalid worker response: {e}"))),
                }
            }
            c.request_repaint();
        }) as Box<dyn FnMut(MessageEvent)>);
        worker.set_onmessage(Some(message.as_ref().unchecked_ref()));
        let m = mail.clone();
        let error = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            m.borrow_mut().result = Some(Err(
                "Emulation worker failed to load or stopped unexpectedly".into(),
            ));
            ctx.request_repaint();
        }) as Box<dyn FnMut(web_sys::Event)>);
        worker.set_onerror(Some(error.as_ref().unchecked_ref()));
        let hidden_worker = worker.clone();
        let visibility = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let hidden = web_sys::window()
                .and_then(|w| w.document())
                .is_some_and(|d| d.hidden());
            if hidden || event.type_() != "visibilitychange" {
                let _ = hidden_worker.post_message(&object("stop"));
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                let _ = document.add_event_listener_with_callback(
                    "visibilitychange",
                    visibility.as_ref().unchecked_ref(),
                );
            }
            for name in ["blur", "pagehide"] {
                let _ = window
                    .add_event_listener_with_callback(name, visibility.as_ref().unchecked_ref());
            }
        }

        let init = object("init");
        let r = js_sys::Uint8Array::from(rom.as_slice());
        let s = js_sys::Uint8Array::from(state.as_slice());
        set(&init, "rom", &r);
        set(&init, "state", &s);
        set(
            &init,
            "setup",
            &serde_json::to_string(&setup)
                .map_err(|e| e.to_string())?
                .into(),
        );
        let transfer = js_sys::Array::new();
        transfer.push(&r.buffer());
        transfer.push(&s.buffer());
        worker
            .post_message_with_transfer(&init, &transfer)
            .map_err(|e| format!("Could not send cartridge: {e:?}"))?;
        if let Some(link) = audio {
            let message = object("audio");
            set(&message, "port", &link.port);
            set(&message, "epoch", &link.epoch.into());
            let transfer = js_sys::Array::new();
            transfer.push(&link.port);
            worker
                .post_message_with_transfer(&message, &transfer)
                .map_err(|e| format!("Could not connect audio: {e:?}"))?;
        }
        Ok(Self {
            worker,
            mail,
            input,
            _message: message,
            _error: error,
            _visibility: visibility,
            lifecycle: Lifecycle::Starting,
        })
    }
    pub fn input(&self, mut next: Input) {
        if let Some(previous) = self.input.borrow().pending {
            next.pulse[0] |= previous.pulse[0];
            next.pulse[1] |= previous.pulse[1];
            next.fire_frames = next.fire_frames.max(previous.fire_frames);
        }
        self.input.borrow_mut().pending = Some(next);
        if let Err(e) = flush_input(&self.worker, &self.input) {
            self.mail.borrow_mut().result = Some(Err(format!("Input transport stopped: {e:?}")));
        }
    }
    pub fn take_frame(&mut self) -> Option<Output> {
        let out = self.mail.borrow_mut().frame.take();
        if out.is_some() && self.lifecycle == Lifecycle::Starting {
            self.lifecycle = Lifecycle::Running;
        }
        out
    }
    pub fn stop(&mut self) {
        self.lifecycle = Lifecycle::Pausing;
        let _ = self.worker.post_message(&object("stop"));
    }
    pub fn result(&mut self) -> Option<Result<Stopped, String>> {
        self.mail.borrow_mut().result.take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                let _ = document.remove_event_listener_with_callback(
                    "visibilitychange",
                    self._visibility.as_ref().unchecked_ref(),
                );
            }
            for name in ["blur", "pagehide"] {
                let _ = window.remove_event_listener_with_callback(
                    name,
                    self._visibility.as_ref().unchecked_ref(),
                );
            }
        }
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.worker.terminate();
    }
}
