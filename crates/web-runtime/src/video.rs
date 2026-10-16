use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct Presenter {
    inner: nes_video::renderer::Presenter,
}
#[wasm_bindgen]
impl Presenter {
    pub async fn create(
        canvas: web_sys::HtmlCanvasElement,
        webgl: bool,
    ) -> Result<Presenter, JsError> {
        canvas.set_width(256);
        canvas.set_height(240);
        let backends = if webgl {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::BROWSER_WEBGPU
        };
        let inner = nes_video::renderer::Presenter::create(
            wgpu::SurfaceTarget::Canvas(canvas),
            256,
            240,
            backends,
        )
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self { inner })
    }
    pub fn backend(&self) -> String {
        self.inner.backend()
    }
    pub fn present(&self, rgba: &[u8]) -> Result<(), JsError> {
        self.inner
            .present(rgba)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}
