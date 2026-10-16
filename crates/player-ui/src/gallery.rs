//! Game-first discovery for the agents actually shipped with the application.
use super::*;

#[derive(Clone)]
pub(super) struct Game {
    pub hash: String,
    title: String,
    experimental: bool,
    art: &'static [u8],
    models: Vec<usize>,
}

pub(super) struct Gallery {
    pub open: bool,
    pub pending_rom: Option<String>,
    games: Vec<Game>,
    textures: HashMap<String, TextureHandle>,
    selected: Option<usize>,
    model: usize,
    modes: [Mode; 2],
    player2_mode: Mode,
    ownership: [u8; 2],
}
impl Default for Gallery {
    fn default() -> Self {
        Self {
            open: false,
            pending_rom: None,
            games: games(),
            textures: HashMap::new(),
            selected: None,
            model: 0,
            modes: [Mode::Human, Mode::Ai],
            player2_mode: Mode::Ai,
            ownership: [2; 2],
        }
    }
}
impl Gallery {
    fn disable_player2(&mut self, disabled: bool) {
        if disabled && self.modes[1] != Mode::Off {
            self.player2_mode = self.modes[1];
            self.modes[1] = Mode::Off;
        } else if !disabled && self.modes[1] == Mode::Off {
            self.modes[1] = self.player2_mode;
        }
    }
    pub fn validate_file(&self, hash: &str) -> Result<(), String> {
        if let Some(expected) = &self.pending_rom
            && expected != hash
        {
            let game = metadata::identify(expected, "Selected game");
            return Err(format!(
                "This agent needs {} ({}, {}).\nExpected SHA-256: {}\nSelected SHA-256: {}\nUse Open ROM from the player to try this file with human controls. Your current game has not changed.",
                game.title, game.region, game.revision, expected, hash
            ));
        }
        Ok(())
    }
}

fn gallery_width(ctx: &Context, available: f32) -> f32 {
    let height = ctx.content_rect().height();
    available.min(if height < 500. {
        760.
    } else if height >= 1000. {
        1008.
    } else {
        876.
    })
}

fn games() -> Vec<Game> {
    let mut games: Vec<Game> = Vec::new();
    for (index, builtin) in metadata::builtins().iter().enumerate() {
        let manifest: serde_json::Value =
            serde_json::from_str(builtin.manifest).expect("bundled manifest");
        let hash = manifest["rom_sha256"]
            .as_str()
            .expect("bundled ROM identity");
        if let Some(game) = games.iter_mut().find(|g| g.hash == hash) {
            game.models.push(index);
            continue;
        }
        let (title, art): (&str, &[u8]) = include!("gallery_art.in");
        games.push(Game {
            hash: hash.into(),
            title: if title.is_empty() {
                metadata::identify(hash, "Game").title
            } else {
                title.into()
            },
            experimental: builtin.experimental,
            art,
            models: vec![index],
        });
    }
    games
}

impl PlayerApp {
    pub(super) fn open_gallery(&mut self, ctx: &Context) {
        if self.defer_pause(ctx, |app, ctx| app.open_gallery(ctx)) {
            return;
        }
        self.pause(ctx);
        self.dialog = None;
        self.back_stack.clear();
        self.gallery.open = true;
        self.gallery.pending_rom = None;
        self.touch_layout = None;
        self.game_rect = Rect::NOTHING;
    }

