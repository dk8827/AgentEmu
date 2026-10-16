use super::*;
impl PlayerApp {
    pub(super) fn workspace(&mut self, ctx: &Context) {
        if self.gallery.open {
            self.gallery_workspace(ctx);
            return;
        }
        CentralPanel::default()
            .frame(Frame::new().fill(PAPER).inner_margin(0))
            .show(ctx, |ui| {
                if self.dialog.is_some()
                    || self.requests.pending().is_some()
                    || self.transitioning()
                {
                    ui.disable();
                }
                let screen = ui.max_rect();
                let phone = screen.width() < 680.;
                let landscape = self.prefs.touch
                    && screen.width() > screen.height() * 1.35
                    && screen.height() < 600.;
                let margin = if phone || landscape { 8. } else { 24. };
                let bounds = screen.shrink(margin);
                let header = Rect::from_min_size(
                    bounds.min,
                    vec2(bounds.width(), if phone || landscape { 48. } else { 56. }),
                );
                self.header(ui, header, phone || landscape, ctx);
                if self.session.is_none() {
                    self.welcome(
                        ui,
                        Rect::from_min_max(pos2(bounds.left(), header.bottom() + 20.), bounds.max),
                        ctx,
                    );
                    return;
                }
                if landscape {
                    self.landscape(
                        ui,
                        Rect::from_min_max(pos2(bounds.left(), header.bottom() + 8.), bounds.max),
                        ctx,
                    );
                    return;
                }
                let touch = self.prefs.touch
                    && self
                        .session
                        .as_ref()
                        .is_none_or(|s| !s.machine.has_zapper());
                let small_phone = phone && screen.height() < 650.;
                let footer = if small_phone {
                    16.
                } else if phone {
                    24.
                } else {
                    26.
                };
                let deck = if touch {
                    if small_phone {
                        180.
                    } else if phone {
                        210.
                    } else {
                        190.
                    }
                } else if phone {
                    56.
                } else {
                    0.
                };
                let cards = if small_phone {
                    48.
                } else if phone {
                    56.
                } else {
                    96.
                };
                let shared: Vec<usize> = self.session.as_ref().map_or_else(Vec::new, |s| {
                    s.modes
                        .iter()
                        .enumerate()
                        .filter_map(|(p, m)| (*m == Mode::Shared).then_some(p))
                        .collect()
                });
                let trace_row = if self.show_trace {
                    if phone { 150. } else { 70. }
                } else {
                    42.
                };
                let trace = shared.len() as f32 * trace_row;
                let mut stage_height =
                    (bounds.height() - header.height() - cards - deck - footer - trace - 32.)
                        .max(60.);
                if phone && self.session.is_some() {
                    stage_height = stage_height
                        .min(bounds.width() * if self.prefs.aspect { 0.75 } else { 240. / 256. });
                }
                let stage_width = bounds.width().min(1200.);
                let stage = Rect::from_min_size(
                    pos2(bounds.center().x - stage_width / 2., header.bottom() + 12.),
                    vec2(stage_width, stage_height),
                );
                self.stage(ui, stage, ctx);
                let bar = Rect::from_min_size(
                    pos2(bounds.left(), stage.bottom() + 12.),
                    vec2(bounds.width(), cards),
                );
                if phone {
                    self.compact_players(ui, bar, ctx)
                } else {
                    self.player_bar(ui, bar, ctx)
                }
                let mut y = bar.bottom();
                for slot in shared {
                    let r = Rect::from_min_size(
                        pos2(bounds.left(), y + 4.),
                        vec2(bounds.width(), trace_row - 4.),
                    );
                    self.trace(ui, r, slot);
                    y = r.bottom();
                }
                if touch {
                    let r = Rect::from_min_size(
                        pos2(bounds.left(), y + 8.),
                        vec2(
                            bounds.width(),
                            if phone {
                                (bounds.bottom() - y - footer - 8.).max(deck)
                            } else {
                                deck
                            },
                        ),
                    );
                    self.touch_deck(ui, r, ctx);
                    y = r.bottom();
                } else {
                    self.touch_layout = None;
                    if phone {
                        let r = Rect::from_min_size(
                            pos2(bounds.left(), y + 8.),
                            vec2(bounds.width(), 48.),
                        );
                        self.menu_buttons(ui, r, ctx);
                        y = r.bottom();
                    }
                }
                if phone
                    && self
                        .session
                        .as_ref()
                        .is_some_and(|s| s.machine.has_zapper())
                    && y + 160. < bounds.bottom()
                {
                    let hint = Rect::from_min_size(
                        pos2(bounds.left(), y + 24.),
                        vec2(bounds.width(), 120.),
                    );
                    let c = pos2(hint.center().x, hint.top() + 24.);
                    ui.painter().circle_stroke(c, 12., Stroke::new(1.5, INK));
                    ui.painter().line_segment(
                        [c - vec2(18., 0.), c + vec2(18., 0.)],
                        Stroke::new(1.5, INK),
                    );
                    ui.painter().line_segment(
                        [c - vec2(0., 18.), c + vec2(0., 18.)],
                        Stroke::new(1.5, INK),
                    );
                    ui.painter().text(
                        pos2(hint.center().x, hint.top() + 58.),
                        Align2::CENTER_TOP,
                        "The screen is your controller",
                        bold(18.),
                        INK,
                    );
                    ui.painter().text(
                        pos2(hint.center().x, hint.top() + 88.),
                        Align2::CENTER_TOP,
                        "Aim with a tap. Start / Select control the menu.",
                        FontId::proportional(12.),
                        MUTED,
                    );
                }
                let caption = if phone {
                    if self
                        .session
                        .as_ref()
                        .is_some_and(|s| s.machine.has_zapper())
                    {
                        "Tap the game to aim and fire"
                    } else if self.running() {
                        "Start / Select belong to the game"
                    } else {
                        "Resume, then use NES Start to begin"
                    }
                } else if self.running() {
                    if self
                        .session
                        .as_ref()
                        .is_some_and(|s| s.machine.has_zapper())
                    {
                        "Light gun · click or tap the game to aim and fire"
                    } else if self.playback.is_some() {
                        "Replay · recorded controller input"
                    } else {
                        "Escape pauses · NES Start belongs to the game"
                    }
                } else if self
                    .session
                    .as_ref()
                    .is_some_and(|s| s.machine.has_zapper())
                {
                    "Light gun · click or tap the game to aim and fire. Use Start to begin."
                } else if self.session.is_some() {
                    "NES Start begins the game · Escape opens the menu"
                } else {
                    "Local games. Your controls. An AI teammate when you want one."
                };
                if y + 20. < bounds.bottom() {
                    ui.painter().text(
                        pos2(bounds.center().x, bounds.bottom() - 3.),
                        Align2::CENTER_BOTTOM,
                        if !self.notice.is_empty() {
                            &self.notice
                        } else {
                            caption
                        },
                        FontId::proportional(if phone { 11. } else { 13. }),
                        MUTED,
                    );
                }
            });
    }
    fn welcome(&mut self, ui: &mut Ui, r: Rect, ctx: &Context) {
        self.touch_layout = None;
        let width = r.width().min(560.);
        let rect = Rect::from_min_size(
            pos2(
                r.center().x - width / 2.,
                r.top() + if r.height() > 500. { 60. } else { 8. },
            ),
            vec2(width, r.height()),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ScrollArea::vertical()
                    .max_height(r.bottom() - rect.top())
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Your games.\nYour way to play.")
                                .font(bold(if width < 400. { 34. } else { 44. })),
                        );
                        ui.add_space(8.);
                        ui.label(
                            RichText::new(
                                "Play a classic, team up with AI, or share a controller.",
                            )
                            .size(18.)
                            .color(MUTED),
                        );
                        ui.add_space(24.);
                        if ui
                            .add_sized(
                                [width, 52.],
                                Button::new(
                                    RichText::new("Choose game ROM")
                                        .color(Color32::WHITE)
                                        .font(bold(18.)),
                                )
                                .fill(ACCENT),
                            )
                            .clicked()
                        {
                            self.pick(FileKind::Rom, ctx);
                        }
                        ui.label(
                            RichText::new(
                                "Open a .nes file from your device. Game files stay local.",
                            )
                            .size(14.)
                            .color(MUTED),
                        );
                        ui.add_space(16.);
                        if ui
                            .add_sized([width, 48.], Button::new("Explore AI games"))
                            .clicked()
                        {
                            self.open_gallery(ctx);
                        }
                        ui.add_space(12.);
                        if ui
                            .add_sized([width, 40.], Button::new("Help & about").frame(false))
                            .clicked()
                        {
                            self.open(Dialog::Help, ctx);
                        }
                    });
            },
        );
    }
    pub(super) fn touch_preview(&self, ui: &mut Ui) {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 100.), Sense::hover());
        ui.painter()
            .rect_filled(r, 4., Color32::from_rgb(233, 231, 224));
        let size = 26. * self.prefs.touch_scale;
        let (left, right) = if self.prefs.left_handed {
            (r.right() - 50., r.left() + 50.)
        } else {
            (r.left() + 50., r.right() - 50.)
        };
        let c = pos2(left, r.center().y);
        ui.painter().rect_filled(
            Rect::from_center_size(c, vec2(size * 2., size * 0.65)),
            3.,
            INK,
        );
        ui.painter().rect_filled(
            Rect::from_center_size(c, vec2(size * 0.65, size * 2.)),
            3.,
            INK,
        );
        for (label, delta) in [("A", vec2(14., -10.)), ("B", vec2(-14., 16.))] {
            let c = pos2(right, r.center().y) + delta;
            ui.painter().circle_filled(c, size * 0.6, PAPER);
            ui.painter()
                .text(c, Align2::CENTER_CENTER, label, bold(14.), INK);
        }
    }
    fn header(&mut self, ui: &mut Ui, r: Rect, compact: bool, ctx: &Context) {
        ui.painter()
            .rect_stroke(r, 0, Stroke::new(1., INK), StrokeKind::Inside);
        let kind = self.session.as_ref().map(|s| {
            if s.machine.has_zapper() {
                "Light gun"
            } else if s.info.players == 1 {
                "1 player"
            } else if s.info.play_mode == "alternating" {
                "2 players · take turns"
            } else {
                if s.modes[1] == Mode::Off {
                    "Solo"
                } else {
                    "2 controllers"
                }
            }
        });
        let title = self
            .session
            .as_ref()
            .map_or_else(|| "AgentEmu".to_string(), |s| s.info.title.clone());
        let right = if compact { 116. } else { 560. };
        let title_rect = Rect::from_min_max(
            r.min,
            pos2((r.right() - right).max(r.left() + 80.), r.bottom()),
        );
        let response = ui.interact(title_rect, ui.id().with("game-menu"), Sense::click());
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), "Game menu"));
        if response.clicked() {
            self.open(Dialog::Menu, ctx)
        }
        let painter = ui.painter().with_clip_rect(title_rect.shrink(8.));
        let font = bold(if compact { 22. } else { 29. });
        let title_width = painter
            .layout_no_wrap(title.clone(), font.clone(), INK)
            .size()
            .x;
        painter.text(
            pos2(r.left() + 16., r.center().y),
            Align2::LEFT_CENTER,
            &title,
            font,
            INK,
        );
        let chevron = pos2(
            (r.left() + title_width + 31.).min(title_rect.right() - 15.),
            r.center().y,
        );
        painter.line_segment(
            [chevron + vec2(-4., -2.), chevron + vec2(0., 2.)],
            Stroke::new(1.5, INK),
        );
        painter.line_segment(
            [chevron + vec2(0., 2.), chevron + vec2(4., -2.)],
            Stroke::new(1.5, INK),
        );
        if !compact && let Some(kind) = kind {
            let kind_width = painter
                .layout_no_wrap(kind.into(), FontId::proportional(12.), MUTED)
                .size()
                .x;
            if chevron.x + 26. + kind_width < title_rect.right() {
                painter.text(
                    pos2(chevron.x + 26., r.center().y),
                    Align2::LEFT_CENTER,
                    kind,
                    FontId::proportional(12.),
                    MUTED,
                );
            }
        }
        if compact {
            let transport = Rect::from_min_max(pos2(r.right() - 112., r.top()), r.max);
            let label = if self.session.is_none() {
                "AI games"
            } else {
                self.transport_label()
            };
            if button(ui, transport, label, Kind::Primary, true).clicked() {
                if self.session.is_none() {
                    self.open_gallery(ctx);
                } else if self.running() {
                    self.open(Dialog::Menu, ctx)
                } else {
                    self.toggle(ctx)
                }
            }
        } else {
            let items = [
                ("AI games", 108.),
                ("Open ROM", 132.),
                ("Sound", 110.),
                ("Fullscreen", 122.),
                ("Menu", 88.),
            ];
            let mut x = r.right() - items.iter().map(|(_, w)| w).sum::<f32>();
            for (label, w) in items {
                let rect = Rect::from_min_size(pos2(x, r.top()), vec2(w, r.height()));
                let display = if label == "Sound" {
                    if self.prefs.muted {
                        "Sound off"
                    } else {
                        "Sound on"
                    }
                } else {
                    label
                };
                if button(ui, rect, display, Kind::Normal, true).clicked() {
                    match label {
                        "AI games" => self.open_gallery(ctx),
                        "Open ROM" => self.pick(FileKind::Rom, ctx),
                        "Sound" => {
                            self.prefs.muted = !self.prefs.muted;
                            self.host.audio_clear();
                            if !self.prefs.muted && self.running() {
                                self.host.start_audio(self.prefs.volume);
                            }
                        }
                        "Fullscreen" => platform::fullscreen(ctx),
                        _ => self.open(Dialog::Menu, ctx),
                    }
                }
                x += w;
            }
        }
    }
    pub(super) fn transport_label(&self) -> &'static str {
        if let Some(worker) = &self.worker {
            match worker.lifecycle {
                crate::runtime::Lifecycle::Starting => return "Starting…",
                crate::runtime::Lifecycle::Pausing => return "Pausing…",
                _ => {}
            }
        }
        if self.running() {
            "Pause"
        } else if self.session.as_ref().is_some_and(|s| s.has_run) {
            "Resume"
        } else {
            "Play"
        }
    }
    fn stage(&mut self, ui: &mut Ui, r: Rect, ctx: &Context) {
        ui.painter().rect_filled(r, 0, DARK);
        let aspect = if self.prefs.aspect {
            4. / 3.
        } else {
            256. / 240.
        };
        let mut size = vec2(
            (r.height() * aspect).min(r.width()),
            r.height().min(r.width() / aspect),
        );
        if self.prefs.integer_scale {
            let scale = (size.x / 256.).min(size.y / 240.).floor().max(1.);
            if scale * 256. <= r.width() && scale * 240. <= r.height() {
                size = vec2(256., 240.) * scale;
            }
        }
        let image = Rect::from_center_size(r.center(), size);
        self.game_rect = image;
        if let Some(t) = &self.texture {
            ui.painter().image(
                t.id(),
                image,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
        }
        let zapper = self
            .session
            .as_ref()
            .is_some_and(|s| s.machine.has_zapper());
        let sense = ui.interact(image, ui.id().with("game-screen"), Sense::click_and_drag());
        sense.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                ui.is_enabled(),
                if zapper {
                    "Game screen · aim and fire"
                } else {
                    "Game screen · focus controls"
                },
            )
        });
        self.game_focus = Some(sense.id);
        if sense.clicked() {
            sense.request_focus();
        }
        if zapper && self.running() && self.dialog.is_none() {
            if let Some(pos) = ctx
                .input(|i| i.pointer.hover_pos())
                .filter(|p| image.contains(*p))
            {
                self.zapper_aim = [
                    ((pos.x - image.left()) / image.width() * 256.) as i16,
                    ((pos.y - image.top()) / image.height() * 240.) as i16,
                ];
                ctx.set_cursor_icon(CursorIcon::Crosshair);
            }
            if sense.clicked() || sense.drag_started() {
                self.zapper_frames = 3;
            }
        }
        if !self.running() && self.dialog.is_none() {
            let has_run = self.session.as_ref().is_some_and(|s| s.has_run);
            let badge = Rect::from_min_size(
                image.left_top() + vec2(16., 16.),
                vec2(if has_run { 88. } else { 156. }, 34.),
            );
            ui.painter().rect_filled(badge, 0, PAPER);
            text(
                ui,
                badge.min + vec2(10., 6.),
                if has_run { "PAUSED" } else { "READY TO PLAY" },
                13.,
                INK,
                true,
            );
        }

        if !self.running() && self.dialog.is_none() && !self.gallery.open {
            let ready = self.session.as_ref().is_some_and(|s| !s.has_run);
            if ready {
                let r = Rect::from_center_size(image.center(), vec2(180., 48.));
                if button(ui, r, "Play", Kind::Primary, true).clicked() {
                    self.toggle(ctx);
                }
            }
        }
    }
    fn player_bar(&mut self, ui: &mut Ui, r: Rect, ctx: &Context) {
        let players = self.visible_players();
        let button_w = 150.;
        let add = players == 1 && self.session.as_ref().is_some_and(|s| s.info.players > 1);
        let add_width = if add { 130. } else { 0. };
        let w = (r.width() - button_w - add_width) / players as f32;
        if add
            && button(
                ui,
                Rect::from_min_size(
                    pos2(r.right() - button_w - add_width, r.top()),
                    vec2(add_width, r.height()),
                ),
                "+ Player 2",
                Kind::Normal,
                true,
            )
            .clicked()
        {
            self.open(Dialog::Player(1), ctx);
        }
        for slot in 0..players {
            let card = Rect::from_min_size(
                pos2(r.left() + w * slot as f32, r.top()),
                vec2(w, r.height()),
            );
            self.player_card(ui, card, slot, ctx);
        }
        let primary = Rect::from_min_max(pos2(r.right() - button_w, r.top()), r.max);
        if button(
            ui,
            primary,
            self.transport_label(),
            Kind::Primary,
            self.session.is_some(),
        )
        .clicked()
        {
            self.toggle(ctx)
        }
    }
    fn player_card(&mut self, ui: &mut Ui, r: Rect, slot: usize, ctx: &Context) {
        ui.painter()
            .rect_stroke(r, 0, Stroke::new(1., INK), StrokeKind::Inside);
        let color = if slot == 0 { BLUE } else { ORANGE };
        ui.painter()
            .circle_filled(r.min + vec2(22., 25.), 7., color);
        let mode = self.session.as_ref().map_or(Mode::Human, |s| s.modes[slot]);
        let title = format!("Player {}  ·  {}", slot + 1, mode.label());
        let top = Rect::from_min_max(r.min + vec2(38., 8.), pos2(r.right() - 12., r.top() + 39.));
        let res = ui
            .interact(top, ui.id().with(("player-heading", slot)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        res.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &title));
        ui.painter().with_clip_rect(top).text(
            top.left_center(),
            Align2::LEFT_CENTER,
            &title,
            bold(19.),
            INK,
        );
        if res.clicked() {
            self.open(Dialog::Player(slot), ctx)
        }
        let width = ((r.width() - 40.) / 3.).min(115.);
        let y = r.bottom() - 48.;
        for (n, label, mask) in [(0, "Player setup", 0), (1, "Select", 4), (2, "Start", 8)] {
            let rect = Rect::from_min_size(
                pos2(r.left() + 12. + (width + 6.) * n as f32, y),
                vec2(width, 44.),
            );
            let enabled = if mask == 0 {
                true
            } else {
                self.running() && mode != Mode::Off && self.playback.is_none()
            };
            let response = button(ui, rect, label, Kind::Normal, enabled);
            response.widget_info(|| {
                WidgetInfo::labeled(
                    WidgetType::Button,
                    enabled && ui.is_enabled(),
                    format!("Player {} {label}", slot + 1),
                )
            });
            if response.clicked() {
                if mask == 0 {
                    self.open(Dialog::Player(slot), ctx)
                } else if let Some(s) = &mut self.session {
                    s.pulse[slot] |= mask;
                    response.surrender_focus();
                }
            }
        }
    }
    fn compact_players(&mut self, ui: &mut Ui, r: Rect, ctx: &Context) {
        ui.painter()
            .rect_stroke(r, 0, Stroke::new(1., INK), StrokeKind::Inside);
        let players = self.visible_players();
        let add = players == 1 && self.session.as_ref().is_some_and(|s| s.info.players > 1);
        let w = (r.width() - if add { 112. } else { 0. }) / players as f32;
        if add
            && button(
                ui,
                Rect::from_min_max(pos2(r.right() - 112., r.top()), r.max),
                "+ Player 2",
                Kind::Normal,
                true,
            )
            .clicked()
        {
            self.open(Dialog::Player(1), ctx);
        }

        for slot in 0..players {
            let rect = Rect::from_min_size(
                pos2(r.left() + w * slot as f32, r.top()),
                vec2(w, r.height()),
            );
            let mode = self.session.as_ref().map_or(Mode::Human, |s| s.modes[slot]);
            let label = if mode == Mode::Human {
                self.prefs.devices[slot].label()
            } else {
                mode.label().into()
            };
            let response = button(
                ui,
                rect,
                &format!("P{} · {}  ›", slot + 1, label),
                Kind::Normal,
                true,
            );
            response.widget_info(|| {
                WidgetInfo::labeled(
                    WidgetType::Button,
                    ui.is_enabled(),
                    format!("Player {} · {}", slot + 1, label),
                )
            });
            if response.clicked() {
                self.open(Dialog::Player(slot), ctx)
            }
        }
    }
    fn menu_buttons(&mut self, ui: &mut Ui, r: Rect, _ctx: &Context) {
        let players = self.visible_players();
        if self.menu_port >= players {
            self.menu_port = 0;
        }
        let count = if players > 1 { 3. } else { 2. };
        let w = (r.width() - 8. * (count - 1.)) / count;
        for (n, label, mask) in [(0, "Select", 4), (1, "Start", 8)] {
            let rect = Rect::from_min_size(
                pos2(r.left() + (w + 8.) * n as f32, r.top()),
                vec2(w, r.height()),
            );
            let enabled = self.running()
                && self.playback.is_none()
                && self
                    .session
                    .as_ref()
                    .is_some_and(|s| s.modes[self.menu_port] != Mode::Off);
            let response = button(
                ui,
                rect,
                &format!("P{} {label}", self.menu_port + 1),
                Kind::Normal,
                enabled,
            );
            response.widget_info(|| {
                WidgetInfo::labeled(
                    WidgetType::Button,
                    enabled && ui.is_enabled(),
                    format!("Player {} {label}", self.menu_port + 1),
                )
            });
            if response.clicked()
                && let Some(s) = &mut self.session
            {
                s.pulse[self.menu_port] |= mask;
                response.surrender_focus();
            }
        }
        if players > 1 {
            let rect =
                Rect::from_min_size(pos2(r.left() + 2. * (w + 8.), r.top()), vec2(w, r.height()));
            let response = button(
                ui,
                rect,
                &format!("P{} menu", 2 - self.menu_port),
                Kind::Normal,
                true,
            );
            response.widget_info(|| {
                WidgetInfo::labeled(
                    WidgetType::Button,
                    ui.is_enabled(),
                    format!("Player {} menu controls", 2 - self.menu_port),
                )
            });
            if response.clicked() {
                self.menu_port = 1 - self.menu_port;
            }
        }
    }
    fn touch_deck(&mut self, ui: &mut Ui, r: Rect, ctx: &Context) {
        let size = (r.height() - 60.).min(180.).min(r.width() * 0.43)
            * self.prefs.touch_scale.clamp(0.8, 1.1);
        let left = pos2(r.left() + size / 2. + 4., r.top() + (r.height() - 60.) / 2.);
        let right = pos2(
            r.right() - size / 2. - 4.,
            r.top() + (r.height() - 60.) / 2.,
        );
        let (dpad, buttons) = if self.prefs.left_handed {
            (right, left)
        } else {
            (left, right)
        };
        let button_size = (size * 0.40).clamp(46., 64.);
        let l = TouchLayout {
            dpad: Rect::from_center_size(dpad, Vec2::splat(size)),
            a: Rect::from_center_size(buttons + vec2(size * 0.23, 0.), Vec2::splat(button_size)),
            b: Rect::from_center_size(
                buttons + vec2(-size * 0.22, size * 0.27),
                Vec2::splat(button_size),
            ),
            ab: Rect::from_center_size(
                buttons + vec2(-size * 0.21, -size * 0.27),
                Vec2::splat((button_size * 0.9).max(44.)),
            ),
        };
        self.paint_touch(ui, l, ctx);
        let menu = Rect::from_min_size(pos2(r.left(), r.bottom() - 48.), vec2(r.width(), 44.));
        self.menu_buttons(ui, menu, ctx);
    }
    fn paint_touch(&mut self, ui: &mut Ui, l: TouchLayout, ctx: &Context) {
        self.touch_layout = Some(l);
        let enabled = self.running()
            && self.playback.is_none()
            && self.dialog.is_none()
            && self.prefs.devices[self.touch_port] == Device::Touch;
        let ai_groups = self
            .session
            .as_ref()
            .map_or(0, |s| match s.modes[self.touch_port] {
                Mode::Shared => s.ownership[self.touch_port],
                Mode::Ai => contra_agent::control::GAMEPLAY,
                _ => 0,
            });
        let d = l.dpad;
        let third = d.width() / 3.;
        let center = d.center();
        let p = ui.painter();
        let fill = if enabled {
            INK
        } else {
            Color32::from_gray(165)
        };
        p.rect_filled(
            Rect::from_center_size(center, vec2(third, d.height())),
            5,
            fill,
        );
        p.rect_filled(
            Rect::from_center_size(center, vec2(d.width(), third)),
            5,
            fill,
        );
        for (delta, mask) in [
            (vec2(0., -third), 16),
            (vec2(0., third), 32),
            (vec2(-third, 0.), 64),
            (vec2(third, 0.), 128),
        ] {
            let tip = delta.normalized() * 6.;
            let side = vec2(-tip.y, tip.x);
            let c = center + delta;
            p.add(Shape::convex_polygon(
                vec![c + tip, c - tip + side, c - tip - side],
                if self.touch_mask & mask != 0 {
                    Color32::WHITE
                } else {
                    Color32::from_gray(153)
                },
                Stroke::NONE,
            ));
        }
        for (rect, label, mask) in [(l.a, "A", 1), (l.b, "B", 2), (l.ab, "A+B", 3)] {
            let ai_owned = ai_groups & mask != 0;
            let pressed = self.touch_mask & mask == mask && !ai_owned;
            let c = if pressed { BLUE } else { PAPER };
            p.circle_filled(rect.center(), rect.width() / 2., c);
            p.circle_stroke(
                rect.center(),
                rect.width() / 2.,
                Stroke::new(
                    2.,
                    if ai_owned {
                        ORANGE
                    } else if enabled {
                        INK
                    } else {
                        LINE
                    },
                ),
            );
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                bold(if label.len() > 1 { 18. } else { 26. }),
                if pressed {
                    Color32::WHITE
                } else if ai_owned {
                    ORANGE
                } else {
                    INK
                },
            );
        }
        for (rect, mask) in [(l.a, 1), (l.b, 2), (l.ab, 3)] {
            if ai_groups & mask != 0 {
                control_icon(
                    p,
                    Rect::from_center_size(
                        pos2(rect.right() - 2., rect.top() + 4.),
                        vec2(16., 16.),
                    ),
                    Mode::Ai,
                    ORANGE,
                );
            }
        }
        if ai_groups & 240 != 0 {
            control_icon(
                p,
                Rect::from_center_size(l.dpad.center(), vec2(22., 22.)),
                Mode::Ai,
                ORANGE,
            );
        }
        // Mouse support is also useful for large-screen touch layouts. Raw touch
        // events above retain independent ownership for every finger.
        if !ctx.input(|i| i.any_touches()) {
            let pos = ctx.input(|i| i.pointer.interact_pos());
            if enabled
                && ctx.input(|i| i.pointer.primary_pressed())
                && let Some(pos) = pos
            {
                self.touch.begin(u64::MAX, pos, &l);
                self.mouse_touch = true;
            }
            if self.mouse_touch {
                if let Some(pos) = pos {
                    self.touch.move_to(u64::MAX, pos);
                }
                if ctx.input(|i| i.pointer.primary_released()) {
                    self.touch.end(u64::MAX);
                    self.mouse_touch = false;
                }
            }
        }
        self.touch_mask = if enabled { self.touch.buttons(&l) } else { 0 };
    }
    fn landscape(&mut self, ui: &mut Ui, r: Rect, ctx: &Context) {
        let zapper = self
            .session
            .as_ref()
            .is_some_and(|s| s.machine.has_zapper());
        let shared: Vec<usize> = self.session.as_ref().map_or_else(Vec::new, |s| {
            s.modes
                .iter()
                .enumerate()
                .filter_map(|(p, m)| (*m == Mode::Shared).then_some(p))
                .collect()
        });
        let trace_row = if self.show_trace { 66. } else { 42. };
        let trace_height = shared.len() as f32 * trace_row;
        let side = if zapper { 8. } else { 150. };
        let stage = Rect::from_min_max(
            pos2(r.left() + side, r.top()),
            pos2(r.right() - side, r.bottom() - 50. - trace_height),
        );
        self.stage(ui, stage, ctx);
        if !zapper {
            let mut dpad = Rect::from_center_size(
                pos2(r.left() + 70., stage.center().y.max(r.top() + 65.)),
                vec2(130., 130.) * self.prefs.touch_scale,
            );
            let mut buttons = pos2(r.right() - 72., stage.center().y.max(r.top() + 65.));
            if self.prefs.left_handed {
                let old = dpad.center().x;
                dpad = dpad.translate(vec2(buttons.x - old, 0.));
                buttons.x = old;
            }
            let l = TouchLayout {
                dpad,
                a: Rect::from_center_size(
                    buttons + vec2(35., 0.),
                    vec2(54., 54.) * self.prefs.touch_scale,
                ),
                b: Rect::from_center_size(
                    buttons + vec2(-27., 30.),
                    vec2(54., 54.) * self.prefs.touch_scale,
                ),
                ab: Rect::from_center_size(
                    buttons + vec2(-27., -35.),
                    vec2(48., 48.) * self.prefs.touch_scale,
                ),
            };
            self.paint_touch(ui, l, ctx);
        } else {
            self.touch_layout = None;
        }
        for (i, slot) in shared.into_iter().enumerate() {
            self.trace(
                ui,
                Rect::from_min_size(
                    pos2(
                        r.left(),
                        r.bottom() - 50. - trace_height + i as f32 * trace_row,
                    ),
                    vec2(r.width(), trace_row),
                ),
                slot,
            );
        }
        let menu = Rect::from_min_size(
            pos2(stage.left(), r.bottom() - 44.),
            vec2(stage.width(), 44.),
        );
        self.menu_buttons(ui, menu, ctx);
    }
    fn trace(&mut self, ui: &mut Ui, r: Rect, slot: usize) {
        if !self.show_trace {
            let Some(s) = &self.session else { return };
            let mask = s.ownership[slot];
            let actions = metadata::actions(&s.machine.cartridge_hash());
            let summary = if mask == 2 {
                format!("You move · AI: {}", actions[1])
            } else if mask == 1 {
                format!("You move · AI: {}", actions[0])
            } else if mask == 3 {
                "You move · AI: A + B".into()
            } else {
                "You + AI · custom controls".into()
            };
            ui.painter().text(
                pos2(r.left() + 4., r.center().y),
                Align2::LEFT_CENTER,
                summary,
                bold(13.),
                INK,
            );
            if button(
                ui,
                Rect::from_min_size(pos2(r.right() - 74., r.top()), vec2(74., 36.)),
                "Inputs",
                Kind::Normal,
                true,
            )
            .clicked()
            {
                self.show_trace = true;
            }
            return;
        }
        let hide = Rect::from_min_size(pos2(r.right() - 58., r.top()), vec2(58., 24.));
        if ui.put(hide, Button::new("Hide").frame(false)).clicked() {
            self.show_trace = false;
        }
        let stacked = r.width() < 600. && r.height() > 100.;
        let r = if stacked {
            r
        } else {
            Rect::from_min_max(r.min, pos2(r.right() - 60., r.bottom()))
        };
        rule(ui, r.left_top(), r.right_top());
        if stacked {
            text(
                ui,
                r.min + vec2(4., 8.),
                if self.running() {
                    "Live controller"
                } else {
                    "Last input · paused"
                },
                13.,
                MUTED,
                true,
            );
        }
        let Some(s) = &self.session else { return };
        let decision = s.decisions.as_ref().map(|ds| &ds[slot]);
        let columns = [
            ("YOU", decision.map_or(0, |d| d.human), BLUE),
            (
                "AI PROPOSES",
                decision.and_then(|d| d.proposal).unwrap_or(0),
                ORANGE,
            ),
            ("APPLIED", decision.map_or(0, |d| d.applied), INK),
        ];
        let w = if stacked {
            r.width() - 92.
        } else {
            r.width() / 3.
        };
        for (i, (label, mask, color)) in columns.into_iter().enumerate() {
            let x = if stacked {
                r.left()
            } else {
                r.left() + w * i as f32
            };
            let y = if stacked {
                r.top() + 48. + 32. * i as f32
            } else {
                r.top()
            };
            text(
                ui,
                pos2(x + 4., y + 6.),
                &format!("P{} {label}", slot + 1),
                11.,
                color,
                true,
            );
            let chip = ((w - 12.) / 6.).min(40.);
            for (n, (label, g)) in [
                ("L/R", 192),
                ("U/D", 48),
                ("A", 1),
                ("B", 2),
                ("Se", 4),
                ("St", 8),
            ]
            .into_iter()
            .enumerate()
            {
                let label = match (g, mask & g) {
                    (192, 64) => "L",
                    (192, 128) => "R",
                    (48, 16) => "U",
                    (48, 32) => "D",
                    _ => label,
                };
                let owned = i != 1 || s.ownership[slot] & g != 0;
                let rect = Rect::from_min_size(
                    pos2(
                        x + 4. + if stacked { 92. } else { 0. } + chip * n as f32,
                        y + if stacked { 0. } else { 27. },
                    ),
                    vec2(chip - 2., 26.),
                );
                let active = mask & g != 0;
                ui.painter().rect_filled(
                    rect,
                    0,
                    if active && owned {
                        color
                    } else {
                        Color32::from_rgb(233, 231, 224)
                    },
                );
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    bold(if chip < 22. { 9. } else { 12. }),
                    if active && owned {
                        Color32::WHITE
                    } else {
                        MUTED
                    },
                );
                if !owned {
                    ui.painter().line_segment(
                        [rect.left_bottom(), rect.right_top()],
                        Stroke::new(1., MUTED),
                    );
                }
            }
        }
    }
}
