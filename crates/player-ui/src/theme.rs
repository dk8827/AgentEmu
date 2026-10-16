use egui::*;
pub const PAPER: Color32 = Color32::from_rgb(247, 245, 239);
pub const INK: Color32 = Color32::from_rgb(32, 35, 34);
pub const MUTED: Color32 = Color32::from_rgb(103, 107, 102);
pub const ACCENT: Color32 = Color32::from_rgb(199, 52, 27);
pub const BLUE: Color32 = Color32::from_rgb(18, 105, 223);
pub const ORANGE: Color32 = Color32::from_rgb(193, 80, 18);
pub const LINE: Color32 = Color32::from_rgb(185, 186, 178);
pub const DARK: Color32 = Color32::from_rgb(23, 26, 25);
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Plex bold".into()))
}
pub fn setup(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Plex".into(),
        FontData::from_static(include_bytes!("../assets/PlexSans-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "Plex bold".into(),
        FontData::from_static(include_bytes!("../assets/PlexSans-SemiBold.ttf")).into(),
    );
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Plex".into());
    fonts
        .families
        .insert(FontFamily::Name("Plex bold".into()), {
            let mut fallback = fonts.families[&FontFamily::Proportional].clone();
            fallback.insert(0, "Plex bold".into());
            fallback
        });
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = Visuals::light();
    style.visuals.panel_fill = PAPER;
    style.visuals.window_fill = PAPER;
    style.visuals.override_text_color = Some(INK);
    style.visuals.selection.bg_fill = BLUE;
    style.visuals.selection.stroke = Stroke::new(1., Color32::WHITE);
    style.visuals.window_corner_radius = CornerRadius::ZERO;
    style.visuals.window_shadow = epaint::Shadow::NONE;
    style.visuals.window_stroke = Stroke::new(1., INK);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::ZERO;
        widget.bg_stroke = Stroke::new(1., INK);
    }
    style.visuals.widgets.inactive.weak_bg_fill = PAPER;
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(232, 230, 222);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(219, 217, 207);
    style.spacing.item_spacing = vec2(12., 12.);
    style.spacing.button_padding = vec2(14., 10.);
    style.spacing.interact_size = vec2(48., 44.);
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.spacing.slider_width = 160.;
    style.spacing.text_edit_width = 240.;
    style.spacing.window_margin = Margin::same(24);
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(16.));
    style.text_styles.insert(TextStyle::Button, bold(16.));
    style.text_styles.insert(TextStyle::Heading, bold(27.));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(13.));
    ctx.set_style_of(Theme::Dark, style.clone());
    ctx.set_style_of(Theme::Light, style);
    ctx.set_theme(Theme::Light);
}
#[derive(Clone, Copy)]
pub enum Kind {
    Normal,
    Primary,
}
pub fn button(ui: &mut Ui, rect: Rect, label: &str, kind: Kind, enabled: bool) -> Response {
    let (fill, text, stroke) = match kind {
        Kind::Primary => (ACCENT, Color32::WHITE, Stroke::NONE),
        Kind::Normal => (PAPER, INK, Stroke::new(1., INK)),
    };
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::centered_and_justified(Direction::LeftToRight)),
        |ui| {
            ui.add_enabled(
                enabled,
                Button::new(RichText::new(label).font(bold(16.)).color(if enabled {
                    text
                } else {
                    MUTED
                }))
                .fill(if enabled {
                    fill
                } else {
                    Color32::from_rgb(234, 232, 225)
                })
                .stroke(stroke)
                .corner_radius(0)
                .min_size(rect.size()),
            )
        },
    )
    .inner
}
pub fn text(ui: &Ui, pos: Pos2, label: &str, size: f32, color: Color32, strong: bool) {
    ui.painter().text(
        pos,
        Align2::LEFT_TOP,
        label,
        if strong {
            bold(size)
        } else {
            FontId::proportional(size)
        },
        color,
    );
}
pub fn rule(ui: &Ui, a: Pos2, b: Pos2) {
    ui.painter().line_segment([a, b], Stroke::new(1., LINE));
}

