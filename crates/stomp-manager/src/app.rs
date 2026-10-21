//! The main window.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Align, Align2, Color32, CornerRadius, Layout, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use egui_phosphor::regular as icon;
use stomp_proto::catalog::{ParamKind, drum_pattern_name};
use stomp_proto::records::DEFAULT_CHAIN;
use stomp_proto::state::preset_label;
use stomp_proto::{Category, DeviceState, Module, Origin};

use crate::device::{self, Command, Status, Update};
use crate::logbook::{Filter, LogBook, Row, RowKind};
use crate::theme::*;
use crate::widgets::*;

/// Seconds a change keeps glowing.
const FLASH_SECS: f64 = 0.9;

pub struct Options {
    pub replay: Option<PathBuf>,
    pub speed: f32,
}

pub struct StompApp {
    dev: device::Handle,
    state: DeviceState,
    log: LogBook,
    filter: Filter,
    status: Status,
    ports: Vec<String>,
    port_choice: Option<String>,
    sync: (usize, usize),
    listen_only: bool,
    /// Module shown in the detail card; `None` follows the pedal.
    pinned: Option<Module>,
    followed: Module,
    module_flash: HashMap<Module, f64>,
    param_flash: HashMap<(Module, usize), f64>,
    preset_flash: f64,
    row_flash: HashMap<u64, f64>,
    rx_flash: f64,
    tx_flash: f64,
    scrolled_to: Option<u8>,
    autoscroll: bool,
    show_raw: bool,
    replay_path: String,
    replay_speed: f32,
    view_cache: (u64, Filter, Vec<usize>),
}

