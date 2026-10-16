use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{AudioContext, AudioWorkletNode, GainNode, MessageEvent, MessagePort};
#[derive(Default)]
struct AudioState {
    node: Option<AudioWorkletNode>,
    pending: Vec<(MessagePort, u32)>,
    epoch: u32,
    queued: usize,
    samples: u64,
    error: Option<String>,
    callback: Option<Closure<dyn FnMut(MessageEvent)>>,
}
struct Audio {
    context: AudioContext,
    gain: GainNode,
    state: Rc<RefCell<AudioState>>,
}
impl Drop for Audio {
    fn drop(&mut self) {
        if let Some(node) = &self.state.borrow().node
            && let Ok(port) = node.port()
        {
            port.set_onmessage(None);
        }
        let _ = self.context.close();
    }
}
pub struct AudioLink {
    pub port: MessagePort,
    pub epoch: u32,
}
#[derive(Default)]
pub struct Platform {
    audio: Option<Audio>,
    pub audio_error: Option<String>,
}
fn object(fields: &[(&str, JsValue)]) -> js_sys::Object {
    let object = js_sys::Object::new();
    for (k, v) in fields {
        let _ = js_sys::Reflect::set(&object, &JsValue::from_str(k), v);
    }
    object
}
fn source(node: &AudioWorkletNode, port: MessagePort, epoch: u32) -> Result<(), JsValue> {
    let transfer = js_sys::Array::new();
    transfer.push(&port);
    node.port()?.post_message_with_transferable(
        &object(&[("source", port.into()), ("epoch", JsValue::from(epoch))]),
        &transfer,
    )
}
impl Platform {
    pub fn start_audio(&mut self, volume: f32) {
        if self.audio.is_none() {
            let result = (|| -> Result<Audio, JsValue> {
                let context = AudioContext::new()?;
                let gain = context.create_gain()?;
                gain.connect_with_audio_node(&context.destination())?;
                gain.gain().set_value(volume);
                let state = Rc::new(RefCell::new(AudioState::default()));
                let (c, g, s) = (context.clone(), gain.clone(), state.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    let result = async {
                        wasm_bindgen_futures::JsFuture::from(
                            c.audio_worklet()?.add_module("./runtime-pcm-worklet.mjs")?,
                        )
                        .await?;
                        let node = AudioWorkletNode::new(&c, "nes-runtime-pcm")?;
                        node.connect_with_audio_node(&g)?;
                        let weak = Rc::downgrade(&s);
                        let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
                            if let Some(s) = weak.upgrade() {
                                let value = event.data();
                                let mut s = s.borrow_mut();
                                s.queued = js_sys::Reflect::get(&value, &"queued".into())
                                    .ok()
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.)
                                    as usize;
                                s.samples = js_sys::Reflect::get(&value, &"samples".into())
                                    .ok()
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.)
                                    as u64;
                            }
                        })
                            as Box<dyn FnMut(MessageEvent)>);
                        node.port()?
                            .set_onmessage(Some(callback.as_ref().unchecked_ref()));
                        let mut s = s.borrow_mut();
                        for (port, epoch) in s.pending.drain(..) {
                            source(&node, port, epoch)?;
                        }
                        s.callback = Some(callback);
                        s.node = Some(node);
                        Ok::<_, JsValue>(())
                    }
                    .await;
                    if let Err(e) = result {
                        let mut s = s.borrow_mut();
                        for (port, _) in s.pending.drain(..) {
                            port.close();
                        }
                        s.error = Some(format!("Could not start sound: {e:?}"));
                    }
                });
                Ok(Audio {
                    context,
                    gain,
                    state,
                })
            })();
            match result {
                Ok(a) => self.audio = Some(a),
                Err(e) => self.audio_error = Some(format!("Could not start sound: {e:?}")),
            }
        }
        if let Some(a) = &self.audio {
            let _ = a.context.resume();
            a.gain.gain().set_value(volume);
        }
    }
    pub fn audio_sink(&mut self) -> Option<AudioLink> {
        let a = self.audio.as_ref()?;
        let channel = web_sys::MessageChannel::new().ok()?;
        let mut s = a.state.borrow_mut();
        s.epoch = s.epoch.wrapping_add(1);
        let epoch = s.epoch;
        if let Some(node) = &s.node {
            if source(node, channel.port2(), epoch).is_err() {
                return None;
            }
        } else {
            s.pending.push((channel.port2(), epoch));
        }
        Some(AudioLink {
            port: channel.port1(),
            epoch,
        })
    }
    pub fn audio_settings(&mut self, muted: bool, volume: f32) {
        if let Some(a) = &self.audio {
            a.gain.gain().set_value(if muted { 0. } else { volume });
        }
    }
    pub fn audio_stop(&mut self) {
        if let Some(a) = &self.audio {
            let mut s = a.state.borrow_mut();
            for (port, _) in s.pending.drain(..) {
                port.close();
            }
            if let Some(node) = &s.node
                && let Ok(port) = node.port()
            {
                let _ = port.post_message(&object(&[("stop", true.into())]));
            }
            s.queued = 0;
        }
    }
    pub fn audio_clear(&mut self) {
        if let Some(a) = &self.audio {
            let mut s = a.state.borrow_mut();
            if let Some(node) = &s.node
                && let Ok(port) = node.port()
            {
                let _ = port.post_message(&object(&[("clear", true.into())]));
            }
            s.queued = 0;
        }
    }
    pub fn audio_samples(&self) -> u64 {
        self.audio.as_ref().map_or(0, |a| a.state.borrow().samples)
    }
    pub fn audio_queued(&self) -> usize {
        self.audio.as_ref().map_or(0, |a| a.state.borrow().queued)
    }
    pub fn poll(&mut self) {
        if let Some(a) = &self.audio
            && let Some(error) = a.state.borrow_mut().error.take()
        {
            self.audio_error = Some(error);
        }
    }
    pub fn gamepads(&self) -> Vec<(usize, String)> {
        pads()
            .into_iter()
            .filter(|g| g.connected())
            .map(|g| (g.index() as usize, g.id()))
            .collect()
    }
    pub fn pad_buttons(&self, index: usize) -> u8 {
        let Some(g) = pads()
            .into_iter()
            .find(|g| g.index() as usize == index && g.connected())
        else {
            return 0;
        };
        let buttons = g.buttons();
        let axes = g.axes();
        let mut b = 0;
        for (bit, i) in [0, 2, 8, 9, 12, 13, 14, 15].into_iter().enumerate() {
            if buttons
                .get(i)
                .dyn_into::<web_sys::GamepadButton>()
                .is_ok_and(|b| b.pressed())
            {
                b |= 1 << bit;
            }
        }
        let x = axes.get(0).as_f64().unwrap_or(0.);
        let y = axes.get(1).as_f64().unwrap_or(0.);
        if x > 0.4 {
            b |= 128
        } else if x < -0.4 {
            b |= 64
        }
        if y > 0.4 {
            b |= 32
        } else if y < -0.4 {
            b |= 16
        }
        contra_agent::control::normalize(b)
    }
    pub fn touch_device(&self) -> bool {
        web_sys::window().is_some_and(|w| w.navigator().max_touch_points() > 0)
    }
}
fn pads() -> Vec<web_sys::Gamepad> {
    web_sys::window()
        .and_then(|w| w.navigator().get_gamepads().ok())
        .map(|a| a.iter().filter_map(|v| v.dyn_into().ok()).collect())
        .unwrap_or_default()
}
fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("No browser window")?
        .local_storage()
        .map_err(|_| "Browser storage is unavailable")?
        .ok_or("Browser storage is unavailable".into())
}
pub fn battery_read(hash: &str) -> Result<Option<Vec<u8>>, String> {
    storage()?
        .get_item(&format!("nes-battery-v1:{hash}"))
        .map_err(|_| "Could not read battery save".to_string())?
        .map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        .transpose()
}
pub fn battery_write(hash: &str, bytes: &[u8]) -> Result<(), String> {
    storage()?
        .set_item(
            &format!("nes-battery-v1:{hash}"),
            &serde_json::to_string(bytes).map_err(|e| e.to_string())?,
        )
        .map_err(|_| "Could not store battery save. Download a save before closing.".into())
}
pub fn fullscreen(_ctx: &egui::Context) {
    if let Some(d) = web_sys::window().and_then(|w| w.document()) {
        if d.fullscreen_element().is_some() {
            d.exit_fullscreen();
        } else if let Some(e) = d.document_element() {
            let _ = e.request_fullscreen();
        }
    }
}