/// Controller choices use vector marks so native and WASM don't depend on emoji fonts.
pub fn control_choice(
    ui: &mut Ui,
    mode: crate::session::Mode,
    selected: bool,
    width: f32,
) -> Response {
    use crate::session::Mode;
    let color = match mode {
        Mode::Human => BLUE,
        Mode::Ai => ORANGE,
        Mode::Shared => ACCENT,
        Mode::Off => MUTED,
    };
    let response = ui.add_sized(
        [width, 44.],
        Button::new("")
            .selected(selected)
            .fill(if selected { color } else { PAPER }),
    );
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::RadioButton,
            ui.is_enabled(),
            selected,
            mode.label(),
        )
    });
    let foreground = if !ui.is_enabled() {
        MUTED
    } else if selected {
        Color32::WHITE
    } else {
        INK
    };
    let icon_color = if !ui.is_enabled() {
        MUTED
    } else if selected {
        Color32::WHITE
    } else {
        color
    };
    let stacked = width < 108.;
    let font = bold(if stacked { 13. } else { 15. });
    let label = ui
        .painter()
        .layout_no_wrap(mode.label().into(), font, foreground);
    let icon_size = vec2(
        if mode == Mode::Shared { 32. } else { 22. },
        if stacked { 18. } else { 22. },
    );
    let (icon_center, label_origin) = if stacked {
        (
            pos2(response.rect.center().x, response.rect.top() + 12.),
            pos2(
                response.rect.center().x - label.size().x * 0.5,
                response.rect.bottom() - label.size().y - 3.,
            ),
        )
    } else {
        let left = response.rect.center().x - (icon_size.x + 8. + label.size().x) * 0.5;
        (
            pos2(left + icon_size.x * 0.5, response.rect.center().y),
            pos2(
                left + icon_size.x + 8.,
                response.rect.center().y - label.size().y * 0.5,
            ),
        )
    };
    control_icon(
        ui.painter(),
        Rect::from_center_size(icon_center, icon_size),
        mode,
        icon_color,
    );
    ui.painter().galley(label_origin, label, foreground);
    response
}

pub fn control_icon(painter: &Painter, rect: Rect, mode: crate::session::Mode, color: Color32) {
    use crate::session::Mode;
    if mode == Mode::Shared {
        // A person and robot together: one shared controller, not two player slots.
        let side = rect.height().min(rect.width() * 0.5);
        control_icon(
            painter,
            Rect::from_center_size(
                pos2(rect.left() + side * 0.5, rect.center().y),
                Vec2::splat(side),
            ),
            Mode::Human,
            color,
        );
        control_icon(
            painter,
            Rect::from_center_size(
                pos2(rect.right() - side * 0.5, rect.center().y),
                Vec2::splat(side),
            ),
            Mode::Ai,
            color,
        );
        return;
    }
    let point = |x: f32, y: f32| {
        pos2(
            rect.left() + x / 24. * rect.width(),
            rect.top() + y / 24. * rect.height(),
        )
    };
    let stroke = Stroke::new(1.6, color);
    match mode {
        Mode::Human => {
            painter.circle_stroke(point(12., 7.), rect.height() * 3.5 / 24., stroke);
            painter.add(Shape::line(
                vec![
                    point(4., 21.),
                    point(4., 19.),
                    point(6., 15.),
                    point(9., 14.),
                    point(15., 14.),
                    point(18., 15.),
                    point(20., 19.),
                    point(20., 21.),
                ],
                stroke,
            ));
        }
        Mode::Ai => {
            painter.rect_stroke(
                Rect::from_min_max(point(4., 7.), point(20., 21.)),
                3,
                stroke,
                StrokeKind::Inside,
            );
            painter.line_segment([point(12., 3.), point(12., 7.)], stroke);
            painter.circle_filled(point(12., 2.), 1.5, color);
            painter.circle_filled(point(9., 12.), 1.3, color);
            painter.circle_filled(point(15., 12.), 1.3, color);
            painter.line_segment([point(9., 17.), point(15., 17.)], stroke);
            painter.line_segment([point(1., 12.), point(4., 12.)], stroke);
            painter.line_segment([point(20., 12.), point(23., 12.)], stroke);
        }
        Mode::Off => {
            painter.circle_stroke(rect.center(), rect.height() * 0.4, stroke);
            painter.line_segment([point(5., 19.), point(19., 5.)], stroke);
        }
        Mode::Shared => unreachable!(),
    }
}
