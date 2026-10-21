//! Custom-painted widgets: cards, knobs, meters, LEDs, chips.

use std::f32::consts::PI;

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

use crate::theme::*;

/// A rounded card with an icon + title header.
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
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).font(font(14.0)).color(TEXT_FAINT));
                ui.label(egui::RichText::new(title).font(font_sb(13.0)).color(TEXT_DIM));
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

fn sense(editable: bool) -> Sense {
    if editable { Sense::click_and_drag() } else { Sense::hover() }
}

/// One wheel notch this frame as ±1 (up = +1). Consumes the scroll so an enclosing
/// scroll area doesn't move as well.
fn wheel_step(ui: &Ui) -> Option<i16> {
    let dy: f32 = ui.input(|i| {
        i.raw
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                _ => None,
            })
            .sum()
    });
    ui.ctx().input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
    (dy != 0.0).then_some(if dy > 0.0 { 1 } else { -1 })
}

/// Turn drags (up/right = more) and scroll-wheel notches on `resp` into a new value in
/// `0..=max`. Returns `Some` only when the value changes.
pub fn drag_input(ui: &Ui, resp: &Response, value: u8, max: u8) -> Option<u8> {
    let id = resp.id.with("drag-acc");
    let mut next = None;
    if resp.drag_started() {
        ui.data_mut(|d| d.insert_temp(id, value as f32));
    }
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        let speed = if fine { 0.08 } else { 0.4 } * (max as f32 / 100.0).max(0.2);
        let d = resp.drag_delta();
        let acc = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(value as f32) + (d.x - d.y) * speed;
        let acc = acc.clamp(0.0, max as f32);
        ui.data_mut(|dm| dm.insert_temp(id, acc));
        next = Some(acc.round() as u8);
    }
    if resp.hovered()
        && let Some(step) = wheel_step(ui)
    {
        next = Some((value as i16 + step).clamp(0, max as i16) as u8);
    }
    next.filter(|n| *n != value)
}

/// A rotary knob. `flash` (0..1) highlights a recent change. When `editable`, the response
/// senses drags (feed it to [`drag_input`]).
#[allow(clippy::too_many_arguments)]
pub fn knob(
    ui: &mut Ui,
    label: &str,
    value: &str,
    frac: f32,
    bipolar: bool,
    color: Color32,
    flash: f32,
    editable: bool,
) -> Response {
    let size = vec2(78.0, 96.0);
    let (rect, resp) = ui.allocate_exact_size(size, sense(editable));
    let active = resp.hovered() || resp.dragged();
    let flash = if editable && active { flash.max(0.35) } else { flash };
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
    p.text(c, Align2::CENTER_CENTER, value, font_sb(if value.len() > 4 { 10.0 } else { 13.0 }), TEXT);
    if editable {
        if resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
    }
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

/// A choice parameter shown as a pill, same footprint as a knob. Clickable when `editable`.
pub fn choice(ui: &mut Ui, label: &str, value: &str, color: Color32, flash: f32, editable: bool) -> Response {
    let size = vec2(78.0, 96.0);
    let (rect, resp) = ui.allocate_exact_size(size, if editable { Sense::click() } else { Sense::hover() });
    let flash = if editable && resp.hovered() { flash.max(0.35) } else { flash };
    let p = ui.painter_at(rect.expand(8.0));
    let pill = Rect::from_center_size(pos2(rect.center().x, rect.top() + 36.0), vec2(72.0, 30.0));
    p.rect_filled(pill, CornerRadius::same(15), mix(CARD_HI, color, 0.18 + 0.25 * flash));
    p.text(pill.center(), Align2::CENTER_CENTER, value, font_b(12.0), TEXT);
    p.text(pos2(rect.center().x, rect.bottom() - 12.0), Align2::CENTER_CENTER, label, font_sb(10.5), TEXT_DIM);
    if editable { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Horizontal level bar. When `editable`, a taller hit area senses clicks and drags.
pub fn bar(ui: &mut Ui, frac: Option<f32>, color: Color32, width: f32, editable: bool) -> Response {
    let (hit, resp) = ui.allocate_exact_size(vec2(width, if editable { 16.0 } else { 6.0 }), sense(editable));
    let rect = Rect::from_center_size(hit.center(), vec2(width, 6.0));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(3), CARD_HI);
    if let Some(f) = frac {
        let mut fill = rect;
        fill.set_width((rect.width() * f.clamp(0.0, 1.0)).max(6.0));
        p.rect_filled(fill, CornerRadius::same(3), color);
        if editable && (resp.hovered() || resp.dragged()) {
            p.circle_filled(pos2(fill.right() - 3.0, rect.center().y), 6.0, mix(color, Color32::WHITE, 0.3));
        }
    }
    if editable { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Labelled level bar row (`LABEL ▓▓▓▓░░ 64`) in `0..=max`; when `editable`, click/drag on the bar or scroll to set.
/// Returns the new value when the user changes it.
pub fn level_slider(
    ui: &mut Ui,
    label: &str,
    v: Option<u8>,
    max: u8,
    fmt: &dyn Fn(u8) -> String,
    color: Color32,
    editable: bool,
) -> Option<u8> {
    let editable = editable && v.is_some();
    ui.horizontal(|ui| {
        ui.add_sized([86.0, 16.0], egui::Label::new(egui::RichText::new(label).font(font_sb(11.0)).color(TEXT_DIM)));
        let w = (ui.available_width() - 52.0).max(40.0);
        let resp = bar(ui, v.map(|v| v as f32 / max as f32), color, w, editable);
        let txt = v.map(fmt).unwrap_or_else(|| "—".into());
        ui.label(egui::RichText::new(txt).font(font(12.0)).color(TEXT));
        let v = v?;
        if !editable {
            return None;
        }
        if (resp.clicked() || resp.dragged())
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let f = ((pos.x - resp.rect.left()) / resp.rect.width()).clamp(0.0, 1.0);
            let n = (f * max as f32).round() as u8;
            return (n != v).then_some(n);
        }
        if resp.hovered()
            && let Some(step) = wheel_step(ui)
        {
            let n = (v as i16 + step).clamp(0, max as i16) as u8;
            return (n != v).then_some(n);
        }
        None
    })
    .inner
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
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(13), if hovered { mix(bg, TEXT, 0.05) } else { bg });
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