impl StompApp {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: Options) -> Self {
        install(&cc.egui_ctx);
        let dev = device::spawn(cc.egui_ctx.clone());
        let default_log = PathBuf::from("captures/logs/01-main-session.log");
        let replay_path = opts
            .replay
            .clone()
            .or_else(|| default_log.exists().then_some(default_log))
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        if let Some(path) = opts.replay {
            dev.send(Command::Replay { path, speed: opts.speed });
        }
        StompApp {
            dev,
            state: DeviceState::default(),
            log: LogBook::default(),
            filter: Filter::default(),
            status: Status::Disconnected,
            ports: vec![],
            port_choice: None,
            sync: (0, 0),
            listen_only: false,
            pinned: None,
            followed: Module::Amp,
            module_flash: HashMap::new(),
            param_flash: HashMap::new(),
            preset_flash: -10.0,
            row_flash: HashMap::new(),
            rx_flash: -10.0,
            tx_flash: -10.0,
            scrolled_to: None,
            autoscroll: true,
            show_raw: false,
            replay_path,
            replay_speed: opts.speed,
            view_cache: (u64::MAX, Filter::default(), vec![]),
        }
    }

    fn poll(&mut self, now: f64) {
        while let Ok(u) = self.dev.rx.try_recv() {
            match u {
                Update::Ports(p) => {
                    self.ports = p;
                    if self.port_choice.as_ref().is_none_or(|c| !self.ports.contains(c)) {
                        self.port_choice =
                            self.ports.iter().find(|p| device::is_stomp_port(p)).or(self.ports.first()).cloned();
                    }
                }
                Update::Status(s) => self.status = s,
                Update::Sync { done, total } => self.sync = (done, total),
                Update::Reset => {
                    self.state = DeviceState::default();
                    self.scrolled_to = None;
                }
                Update::Mark { at, text } => self.log.push_note(at, RowKind::Mark, text),
                Update::Info { at, text, error } => {
                    self.log.push_note(at, if error { RowKind::Error } else { RowKind::Info }, text)
                }
                Update::Message { at, origin, bytes } => {
                    match origin {
                        Origin::Pedal => self.rx_flash = now,
                        Origin::Host => self.tx_flash = now,
                    }
                    let ev = self.state.apply(&bytes, origin);
                    if !ev.redundant {
                        self.note_activity(&ev, &bytes, now);
                    }
                    self.log.push_event(at, ev);
                    if let Some(r) = self.log.rows.last() {
                        self.row_flash.insert(r.id, now);
                    }
                }
            }
        }
        if self.row_flash.len() > 256 {
            self.row_flash.retain(|_, t| now - *t < FLASH_SECS);
        }
    }

    fn note_activity(&mut self, ev: &stomp_proto::Event, bytes: &[u8], now: f64) {
        match ev.category {
            Category::Effect(m) => {
                self.module_flash.insert(m, now);
                self.followed = m;
                if bytes.len() == 3
                    && bytes[0] & 0xF0 == 0xB0
                    && let Some((pm, slot)) = Module::from_param_cc(bytes[1])
                {
                    self.param_flash.insert((pm, slot), now);
                }
            }
            Category::Looper => {
                self.module_flash.insert(Module::Loop, now);
            }
            Category::Drum => {
                self.module_flash.insert(Module::Drum, now);
            }
            Category::Preset => self.preset_flash = now,
            Category::Volume => {
                self.preset_flash = now;
            }
            Category::Navigation => {
                if let Some(p) = self.state.page {
                    self.followed = p;
                    self.module_flash.insert(p, now);
                }
            }
            _ => {}
        }
    }

    fn flash(t: f64, now: f64) -> f32 {
        let d = now - t;
        if (0.0..FLASH_SECS).contains(&d) { (1.0 - d / FLASH_SECS).powf(1.6) as f32 } else { 0.0 }
    }

    fn mod_flash(&self, m: Module, now: f64) -> f32 {
        self.module_flash.get(&m).map_or(0.0, |t| Self::flash(*t, now))
    }

    fn animating(&self, now: f64) -> bool {
        let recent = |t: &f64| now - *t < FLASH_SECS;
        self.module_flash.values().any(recent)
            || self.param_flash.values().any(recent)
            || self.row_flash.values().any(recent)
            || recent(&self.preset_flash)
            || now - self.rx_flash < 0.3
            || now - self.tx_flash < 0.3
            || self.state.looper.state == 1
            || self.state.looper.state == 3
    }

    // ───────────────────────────── top bar ─────────────────────────────

    fn top_bar(&mut self, ui: &mut Ui, now: f64) {
        ui.horizontal_centered(|ui| {
            // logo
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(r, CornerRadius::same(9), ACCENT);
            p.circle_stroke(r.center() + vec2(-6.0, 0.0), 6.5, Stroke::new(2.5, BG));
            p.circle_stroke(r.center() + vec2(6.0, 0.0), 6.5, Stroke::new(2.5, BG));
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(RichText::new("Dual Stomp").font(font_b(16.0)).color(TEXT));
                ui.label(RichText::new("MANAGER · MONITOR").font(font_sb(9.5)).color(TEXT_FAINT).extra_letter_spacing(1.5));
            });
            ui.add_space(18.0);
            self.status_pill(ui, now);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.connection_controls(ui);
            });
        });
    }

    fn status_pill(&mut self, ui: &mut Ui, now: f64) {
        let (dot, text, sub) = match &self.status {
            Status::Connected { .. } => {
                let name = self.state.product.clone().unwrap_or_else(|| "Pedal".into());
                let fw = self.state.firmware.clone().unwrap_or_default();
                (GOOD, format!("Connected · {name}"), fw)
            }
            Status::Replaying { file, progress } => {
                (MARK, format!("Replaying {file}"), format!("{:.0}%", progress * 100.0))
            }
            Status::Disconnected => {
                let pulse = 0.5 + 0.5 * (now * 3.0).sin() as f32;
                (mix(TEXT_FAINT, ACCENT, pulse), "Searching for pedal…".into(), "plug in via USB".into())
            }
        };
        egui::Frame::new()
            .fill(CARD)
            .stroke(Stroke::new(1.0, STROKE))
            .corner_radius(CornerRadius::same(18))
            .inner_margin(egui::Margin::symmetric(14, 7))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 18.0), Sense::hover());
                    led(ui.painter(), r.center(), 4.5, true, dot);
                    ui.label(RichText::new(text).font(font_sb(13.0)).color(TEXT));
                    if !sub.is_empty() {
                        ui.label(RichText::new(sub).font(mono(11.5)).color(TEXT_DIM));
                    }
                    let (done, total) = self.sync;
                    if total > 0 {
                        ui.add_space(6.0);
                        ui.add(
                            egui::ProgressBar::new(done as f32 / total as f32)
                                .desired_width(110.0)
                                .desired_height(6.0)
                                .fill(ACCENT)
                                .corner_radius(CornerRadius::same(3)),
                        );
                        ui.label(RichText::new(format!("syncing {done}/{total}")).font(font(11.0)).color(TEXT_DIM));
                    }
                    // traffic LEDs
                    ui.add_space(6.0);
                    for (label, t, col) in [("IN", self.rx_flash, ACCENT), ("OUT", self.tx_flash, HOST)] {
                        let (r, _) = ui.allocate_exact_size(vec2(8.0, 18.0), Sense::hover());
                        led(ui.painter(), r.center(), 3.5, now - t < 0.12, col);
                        ui.label(RichText::new(label).font(font_sb(9.5)).color(TEXT_FAINT));
                    }
                });
            });
    }

    fn connection_controls(&mut self, ui: &mut Ui) {
        // (right-to-left layout: first added = rightmost)
        let replaying = matches!(self.status, Status::Replaying { .. });
        if replaying {
            if ui.button(format!("{}  Stop replay", icon::STOP)).clicked() {
                self.dev.send(Command::StopReplay);
            }
        } else {
            ui.menu_button(format!("{}  Replay capture", icon::FILM_STRIP), |ui| {
                ui.set_min_width(360.0);
                ui.label(RichText::new("Play back a capture log from captures/logs/").color(TEXT_DIM));
                ui.add(egui::TextEdit::singleline(&mut self.replay_path).hint_text("path/to/capture.log").desired_width(340.0));
                ui.horizontal(|ui| {
                    ui.label("Speed");
                    ui.add(egui::Slider::new(&mut self.replay_speed, 0.25..=50.0).logarithmic(true).suffix("×"));
                });
                if ui.button(format!("{}  Start", icon::PLAY)).clicked() && !self.replay_path.is_empty() {
                    self.dev.send(Command::Replay { path: PathBuf::from(&self.replay_path), speed: self.replay_speed });
                    ui.close();
                }
            });
        }

        ui.add_space(4.0);
        let connected = matches!(self.status, Status::Connected { .. });
        if connected {
            if ui.button(format!("{}  Disconnect", icon::PLUGS)).clicked() {
                self.dev.send(Command::Disconnect);
            }
            if ui
                .button(icon::ARROWS_CLOCKWISE)
                .on_hover_text("Re-read presets and state from the pedal")
                .clicked()
            {
                self.dev.send(Command::Resync);
            }
        } else if let Some(port) = self.port_choice.clone()
            && ui.button(format!("{}  Connect", icon::PLUGS_CONNECTED)).clicked()
        {
            self.dev.send(Command::Connect(port));
        }

        let label = self.port_choice.clone().unwrap_or_else(|| "No MIDI ports".into());
        egui::ComboBox::from_id_salt("port")
            .selected_text(RichText::new(elide(ui, &label, font(13.0), 190.0)).font(font(13.0)))
            .width(210.0)
            .show_ui(ui, |ui| {
                for p in self.ports.clone() {
                    let star = if device::is_stomp_port(&p) { format!("{}  ", icon::GUITAR) } else { String::new() };
                    if ui.selectable_label(self.port_choice.as_ref() == Some(&p), format!("{star}{p}")).clicked() {
                        self.port_choice = Some(p.clone());
                        self.dev.send(Command::Connect(p));
                    }
                }
            });

        let mut listen = self.listen_only;
        if ui
            .toggle_value(&mut listen, format!("{}  Listen only", icon::EAR))
            .on_hover_text(
                "When off, the app reads presets and state from the pedal (read-only SysEx requests, \
                 the same ones the official app sends on connect). When on, it never sends anything.",
            )
            .changed()
        {
            self.listen_only = listen;
            self.dev.send(Command::SetSync(!listen));
            if !listen {
                self.dev.send(Command::Resync);
            }
        }
    }

    // ───────────────────────────── preset list ─────────────────────────────

    fn preset_list(&mut self, ui: &mut Ui, now: f64) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(icon::LIST).font(font(14.0)).color(TEXT_FAINT));
            ui.label(RichText::new("PRESETS").font(font_sb(11.5)).color(TEXT_DIM).extra_letter_spacing(1.2));
            let known = self.state.presets.iter().filter(|p| p.is_some()).count();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{known}/99")).font(mono(11.0)).color(TEXT_FAINT));
            });
        });
        ui.add_space(6.0);
        let current = self.state.current;
        let scroll_now = current.is_some() && current != self.scrolled_to;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for i in 0..99u8 {
                let active = current == Some(i);
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
                let p = ui.painter();
                let fl = if active { Self::flash(self.preset_flash, now) } else { 0.0 };
                if active {
                    p.rect_filled(rect, CornerRadius::same(8), mix(CARD_HI, ACCENT, 0.10 + 0.25 * fl));
                    p.rect_filled(
                        egui::Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(3.0, rect.height() - 12.0)),
                        CornerRadius::same(2),
                        ACCENT,
                    );
                } else if resp.hovered() {
                    p.rect_filled(rect, CornerRadius::same(8), CARD);
                }
                p.text(
                    pos2(rect.left() + 14.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    preset_label(i),
                    if active { mono_b(12.0) } else { mono(12.0) },
                    if active { ACCENT } else { TEXT_FAINT },
                );
                let (name, col) = match self.state.preset_name(i) {
                    Some("") => ("(unnamed)".to_owned(), TEXT_FAINT),
                    Some(n) => (n.to_owned(), if active { TEXT } else { TEXT_DIM }),
                    None => ("—".to_owned(), STROKE),
                };
                let f = if active { font_sb(13.0) } else { font(13.0) };
                let name = elide(ui, &name, f.clone(), rect.width() - 70.0);
                p.text(pos2(rect.left() + 52.0, rect.center().y), Align2::LEFT_CENTER, name, f, col);
                if active && self.state.modified() {
                    p.circle_filled(pos2(rect.right() - 12.0, rect.center().y), 3.5, ACCENT);
                }
                if active && scroll_now {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                }
            }
        });
        if scroll_now {
            self.scrolled_to = current;
        }
    }

    // ───────────────────────────── main view ─────────────────────────────

    fn main_view(&mut self, ui: &mut Ui, now: f64) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 14.0;
            self.hero(ui, now);
            self.chain(ui, now);
            self.detail(ui, now);
            let w = ui.available_width();
            if w > 900.0 {
                ui.columns(3, |c| {
                    self.tuner_card(&mut c[0]);
                    self.looper_card(&mut c[1], now);
                    self.drum_card(&mut c[2]);
                });
            } else {
                self.tuner_card(ui);
                ui.columns(2, |c| {
                    self.looper_card(&mut c[0], now);
                    self.drum_card(&mut c[1]);
                });
            }
            self.device_card(ui);
            ui.add_space(8.0);
        });
    }

    fn hero(&mut self, ui: &mut Ui, now: f64) {
        let fl = Self::flash(self.preset_flash, now);
        egui::Frame::new()
            .fill(mix(CARD, ACCENT, 0.05 + 0.10 * fl))
            .stroke(Stroke::new(1.0, mix(STROKE, ACCENT, 0.25 + 0.5 * fl)))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(egui::Margin::symmetric(24, 18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let Some(cur) = self.state.current else {
                    self.hero_empty(ui);
                    return;
                };
                ui.horizontal(|ui| {
                    // big preset number, LCD style
                    let (r, _) = ui.allocate_exact_size(vec2(150.0, 92.0), Sense::hover());
                    let p = ui.painter();
                    p.rect(r, CornerRadius::same(12), BG, Stroke::new(1.0, STROKE), StrokeKind::Inside);
                    p.text(r.center() + vec2(0.0, 2.0), Align2::CENTER_CENTER, "888", mono_b(54.0), with_alpha(ACCENT, 0.05));
                    p.text(
                        r.center() + vec2(0.0, 2.0),
                        Align2::CENTER_CENTER,
                        format!("{:02}", cur as u16 + 1),
                        mono_b(54.0),
                        mix(ACCENT, Color32::WHITE, 0.3 * fl),
                    );
                    p.text(r.left_top() + vec2(10.0, 8.0), Align2::LEFT_TOP, "PRESET", font_sb(9.0), TEXT_FAINT);
                    ui.add_space(18.0);
                    let name_w = (ui.available_width() - 80.0).max(120.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(name_w);
                        ui.add_space(8.0);
                        let name = if self.state.live.name.is_empty() {
                            self.state.preset_name(cur).unwrap_or("").to_owned()
                        } else {
                            self.state.live.name.clone()
                        };
                        let name = if name.is_empty() { "Untitled".to_owned() } else { name };
                        let name = elide(ui, &name, font_b(34.0), name_w);
                        ui.label(RichText::new(name).font(font_b(34.0)).color(TEXT));
                        ui.add_space(2.0);
                        ui.horizontal_wrapped(|ui| {
                            if self.state.modified() {
                                chip(ui, "● EDITED", BG, ACCENT).on_hover_text("The live sound differs from the saved preset");
                            } else if self.state.live_known {
                                chip(ui, "SAVED", GOOD, with_alpha(GOOD, 0.14));
                            }
                            if let Some(page) = self.state.page {
                                chip(ui, &format!("PEDAL SHOWS {}", page.name()), TEXT_DIM, CARD_HI);
                            }
                            let on = Module::EFFECTS.iter().filter(|m| self.state.live.modules[m.index()].on).count();
                            chip(ui, &format!("{on}/8 BLOCKS ON"), TEXT_DIM, CARD_HI);
                        });
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.volume_meter(ui);
                    });
                });
            });
    }

    fn hero_empty(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(92.0, 92.0), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::same(46), CARD_HI);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, icon::PLUGS, font(40.0), ACCENT);
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.add_space(14.0);
                let (title, body) = match &self.status {
                    Status::Connected { .. } => (
                        "Waiting for the pedal…",
                        "Change a preset on the pedal, or wait for the sync to finish, to see the active preset.",
                    ),
                    Status::Replaying { .. } => ("Replaying…", "The active preset appears at the first preset change."),
                    Status::Disconnected => (
                        "No pedal connected",
                        "Plug in the Dual Stomp over USB and it connects automatically. \
                         Or use “Replay capture” to play back a recorded session.",
                    ),
                };
                ui.label(RichText::new(title).font(font_b(24.0)).color(TEXT));
                ui.label(RichText::new(body).color(TEXT_DIM));
            });
        });
    }

    fn volume_meter(&self, ui: &mut Ui) {
        let v = self.state.live_known.then_some(self.state.live.volume);
        let (r, _) = ui.allocate_exact_size(vec2(64.0, 92.0), Sense::hover());
        let p = ui.painter();
        let c = pos2(r.center().x, r.top() + 36.0);
        let frac = v.map(|v| v as f32 / 100.0).unwrap_or(0.0);
        arc(p, c, 28.0, 0.75 * std::f32::consts::PI, 2.25 * std::f32::consts::PI, Stroke::new(5.0, STROKE));
        if v.is_some() {
            arc(
                p,
                c,
                28.0,
                0.75 * std::f32::consts::PI,
                0.75 * std::f32::consts::PI + 1.5 * std::f32::consts::PI * frac,
                Stroke::new(5.0, ACCENT),
            );
        }
        let txt = v.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
        p.text(c, Align2::CENTER_CENTER, txt, mono_b(16.0), TEXT);
        p.text(pos2(r.center().x, r.bottom() - 8.0), Align2::CENTER_CENTER, "VOLUME", font_sb(10.0), TEXT_DIM);
    }

    fn chain_order(&self) -> Vec<Module> {
        let chain = if self.state.live_known { self.state.live.chain } else { DEFAULT_CHAIN };
        let mut v = vec![Module::Gate];
        v.extend(chain.iter().filter_map(|i| Module::from_index(*i)));
        v.extend([Module::Loop, Module::Drum]);
        v
    }

    fn block_state(&self, m: Module) -> (bool, String) {
        let s = &self.state;
        match m {
            Module::Loop => (s.looper.state != 0, s.looper.state_name().to_owned()),
            Module::Drum => (
                s.drum.playing,
                s.drum.pattern.map(drum_pattern_name).unwrap_or_else(|| "Drums".into()),
            ),
            _ => {
                let st = &s.live.modules[m.index()];
                if !s.live_known && st.model == 0 && !st.on {
                    return (false, "—".into());
                }
                (st.on, s.model_name(m, st.model))
            }
        }
    }

    fn shown_module(&self) -> Module {
        self.pinned.unwrap_or(self.followed)
    }

    fn chain(&mut self, ui: &mut Ui, now: f64) {
        let order = self.chain_order();
        let shown = self.shown_module();
        let mut clicked = None;
        card(ui, icon::FLOW_ARROW, "SIGNAL CHAIN", |ui| {
            // fit all blocks on one row if possible, otherwise wrap onto several rows
            let n = order.len();
            let gap = 12.0;
            let avail = ui.available_width();
            let per_row = (((avail + gap) / (78.0 + gap)).floor() as usize).clamp(1, n);
            let rows = n.div_ceil(per_row);
            let per_row = n.div_ceil(rows);
            let w = ((avail - gap * (per_row as f32 - 1.0)) / per_row as f32).min(150.0);
            let h = 104.0;
            let row_gap = 18.0;
            let total = w * per_row as f32 + gap * (per_row as f32 - 1.0);
            let (area, _) =
                ui.allocate_exact_size(vec2(avail, rows as f32 * h + (rows - 1) as f32 * row_gap + 10.0), Sense::hover());
            let x0 = area.left() + (area.width() - total).max(0.0) / 2.0;
            let mut prev_right: Option<egui::Pos2> = None;
            for (k, m) in order.iter().enumerate() {
                let (row, col_i) = (k / per_row, k % per_row);
                let rect = egui::Rect::from_min_size(
                    pos2(x0 + col_i as f32 * (w + gap), area.top() + 4.0 + row as f32 * (h + row_gap)),
                    vec2(w, h),
                );
                if col_i == 0 {
                    prev_right = None;
                }
                let resp = ui.interact(rect, ui.id().with(("block", k)), Sense::click());
                let (on, model) = self.block_state(*m);
                let col = module_color(*m);
                let fl = self.mod_flash(*m, now);
                let p = ui.painter();
                if let Some(pr) = prev_right {
                    let y = rect.center().y;
                    let wire = if k >= order.len() - 2 { STROKE } else { mix(STROKE, TEXT_FAINT, 0.4) };
                    p.line_segment([pos2(pr.x + 2.0, y), pos2(rect.left() - 2.0, y)], Stroke::new(2.0, wire));
                }
                prev_right = Some(rect.right_center());
                if fl > 0.0 {
                    p.rect_filled(rect.expand(4.0 * fl), CornerRadius::same(14), with_alpha(col, 0.25 * fl));
                }
                let fill = if on { mix(CARD_HI, col, 0.22) } else { mix(BG, CARD, 0.6) };
                let selected = *m == shown;
                let stroke = if selected {
                    Stroke::new(2.0, mix(col, Color32::WHITE, 0.35))
                } else if on {
                    Stroke::new(1.0, mix(STROKE, col, 0.6))
                } else {
                    Stroke::new(1.0, STROKE)
                };
                p.rect(rect, CornerRadius::same(12), fill, stroke, StrokeKind::Inside);
                if resp.hovered() {
                    p.rect_filled(rect, CornerRadius::same(12), with_alpha(Color32::WHITE, 0.03));
                }
                led(p, rect.left_top() + vec2(14.0, 14.0), 4.0, on, col);
                p.text(
                    rect.right_top() + vec2(-10.0, 13.0),
                    Align2::RIGHT_CENTER,
                    module_icon(*m),
                    font(15.0),
                    if on { col } else { TEXT_FAINT },
                );
                p.text(
                    pos2(rect.center().x, rect.top() + 46.0),
                    Align2::CENTER_CENTER,
                    m.name(),
                    font_b(if w < 80.0 { 12.0 } else { 14.0 }),
                    if on { TEXT } else { TEXT_DIM },
                );
                let model_txt = elide(ui, &model, font(11.0), w - 12.0);
                p.text(
                    pos2(rect.center().x, rect.top() + 68.0),
                    Align2::CENTER_CENTER,
                    model_txt,
                    font(11.0),
                    if on { mix(TEXT_DIM, col, 0.35) } else { TEXT_FAINT },
                );
                let status = if on { "ON" } else { "OFF" };
                p.text(
                    pos2(rect.center().x, rect.bottom() - 14.0),
                    Align2::CENTER_CENTER,
                    status,
                    font_b(9.5),
                    if on { col } else { TEXT_FAINT },
                );
                if selected {
                    let c = pos2(rect.center().x, rect.bottom() + 6.0);
                    p.add(egui::Shape::convex_polygon(
                        vec![c + vec2(-6.0, 0.0), c + vec2(6.0, 0.0), c + vec2(0.0, -5.0)],
                        mix(col, Color32::WHITE, 0.35),
                        Stroke::NONE,
                    ));
                }
                let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(format!(
                    "{} · {model}\nClick to pin this block in the detail view",
                    m.name()
                ));
                if resp.clicked() {
                    clicked = Some(*m);
                }
            }
        });
        if let Some(m) = clicked {
            self.pinned = if self.pinned == Some(m) { None } else { Some(m) };
        }
    }

    fn detail(&mut self, ui: &mut Ui, now: f64) {
        let m = self.shown_module();
        let col = module_color(m);
        let pinned = self.pinned.is_some();
        let mut unpin = false;
        let title = format!("{} DETAIL", m.name());
        card_with(
            ui,
            module_icon(m),
            &title,
            |ui| {
                if pinned {
                    if ui.button(format!("{}  Pinned · follow pedal", icon::PUSH_PIN)).clicked() {
                        unpin = true;
                    }
                } else {
                    ui.label(RichText::new(format!("{}  following pedal activity", icon::BROADCAST)).font(font(11.5)).color(TEXT_FAINT));
                }
            },
            |ui| match m {
                Module::Loop => self.looper_body(ui, now),
                Module::Drum => self.drum_body(ui),
                _ => self.effect_body(ui, m, col, now),
            },
        );
        if unpin {
            self.pinned = None;
        }
    }

    fn effect_body(&self, ui: &mut Ui, m: Module, col: Color32, now: f64) {
        let st = self.state.live.modules[m.index()];
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("#{:02}", st.model + 1)).font(mono_b(13.0)).color(col));
            ui.label(RichText::new(self.state.model_name(m, st.model)).font(font_b(22.0)).color(TEXT));
            ui.add_space(8.0);
            if st.on {
                chip(ui, "ON", BG, col);
            } else {
                chip(ui, "BYPASSED", TEXT_DIM, CARD_HI);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let models = m.models().len();
                ui.label(RichText::new(format!("{} models", models)).font(font(11.5)).color(TEXT_FAINT));
            });
        });
        ui.add_space(6.0);
        let params = m.params(st.model);
        if params.is_empty() {
            ui.label(RichText::new("No parameters").color(TEXT_FAINT));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for pd in params {
                let slot = pd.slot as usize;
                let v = st.params[slot];
                let fl = self.param_flash.get(&(m, slot)).map_or(0.0, |t| Self::flash(*t, now));
                let known = self.state.live_known || fl > 0.0 || v != 0;
                let txt = if known { pd.format(v) } else { "—".into() };
                match pd.kind {
                    ParamKind::Enum(_) => choice(ui, pd.name, &txt, col, fl),
                    ParamKind::Knob => knob(ui, pd.name, &txt, v as f32 / 100.0, false, col, fl),
                    ParamKind::Bipolar => knob(ui, pd.name, &txt, v as f32 / 100.0, true, col, fl),
                };
            }
        });
    }

    fn tuner_card(&self, ui: &mut Ui) {
        let t = &self.state.tuner;
        card(ui, icon::MUSIC_NOTES, "TUNER", |ui| {
            ui.set_min_height(150.0);
            let live = t.detected && t.note.is_some();
            ui.horizontal(|ui| {
                let note = if live { stomp_proto::catalog::NOTES[t.note.unwrap() as usize] } else { "—" };
                let col = match (live, t.cents_offset()) {
                    (true, Some(c)) => tuning_color(c),
                    (true, None) => TEXT,
                    _ => TEXT_FAINT,
                };
                ui.label(RichText::new(note).font(font_b(52.0)).color(col));
                ui.vertical(|ui| {
                    ui.add_space(10.0);
                    let cents = match (live, t.cents_offset()) {
                        (true, Some(c)) => format!("{c:+} ¢"),
                        _ => "—".into(),
                    };
                    ui.label(RichText::new(cents).font(mono_b(18.0)).color(col));
                    let status = if t.active || live { "listening" } else { "tuner off" };
                    ui.label(RichText::new(status).font(font(11.5)).color(TEXT_FAINT));
                });
            });
            cents_meter(ui, t.cents_offset(), live);
        });
    }

    fn looper_card(&self, ui: &mut Ui, now: f64) {
        card(ui, icon::CASSETTE_TAPE, "LOOPER", |ui| {
            ui.set_min_height(150.0);
            self.looper_body(ui, now);
        });
    }

    fn looper_body(&self, ui: &mut Ui, now: f64) {
        let l = &self.state.looper;
        let (ic, col) = match l.state {
            1 => (icon::RECORD, BAD),
            2 => (icon::PLAY, GOOD),
            3 => (icon::RECORD, ACCENT),
            4 => (icon::STOP, TEXT_DIM),
            _ => (icon::CASSETTE_TAPE, TEXT_FAINT),
        };
        let pulse = if l.state == 1 || l.state == 3 { 0.6 + 0.4 * (now * 6.0).sin().abs() as f32 } else { 1.0 };
        ui.horizontal(|ui| {
            ui.label(RichText::new(ic).font(font(34.0)).color(with_alpha(col, pulse)));
            ui.vertical(|ui| {
                ui.label(RichText::new(l.state_name().to_uppercase()).font(font_b(20.0)).color(TEXT));
                let layers = match l.layers {
                    1 => "overdub layer · undo available",
                    2 => "base layer",
                    _ => "no layers",
                };
                ui.label(RichText::new(layers).font(font(11.5)).color(TEXT_FAINT));
            });
        });
        ui.add_space(4.0);
        level_row(ui, "REC LEVEL", l.rec_level, module_color(Module::Loop));
        level_row(ui, "PLAYBACK", l.play_level, module_color(Module::Loop));
        ui.horizontal(|ui| {
            let sync = match l.sync {
                Some(true) => "SYNC ON",
                Some(false) => "SYNC OFF",
                None => "SYNC —",
            };
            chip(ui, sync, TEXT_DIM, CARD_HI);
            let mode = match l.mode {
                Some(0) => "MODE NR",
                Some(_) => "MODE AR",
                None => "MODE —",
            };
            chip(ui, mode, TEXT_DIM, CARD_HI);
        });
    }

    fn drum_card(&self, ui: &mut Ui) {
        card(ui, icon::METRONOME, "DRUM MACHINE", |ui| {
            ui.set_min_height(150.0);
            self.drum_body(ui);
        });
    }

    fn drum_body(&self, ui: &mut Ui) {
        let d = &self.state.drum;
        let col = module_color(Module::Drum);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if d.playing { icon::PLAY } else { icon::STOP })
                    .font(font(34.0))
                    .color(if d.playing { col } else { TEXT_FAINT }),
            );
            ui.vertical(|ui| {
                let pat = d.pattern.map(drum_pattern_name).unwrap_or_else(|| "—".into());
                ui.label(RichText::new(pat).font(font_b(20.0)).color(TEXT));
                ui.label(RichText::new(if d.playing { "playing" } else { "stopped" }).font(font(11.5)).color(TEXT_FAINT));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("BPM").font(font_sb(10.5)).color(TEXT_DIM));
                let bpm = d.bpm.map(|b| b.to_string()).unwrap_or_else(|| "—".into());
                ui.label(RichText::new(bpm).font(mono_b(30.0)).color(TEXT));
            });
        });
        ui.add_space(4.0);
        level_row(ui, "LEVEL", d.level, col);
    }

    fn device_card(&self, ui: &mut Ui) {
        card(ui, icon::GEAR, "DEVICE & SYSTEM SETTINGS", |ui| {
            if ui.available_width() > 820.0 {
                ui.columns(3, |c| {
                    self.device_info(&mut c[0]);
                    self.global_eq(&mut c[1]);
                    self.user_slots(&mut c[2]);
                });
            } else {
                ui.columns(2, |c| {
                    self.device_info(&mut c[0]);
                    self.global_eq(&mut c[1]);
                });
                ui.add_space(10.0);
                self.user_slots(ui);
            }
        });
    }

    fn device_info(&self, ui: &mut Ui) {
        let s = &self.state;
        let kv = |ui: &mut Ui, k: &str, v: Option<String>| {
            ui.horizontal(|ui| {
                ui.add_sized([86.0, 18.0], egui::Label::new(RichText::new(k).font(font_sb(11.0)).color(TEXT_DIM)));
                let v = v.unwrap_or_else(|| "—".into());
                ui.label(RichText::new(elide(ui, &v, mono(12.5), ui.available_width())).font(mono(12.5)).color(TEXT));
            });
        };
        let set = s.settings.as_ref();
        let onoff = |b: bool| if b { "On".to_owned() } else { "Off".to_owned() };
        kv(ui, "PRODUCT", s.product.clone());
        kv(ui, "FIRMWARE", s.firmware.clone());
        kv(ui, "MODEL ID", s.model_id.clone());
        kv(ui, "INPUT TRIM", set.map(|x| format!("{:+} dB", x.input_trim_db())));
        kv(ui, "WIRELESS", set.map(|x| onoff(x.wireless)));
        kv(ui, "LOOPBACK", set.map(|x| onoff(x.loopback)));
    }

    fn global_eq(&self, ui: &mut Ui) {
        let set = self.state.settings.as_ref();
        ui.label(RichText::new("GLOBAL EQ").font(font_sb(11.0)).color(TEXT_DIM));
        let eq = module_color(Module::Eq);
        level_row(ui, "BASS", set.map(|x| x.eq_bass), eq);
        level_row(ui, "MID", set.map(|x| x.eq_mid), eq);
        level_row(ui, "TREBLE", set.map(|x| x.eq_treble), eq);
        ui.label(
            RichText::new("Changes made on the pedal itself are not reported over USB.").font(font(11.0)).color(TEXT_FAINT),
        );
    }

    fn user_slots(&self, ui: &mut Ui) {
        let s = &self.state;
        ui.label(RichText::new("USER SLOTS").font(font_sb(11.0)).color(TEXT_DIM));
        let name_w = ((ui.available_width() - 40.0) / 2.0).max(60.0);
        egui::Grid::new("user_slots").num_columns(3).spacing([10.0, 3.0]).show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new("AMP (NAM)").font(font_sb(10.0)).color(TEXT_FAINT));
            ui.label(RichText::new("CAB (IR)").font(font_sb(10.0)).color(TEXT_FAINT));
            ui.end_row();
            let name = |n: &Option<String>| match n.as_deref() {
                Some(n) if !n.is_empty() && !n.starts_with("--") => (n.to_owned(), TEXT),
                Some(_) => ("empty".to_owned(), TEXT_FAINT),
                None => ("—".to_owned(), TEXT_FAINT),
            };
            for i in 0..10 {
                ui.label(RichText::new(format!("{:02}", i + 1)).font(mono(11.0)).color(TEXT_FAINT));
                for (txt, c) in [name(&s.user_amps[i]), name(&s.user_irs[i])] {
                    ui.label(RichText::new(elide(ui, &txt, font(11.5), name_w)).font(font(11.5)).color(c));
                }
                ui.end_row();
            }
        });
    }

    // ───────────────────────────── activity log ─────────────────────────────

    fn log_panel(&mut self, ui: &mut Ui, now: f64) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::PULSE).font(font(14.0)).color(TEXT_FAINT));
            ui.label(RichText::new("ACTIVITY").font(font_sb(11.5)).color(TEXT_DIM).extra_letter_spacing(1.2));
            ui.label(RichText::new(format!("{}", self.view_cache.2.len())).font(mono(11.0)).color(TEXT_FAINT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, icon::TRASH, false, "Clear the log").clicked() {
                    self.log.clear();
                }
                if icon_button(ui, icon::COPY, false, "Copy the visible log as text").clicked() {
                    ui.ctx().copy_text(self.log.export_text(&self.filter));
                }
                if icon_button(ui, icon::BRACKETS_CURLY, self.show_raw, "Show raw MIDI bytes").clicked() {
                    self.show_raw = !self.show_raw;
                }
                if icon_button(ui, icon::ARROW_LINE_DOWN, self.autoscroll, "Follow new events").clicked() {
                    self.autoscroll = !self.autoscroll;
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            let f = &mut self.filter;
            filter_chip(ui, &mut f.presets, "Presets", ACCENT);
            filter_chip(ui, &mut f.effects, "Effects", module_color(Module::Amp));
            filter_chip(ui, &mut f.looper, "Looper", module_color(Module::Loop));
            filter_chip(ui, &mut f.drum, "Drum", module_color(Module::Drum));
            filter_chip(ui, &mut f.tuner, "Tuner", category_color(Category::Tuner));
            filter_chip(ui, &mut f.device, "Device", category_color(Category::Device));
            filter_chip(ui, &mut f.sysex, "SysEx", TEXT_DIM).on_hover_text("Request/reply plumbing: sync reads, acks, uploads");
            filter_chip(ui, &mut f.verbose, "Redundant", TEXT_DIM)
                .on_hover_text("Values the pedal re-sends after a preset change that were already known");
            filter_chip(ui, &mut f.pedal, "From pedal", ACCENT);
            filter_chip(ui, &mut f.host, "From host", HOST);
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::MAGNIFYING_GLASS).color(TEXT_FAINT));
            ui.add(
                egui::TextEdit::singleline(&mut self.filter.search)
                    .hint_text("Filter… (e.g. gain, looper, P05)")
                    .desired_width(f32::INFINITY),
            );
        });
        ui.add_space(6.0);

        // filtered index, recomputed only when rows or the filter change
        if self.view_cache.0 != self.log.generation || self.view_cache.1 != self.filter {
            let idx = self.log.rows.iter().enumerate().filter(|(_, r)| self.filter.accepts(r)).map(|(i, _)| i).collect();
            self.view_cache = (self.log.generation, self.filter.clone(), idx);
        }
        let row_h = if self.show_raw { 46.0 } else { 30.0 };
        let n = self.view_cache.2.len();
        egui::Frame::new().fill(BG).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
            if n == 0 {
                ui.set_min_size(ui.available_size());
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(format!("{}\nNothing yet: press a footswitch or turn a knob", icon::HAND_TAP))
                            .color(TEXT_FAINT),
                    );
                });
                return;
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(self.autoscroll)
                .show_rows(ui, row_h, n, |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for vi in range {
                        let row = &self.log.rows[self.view_cache.2[vi]];
                        let fl = self.row_flash.get(&row.id).map_or(0.0, |t| Self::flash(*t, now));
                        row_view(ui, row, row_h, vi, fl, self.show_raw);
                    }
                });
        });
    }
}