pub fn pick(
    id: u64,
    kind: super::FileKind,
    tx: std::sync::mpsc::Sender<super::Message>,
    ctx: egui::Context,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let result = pick_file(kind).await;
        let event = match result {
            Ok(file) => super::Event::File(kind, file),
            Err(e) => super::Event::Error(format!("Could not open file: {e:?}")),
        };
        let _ = tx.send((id, event));
        ctx.request_repaint();
    });
}
async fn pick_file(
    kind: super::FileKind,
) -> Result<Option<(String, Vec<u8>)>, wasm_bindgen::JsValue> {
    use wasm_bindgen::{JsValue, closure::Closure};
    let document = web_sys::window().unwrap().document().unwrap();
    let input: web_sys::HtmlInputElement = document.create_element("input")?.dyn_into()?;
    input.set_type("file");
    input.set_accept(match kind {
        super::FileKind::Rom => ".nes",
        super::FileKind::State => ".state,.json",
        _ => ".json",
    });
    input.style().set_property("display", "none")?;
    document.body().unwrap().append_child(&input)?;
    let mut change = None;
    let mut cancel = None;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let r = resolve.clone();
        let c = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let _ = r.call0(&JsValue::UNDEFINED);
        });
        input.set_onchange(Some(c.as_ref().unchecked_ref()));
        change = Some(c);
        let r = resolve.clone();
        let c = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let _ = r.call0(&JsValue::UNDEFINED);
        });
        input.set_oncancel(Some(c.as_ref().unchecked_ref()));
        cancel = Some(c);
    });
    input.click();
    wasm_bindgen_futures::JsFuture::from(promise).await?;
    input.set_onchange(None);
    input.set_oncancel(None);
    let file = input.files().and_then(|f| f.item(0));
    input.remove();
    drop((change, cancel));
    if let Some(file) = file {
        if file.size() > 16. * 1024. * 1024. {
            return Err(JsValue::from_str("File exceeds 16 MiB"));
        }
        let name = file.name();
        let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await?;
        Ok(Some((name, js_sys::Uint8Array::new(&buffer).to_vec())))
    } else {
        Ok(None)
    }
}
pub fn download(
    id: u64,
    name: String,
    bytes: Vec<u8>,
    tx: std::sync::mpsc::Sender<super::Message>,
    ctx: egui::Context,
) {
    let result = (|| -> Result<(), wasm_bindgen::JsValue> {
        let array = js_sys::Array::new();
        array.push(&js_sys::Uint8Array::from(bytes.as_slice()));
        let blob = web_sys::Blob::new_with_u8_array_sequence(&array)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let d = web_sys::window().unwrap().document().unwrap();
        let a: web_sys::HtmlAnchorElement = d.create_element("a")?.dyn_into()?;
        a.set_href(&url);
        a.set_download(&name);
        a.style().set_property("display", "none")?;
        d.body().unwrap().append_child(&a)?;
        a.click();
        a.remove();
        let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
            let _ = web_sys::Url::revoke_object_url(&url);
        });
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.unchecked_ref(),
                1000,
            )?;
        Ok(())
    })()
    .map_err(|e| format!("Could not download save: {e:?}"));
    let _ = tx.send((id, super::Event::Written(result)));
    ctx.request_repaint();
}

