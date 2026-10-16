#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Sender;
#[cfg(not(target_arch = "wasm32"))]
#[path = "platform/native.rs"]
mod implementation;
#[cfg(target_arch = "wasm32")]
#[path = "platform/web.rs"]
mod implementation;
pub use implementation::*;

#[derive(Clone, Copy, Debug)]
pub enum FileKind {
    Rom,
    State,
    Replay,
    Report,
}
pub enum Event {
    File(FileKind, Option<(String, Vec<u8>)>),
    Written(Result<(), String>),
    Error(String),
}
pub type Message = (u64, Event);
#[cfg(not(target_arch = "wasm32"))]
fn launch(future: impl std::future::Future<Output = ()> + 'static + Send) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || pollster::block_on(future));
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(future);
}
#[cfg(not(target_arch = "wasm32"))]
pub fn pick(id: u64, kind: FileKind, tx: Sender<Message>, ctx: egui::Context) {
    let operation = async move {
        let extensions: &[&str] = match kind {
            FileKind::Rom => &["nes"],
            FileKind::State => &["state", "json"],
            _ => &["json"],
        };
        let file = rfd::AsyncFileDialog::new()
            .add_filter("Compatible files", extensions)
            .pick_file()
            .await;
        let event = if let Some(file) = file {
            let name = file.file_name();
            let bytes = file.read().await;
            if bytes.len() > 16 * 1024 * 1024 {
                Event::Error("This file exceeds the 16 MiB limit.".into())
            } else {
                Event::File(kind, Some((name, bytes)))
            }
        } else {
            Event::File(kind, None)
        };
        let _ = tx.send((id, event));
        ctx.request_repaint();
    };
    #[cfg(not(target_arch = "wasm32"))]
    launch(operation);
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(operation);
}
#[cfg(not(target_arch = "wasm32"))]
pub fn download(id: u64, name: String, bytes: Vec<u8>, tx: Sender<Message>, ctx: egui::Context) {
    let operation = async move {
        let result = if let Some(file) = rfd::AsyncFileDialog::new()
            .set_file_name(&name)
            .save_file()
            .await
        {
            file.write(&bytes).await.map_err(|e| e.to_string())
        } else {
            Ok(())
        };
        let _ = tx.send((id, Event::Written(result)));
        ctx.request_repaint();
    };
    #[cfg(not(target_arch = "wasm32"))]
    launch(operation);
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(operation);
}