fn row_view(ui: &mut Ui, r: &Row, h: f32, vi: usize, flash: f32, raw: bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let p = ui.painter_at(rect);
    let col = category_color(r.category);
    if vi % 2 == 1 {
        p.rect_filled(rect, CornerRadius::same(6), with_alpha(CARD, 0.5));
    }
    if flash > 0.0 {
        p.rect_filled(rect, CornerRadius::same(6), with_alpha(col, 0.16 * flash));
    }
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), with_alpha(Color32::WHITE, 0.03));
    }
    let y = rect.top() + 15.0;
    let mut x = rect.left() + 8.0;
    p.text(pos2(x, y), Align2::LEFT_CENTER, r.at.format("%H:%M:%S%.3f").to_string(), mono(11.0), TEXT_FAINT);
    x += 92.0;
    match r.kind {
        RowKind::Mark => {
            let band = egui::Rect::from_min_max(pos2(x - 4.0, rect.top() + 3.0), pos2(rect.right() - 4.0, rect.bottom() - 3.0));
            p.rect_filled(band, CornerRadius::same(6), with_alpha(MARK, 0.12));
            p.text(pos2(x + 4.0, y), Align2::LEFT_CENTER, format!("{}  {}", icon::FLAG, r.subject), font_sb(12.5), MARK);
        }
        RowKind::Info | RowKind::Error => {
            let (ic, c) = if r.kind == RowKind::Error { (icon::WARNING, BAD) } else { (icon::INFO, TEXT_DIM) };
            p.text(pos2(x, y), Align2::LEFT_CENTER, format!("{ic}  {}", r.subject), font(12.5), c);
        }
        RowKind::Event => {
            // origin badge
            let (who, wc) = match r.origin {
                Origin::Pedal => ("PEDAL", ACCENT),
                Origin::Host => ("HOST", HOST),
            };
            let badge = egui::Rect::from_min_size(pos2(x, y - 8.0), vec2(42.0, 16.0));
            p.rect_filled(badge, CornerRadius::same(4), with_alpha(wc, 0.13));
            p.text(badge.center(), Align2::CENTER_CENTER, who, font_b(9.0), wc);
            x += 50.0;
            // category
            p.circle_filled(pos2(x + 3.0, y), 3.5, col);
            p.text(pos2(x + 12.0, y), Align2::LEFT_CENTER, r.category.label(), font_b(10.5), col);
            x += 78.0;
            // subject
            let subject = elide(ui, &r.subject, font(12.5), (rect.right() - x - 60.0).max(40.0));
            let sub = p.layout_no_wrap(subject, font(12.5), if r.is_plumbing() { TEXT_DIM } else { TEXT });
            let sub_w = sub.size().x;
            p.galley(pos2(x, y - sub.size().y / 2.0), sub, TEXT);
            x += sub_w + 10.0;
            // value (with sweep start)
            if let Some(from) = &r.from {
                let g = p.layout_no_wrap(format!("{from} →"), mono(12.0), TEXT_FAINT);
                let w = g.size().x;
                p.galley(pos2(x, y - g.size().y / 2.0), g, TEXT_FAINT);
                x += w + 6.0;
            }
            let room = rect.right() - x - 8.0;
            let value = elide(ui, &r.value, mono_b(12.5), room.max(24.0));
            let g = p.layout_no_wrap(value, mono_b(12.5), mix(TEXT, col, 0.35));
            let w = g.size().x;
            p.galley(pos2(x, y - g.size().y / 2.0), g, TEXT);
            x += w + 8.0;
            if r.count > 1 && x + 30.0 < rect.right() {
                let txt = format!("×{}", r.count);
                let g = p.layout_no_wrap(txt, font_sb(10.0), TEXT_DIM);
                let b = egui::Rect::from_min_size(pos2(x, y - 8.0), g.size() + vec2(10.0, 4.0));
                p.rect_filled(b, CornerRadius::same(8), CARD_HI);
                p.galley(b.min + vec2(5.0, 2.0), g, TEXT_DIM);
            }
        }
    }
    if raw && !r.raw.is_empty() {
        p.text(
            pos2(rect.left() + 100.0, rect.top() + 34.0),
            Align2::LEFT_CENTER,
            stomp_proto::midi::hex(&r.raw),
            mono(10.5),
            TEXT_FAINT,
        );
    }
    if !r.raw.is_empty() || r.from.is_some() {
        resp.on_hover_ui(|ui| {
            ui.set_max_width(420.0);
            ui.label(RichText::new(format!("{} · {}", r.category.label(), r.subject)).font(font_b(13.0)));
            if let Some(f) = &r.from {
                ui.label(format!("{f} → {}  ({} updates)", r.value, r.count));
            } else {
                ui.label(&r.value);
            }
            ui.label(RichText::new(r.at.format("%Y-%m-%d %H:%M:%S%.3f").to_string()).color(TEXT_FAINT));
            if !r.raw.is_empty() {
                ui.label(RichText::new(stomp_proto::midi::hex(&r.raw[..r.raw.len().min(96)])).font(mono(11.0)).color(TEXT_DIM));
            }
        });
    }
}

