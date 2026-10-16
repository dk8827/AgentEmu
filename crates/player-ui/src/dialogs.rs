use super::*;
fn action(ui: &mut Ui, label: &str, primary: bool, enabled: bool) -> bool {
    let w = ui.available_width();
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(label).font(bold(17.)).color(if primary {
            Color32::WHITE
        } else {
            INK
        }))
        .fill(if primary { ACCENT } else { PAPER })
        .stroke(Stroke::new(1., if primary { ACCENT } else { INK }))
        .min_size(vec2(w, 46.)),
    )
    .clicked()
}
fn choice(ui: &mut Ui, selected: bool, label: impl Into<String>) -> Response {
    ui.add(
        Button::new(
            RichText::new(label.into())
                .font(bold(15.))
                .color(if selected { Color32::WHITE } else { INK }),
        )
        .fill(if selected { INK } else { PAPER })
        .stroke(Stroke::new(1., LINE))
        .min_size(vec2(65., 44.)),
    )
}
fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(14.).color(MUTED));
}
fn percent_slider(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
    label: &str,
) -> Response {
    ui.label(label);
    ui.spacing_mut().slider_width = (ui.available_width() - 90.).clamp(40., 360.);
    let response = ui.add(
        Slider::new(value, range)
            .step_by(step)
            .fixed_decimals(0)
            .suffix("%"),
    );
    response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), f64::from(*value), label));
    if response.has_focus() && !ui.clip_rect().contains_rect(response.rect) {
        response.scroll_to_me_animation(Some(Align::Center), egui::style::ScrollAnimation::none());
    }
    response
}
impl PlayerApp {
    pub(super) fn dialogs(&mut self, ctx: &Context) {
        if let Ok(mut bridge) = self.accessibility.lock() {
            bridge.modal = None;
        }
        if self.transitioning() {
            Modal::new(Id::new("runtime-transition")).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(self.transport_label());
                });
            });
            return;
        }
        if let Some(p) = self.requests.pending() {
            let label = p.label.clone();
            Modal::new(Id::new("pending"))
                .frame(
                    Frame::new()
                        .fill(PAPER)
                        .stroke(Stroke::new(1., INK))
                        .inner_margin(24),
                )
                .show(ctx, |ui| {
                    ui.set_width(280.);
                    ui.heading(label);
                    ui.spinner();
                    caption(ui, "Your game stays paused.");
                    if action(ui, "Cancel", false, true) {
                        self.cancel_pending();
                    }
                });
            return;
        }
        let Some(dialog) = self.dialog else { return };
        let screen = ctx.content_rect();
        let short_gallery = dialog == Dialog::GallerySetup && screen.height() < 500.;
        let padding = if short_gallery {
            12
        } else if screen.width() < 600. {
            16
        } else {
            26
        };
        let width = (screen.width() - 2. * padding as f32 - 24.)
            .max(120.)
            .min(if short_gallery { 760. } else { 570. });
        let id = Id::new(("player-dialog-v2", dialog));
        Modal::new(id)
            .area(Modal::default_area(id).default_size(vec2(width, screen.height() - 48.)))
            .frame(
                Frame::new()
                    .fill(PAPER)
                    .stroke(Stroke::new(1., INK))
                    .inner_margin(padding),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                // Areas remember their previous size; allow a later, longer
                // confirmation to grow rather than clipping its Cancel button.
                ui.set_max_height(screen.height() - 2. * padding as f32 - 24.);
                let title = match dialog {
                    Dialog::Menu => "Game menu".into(),
                    Dialog::GallerySetup => "Play with AI".into(),
                    Dialog::Player(p) => format!("Player {}", p + 1),
                    Dialog::Agents(p) => format!("Choose an agent · P{}", p + 1),
                    Dialog::Controls(p) => format!("Keyboard · P{}", p + 1),
                    Dialog::Settings => "Settings".into(),
                    Dialog::Saves => "Saved games".into(),
                    Dialog::Touch => "Touch controls".into(),
                    Dialog::Help => "Help & about".into(),
                    Dialog::Workshop => "Training tools".into(),
                    Dialog::Confirm => "Confirm change".into(),
                    Dialog::Error => self.error_title().into(),
                };
                if let Ok(mut bridge) = self.accessibility.lock() {
                    bridge.modal = Some(ui.unique_id().value().to_string());
                }
                ctx.accesskit_node_builder(ui.unique_id(), |node| {
                    node.set_role(egui::accesskit::Role::Dialog);
                    node.set_label(title.clone());
                });
                ui.horizontal(|ui| {
                    ui.allocate_ui(vec2((ui.available_width() - 78.).max(40.), 40.), |ui| {
                        ui.add(
                            Label::new(RichText::new(title).font(bold(if screen.width() < 260. {
                                18.
                            } else if screen.width() < 600. {
                                22.
                            } else {
                                26.
                            })))
                            .truncate(),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add_sized([66., 40.], Button::new("Done")).clicked() {
                            self.close_dialog(ctx);
                        }
                    });
                });
                ui.separator();
                if !matches!(
                    dialog,
                    Dialog::Menu | Dialog::GallerySetup | Dialog::Error | Dialog::Confirm
                ) && ui
                    .add_sized([100., 36.], Button::new("‹ Back").frame(false))
                    .clicked()
                {
                    self.back(ctx);
                }
                if matches!(dialog, Dialog::Agents(_)) {
                    ui.set_min_height((screen.height() - 150.).clamp(200., 650.));
                }
                ScrollArea::vertical()
                    .id_salt(dialog)
                    .max_height(
                        (screen.height()
                            - if short_gallery { 132. } else { 235. }
                            - if dialog == Dialog::GallerySetup {
                                60.
                            } else {
                                0.
                            })
                        .max(85.),
                    )
                    .show(ui, |ui| match dialog {
                        Dialog::Menu => self.menu_dialog(ui, ctx),
                        Dialog::GallerySetup => self.gallery_setup(ui, ctx),
                        Dialog::Player(slot) => self.player_dialog(ui, slot, ctx),
                        Dialog::Agents(slot) => self.agent_dialog(ui, slot, ctx),
                        Dialog::Controls(slot) => self.controls_dialog(ui, slot, ctx),
                        Dialog::Settings => self.settings_dialog(ui, ctx),
                        Dialog::Saves => self.saves_dialog(ui, ctx),
                        Dialog::Touch => self.touch_dialog(ui, ctx),
                        Dialog::Help => self.help_dialog(ui, ctx),
                        Dialog::Workshop => self.workshop_dialog(ui, ctx),
                        Dialog::Confirm => self.confirm_dialog(ui, ctx),
                        Dialog::Error => self.error_dialog(ui, ctx),
                    });
                if dialog == Dialog::GallerySetup && self.dialog == Some(dialog) {
                    ui.separator();
                    self.gallery_setup_action(ui, ctx);
                }
            });
    }
    fn menu_dialog(&mut self, ui: &mut Ui, ctx: &Context) {
        let loaded = self.session.is_some();
        if loaded && action(ui, "Resume game", true, true) {
            self.toggle(ctx);
        }
        for (label, d) in [
            ("Players & controls", Dialog::Player(0)),
            ("Saved games", Dialog::Saves),
            ("Settings", Dialog::Settings),
            ("Help & about", Dialog::Help),
        ] {
            if action(
                ui,
                label,
                false,
                loaded || matches!(d, Dialog::Help | Dialog::Settings),
            ) {
                self.open(d, ctx);
            }
        }
        ui.separator();
        if action(ui, "AI games", false, true) {
            self.open_gallery(ctx);
        }
        if action(ui, "Choose game ROM", false, true) {
            self.pick(FileKind::Rom, ctx);
        }
        if loaded
            && ui
                .add_sized(
                    [ui.available_width(), 44.],
                    Button::new("Restart game…").frame(false),
                )
                .clicked()
        {
            self.replacement = Some(Replacement::Restart);
            self.open(Dialog::Confirm, ctx);
        }
        if !self.notice.is_empty() {
            caption(ui, &self.notice);
        }
    }

    fn player_dialog(&mut self, ui: &mut Ui, slot: usize, ctx: &Context) {
        let players = self
            .session
            .as_ref()
            .map_or(2, |s| s.info.players)
            .max(if self.extra_port { 2 } else { 1 });
        if players > 1 {
            ui.horizontal(|ui| {
                for p in 0..players {
                    if choice(ui, slot == p, format!("Player {}", p + 1)).clicked() {
                        self.dialog = Some(Dialog::Player(p));
                    }
                }
            });
        } else {
            caption(ui, "This game uses one player.");
        }
        let mode = self.session.as_ref().map_or(Mode::Human, |s| s.modes[slot]);
        ui.label(RichText::new("Who controls this player?").font(bold(19.)));
        let has_agent = self.has_bundled_agent()
            || self
                .session
                .as_ref()
                .is_some_and(|s| s.models[slot].is_some());
        if !has_agent {
            caption(
                ui,
                "No AI agent is included for this game yet. Human play is available.",
            );
        }
        ui.horizontal_wrapped(|ui| {
            for m in [Mode::Human, Mode::Ai, Mode::Shared, Mode::Off] {
                if ui
                    .add_enabled_ui(has_agent || !matches!(m, Mode::Ai | Mode::Shared), |ui| {
                        control_choice(
                            ui,
                            m,
                            m == mode,
                            if ui.max_rect().width() >= 450. {
                                130.
                            } else {
                                100.
                            },
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.mode(slot, m, ctx)
                }
            }
        });
        if players > 1 {
            caption(
                ui,
                "Choose the matching 1 or 2 player mode in the game’s own menu.",
            );
        }
        if matches!(mode, Mode::Ai | Mode::Shared) {
            ui.separator();
            if let Some(model) = self.session.as_ref().and_then(|s| s.models[slot].as_ref()) {
                ui.label(RichText::new(&model.name).font(bold(18.)));
                caption(ui, &model.scope);
            }
            if action(ui, "Change agent", false, true) {
                self.desired_mode[slot] = mode;
                self.open(Dialog::Agents(slot), ctx)
            }
        }
        if mode == Mode::Shared {
            self.ownership_dialog(ui, slot, ctx);
        }
        if matches!(mode, Mode::Ai | Mode::Shared) && action(ui, "Take full control", false, true) {
            self.mode(slot, Mode::Human, ctx)
        }
        ui.separator();
        ui.label(RichText::new("Input device").font(bold(18.)));
        let mut selected = self.prefs.devices[slot].clone();
        let old = selected.clone();
        ui.horizontal_wrapped(|ui| {
            if choice(ui, selected == Device::Keyboard, "Keyboard").clicked() {
                selected = Device::Keyboard;
            }
            if choice(ui, selected == Device::Touch, "Touch").clicked() {
                selected = Device::Touch;
            }
        });
        for (id, name) in self.host.gamepads() {
            if choice(ui, selected == Device::Gamepad(id), name).clicked() {
                selected = Device::Gamepad(id);
            }
        }
        if selected != old {
            if matches!(selected, Device::Touch | Device::Gamepad(_))
                && self.prefs.devices[1 - slot] == selected
            {
                self.notice = format!(
                    "That device belongs to Player {}. Change that player's device first.",
                    2 - slot
                );
            } else {
                self.clear(ctx);
                self.prefs.devices[slot] = selected.clone();
                if selected == Device::Touch {
                    self.touch_port = slot;
                    self.prefs.touch = true;
                }
            }
        }
        if action(ui, "Customize keyboard", false, true) {
            self.open(Dialog::Controls(slot), ctx)
        }
        caption(
            ui,
            "Pair gamepads in your device settings, then press a button to make them available here.",
        );
        caption(
            ui,
            "NES Start / Select stay with you. Turning a player off stops their input; it does not remove a character already in the game.",
        );
        if !self.notice.is_empty() {
            caption(ui, &self.notice);
        }
    }
    fn ownership_dialog(&mut self, ui: &mut Ui, slot: usize, ctx: &Context) {
        ui.separator();
        ui.label(RichText::new("Share one controller").font(bold(21.)));
        let Some(s) = &self.session else { return };
        let mut mask = s.ownership[slot];
        let old = mask;
        let hash = s.machine.cartridge_hash();
        let actions = metadata::actions(&hash);
        ui.horizontal_wrapped(|ui| {
            for (label, m) in [
                ("AI handles B", 2),
                ("AI handles A", 1),
                ("AI handles A+B", 3),
            ] {
                if choice(ui, mask == m, label).clicked() {
                    mask = m;
                }
            }
        });
        if hash == contra_agent::adapter::ROM_SHA256 {
            caption(
                ui,
                "In Contra, directions control aiming as well as movement. A jumps; B fires.",
            );
        } else if hash == contra_agent::snow::ROM_SHA256 {
            caption(
                ui,
                "In Snow Bros., B throws snow or launches a snowball depending on the situation.",
            );
        }
        ui.collapsing("Customize button ownership", |ui| {
            Grid::new("ownership")
                .num_columns(3)
                .spacing(vec2(8., 12.))
                .show(ui, |ui| {
                    for (label, g) in [
                        ("Left / Right".to_string(), 192),
                        ("Up / Down".into(), 48),
                        (format!("A · {}", actions[0]), 1),
                        (format!("B · {}", actions[1]), 2),
                    ] {
                        ui.label(label);
                        if choice(ui, mask & g == 0, "Human").clicked() {
                            mask &= !g;
                        }
                        if choice(ui, mask & g == g, "AI").clicked() {
                            mask |= g;
                        }
                        ui.end_row();
                    }
                    ui.label("Start / Select");
                    ui.label(RichText::new("Always human").color(BLUE));
                    ui.end_row();
                });
        });
        if mask != old {
            self.clear(ctx);
            if let Some(s) = &mut self.session
                && let Err(e) = s.set_ownership(slot, mask)
            {
                self.fail(e, ctx)
            }
        }
    }
    fn controls_dialog(&mut self, ui: &mut Ui, slot: usize, ctx: &Context) {
        caption(
            ui,
            "Select a binding, then press a key. Escape cancels. Each button needs its own key.",
        );
        Grid::new("keyboard-bindings")
            .num_columns(2)
            .min_col_width(70.)
            .spacing(vec2(16., 8.))
            .show(ui, |ui| {
                for (b, label) in crate::input::BUTTONS.iter().enumerate() {
                    ui.label(*label);
                    let key = if self.remap == Some((slot, b)) {
                        "Press a key…".into()
                    } else {
                        self.prefs.keys[slot][b].name().to_string()
                    };
                    if ui.add_sized([150., 44.], Button::new(key)).clicked() {
                        self.remap = Some((slot, b));
                    }
                    ui.end_row();
                }
            });
        if !self.notice.is_empty() {
            caption(ui, &self.notice);
        }
        if action(ui, "Reset this player’s keyboard", false, true) {
            let defaults = Preferences::default().keys[slot];
            if defaults
                .iter()
                .any(|key| self.prefs.keys[1 - slot].contains(key))
            {
                self.notice =
                    "Another player uses one of these default keys. Change that binding first."
                        .into();
            } else {
                self.clear(ctx);
                self.prefs.keys[slot] = defaults;
                self.remap = None;
                self.notice = "Default keys restored for this player.".into();
            }
        }
    }

    fn agent_dialog(&mut self, ui: &mut Ui, slot: usize, ctx: &Context) {
        let hash = self.session.as_ref().map(|s| s.machine.cartridge_hash());
        caption(
            ui,
            "Choose an included agent. The game stays paused while you change controls.",
        );
        ui.horizontal(|ui| {
            ui.add_sized(
                [ui.available_width() - 65., 44.],
                TextEdit::singleline(&mut self.search).hint_text("Search agents"),
            );
            if ui
                .add_sized([56., 44.], Button::new("Clear").frame(false))
                .clicked()
            {
                self.search.clear();
            }
        });
        let search = self.search.to_lowercase();
        let mut compatible = 0;
        let mut found = 0;
        for builtin in metadata::builtins() {
            let v: serde_json::Value =
                serde_json::from_str(builtin.manifest).expect("bundled manifest");
            if hash
                .as_ref()
                .is_some_and(|h| v["rom_sha256"].as_str() != Some(h))
            {
                continue;
            }
            compatible += 1;
            let name = v["name"].as_str().unwrap_or("Agent");
            if !name.to_lowercase().contains(&search) {
                continue;
            }
            found += 1;
            let current = self
                .session
                .as_ref()
                .and_then(|s| s.models[slot].as_ref())
                .is_some_and(|m| m.name == name);
            let experimental = builtin.experimental;
            Frame::new()
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1., LINE))
                .inner_margin(16)
                .show(ui, |ui| {
                    ui.label(RichText::new(name).font(bold(19.)));
                    caption(
                        ui,
                        if current {
                            "Selected"
                        } else if experimental {
                            "Experimental"
                        } else if compatible == 1 {
                            "Recommended"
                        } else {
                            "Included agent"
                        },
                    );
                    caption(ui, metadata::scope(v["observation"].as_str().unwrap_or("")));
                    ui.collapsing("About this agent", |ui| {
                        caption(ui, builtin.notes);
                    });
                    if action(
                        ui,
                        if current {
                            "Keep this agent"
                        } else {
                            "Use this agent"
                        },
                        false,
                        true,
                    ) {
                        match Model::new(builtin.manifest.into(), builtin.weights.to_vec())
                            .and_then(|m| self.apply_model(slot, m, ctx))
                        {
                            Ok(()) => {}
                            Err(e) => self.fail(e, ctx),
                        }
                    }
                });
        }
        if found == 0 {
            caption(
                ui,
                if compatible == 0 {
                    "No AI agent is included for this game yet. You can still play with human controls."
                } else {
                    "No agents match your search. Clear the search to see all included agents for this game."
                },
            );
        }
    }

    fn settings_dialog(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.label(RichText::new("Sound").font(bold(20.)));
        if ui.checkbox(&mut self.prefs.muted, "Mute sound").changed() {
            self.host.audio_clear();
        }
        let mut volume = self.prefs.volume * 100.;
        if percent_slider(ui, &mut volume, 0.0..=100., 1., "Volume").changed() {
            self.prefs.volume = volume / 100.;
        }
        if let Some(e) = &self.host.audio_error {
            caption(ui, e);
        }
        caption(ui, "Sound resumes with the game.");
        ui.separator();
        ui.label(RichText::new("Display").font(bold(20.)));
        ui.checkbox(&mut self.prefs.aspect, "TV proportions (4:3)");
        ui.checkbox(&mut self.prefs.integer_scale, "Crisp whole-pixel scaling");
        let mut scale = self.prefs.ui_scale * 100.;
        if percent_slider(ui, &mut scale, 80.0..=150., 10., "Interface size").changed() {
            self.prefs.ui_scale = scale / 100.;
            ctx.set_zoom_factor(self.prefs.ui_scale);
        }
        if action(ui, "Toggle fullscreen", false, true) {
            platform::fullscreen(ctx);
        }
        if action(ui, "Touch layout", false, true) {
            self.open(Dialog::Touch, ctx);
        }
        ui.collapsing("Advanced", |ui| {
            if self.session.as_ref().is_some_and(|s| s.info.players == 1)
                && ui
                    .checkbox(
                        &mut self.extra_port,
                        "Enable controller 2 for special game commands",
                    )
                    .changed()
            {
                self.clear(ctx);
                if let Some(s) = &mut self.session {
                    let _ = s.set_mode(
                        1,
                        if self.extra_port {
                            Mode::Human
                        } else {
                            Mode::Off
                        },
                    );
                }
            }
            if let Some(s) = &self.session
                && s.rom.len() >= 16
                && ((s.rom[6] >> 4) | (s.rom[7] & 0xf0)) == 4
            {
                let mut alternate = s.alternate;
                if ui
                    .checkbox(&mut alternate, "MMC3A counter (restarts game)")
                    .changed()
                {
                    self.replacement = Some(Replacement::Counter(alternate));
                    self.open(Dialog::Confirm, ctx);
                }
            }
            if action(
                ui,
                "Advance one frame",
                false,
                self.session.is_some() && self.playback.is_none(),
            ) {
                self.clear(ctx);
                let result = self.session.as_mut().unwrap().frame([0; 2]);
                if let Err(e) = result {
                    self.fail(e, ctx)
                } else {
                    self.upload_frame(ctx)
                }
            }
            ui.checkbox(&mut self.advanced, "Show machine report");
            if self.advanced
                && let Some(s) = &self.session
            {
                ui.monospace(s.machine.diagnostic_report());
            }
        });
    }

    fn saves_dialog(&mut self, ui: &mut Ui, ctx: &Context) {
        let loaded = self.session.is_some();
        caption(
            ui,
            "Slots stay on this device after closing. Open the same ROM to load them again. Export a backup before clearing browser data.",
        );
        if !self.notice.is_empty() {
            ui.label(RichText::new(&self.notice).color(BLUE));
        }
        for slot in 0..3 {
            let width = ui.available_width() - 24.;
            Frame::new()
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1., LINE))
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_min_width(width);
                    ui.horizontal(|ui| {
                        if let Some(texture) = self.save_textures.get(&slot) {
                            ui.image((texture.id(), vec2(64., 60.)));
                        }
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "Slot {}{}",
                                    slot + 1,
                                    if slot == 0 { " · Quick save" } else { "" }
                                ))
                                .font(bold(18.)),
                            );
                            caption(
                                ui,
                                self.saves[slot]
                                    .as_ref()
                                    .map_or("Empty", |s| s.saved_at.as_str()),
                            );
                        });
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                loaded,
                                Button::new(if self.saves[slot].is_some() {
                                    "Overwrite…"
                                } else {
                                    "Save here"
                                }),
                            )
                            .clicked()
                        {
                            if self.saves[slot].is_some() {
                                self.replacement = Some(Replacement::SaveSlot(slot));
                                self.open(Dialog::Confirm, ctx);
                            } else {
                                self.store_save(slot, ctx);
                            }
                        }
                        if ui
                            .add_enabled(self.saves[slot].is_some(), Button::new("Load…"))
                            .clicked()
                        {
                            match self.saves[slot].as_ref().unwrap().bytes() {
                                Ok(bytes) => {
                                    self.replacement = Some(Replacement::State(bytes));
                                    self.open(Dialog::Confirm, ctx);
                                }
                                Err(e) => self.fail(e, ctx),
                            }
                        }
                    });
                });
        }
        ui.separator();
        if action(ui, "Export save file", false, loaded) {
            self.save_file(ctx);
        }
        if action(ui, "Import save file…", false, loaded) {
            self.pick(FileKind::State, ctx);
        }
        ui.collapsing("Replays", |ui| {
            caption(
                ui,
                "Watch recorded controller inputs. Your controls are disabled during playback.",
            );
            if action(ui, "Open replay…", false, loaded) {
                self.pick(FileKind::Replay, ctx);
            }
            if let Some(p) = &self.playback {
                ui.add(
                    ProgressBar::new(p.index as f32 / p.replay.inputs.len() as f32)
                        .text("Replay progress"),
                );
                caption(ui, &self.replay_status);
                if action(ui, "Exit replay", false, true) {
                    self.playback = None;
                    self.replay_status.clear();
                    self.clear(ctx);
                }
            }
        });
    }

    fn touch_dialog(&mut self, ui: &mut Ui, ctx: &Context) {
        caption(
            ui,
            "These settings apply in portrait and landscape. Touch controls one player at a time.",
        );
        if ui
            .checkbox(&mut self.prefs.touch, "Show touch controls")
            .changed()
        {
            self.clear(ctx);
        }
        if ui
            .checkbox(&mut self.prefs.left_handed, "Left-handed layout")
            .changed()
        {
            self.clear(ctx);
        }
        let mut scale = self.prefs.touch_scale * 100.;
        if percent_slider(ui, &mut scale, 80.0..=110., 5., "Control size").changed() {
            self.prefs.touch_scale = scale / 100.;
            self.clear(ctx);
        }
        self.touch_preview(ui);
        let players = self.visible_players();
        ui.horizontal_wrapped(|ui| {
            for p in 0..players {
                if ui
                    .selectable_label(self.touch_port == p, format!("Touch: Player {}", p + 1))
                    .clicked()
                {
                    if self.prefs.devices[1 - p] == Device::Touch {
                        self.notice = format!(
                            "Assign Player {} a keyboard or gamepad before moving touch here.",
                            2 - p
                        )
                    } else {
                        self.touch_port = p;
                        self.prefs.devices[p] = Device::Touch;
                        self.clear(ctx);
                    }
                }
            }
        });
        caption(ui, &self.notice);
        caption(
            ui,
            "Hold and slide on the D-pad for diagonals. A+B holds both buttons; it is not turbo. Start / Select below the deck can be routed to either player without moving the deck.",
        );
        if action(ui, "Reset touch layout", false, true) {
            self.prefs.left_handed = false;
            self.prefs.touch_scale = 1.;
            self.clear(ctx);
        }
    }
    fn confirm_dialog(&mut self, ui: &mut Ui, ctx: &Context) {
        let description = match &self.replacement {
            Some(Replacement::SaveSlot(slot)) => format!(
                "Overwrite slot {} with your current progress? The previous save in this slot will be replaced.", slot + 1
            ),
            Some(Replacement::Game(s)) => format!(
                "Open {}? This replaces the current game.", s.info.title
            ),
            Some(Replacement::State(_)) => "Load this save? Progress since that save will be replaced.".into(),
            Some(Replacement::Replay(..)) => "Start this replay? It replaces the current state and controls both ports with recorded input.".into(),
            Some(Replacement::Counter(_)) => "Restart with a different MMC3 IRQ counter revision? Unsaved progress will be replaced.".into(),
            _ => "Restart the game? Current unsaved progress will be replaced.".into(),
        };
        ui.label(description);
        caption(
            ui,
            "Export a backup first if you want to keep your current progress.",
        );
        if action(ui, "Export current save", false, self.session.is_some()) {
            self.save_file(ctx)
        }
        if action(
            ui,
            match self.replacement {
                Some(Replacement::SaveSlot(_)) => "Overwrite save",
                Some(Replacement::State(_)) => "Load save",
                Some(Replacement::Game(_)) => "Open game",
                Some(Replacement::Replay(..)) => "Open replay",
                _ => "Restart game",
            },
            true,
            true,
        ) {
            self.confirm(ctx)
        }
        if action(ui, "Cancel", false, true) {
            self.replacement = None;
            self.gallery.pending_rom = None;
            self.dialog = self.back_stack.pop();
            self.clear(ctx);
        }
    }
    fn error_dialog(&mut self, ui: &mut Ui, ctx: &Context) {
        let file = self.error_file;
        if matches!(file, Some(FileKind::Rom)) {
            if let Some(expected) = &self.gallery.pending_rom {
                let game = metadata::identify(expected, "Selected game");
                ui.label(format!(
                    "This agent needs {} ({}, {}). Choose its matching .nes ROM.",
                    game.title, game.region, game.revision
                ));
                caption(
                    ui,
                    "Your current game has not changed. For human play, return to Player and use Open ROM.",
                );
            } else {
                ui.label("This file could not be opened as a supported NES game. Choose an uncompressed .nes ROM.");
            }
            if action(ui, "Choose another ROM", true, true) {
                self.pick(FileKind::Rom, ctx);
            }
        } else if matches!(
            file,
            Some(FileKind::State | FileKind::Replay | FileKind::Report)
        ) {
            ui.label("This file could not be loaded. Your current game has not been replaced.");
            if action(ui, "Choose another file", true, true) {
                self.pick(file.unwrap(), ctx);
            }
        } else {
            ui.label(&self.error);
            let modes = self.session.as_ref().map_or([Mode::Human; 2], |s| s.modes);
            for (slot, m) in modes.iter().enumerate() {
                if matches!(m, Mode::Ai | Mode::Shared)
                    && action(
                        ui,
                        &format!("Player {} · Take full control", slot + 1),
                        true,
                        true,
                    )
                {
                    self.mode(slot, Mode::Human, ctx);
                    self.error.clear();
                    self.close_dialog(ctx);
                }
            }
            if self.recovery.is_some() && action(ui, "Choose input device", true, true) {
                self.recover_input_device(ctx);
            }
            if action(ui, "Saved games", false, self.session.is_some()) {
                self.open(Dialog::Saves, ctx);
            }
            if action(ui, "Restart game…", false, self.session.is_some()) {
                self.replacement = Some(Replacement::Restart);
                self.open(Dialog::Confirm, ctx);
            }
        }
        if file.is_some() {
            ui.collapsing("Technical details", |ui| {
                ui.monospace(&self.error);
            });
        }
        if action(
            ui,
            if self.session.is_some() {
                "Return to paused game"
            } else {
                "Back"
            },
            false,
            true,
        ) {
            self.close_dialog(ctx);
        }
    }

    fn help_dialog(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.label(RichText::new("Start playing").font(bold(20.)));
        ui.label("Open a .nes ROM, press Play, then use NES Start to begin. Some games use Select or directions to choose 1 or 2 players.");
        ui.label(
            "Pause / Resume control this app. NES Start and Select are buttons inside the game.",
        );
        ui.collapsing("Keyboard shortcuts",|ui|{
            ui.label("Player 1: WASD · J jump/A · L action/B · Enter Start · Space Select.");
            ui.label("Player 2: arrows · comma A · period B · Backspace Start · 0 Select.");
            ui.label("Escape: game menu. F5: save to slot 1. F8: load slot 1. F11: fullscreen. Keys can be customized in Players & controls.");
        });
        ui.collapsing("AI & Shared control",|ui|{ui.label("AI controls a player. Shared lets you and an included agent control different buttons of one player. You always handle NES Start / Select. Agents cover specific sections; the game pauses when you need to take over.");});
        ui.collapsing("Saves & privacy",|ui|{ui.label("ROMs stay on your device. Save slots and cartridge battery saves are stored locally. Clearing browser data removes local saves. Export save files for a portable backup. This version uses bundled agents and does not upload or download models.");});
        ui.collapsing("Light gun games",|ui|{ui.label("In Duck Hunt, click or tap the game to aim and fire. Use NES Start / Select for its menu.");});
        ui.separator();
        ui.label(RichText::new("AgentEmu").font(bold(20.)));
        caption(
            ui,
            concat!(
                "Version ",
                env!("CARGO_PKG_VERSION"),
                " · Rust / WebAssembly"
            ),
        );
        ui.hyperlink_to(
            "Source & documentation",
            "https://github.com/dk8827/AgentEmu",
        );
        ui.hyperlink_to(
            "Report an issue",
            "https://github.com/dk8827/AgentEmu/issues",
        );
        if action(ui, "Training tools for contributors", false, true) {
            self.open(Dialog::Workshop, ctx);
        }
    }

    fn workshop_dialog(&mut self, ui: &mut Ui, ctx: &Context) {
        caption(
            ui,
            "For contributors: train agents with the local Python/Rust command-line tools. This viewer opens their evaluation reports.",
        );
        if action(ui, "Open evaluation report", true, true) {
            self.pick(FileKind::Report, ctx);
        }
        if let Some(report) = &self.report {
            ui.collapsing("Evaluation results", |ui| {
                ui.monospace(serde_json::to_string_pretty(report).unwrap_or_default());
            });
        }
        ui.hyperlink_to(
            "Contra training guide",
            "https://github.com/dk8827/AgentEmu/blob/main/docs/CONTRA-AGENTS.md",
        );
        ui.hyperlink_to(
            "Training and evaluation",
            "https://github.com/dk8827/AgentEmu/blob/main/docs/TRAINING.md",
        );
    }
}
