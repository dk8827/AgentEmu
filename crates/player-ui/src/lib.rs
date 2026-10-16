//! One Rust workspace UI, rendered by wgpu on native desktops and in browsers.
mod accessibility;
mod app;
mod input;
mod metadata;
mod platform;
mod runtime;
mod saves;
mod session;
mod theme;
pub use app::PlayerApp;
#[derive(Default)]
pub struct LaunchOptions {
    pub rom: Option<(String, Vec<u8>)>,
    pub state: Option<Vec<u8>>,
    pub agents: [Option<(String, Vec<u8>)>; 2],
    pub start_task: Option<String>,
    pub alternate: bool,
    pub muted: bool,
    pub autoplay: bool,
    pub ai_games: bool,
    pub exit_frames: Option<u64>,
    pub screenshot: Option<String>,
    pub size: Option<[f32; 2]>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run(options: LaunchOptions) -> eframe::Result {
    eframe::run_native(
        "AgentEmu",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                // Keep the existing preference directory across the product rename.
                .with_app_id("NES — Play together")
                .with_inner_size(options.size.unwrap_or([1280.0, 900.0]))
                .with_min_inner_size([320.0, 480.0]),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(move |cc| {
            let mut app = PlayerApp::new(cc, options.rom.clone());
            app.launch(options, &cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub struct WebPlayer {
    runner: eframe::WebRunner,
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
impl WebPlayer {
    #[wasm_bindgen::prelude::wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            runner: eframe::WebRunner::new(),
        }
    }
    pub async fn start(
        &self,
        canvas: web_sys::HtmlCanvasElement,
        webgl: bool,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let mut options = eframe::WebOptions::default();
        if webgl
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) =
                &mut options.wgpu_options.wgpu_setup
        {
            setup.instance_descriptor.backends = wgpu::Backends::GL;
        }
        self.runner
            .start(
                canvas,
                options,
                Box::new(|cc| Ok(Box::new(PlayerApp::new(cc, None)))),
            )
            .await
    }
    /// Read-only inspection for diagnostics and cross-platform acceptance tests.
    pub fn diagnostics(&self) -> String {
        self.runner
            .app_mut::<PlayerApp>()
            .map(|a| a.diagnostics())
            .unwrap_or_else(|| "{}".into())
    }
    pub fn accessibility_tree(&self) -> String {
        self.runner
            .app_mut::<PlayerApp>()
            .map(|a| a.accessibility_json())
            .unwrap_or_default()
    }
    pub fn accessibility_action(&self, id: &str, action: &str, value: &str) -> bool {
        self.runner
            .app_mut::<PlayerApp>()
            .is_some_and(|mut a| a.accessibility_action(id, action, value))
    }
    pub fn accessibility_focus(&self, focused: bool) {
        if let Some(mut app) = self.runner.app_mut::<PlayerApp>() {
            app.accessibility_focus(focused);
        }
    }
    pub fn destroy(&self) {
        self.runner.destroy();
    }
}
#[cfg(target_arch = "wasm32")]
impl Default for WebPlayer {
    fn default() -> Self {
        Self::new()
    }
}

mod operation;
