use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
impl PlayerApp {
    pub(super) fn load_saves(&mut self, ctx: &Context) {
        self.saves = Default::default();
        self.save_textures.clear();
        let Some(s) = &self.session else { return };
        let hash = s.machine.cartridge_hash();
        for slot in 0..3 {
            match platform::local_read(&format!("{hash}-{slot}")).and_then(|text| {
                text.map(|t| crate::saves::LocalSave::decode(&t, &hash))
                    .transpose()
            }) {
                Ok(save) => self.saves[slot] = save,
                Err(e) => {
                    self.notice = format!(
                        "Slot {} could not be read: {e}. Other slots are unchanged.",
                        slot + 1
                    )
                }
            }
        }
        self.quick_save = self.saves[0].as_ref().and_then(|s| s.bytes().ok());
        self.save_previews(ctx);
    }
    fn save_previews(&mut self, ctx: &Context) {
        for (slot, save) in self.saves.iter().enumerate() {
            if let Some(save) = save
                && let Ok(bytes) = STANDARD.decode(&save.preview)
                && let Ok(image) = image::load_from_memory(&bytes)
            {
                let rgba = image.to_rgba8();
                self.save_textures.insert(
                    slot,
                    ctx.load_texture(
                        format!("save-{slot}"),
                        ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            rgba.as_raw(),
                        ),
                        TextureOptions::NEAREST,
                    ),
                );
            }
        }
    }
    pub(super) fn store_save(&mut self, slot: usize, ctx: &Context) {
        if self.defer_pause(ctx, move |app, ctx| app.store_save(slot, ctx)) {
            return;
        }
        self.pause(ctx);
        let Some(s) = &self.session else { return };
        let result = (|| -> Result<crate::saves::LocalSave, String> {
            let state = nes_core::state_file::export(&s.machine).map_err(|e| e.to_string())?;
            let rgba = nes_video::display::rgba(s.machine.ppu().framebuffer());
            let image = image::RgbaImage::from_raw(256, 240, rgba)
                .ok_or("Could not capture save preview")?;
            let preview =
                image::imageops::resize(&image, 96, 90, image::imageops::FilterType::Nearest);
            let mut png = std::io::Cursor::new(Vec::new());
            preview
                .write_to(&mut png, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            let save = crate::saves::LocalSave::new(
                s.machine.cartridge_hash(),
                platform::save_timestamp(),
                self.current_frame(),
                &state,
                png.get_ref(),
            );
            platform::local_write(
                &format!("{}-{slot}", save.cartridge),
                &serde_json::to_string(&save).map_err(|e| e.to_string())?,
            )?;
            Ok(save)
        })();
        match result {
            Ok(save) => {
                if slot == 0 {
                    self.quick_save = save.bytes().ok();
                }
                self.saves[slot] = Some(save);
                self.save_previews(ctx);
                self.notice = format!("Slot {} saved on this device.", slot + 1);
            }
            Err(e) => self.fail(
                format!(
                    "Could not save on this device: {e}. Export a save file to keep your progress."
                ),
                ctx,
            ),
        }
    }
}
