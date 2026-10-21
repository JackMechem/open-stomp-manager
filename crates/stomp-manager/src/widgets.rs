//! Custom-painted widgets: cards, knobs, meters, LEDs, chips.

use std::f32::consts::PI;

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::theme::*;

/// A rounded card with an icon + caps title header.
pub fn card<R>(ui: &mut Ui, icon: &str, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_with(ui, icon, title, |_| {}, add)
}

/// Like [`card`], with extra widgets right-aligned in the header.
pub fn card_with<R>(
    ui: &mut Ui,
    icon: &str,
    title: &str,
    header_right: impl FnOnce(&mut Ui),
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).font(font(14.0)).color(TEXT_FAINT));
                ui.label(egui::RichText::new(title).font(font_sb(11.5)).color(TEXT_DIM).extra_letter_spacing(1.2));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), header_right);
            });
            ui.add_space(6.0);
            add(ui)
        })
        .inner
}

/// Points along an arc (angles in radians, 0 = right, clockwise because y grows downward).
fn arc_points(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = ((a1 - a0).abs() * r / 3.0).clamp(4.0, 96.0) as usize;
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + r * Vec2::angled(a)
        })
        .collect()
}

pub fn arc(painter: &egui::Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    if (a1 - a0).abs() > 0.001 {
        painter.add(Shape::line(arc_points(c, r, a0, a1), stroke));
    }
}

/// Soft glow: concentric translucent circles.
pub fn glow(painter: &egui::Painter, c: Pos2, r: f32, color: Color32, strength: f32) {
    for i in 0..6 {
        let t = i as f32 / 6.0;
        painter.circle_filled(c, r * (1.0 + 1.6 * (1.0 - t)), with_alpha(color, 0.05 * strength * (0.4 + t)));
    }
}

pub fn led(painter: &egui::Painter, c: Pos2, r: f32, on: bool, color: Color32) {
    if on {
        glow(painter, c, r, color, 1.0);
        painter.circle_filled(c, r, color);
        painter.circle_filled(c + vec2(-r * 0.3, -r * 0.3), r * 0.35, with_alpha(Color32::WHITE, 0.55));
    } else {
        painter.circle_filled(c, r, mix(CARD_HI, color, 0.12));
        painter.circle_stroke(c, r, Stroke::new(1.0, STROKE));
    }
}

const KNOB_START: f32 = 0.75 * PI; // 7:30 o'clock
const KNOB_SWEEP: f32 = 1.5 * PI;

/// A read-only rotary knob display. `flash` (0..1) highlights a recent change.
pub fn knob(ui: &mut Ui, label: &str, value: &str, frac: f32, bipolar: bool, color: Color32, flash: f32) -> Response {
    let size = vec2(78.0, 96.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect.expand(12.0));
    let c = pos2(rect.center().x, rect.top() + 36.0);
    let r = 27.0;
    let frac = frac.clamp(0.0, 1.0);
    if flash > 0.0 {
        for (i, a) in [(0.0, 0.35), (3.0, 0.2), (6.0, 0.1)] {
            p.circle_stroke(c, r + 3.0 + i, Stroke::new(3.0, with_alpha(color, a * flash)));
        }
    }
    // body
    p.circle_filled(c, r - 6.0, mix(CARD_HI, Color32::BLACK, 0.15));
    p.circle_stroke(c, r - 6.0, Stroke::new(1.0, mix(STROKE, color, 0.15 + 0.5 * flash)));
    // track
    arc(&p, c, r, KNOB_START, KNOB_START + KNOB_SWEEP, Stroke::new(4.0, STROKE));
    // value arc
    let a_val = KNOB_START + KNOB_SWEEP * frac;
    let a_from = if bipolar { KNOB_START + KNOB_SWEEP * 0.5 } else { KNOB_START };
    let (a0, a1) = if a_val < a_from { (a_val, a_from) } else { (a_from, a_val) };
    arc(&p, c, r, a0, a1, Stroke::new(4.0, mix(color, Color32::WHITE, 0.25 * flash)));
    // pointer
    let dir = Vec2::angled(a_val);
    p.line_segment([c + dir * (r - 16.0), c + dir * (r - 8.0)], Stroke::new(2.5, TEXT));
    // value
    p.text(c, Align2::CENTER_CENTER, value, mono_b(if value.len() > 4 { 10.0 } else { 13.0 }), TEXT);
    // label
    p.text(
        pos2(rect.center().x, rect.bottom() - 12.0),
        Align2::CENTER_CENTER,
        label,
        font_sb(10.5),
        mix(TEXT_DIM, color, 0.6 * flash),
    );
    resp
}

/// A choice parameter shown as a pill, same footprint as a knob.
pub fn choice(ui: &mut Ui, label: &str, value: &str, color: Color32, flash: f32) -> Response {
    let size = vec2(78.0, 96.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect.expand(8.0));
    let pill = Rect::from_center_size(pos2(rect.center().x, rect.top() + 36.0), vec2(72.0, 30.0));
    p.rect(
        pill,
        CornerRadius::same(15),
        mix(CARD_HI, color, 0.12 + 0.25 * flash),
        Stroke::new(1.0, mix(STROKE, color, 0.5)),
        StrokeKind::Inside,
    );
    p.text(pill.center(), Align2::CENTER_CENTER, value, font_b(12.0), TEXT);
    p.text(pos2(rect.center().x, rect.bottom() - 12.0), Align2::CENTER_CENTER, label, font_sb(10.5), TEXT_DIM);
    resp
}