pub fn launch_query() -> (String, usize) {
    let query = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    let args: std::collections::HashMap<_, _> = query
        .trim_start_matches('?')
        .split('&')
        .filter_map(|s| s.split_once('='))
        .collect();
    (
        args.get("view").unwrap_or(&"").to_string(),
        usize::from(args.get("slot") == Some(&"1")),
    )
}
pub fn migrate_keys(prefs: &mut crate::input::Preferences) {
    let Some(keys) = storage()
        .ok()
        .and_then(|s| s.get_item("nes-keys").ok().flatten())
        .and_then(|s| serde_json::from_str::<[[String; 8]; 2]>(&s).ok())
    else {
        return;
    };
    for (p, player) in keys.iter().enumerate() {
        for (b, code) in player.iter().enumerate() {
            let name = code
                .strip_prefix("Key")
                .or_else(|| code.strip_prefix("Digit"))
                .unwrap_or(code);
            if let Some(key) = egui::Key::from_name(name)
                && !matches!(
                    key,
                    egui::Key::Escape | egui::Key::F5 | egui::Key::F8 | egui::Key::F11
                )
            {
                prefs.keys[p][b] = key;
            }
        }
    }
}

// Each slot is one atomic storage write; quota failures retain the previous record.
pub fn local_read(key: &str) -> Result<Option<String>, String> {
    storage()?
        .get_item(&format!("nes-slot-v1:{key}"))
        .map_err(|_| "Device storage is unavailable".into())
}
pub fn local_write(key: &str, value: &str) -> Result<(), String> {
    storage()?
        .set_item(&format!("nes-slot-v1:{key}"), value)
        .map_err(|_| "Device storage is full or blocked".into())
}
pub fn save_timestamp() -> String {
    js_sys::Date::new_0()
        .to_locale_string("en-GB", &wasm_bindgen::JsValue::UNDEFINED)
        .as_string()
        .unwrap_or_default()
}
