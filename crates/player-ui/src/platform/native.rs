#[path = "native_audio.rs"]
mod native_audio;
use std::path::PathBuf;
pub struct Platform {
    audio: Option<native_audio::Audio>,
    pads: Option<gilrs::Gilrs>,
    pub audio_error: Option<String>,
    volume: f32,
}
impl Default for Platform {
    fn default() -> Self {
        Self {
            audio: None,
            pads: gilrs::Gilrs::new().ok(),
            audio_error: None,
            volume: 0.6,
        }
    }
}
impl Platform {
    pub fn audio_sink(&self) -> Option<session_engine::audio_queue::AudioSink> {
        self.audio.as_ref().filter(|a| a.active()).map(|a| a.sink())
    }
    pub fn audio_settings(&mut self, muted: bool, volume: f32) {
        if let Some(a) = &self.audio {
            a.volume(if muted { 0. } else { volume });
        }
    }
    pub fn start_audio(&mut self, volume: f32) {
        self.volume = volume;
        if self.audio.as_ref().is_none_or(|a| !a.active()) {
            match native_audio::Audio::new() {
                Ok(a) => self.audio = Some(a),
                Err(e) => self.audio_error = Some(e.to_string()),
            }
        }
    }
    pub fn audio_stop(&mut self) {
        self.audio_clear();
    }
    pub fn audio_clear(&mut self) {
        if let Some(a) = &self.audio {
            a.clear()
        }
    }
    pub fn audio_samples(&self) -> u64 {
        self.audio.as_ref().map_or(0, |a| a.samples())
    }
    pub fn audio_queued(&self) -> usize {
        self.audio.as_ref().map_or(0, |a| a.queued())
    }
    pub fn poll(&mut self) {
        if self.audio.as_ref().is_some_and(|a| !a.active()) {
            self.audio_error = Some("Audio device stopped. Reconnect it and resume.".into());
        }
        if let Some(p) = &mut self.pads {
            while p.next_event().is_some() {}
        }
    }
    pub fn gamepads(&self) -> Vec<(usize, String)> {
        self.pads
            .as_ref()
            .map(|p| {
                p.gamepads()
                    .filter(|(_, g)| g.is_connected())
                    .map(|(id, g)| (usize::from(id), g.name().into()))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn pad_buttons(&self, index: usize) -> u8 {
        use gilrs::{Axis, Button::*};
        let Some(p) = &self.pads else { return 0 };
        let Some((_, g)) = p.gamepads().find(|(id, _)| usize::from(*id) == index) else {
            return 0;
        };
        let mut b = 0;
        for (i, key) in [
            South, West, Select, Start, DPadUp, DPadDown, DPadLeft, DPadRight,
        ]
        .iter()
        .enumerate()
        {
            if g.is_pressed(*key) {
                b |= 1 << i;
            }
        }
        let x = g.value(Axis::LeftStickX);
        let y = g.value(Axis::LeftStickY);
        if x > 0.4 {
            b |= 128
        } else if x < -0.4 {
            b |= 64
        }
        if y > 0.4 {
            b |= 16
        } else if y < -0.4 {
            b |= 32
        }
        contra_agent::control::normalize(b)
    }
    pub fn touch_device(&self) -> bool {
        false
    }
}
fn root() -> PathBuf {
    // Stable legacy storage ID: renaming the product must not hide saved games.
    directories::ProjectDirs::from("dev", "emulator", "NES Player")
        .map(|d| d.data_local_dir().to_owned())
        .unwrap_or_else(|| PathBuf::from(".nes-saves"))
}
pub fn battery_read(hash: &str) -> Result<Option<Vec<u8>>, String> {
    let p = root().join("battery").join(format!("{hash}.sav"));
    if !p.exists() {
        return Ok(None);
    }
    std::fs::read(p).map(Some).map_err(|e| e.to_string())
}
pub fn battery_write(hash: &str, bytes: &[u8]) -> Result<(), String> {
    let p = root().join("battery");
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    let temp = p.join(format!("{hash}.sav.tmp"));
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(temp, p.join(format!("{hash}.sav"))).map_err(|e| e.to_string())
}
pub fn fullscreen(ctx: &egui::Context) {
    let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
}

pub fn launch_query() -> (String, usize) {
    (String::new(), 0)
}
pub fn migrate_keys(_prefs: &mut crate::input::Preferences) {}

pub fn local_read(key: &str) -> Result<Option<String>, String> {
    let p = root().join("slots").join(format!("{key}.json"));
    if !p.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(p)
        .map(Some)
        .map_err(|e| e.to_string())
}
pub fn local_write(key: &str, value: &str) -> Result<(), String> {
    let dir = root().join("slots");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let temp = dir.join(format!("{key}.tmp"));
    std::fs::write(&temp, value).map_err(|e| e.to_string())?;
    std::fs::rename(temp, dir.join(format!("{key}.json"))).map_err(|e| e.to_string())
}
pub fn save_timestamp() -> String {
    chrono::Local::now().format("%d %b %Y, %H:%M").to_string()
}