    fn gallery_texture(&mut self, game: &Game, ctx: &Context) -> Option<TextureId> {
        if !self.gallery.textures.contains_key(&game.hash) {
            let image = image::load_from_memory(game.art).ok()?.into_rgba8();
            let pixels = ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            );
            self.gallery.textures.insert(
                game.hash.clone(),
                ctx.load_texture(
                    format!("gallery-{}", game.title),
                    pixels,
                    TextureOptions::LINEAR,
                ),
            );
        }
        self.gallery.textures.get(&game.hash).map(TextureHandle::id)
    }

    fn select_gallery_game(&mut self, index: usize, ctx: &Context) {
        self.gallery.selected = Some(index);
        self.gallery.model = 0;
        self.gallery.modes = [Mode::Human, Mode::Ai];
        self.gallery.player2_mode = Mode::Ai;
        self.gallery.ownership = [2; 2];
        self.open(Dialog::GallerySetup, ctx);
    }

    pub(super) fn gallery_workspace(&mut self, ctx: &Context) {
        let phone = ctx.content_rect().width() < 680.;
        let short = ctx.content_rect().height() < 500.;
        let margin = if phone || short { 12 } else { 28 };
        TopBottomPanel::top("gallery-navigation")
            .frame(
                Frame::new()
                    .fill(PAPER)
                    .inner_margin(Margin::symmetric(margin, 10)),
            )
            .show(ctx, |ui| {
                if self.dialog.is_some()
                    || self.requests.pending().is_some()
                    || self.transitioning()
                {
                    ui.disable();
                }
                let width = gallery_width(ctx, ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.allocate_ui_with_layout(
                        vec2(width, 44.),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = if phone { 4. } else { 16. };
                            ui.spacing_mut().button_padding.x = if phone { 4. } else { 14. };
                            ui.label(RichText::new("AgentEmu").font(bold(if phone {
                                18.
                            } else {
                                25.
                            })));
                            if !phone {
                                ui.add_space(12.);
                            }
                            if ui
                                .add_sized(
                                    [if phone { 46. } else { 70. }, 44.],
                                    Button::new(RichText::new("Player").font(bold(14.)))
                                        .frame(false),
                                )
                                .clicked()
                            {
                                self.gallery.open = false;
                                self.gallery.pending_rom = None;
                                self.clear(ctx);
                            }
                            let active = ui.add_sized(
                                [if phone { 64. } else { 88. }, 44.],
                                Button::new(
                                    RichText::new("AI games").font(bold(14.)).color(ACCENT),
                                )
                                .frame(false),
                            );
                            active.widget_info(|| {
                                WidgetInfo::selected(
                                    WidgetType::SelectableLabel,
                                    ui.is_enabled(),
                                    true,
                                    "AI games",
                                )
                            });
                            ui.painter().line_segment(
                                [
                                    active.rect.left_bottom() + vec2(6., 0.),
                                    active.rect.right_bottom() - vec2(6., 0.),
                                ],
                                Stroke::new(2., ACCENT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add_sized(
                                        [if phone { 76. } else { 110. }, 44.],
                                        Button::new(RichText::new("Open ROM").font(bold(14.))),
                                    )
                                    .clicked()
                                {
                                    self.gallery.pending_rom = None;
                                    self.queued_model = None;
                                    self.pick(FileKind::Rom, ctx);
                                }
                            });
                        },
                    );
                });
            });
        CentralPanel::default()
            .frame(Frame::new().fill(PAPER).inner_margin(Margin::symmetric(margin, if short { 12 } else { 16 })))
            .show(ctx, |ui| {
                if self.dialog.is_some() || self.requests.pending().is_some() || self.transitioning() { ui.disable(); }
                ScrollArea::vertical().id_salt("ai-game-gallery").auto_shrink([false, false]).show(ui, |ui| {
                    let width = gallery_width(ctx, ui.available_width());
                    ui.vertical_centered(|ui| {
                        ui.allocate_ui_with_layout(vec2(width, 0.), Layout::top_down(Align::Min), |ui| {
                            ui.set_max_width(width);
                            ui.spacing_mut().item_spacing.y = 8.;
                            if !short {
                                ui.label(RichText::new(if phone { "Games with AI" } else { "Games you can play with AI" }).font(bold(if phone { 29. } else { 36. })));
                                ui.label(RichText::new("Play alongside AI, share a controller, or watch it play.").size(if phone { 14. } else { 16. }).color(MUTED));
                                ui.add_space(if phone { 8. } else { 6. });
                            }
                            let current = self.session.as_ref().map(|s| s.machine.cartridge_hash());
                            let games: Vec<_> = self.gallery.games.iter().cloned().enumerate().collect();
                            let columns = if (short && width >= 520.) || width >= 840. {
                                        3
                                    } else if width >= 600. { 2 } else { 1 };
                            let gap = if phone { 16. } else { 24. };
                            let card_width = (width - gap * (columns - 1) as f32) / columns as f32;
                            for (row_index, row) in games.chunks(columns).enumerate() {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = gap;
                                    for (index, game) in row {
                                        ui.push_id(index, |ui| {
                                            let ready = current.as_deref() == Some(&game.hash);
                                            self.gallery_card(ui, ctx, *index, game, card_width, ready);
                                        });
                                    }
                                });
                                if row_index + 1 < games.len().div_ceil(columns) { ui.add_space(8.); }
                            }
                        });
                    });
                });
            });
    }

    fn gallery_card(
        &mut self,
        ui: &mut Ui,
        ctx: &Context,
        index: usize,
        game: &Game,
        width: f32,
        ready: bool,
    ) {
        let texture = self.gallery_texture(game, ctx);
        if ctx.content_rect().width() < 600. {
            let iw = (width * 0.40).min(132.);
            let title = ui
                .painter()
                .layout(game.title.clone(), bold(17.), INK, width - iw - 38.);
            let height = (title.size().y + 70.).max(144.);
            let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| {
                WidgetInfo::labeled(
                    WidgetType::Button,
                    ui.is_enabled(),
                    format!("Set up {}", game.title),
                )
            });
            ui.painter().rect_filled(rect, 4., Color32::WHITE);
            ui.painter().rect_stroke(
                rect,
                4.,
                Stroke::new(
                    if response.has_focus() { 2. } else { 1. },
                    if response.has_focus() { BLUE } else { LINE },
                ),
                StrokeKind::Inside,
            );
            let image_rect = Rect::from_center_size(
                pos2(rect.left() + 12. + iw / 2., rect.center().y),
                vec2(iw, iw * 9. / 16.),
            );
            if let Some(texture) = texture {
                ui.painter().image(
                    texture,
                    image_rect,
                    Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                    Color32::WHITE,
                );
            }
            let x = image_rect.right() + 14.;
            ui.painter().galley(pos2(x, rect.top() + 18.), title, INK);
            ui.painter().text(
                pos2(x, rect.bottom() - 22.),
                Align2::LEFT_CENTER,
                "Set up →",
                bold(14.),
                if response.hovered() { ACCENT } else { MUTED },
            );
            if response.clicked() {
                if response.clicked_by(PointerButton::Primary) {
                    response.surrender_focus();
                }
                self.select_gallery_game(index, ctx);
            }
            return;
        }
        let font = bold(if width < 240. { 18. } else { 20. });
        let wrap_title = |title: &str| {
            title
                .replace("Turtles II", "Turtles\u{a0}II")
                .replace("The Revenge", "The\u{a0}Revenge")
        };
        let title = ui
            .painter()
            .layout(wrap_title(&game.title), font.clone(), INK, width - 32.);
        // Reserve equal space for the tallest wrapped title across the grid.
        let title_height = self
            .gallery
            .games
            .iter()
            .map(|game| {
                ui.painter()
                    .layout(wrap_title(&game.title), font.clone(), INK, width - 32.)
                    .size()
                    .y
            })
            .fold(48., f32::max);
        let image_height = (width - 20.) * 9. / 16.;
        let (rect, response) = ui.allocate_exact_size(
            vec2(width, image_height + title_height + 54.),
            Sense::click(),
        );
        let response = response.on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                ui.is_enabled(),
                format!(
                    "Set up {}{}",
                    game.title,
                    if ready { ". Game loaded" } else { "" }
                ),
            )
        });
        let hovered = response.hovered();
        ui.painter().rect_filled(
            rect,
            3,
            if hovered {
                Color32::WHITE
            } else {
                Color32::from_rgb(253, 252, 248)
            },
        );
        ui.painter().rect_stroke(
            rect,
            3,
            Stroke::new(
                1.,
                if hovered {
                    MUTED
                } else {
                    Color32::from_rgb(214, 213, 205)
                },
            ),
            StrokeKind::Inside,
        );
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(2.),
                4,
                Stroke::new(2., BLUE),
                StrokeKind::Outside,
            );
        }
        // The complete source frame fills its container: no crop or artificial sidebars.
        let image_rect =
            Rect::from_min_size(rect.min + vec2(10., 10.), vec2(width - 20., image_height));
        if let Some(texture) = texture {
            ui.painter().image(
                texture,
                image_rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
        }
        ui.painter().galley(
            pos2(rect.left() + 16., image_rect.bottom() + 12.),
            title,
            INK,
        );
        let y = rect.bottom() - 16.;
        ui.painter().text(
            pos2(rect.left() + 16., y),
            Align2::LEFT_CENTER,
            "Set up game",
            bold(14.),
            if hovered { INK } else { MUTED },
        );
        if ready {
            ui.painter().text(
                pos2(rect.right() - 48., y),
                Align2::RIGHT_CENTER,
                if width >= 250. {
                    "Game loaded"
                } else {
                    "Loaded"
                },
                bold(11.),
                BLUE,
            );
        }
        let tip = pos2(rect.right() - 18., y);
        let stroke = Stroke::new(1.6, if hovered { INK } else { MUTED });
        ui.painter()
            .line_segment([tip - vec2(16., 0.), tip], stroke);
        ui.painter().line_segment([tip - vec2(5., 5.), tip], stroke);
        ui.painter()
            .line_segment([tip - vec2(5., -5.), tip], stroke);
        if response.clicked() {
            // Pointer activation isn't a persistent selection. Retain keyboard focus for return navigation.
            if response.clicked_by(PointerButton::Primary) {
                response.surrender_focus();
            }
            self.select_gallery_game(index, ctx);
        }
    }

    pub(super) fn gallery_setup(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.spacing_mut().item_spacing.y = 8.;
        let Some(game) = self
            .gallery
            .selected
            .and_then(|i| self.gallery.games.get(i))
            .cloned()
        else {
            return;
        };
        let short = ctx.content_rect().height() < 500.;
        ui.label(RichText::new(&game.title).font(bold(
            if short || (ui.available_width() < 360. && game.title.len() > 22) {
                22.
            } else {
                28.
            },
        )));
        let builtins = metadata::builtins();
        if !short {
            ui.add_space(8.);
            ui.label(
                RichText::new(if ui.available_width() < 300. {
                    "Player controls"
                } else {
                    "Choose who controls each player"
                })
                .font(bold(18.)),
            );
        }
        if short && ui.available_width() >= 600. {
            ui.columns(2, |columns| {
                for (slot, column) in columns.iter_mut().enumerate() {
                    self.gallery_player_choice(column, slot, &game);
                }
            });
        } else {
            for slot in 0..2 {
                self.gallery_player_choice(ui, slot, &game);
            }
        }
        ui.collapsing("Agent details", |ui| {
            let builtin = &builtins[game.models[self.gallery.model]];
            let manifest: serde_json::Value =
                serde_json::from_str(builtin.manifest).expect("bundled manifest");
            if game.models.len() > 1 {
                ComboBox::from_id_salt("gallery-model")
                    .width((ui.available_width() - 30.).max(160.))
                    .selected_text(manifest["name"].as_str().unwrap_or("Agent"))
                    .show_ui(ui, |ui| {
                        for (index, builtin_index) in game.models.iter().enumerate() {
                            let v: serde_json::Value =
                                serde_json::from_str(builtins[*builtin_index].manifest).unwrap();
                            if ui
                                .selectable_value(
                                    &mut self.gallery.model,
                                    index,
                                    v["name"].as_str().unwrap_or("Agent"),
                                )
                                .clicked()
                            {
                                ui.close();
                            }
                        }
                    });
            }

            let builtin = &builtins[game.models[self.gallery.model]];
            let current: serde_json::Value =
                serde_json::from_str(builtin.manifest).expect("bundled manifest");
            ui.label(metadata::scope(
                current["observation"].as_str().unwrap_or_default(),
            ));
            if game.experimental {
                ui.label(RichText::new("Experimental agent").color(ORANGE));
            }
            ui.label(builtin.notes);
        });
    }

    fn gallery_player_choice(&mut self, ui: &mut Ui, slot: usize, game: &Game) {
        ui.push_id(slot, |ui| {
            Frame::new()
                .fill(Color32::from_rgb(253, 252, 248))
                .stroke(Stroke::new(1., LINE))
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_min_width((ui.available_width() - 1.).max(0.));
                    if slot == 1 {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("PLAYER 2").font(bold(14.)).color(MUTED));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let mut disabled = self.gallery.modes[1] == Mode::Off;
                                if ui
                                    .add_sized(
                                        [135., 44.],
                                        Checkbox::new(&mut disabled, "No player 2"),
                                    )
                                    .changed()
                                {
                                    self.gallery.disable_player2(disabled);
                                }
                            });
                        });
                    } else {
                        ui.label(RichText::new("PLAYER 1").font(bold(14.)).color(MUTED));
                    }
                    if self.gallery.modes[slot] == Mode::Off {
                        ui.label(RichText::new("Play alone with Player 1.").font(bold(15.)));
                        ui.label(
                            RichText::new("Choose 1 PLAYER in the game’s menu.")
                                .size(13.)
                                .color(MUTED),
                        );
                        return;
                    }
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.;
                        let width = (ui.available_width() - 12.) / 3.;
                        for mode in [Mode::Human, Mode::Ai, Mode::Shared] {
                            if control_choice(ui, mode, self.gallery.modes[slot] == mode, width)
                                .clicked()
                            {
                                self.gallery.modes[slot] = mode;
                            }
                        }
                    });
                    match self.gallery.modes[slot] {
                        Mode::Human => {
                            ui.label(
                                RichText::new("You control this player.")
                                    .size(14.)
                                    .color(BLUE),
                            );
                        }
                        Mode::Ai => {
                            ui.label(
                                RichText::new("AI controls this player.")
                                    .size(14.)
                                    .color(ORANGE),
                            );
                        }
                        Mode::Shared => {
                            ui.label(
                                RichText::new("You + AI, one player")
                                    .font(bold(15.))
                                    .color(ACCENT),
                            );
                            let actions = metadata::actions(&game.hash);
                            let mask = self.gallery.ownership[slot];
                            ComboBox::from_id_salt("shared-preset")
                                .width(ui.available_width() - 30.)
                                .selected_text(match mask {
                                    1 => "AI handles A",
                                    3 => "AI handles A + B",
                                    _ => "AI handles B",
                                })
                                .show_ui(ui, |ui| {
                                    for (label, mask) in [
                                        ("AI handles B", 2),
                                        ("AI handles A", 1),
                                        ("AI handles A + B", 3),
                                    ] {
                                        if ui
                                            .selectable_value(
                                                &mut self.gallery.ownership[slot],
                                                mask,
                                                label,
                                            )
                                            .clicked()
                                        {
                                            ui.close();
                                        }
                                    }
                                });
                            let mask = self.gallery.ownership[slot];
                            let description = match mask {
                                1 => {
                                    format!("You: directions + {}. AI: {}.", actions[1], actions[0])
                                }
                                3 => {
                                    format!("You: directions. AI: {} + {}.", actions[0], actions[1])
                                }
                                _ => {
                                    format!("You: directions + {}. AI: {}.", actions[0], actions[1])
                                }
                            };
                            ui.label(RichText::new(description).size(13.).color(MUTED));
                            if game.title == "Contra" {
                                ui.label(
                                    RichText::new("Directions move and aim.")
                                        .size(13.)
                                        .color(MUTED),
                                );
                            }
                        }
                        Mode::Off => {}
                    }
                });
        });
    }

    pub(super) fn gallery_setup_action(&mut self, ui: &mut Ui, ctx: &Context) {
        let Some(game) = self
            .gallery
            .selected
            .and_then(|i| self.gallery.games.get(i))
            .cloned()
        else {
            return;
        };
        let builtins = metadata::builtins();
        let ready = self
            .session
            .as_ref()
            .is_some_and(|s| s.machine.cartridge_hash() == game.hash);
        ui.add_space(4.);
        let label = if ready {
            "Play with this setup"
        } else {
            "Choose game ROM"
        };
        let clicked = ui
            .add_sized(
                [ui.available_width(), 48.],
                Button::new(RichText::new(label).font(bold(17.)).color(Color32::WHITE))
                    .fill(ACCENT),
            )
            .clicked();
        if clicked {
            if ready {
                // Validate both ports on a separate runner before changing any assignment.
                let builtin = &builtins[game.models[self.gallery.model]];
                if let Err(e) = self.apply_gallery_setup(builtin, ctx) {
                    self.fail(e, ctx);
                } else {
                    self.toggle(ctx);
                }
            } else {
                self.queued_model = None;
                self.gallery.pending_rom = Some(game.hash.clone());
                self.pick(FileKind::Rom, ctx);
            }
        }
    }

    pub(super) fn finish_gallery_setup(&mut self, ctx: &Context) {
        if let Some(game) = self
            .gallery
            .selected
            .and_then(|i| self.gallery.games.get(i))
        {
            let builtin = &metadata::builtins()[game.models[self.gallery.model]];
            if let Err(e) = self.apply_gallery_setup(builtin, ctx) {
                self.fail(e, ctx);
            } else {
                self.toggle(ctx);
            }
        }
    }
    fn apply_gallery_setup(
        &mut self,
        builtin: &metadata::Builtin,
        ctx: &Context,
    ) -> Result<(), String> {
        let model = Model::new(builtin.manifest.into(), builtin.weights.to_vec())?;
        let modes = self.gallery.modes;
        let ai = modes.map(|m| matches!(m, Mode::Ai | Mode::Shared));
        let session = self.session.as_mut().ok_or("Open a game first")?;
        let mut agents = contra_agent::Agents::default();
        for (slot, enabled) in ai.iter().enumerate() {
            if *enabled {
                agents
                    .load(&session.machine, slot, &model.manifest, &model.weights)
                    .map_err(|e| e.to_string())?;
                agents
                    .set_ownership(
                        slot,
                        if modes[slot] == Mode::Shared {
                            self.gallery.ownership[slot]
                        } else {
                            contra_agent::control::GAMEPLAY
                        },
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        session.agents = agents;
        session.models = ai.map(|enabled| enabled.then(|| model.clone()));
        session.modes = modes;
        session.ownership = self.gallery.ownership;
        session.decisions = None;
        self.playback = None;
        if self.prefs.touch {
            let human_slot = modes
                .iter()
                .position(|m| matches!(m, Mode::Human | Mode::Shared));
            if let Some(slot) = human_slot {
                if self.prefs.devices[1 - slot] == Device::Touch {
                    self.prefs.devices.swap(0, 1);
                }
                self.touch_port = slot;
            }
        }
        self.clear(ctx);
        self.gallery.open = false;
        self.dialog = None;
        self.back_stack.clear();
        self.notice = if modes[1] == Mode::Off {
            "Solo controls are ready. Choose 1 PLAYER in the game's menu."
        } else {
            "Your controls are ready. Resume when you’re ready."
        }
        .into();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mismatching_rom_retry_keeps_the_gallery_requirement_until_dismissed() {
        let ctx = Context::default();
        let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let expected = app.gallery.games[0].hash.clone();
        app.gallery.pending_rom = Some(expected.clone());
        let bytes = nes_core::diagnostic::controller_rom();
        let error = app
            .prepare_game("Different game", bytes.clone())
            .err()
            .unwrap();
        app.fail_file(error, FileKind::Rom, &ctx);
        assert_eq!(app.gallery.pending_rom.as_deref(), Some(expected.as_str()));
        assert!(app.prepare_game("Different game", bytes.clone()).is_err());
        assert!(app.session.is_none());
        app.close_dialog(&ctx);
        assert!(app.gallery.pending_rom.is_none());
        assert!(app.prepare_game("Different game", bytes).is_ok());
    }
    #[test]
    fn solo_toggle_preserves_player1_and_restores_player2_choice() {
        let mut gallery = Gallery {
            modes: [Mode::Shared, Mode::Human],
            ..Gallery::default()
        };
        for previous in [Mode::Human, Mode::Ai, Mode::Shared] {
            gallery.modes[1] = previous;
            gallery.disable_player2(true);
            gallery.disable_player2(true);
            assert!(gallery.modes == [Mode::Shared, Mode::Off]);
            gallery.disable_player2(false);
            assert!(gallery.modes == [Mode::Shared, previous]);
        }
    }

    #[test]
    #[ignore = "requires the user's local Contra cartridge; never included in CI"]
    fn local_contra_solo_shared_disables_second_controller_and_agent() {
        let ctx = Context::default();
        let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let rom =
            std::fs::read(std::env::var("GALLERY_CONTRA_ROM").expect("set GALLERY_CONTRA_ROM"))
                .unwrap();
        app.session = Some(Session::new("Contra", rom).unwrap());
        app.gallery.modes = [Mode::Shared, Mode::Ai];
        app.apply_gallery_setup(&metadata::builtins()[0], &ctx)
            .unwrap();
        assert_eq!(app.session.as_ref().unwrap().agents.active(), [true, true]);
        app.session.as_mut().unwrap().pulse[1] = 12;
        app.gallery.disable_player2(true);
        app.prefs.touch = true;
        app.prefs.devices = [Device::Keyboard, Device::Touch];
        app.apply_gallery_setup(&metadata::builtins()[0], &ctx)
            .unwrap();
        assert_eq!(app.touch_port, 0);
        let session = app.session.as_mut().unwrap();
        assert!(session.modes == [Mode::Shared, Mode::Off]);
        assert_eq!(session.agents.active(), [true, false]);
        assert!(session.models[1].is_none());
        assert_eq!(session.pulse, [0, 0]);
        assert!(!session.running());
        // Use the actual one-player title-menu path, without patching game state.
        for _ in 0..600 {
            session.frame([0, 255]).unwrap();
        }
        session.frame([8, 255]).unwrap();
        for _ in 0..1200 {
            let state = contra_agent::adapter::observe(&session.machine);
            if state.gameplay && state.players[0].grounded {
                break;
            }
            session.frame([0, 255]).unwrap();
        }
        let state = contra_agent::adapter::observe(&session.machine);
        assert!(state.gameplay && state.players[0].active);
        assert!(!state.two_players && !state.players[1].active);
        for human in [0, 0x91, 0x42, 0x0c] {
            session.frame([human, 255]).unwrap();
            let d = session.decisions.as_ref().unwrap();
            assert_eq!(d[0].ai_groups, 2);
            assert_eq!(
                d[0].applied,
                contra_agent::control::arbitrate(human, d[0].proposal.unwrap_or(0), 2)
            );
            assert_eq!(d[1].human, 0);
            assert_eq!(d[1].applied, 0);
            assert!(d[1].proposal.is_none());
        }
    }

    #[test]
    #[ignore = "requires six local cartridges via SOLO_ROM_DIR"]
    fn local_gallery_solo_ai_and_shared_keep_p2_off_in_every_game() {
        let directory = std::path::PathBuf::from(std::env::var("SOLO_ROM_DIR").unwrap());
        for filename in [
            "Contra (U).nes",
            "Snow Bros (U).nes",
            "Bubble Bobble (U).nes",
            "Ice Climber (U).nes",
            "Double Dragon 2 - The Revenge (U).nes",
            "Teenage Mutant Ninja Turtles 2 (U).nes",
        ] {
            for mode in [Mode::Ai, Mode::Shared] {
                let ctx = Context::default();
                let mut app =
                    PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
                app.session = Some(
                    Session::new(filename, std::fs::read(directory.join(filename)).unwrap())
                        .unwrap(),
                );
                let hash = app.session.as_ref().unwrap().machine.cartridge_hash();
                let game = app.gallery.games.iter().find(|g| g.hash == hash).unwrap();
                let builtin = &metadata::builtins()[game.models[0]];
                let manifest: serde_json::Value = serde_json::from_str(builtin.manifest).unwrap();
                let definition =
                    contra_agent::registry::schema(manifest["observation"].as_str().unwrap())
                        .unwrap();
                app.gallery.modes = [mode, Mode::Ai];
                app.gallery.disable_player2(true);
                app.apply_gallery_setup(builtin, &ctx).unwrap();
                let session = app.session.as_mut().unwrap();
                assert_eq!(session.agents.active(), [true, false]);
                assert!(session.models[1].is_none());
                let mut found = false;
                'menu: for (frames, masks) in
                    definition.solo_menu.iter().copied().chain([(1200, [0, 0])])
                {
                    for _ in 0..frames {
                        let (phase, _) = contra_agent::registry::policy_observation(
                            &session.machine,
                            0,
                            definition.observation,
                        )
                        .unwrap();
                        if phase == contra_agent::control::Phase::SupportedGameplay {
                            found = true;
                            break 'menu;
                        }
                        session.frame([masks[0], 255]).unwrap();
                    }
                }
                assert!(found, "{filename}: solo gameplay never reached");
                for human in [0, 0x91, 0x42] {
                    session.frame([human, 255]).unwrap();
                    let d = session.decisions.as_ref().unwrap();
                    assert!(d[0].proposal.is_some(), "{filename}: P1 policy did not run");
                    assert_eq!(
                        d[0].applied,
                        contra_agent::control::arbitrate(
                            human,
                            d[0].proposal.unwrap(),
                            if mode == Mode::Ai { 243 } else { 2 }
                        )
                    );
                    assert_eq!(d[1].applied, 0);
                    assert_eq!(d[1].human, 0);
                    assert!(d[1].proposal.is_none());
                }
            }
        }
    }

    #[test]
    fn gallery_groups_actual_bundles_by_exact_game() {
        let gallery = Gallery::default();
        assert_eq!(gallery.games.len(), 6);
        assert_eq!(gallery.games[0].models.len(), 3);
        assert_eq!(
            gallery.games.iter().map(|g| g.models.len()).sum::<usize>(),
            metadata::builtins().len()
        );
        for game in &gallery.games {
            let artwork = image::load_from_memory(game.art).unwrap();
            assert_eq!((artwork.width(), artwork.height()), (960, 540));
            for index in &game.models {
                let model: serde_json::Value =
                    serde_json::from_str(metadata::builtins()[*index].manifest).unwrap();
                assert_eq!(model["rom_sha256"], game.hash);
            }
        }
    }
    #[test]
    fn gallery_file_validation_uses_real_identity() {
        let mut gallery = Gallery::default();
        let game = gallery.games[1].clone();
        gallery.pending_rom = Some(game.hash.clone());
        assert!(gallery.validate_file(&game.hash).is_ok());
        assert!(gallery.validate_file("different-revision").is_err());
    }

    #[test]
    fn gallery_file_selection_is_transactional_and_returns_to_setup() {
        let ctx = Context::default();
        let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let rom = nes_core::diagnostic::controller_rom();
        let original = Session::new("Original", rom.clone()).unwrap();
        let hash = original.machine.cartridge_hash();
        app.session = Some(original);
        app.gallery.open = true;
        app.gallery.pending_rom = Some("wrong game".into());
        assert!(
            app.file(FileKind::Rom, "Different", rom.clone(), &ctx)
                .is_err()
        );
        assert_eq!(app.session.as_ref().unwrap().info.title, "Original");
        assert!(app.replacement.is_none());

        app.gallery.pending_rom = Some(hash);
        app.file(FileKind::Rom, "Replacement", rom, &ctx).unwrap();
        assert_eq!(app.session.as_ref().unwrap().info.title, "Original");
        assert!(app.dialog == Some(Dialog::Confirm));
        app.confirm(&ctx);
        assert_eq!(app.session.as_ref().unwrap().info.title, "Replacement");
        assert!(app.dialog == Some(Dialog::GallerySetup));
        assert!(app.gallery.pending_rom.is_none());
        assert!(app.gallery.open);
        assert!(!app.running());
    }

    #[test]
    fn rejected_gallery_agent_never_replaces_existing_controls() {
        let ctx = Context::default();
        let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        app.session =
            Some(Session::new("Homebrew", nes_core::diagnostic::controller_rom()).unwrap());
        app.gallery.modes = [Mode::Ai; 2];
        let before = app.session.as_ref().unwrap().machine.save_state();
        assert!(
            app.apply_gallery_setup(&metadata::builtins()[0], &ctx)
                .is_err()
        );
        let s = app.session.as_ref().unwrap();
        assert!(s.modes == [Mode::Human; 2]);
        assert!(s.models.iter().all(Option::is_none));
        assert_eq!(s.agents.active(), [false; 2]);
        assert_eq!(s.machine.save_state(), before);
    }

    #[test]
    fn canceled_gallery_file_response_cannot_replace_the_session() {
        let ctx = Context::default();
        let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        app.session =
            Some(Session::new("Original", nes_core::diagnostic::controller_rom()).unwrap());
        app.gallery.pending_rom = Some(app.gallery.games[0].hash.clone());
        let id = app.operation(Operation::File, "Opening file");
        app.cancel_pending();
        app.tx
            .send((
                id,
                Event::File(
                    FileKind::Rom,
                    Some(("Late file".into(), nes_core::diagnostic::controller_rom())),
                ),
            ))
            .unwrap();
        app.results(&ctx);
        assert_eq!(app.session.as_ref().unwrap().info.title, "Original");
        assert!(app.replacement.is_none());
        assert!(app.gallery.pending_rom.is_none());
    }

    #[test]
    #[ignore = "requires the user's local Contra cartridge; never included in CI"]
    fn local_contra_gallery_roles_and_touch_routing() {
        let ctx = Context::default();
        let mut app = PlayerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let path = std::env::var("GALLERY_CONTRA_ROM").expect("set GALLERY_CONTRA_ROM");
        app.gallery.open = true;
        app.gallery.selected = Some(0);
        app.gallery.pending_rom = Some(app.gallery.games[0].hash.clone());
        app.file(FileKind::Rom, "Contra", std::fs::read(path).unwrap(), &ctx)
            .unwrap();
        assert!(app.dialog.is_none());
        assert!(app.running());
        assert!(app.session.as_ref().unwrap().modes == [Mode::Human, Mode::Ai]);
        assert!(app.session.as_ref().unwrap().models[1].is_some());
        app.pause(&ctx);
        app.prefs.touch = true;
        app.prefs.devices = [Device::Touch, Device::Keyboard];
        for modes in [
            [Mode::Human, Mode::Ai],
            [Mode::Ai, Mode::Human],
            [Mode::Ai; 2],
            [Mode::Shared, Mode::Ai],
            [Mode::Ai, Mode::Shared],
            [Mode::Shared; 2],
            [Mode::Human; 2],
        ] {
            app.gallery.modes = modes;
            app.gallery.ownership = [2, 3];
            app.apply_gallery_setup(&metadata::builtins()[0], &ctx)
                .unwrap();
            let s = app.session.as_ref().unwrap();
            assert!(s.modes == modes);
            assert_eq!(
                s.agents.active(),
                modes.map(|mode| matches!(mode, Mode::Ai | Mode::Shared))
            );
            assert!(!s.running());
            if let Some(slot) = modes
                .iter()
                .position(|m| matches!(m, Mode::Human | Mode::Shared))
            {
                assert_eq!(app.touch_port, slot);
            }
            assert_eq!(s.ownership, [2, 3]);
            assert!(!app.gallery.open);
            let s = app.session.as_mut().unwrap();
            s.contra_two_player().unwrap();
            s.frame([0x91, 0x42]).unwrap();
            let decisions = s.decisions.as_ref().unwrap();
            for (slot, decision) in decisions.iter().enumerate() {
                let mask = match modes[slot] {
                    Mode::Shared => [2, 3][slot],
                    Mode::Ai => contra_agent::control::GAMEPLAY,
                    _ => 0,
                };
                assert_eq!(decision.ai_groups, mask);
                assert_eq!(
                    decision.applied,
                    contra_agent::control::arbitrate(
                        decision.human,
                        decision.proposal.unwrap_or(0),
                        mask
                    )
                );
            }
            s.pause();
        }
    }
}