/// `STOMP_SCREENSHOT=<file.ppm>:<seconds>` saves a screenshot after that many seconds and quits
/// (for docs and visual checks).
fn screenshot_hook(ctx: &egui::Context, now: f64) {
    let Ok(spec) = std::env::var("STOMP_SCREENSHOT") else { return };
    let (path, secs) = spec.rsplit_once(':').unwrap_or((&spec, "5"));
    let secs: f64 = secs.parse().unwrap_or(5.0);
    let requested = egui::Id::new("screenshot-requested");
    if now >= secs && !ctx.data(|d| d.get_temp::<bool>(requested).unwrap_or(false)) {
        ctx.data_mut(|d| d.insert_temp(requested, true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
    }
    let shot = ctx.input(|i| {
        i.raw.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let Some(img) = shot {
        let mut out = format!("P6\n{} {}\n255\n", img.size[0], img.size[1]).into_bytes();
        for p in &img.pixels {
            out.extend_from_slice(&[p.r(), p.g(), p.b()]);
        }
        let _ = std::fs::write(path, out);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

impl eframe::App for StompApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let now = ui.input(|i| i.time);
        self.poll(now);
        screenshot_hook(ui.ctx(), now);

        egui::Panel::top("top")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(16, 10)))
            .show(ui, |ui| self.top_bar(ui, now));
        let full_w = ui.available_width();
        egui::Panel::left("presets")
            .resizable(true)
            .default_size(if full_w < 1400.0 { 220.0 } else { 250.0 })
            .size_range(200.0..=380.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(10, 4)))
            .show(ui, |ui| self.preset_list(ui, now));
        egui::Panel::right("log")
            .resizable(true)
            .default_size((full_w * 0.34).clamp(380.0, 600.0))
            .size_range(340.0..=1000.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 4)))
            .show(ui, |ui| self.log_panel(ui, now));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(18)))
            .show(ui, |ui| self.main_view(ui, now));

        if self.animating(now) {
            ui.ctx().request_repaint();
        } else if matches!(self.status, Status::Disconnected) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }
}