/// Horizontal level bar.
pub fn bar(ui: &mut Ui, frac: Option<f32>, color: Color32, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 6.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(3), CARD_HI);
    if let Some(f) = frac {
        let mut fill = rect;
        fill.set_width((rect.width() * f.clamp(0.0, 1.0)).max(6.0));
        p.rect_filled(fill, CornerRadius::same(3), color);
    }
    resp
}

/// Labelled level bar row: `LABEL ▓▓▓▓░░ 64`.
pub fn level_row(ui: &mut Ui, label: &str, v: Option<u8>, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_sized([86.0, 16.0], egui::Label::new(egui::RichText::new(label).font(font_sb(11.0)).color(TEXT_DIM)));
        let w = (ui.available_width() - 40.0).max(40.0);
        bar(ui, v.map(|v| v as f32 / 100.0), color, w);
        let txt = v.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
        ui.label(egui::RichText::new(txt).font(mono(12.0)).color(TEXT));
    });
}

/// A small rounded tag.
pub fn chip(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font_b(10.5), fg);
    let size = galley.size() + vec2(14.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// A toggle chip for filters, with a coloured dot.
pub fn filter_chip(ui: &mut Ui, on: &mut bool, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font_sb(11.5), if *on { TEXT } else { TEXT_FAINT });
    let size = galley.size() + vec2(30.0, 10.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let hovered = resp.hovered();
    let bg = if *on { mix(CARD_HI, color, 0.16) } else { CARD };
    let stroke = if *on { mix(STROKE, color, 0.55) } else { STROKE };
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(13),
        if hovered { mix(bg, TEXT, 0.05) } else { bg },
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    let dot = pos2(rect.left() + 12.0, rect.center().y);
    p.circle_filled(dot, 3.5, if *on { color } else { TEXT_FAINT });
    p.galley(pos2(rect.left() + 21.0, rect.center().y - galley.size().y / 2.0), galley, TEXT);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Icon-only toggle/button for toolbars.
pub fn icon_button(ui: &mut Ui, icon: &str, active: bool, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 28.0), Sense::click());
    let p = ui.painter();
    let bg = if active {
        with_alpha(ACCENT, 0.18)
    } else if resp.hovered() {
        CARD_HI
    } else {
        Color32::TRANSPARENT
    };
    p.rect_filled(rect, CornerRadius::same(7), bg);
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        font(16.0),
        if active { ACCENT } else if resp.hovered() { TEXT } else { TEXT_DIM },
    );
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Tuner cents meter: −50…+50 with a green centre zone.
pub fn cents_meter(ui: &mut Ui, cents: Option<i16>, active: bool) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
    let p = ui.painter();
    let track = Rect::from_center_size(rect.center(), vec2(w - 8.0, 6.0));
    p.rect_filled(track, CornerRadius::same(3), CARD_HI);
    let zone = Rect::from_center_size(track.center(), vec2(track.width() * 0.06, 6.0));
    p.rect_filled(zone, CornerRadius::same(3), with_alpha(GOOD, 0.5));
    for i in -5..=5 {
        let x = track.center().x + track.width() / 2.0 * i as f32 / 5.0;
        let h = if i == 0 { 10.0 } else if i % 5 == 0 { 8.0 } else { 5.0 };
        p.line_segment(
            [pos2(x, track.top() - 4.0 - h), pos2(x, track.top() - 4.0)],
            Stroke::new(1.0, if i == 0 { TEXT_DIM } else { TEXT_FAINT }),
        );
    }
    if let (Some(c), true) = (cents, active) {
        let f = (c as f32 / 50.0).clamp(-1.0, 1.0);
        let x = track.center().x + track.width() / 2.0 * f;
        let col = tuning_color(c);
        let needle = pos2(x, track.center().y);
        glow(p, needle, 6.0, col, 1.2);
        p.circle_filled(needle, 6.0, col);
        p.circle_stroke(needle, 6.0, Stroke::new(2.0, BG));
    }
    p.text(pos2(track.left(), rect.bottom()), Align2::LEFT_BOTTOM, "−50", font(10.0), TEXT_FAINT);
    p.text(pos2(track.right(), rect.bottom()), Align2::RIGHT_BOTTOM, "+50", font(10.0), TEXT_FAINT);
}

pub fn tuning_color(cents: i16) -> Color32 {
    match cents.unsigned_abs() {
        0..=3 => GOOD,
        4..=15 => ACCENT,
        _ => BAD,
    }
}

/// Elide a string to fit `max_w` at `font`.
pub fn elide(ui: &Ui, text: &str, font: egui::FontId, max_w: f32) -> String {
    let fits = |s: &str| ui.painter().layout_no_wrap(s.to_owned(), font.clone(), TEXT).size().x <= max_w;
    if fits(text) {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let s: String = chars[..mid].iter().collect::<String>() + "…";
        if fits(&s) { lo = mid } else { hi = mid - 1 }
    }
    chars[..lo].iter().collect::<String>() + "…"
}
