//! The main window.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Align, Align2, Color32, CornerRadius, Layout, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use egui_phosphor::regular as icon;
use open_stomp::catalog::{DRUM_STYLES, ParamKind, drum_pattern_name};
use open_stomp::midi::host;
use open_stomp::records::{DEFAULT_CHAIN, Settings};
use open_stomp::state::preset_label;
use open_stomp::{Category, DeviceState, Module, Origin};

use chrono::{DateTime, Local};
use open_stomp::device::{Config, Device, Status, Update};
use crate::logbook::{Filter, LogBook, Row, RowKind};
use crate::slot_import::{ImportDialog, Kind};
use crate::theme::*;
use crate::widgets::*;

fn local(at: std::time::SystemTime) -> DateTime<Local> {
    DateTime::from(at)
}

/// Shows / hides the activity log.
/// Focuses the preset search.
const SEARCH_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::F);

/// Does preset `i` match a search? Matches the name, the label (`P05`) or the number (`5`).
fn preset_matches(i: u8, name: Option<&str>, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let number = q.strip_prefix('p').unwrap_or(&q).parse::<u16>().ok();
    number == Some(i as u16 + 1)
        || preset_label(i).to_lowercase().contains(&q)
        || name.is_some_and(|n| n.to_lowercase().contains(&q))
}

const LOG_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::L);

/// Horizontal inset of menu text, matching the padding of menu items.
const MENU_PAD: f32 = 12.0;

/// A small heading inside a menu.
fn section(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(MENU_PAD);
        ui.label(RichText::new(text).font(font_sb(11.5)).color(TEXT_FAINT));
    });
}

fn menu_separator(ui: &mut Ui) {
    ui.add(egui::Separator::default().spacing(14.0));
}

/// Seconds a change keeps glowing.
const FLASH_SECS: f64 = 0.9;
/// After a preset change from the pedal, messages are held until the pedal has been quiet this
/// long, then applied together: turning the preset knob several clicks jumps to where it stopped
/// instead of replaying every preset on the way. Over Bluetooth each click's data trickles in for
/// a few tenths of a second, so the pedal only goes quiet once the last click has arrived.
const SETTLE_SECS: f64 = 0.15;
/// Never hold longer than this, so a long scroll still shows progress.
const MAX_HOLD_SECS: f64 = 5.0;

pub struct Options {
    pub replay: Option<PathBuf>,
    pub speed: f32,
}

/// The "save preset" dialog.
struct SaveDialog {
    slot: u8,
    name: String,
}

/// "Rename…" on a user AMP / IR slot: only the name record is sent, the sound stays.
struct RenameDialog {
    kind: Kind,
    slot: u8,
    name: String,
}

pub struct StompApp {
    dev: Device,
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
    /// Last message from the pedal.
    pedal_msg_at: f64,
    /// Messages held after a preset change from the pedal (see [`SETTLE_SECS`]), and since when.
    held: Vec<(std::time::SystemTime, Origin, Vec<u8>)>,
    held_since: Option<f64>,
    scrolled_to: Option<u8>,
    preset_search: String,
    autoscroll: bool,
    show_raw: bool,
    /// The activity log panel is open (hidden by default; the menu or Ctrl+L toggles it).
    show_log: bool,
    replay_path: String,
    replay_speed: f32,
    view_cache: (u64, Filter, Vec<usize>),
    save_dialog: Option<SaveDialog>,
    /// The "Import NAM model" dialog.
    slot_import: Option<ImportDialog>,
    rename_dialog: Option<RenameDialog>,
    /// `STOMP_DEV=menu`: open the menu once at startup (for screenshots while styling).
    dev_open_menu: bool,
    /// F12 in debug builds: egui's style editor and inspector.
    dev_inspector: bool,
}

impl StompApp {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: Options) -> Self {
        install(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let config = Config { client_name: "Open Stomp Manager".into(), ..Config::default() };
        let dev = Device::spawn_with_waker(config, move || ctx.request_repaint());
        let default_log = PathBuf::from("captures/logs/01-main-session.log");
        let replay_path = opts
            .replay
            .clone()
            .or_else(|| default_log.exists().then_some(default_log))
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        if let Some(path) = opts.replay {
            dev.replay(path, opts.speed);
        }
        // `STOMP_DEV=log,menu` opens those at startup; see docs/building.md (live preview)
        let dev_flags = std::env::var("STOMP_DEV").unwrap_or_default();
        let dev_flag = |f: &str| dev_flags.split(',').any(|x| x.trim() == f);
        // `import` opens Import NAM, `import-ir` Import IR
        let mut slot_import = (dev_flag("import").then_some(Kind::Amp))
            .or(dev_flag("import-ir").then_some(Kind::Ir))
            .map(|k| ImportDialog::new(k, 0));
        if let (Some(d), Ok(p)) = (&mut slot_import, std::env::var("STOMP_DEBUG_IMPORT")) {
            d.debug_start(&cc.egui_ctx, p.into());
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
            pedal_msg_at: -10.0,
            held: Vec::new(),
            held_since: None,
            scrolled_to: None,
            preset_search: String::new(),
            autoscroll: true,
            show_raw: false,
            show_log: dev_flag("log"),
            replay_path,
            replay_speed: opts.speed,
            view_cache: (u64::MAX, Filter::default(), vec![]),
            save_dialog: None,
            slot_import,
            // `rename` opens the Rename dialog on USER AMP 01
            rename_dialog: dev_flag("rename").then(|| RenameDialog { kind: Kind::Amp, slot: 0, name: "SVT G10 .5".into() }),
            dev_open_menu: dev_flag("menu"),
            dev_inspector: false,
        }
    }

    // ───────────────────────────── sending ─────────────────────────────

    /// The app may send to the pedal: connected and not in listen-only mode.
    fn editable(&self) -> bool {
        matches!(self.status, Status::Connected { .. }) && !self.listen_only
    }

    /// Effect edits also need the live preset, since a head CC carries model and on/off together.
    fn can_edit_sound(&self) -> bool {
        self.editable() && self.state.live_known
    }

    /// Send a live CC/PC. The engine echoes it back as a host message, which updates the model.
    fn send(&self, bytes: Vec<u8>) {
        self.dev.send(bytes);
    }

    fn set_head(&self, m: Module, model: u8, on: bool) {
        self.dev.set_head(m, model, on);
    }

    fn set_settings(&self, f: impl FnOnce(&mut Settings)) {
        if let Some(s) = &self.state.settings {
            let mut s = s.clone();
            f(&mut s);
            self.dev.write_settings(&s);
        }
    }

    /// Store the live sound in `slot` under `name`, then select it (the official app's sequence).
    fn save_preset(&self, slot: u8, name: &str) {
        let (Some(cur), true) = (self.state.current, self.state.live_known) else { return };
        let mut p = self.state.live.clone();
        p.index = slot;
        p.name = name.trim().to_owned();
        self.dev.save_preset(cur, &p);
    }

    /// Open "Import NAM model" / "Import IR" for `slot`, or the first empty slot of that kind.
    fn open_slot_import(&mut self, kind: Kind, slot: Option<u8>) {
        if self.slot_import.as_ref().is_some_and(|d| d.busy()) {
            return;
        }
        let names = match kind {
            Kind::Amp => &self.state.user_amps,
            Kind::Ir => &self.state.user_irs,
        };
        let empty = |n: &Option<String>| n.is_some() && crate::slot_import::slot_in_use(n.as_deref()).is_none();
        let slot = slot.or_else(|| names.iter().position(empty).map(|i| i as u8)).unwrap_or(0);
        self.slot_import = Some(ImportDialog::new(kind, slot));
    }

    fn open_save_dialog(&mut self, slot: u8) {
        let name = if self.state.live.name.is_empty() {
            self.state.preset_name(slot).unwrap_or("").to_owned()
        } else {
            self.state.live.name.clone()
        };
        self.save_dialog = Some(SaveDialog { slot, name });
    }

    fn poll(&mut self, now: f64) {
        while let Ok(u) = self.dev.try_recv() {
            match u {
                Update::Ports(p) => {
                    self.ports = p;
                    if self.port_choice.as_ref().is_none_or(|c| !self.ports.contains(c)) {
                        // the pedal over USB first, then over Bluetooth, like the engine's auto-connect
                        let pedal = |ble: bool| {
                            let is = |p: &&String| open_stomp::is_stomp_port(p) && open_stomp::is_ble_port(p) == ble;
                            self.ports.iter().find(is)
                        };
                        self.port_choice = pedal(false).or_else(|| pedal(true)).or(self.ports.first()).cloned();
                    }
                }
                Update::Status(s) => self.status = s,
                Update::Sync { done, total } => self.sync = (done, total),
                Update::Reset => {
                    self.held.clear();
                    self.held_since = None;
                    self.state = DeviceState::default();
                    self.scrolled_to = None;
                }
                Update::Mark { at, text } => self.log.push_note(local(at), RowKind::Mark, text),
                Update::Info { at, text, error } => {
                    self.log.push_note(local(at), if error { RowKind::Error } else { RowKind::Info }, text)
                }
                Update::Message { at, origin, bytes } => {
                    match origin {
                        Origin::Pedal => self.rx_flash = now,
                        Origin::Host => self.tx_flash = now,
                    }
                    if origin == Origin::Pedal {
                        if bytes.first().is_some_and(|b| b & 0xF0 == 0xC0) && self.held_since.is_none() {
                            self.held_since = Some(now);
                        }
                        self.pedal_msg_at = now;
                    }
                    // everything waits while holding, so host edits keep their order
                    if self.held_since.is_some() {
                        self.held.push((at, origin, bytes));
                    } else {
                        self.apply_message(at, origin, &bytes, now);
                    }
                }
            }
        }
        if let Some(since) = self.held_since
            && (now - self.pedal_msg_at >= SETTLE_SECS || now - since >= MAX_HOLD_SECS)
        {
            self.held_since = None;
            for (at, origin, bytes) in std::mem::take(&mut self.held) {
                self.apply_message(at, origin, &bytes, now);
            }
        }
        if self.row_flash.len() > 256 {
            self.row_flash.retain(|_, t| now - *t < FLASH_SECS);
        }
    }

    fn apply_message(&mut self, at: std::time::SystemTime, origin: Origin, bytes: &[u8], now: f64) {
        let ev = self.state.apply(bytes, origin);
        if !ev.redundant {
            self.note_activity(&ev, bytes, now);
        }
        self.log.push_event(local(at), ev);
        if let Some(r) = self.log.rows.last() {
            self.row_flash.insert(r.id, now);
        }
    }

    fn note_activity(&mut self, ev: &open_stomp::Event, bytes: &[u8], now: f64) {
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
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(RichText::new("Open Stomp").font(font_b(16.0)).color(TEXT));
                    chip(ui, &format!("v{}", crate::VERSION), TEXT_DIM, CARD_HI).on_hover_text(format!(
                        "Open Stomp Manager {}\nopen-stomp library {}\n{} · {}",
                        crate::version_long(),
                        crate::LIB_VERSION,
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    ));
                });
                let mode = if self.editable() { "Editor" } else { "Monitor only" };
                ui.label(RichText::new(mode).font(font(11.5)).color(TEXT_FAINT));
            });
            ui.add_space(18.0);
            self.status_pill(ui, now);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.app_menu(ui);
            });
        });
    }

    fn status_pill(&mut self, ui: &mut Ui, now: f64) {
        let (dot, text, sub) = match &self.status {
            Status::Connected { port, .. } => {
                let name = self.state.product.clone().unwrap_or_else(|| "Pedal".into());
                let fw = self.state.firmware.clone().unwrap_or_default();
                let via = if open_stomp::is_ble_port(port) {
                    format!(" · {} Bluetooth", icon::BLUETOOTH)
                } else {
                    String::new()
                };
                (GOOD, format!("Connected · {name}{via}"), fw)
            }
            Status::Replaying { file, progress } => {
                (MARK, format!("Replaying {file}"), format!("{:.0}%", progress * 100.0))
            }
            Status::Disconnected => {
                let pulse = 0.5 + 0.5 * (now * 3.0).sin() as f32;
                let hint = "plug in via USB, or turn on Wireless".into();
                (mix(TEXT_FAINT, ACCENT, pulse), "Searching for pedal…".into(), hint)
            }
        };
        egui::Frame::new()
            .fill(CARD)
            .corner_radius(CornerRadius::same(18))
            .inner_margin(egui::Margin::symmetric(14, 7))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 18.0), Sense::hover());
                    led(ui.painter(), r.center(), 4.5, true, dot);
                    ui.label(RichText::new(text).font(font_sb(13.0)).color(TEXT));
                    if !sub.is_empty() {
                        ui.label(RichText::new(sub).font(font(11.5)).color(TEXT_DIM));
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
                    for (label, t, col) in [("In", self.rx_flash, ACCENT), ("Out", self.tx_flash, HOST)] {
                        let (r, _) = ui.allocate_exact_size(vec2(8.0, 18.0), Sense::hover());
                        led(ui.painter(), r.center(), 3.5, now - t < 0.12, col);
                        ui.label(RichText::new(label).font(font(11.0)).color(TEXT_FAINT));
                    }
                });
            });
    }

    fn set_listen_only(&mut self, listen: bool) {
        self.listen_only = listen;
        self.dev.set_sync(!listen);
        if !listen {
            self.dev.resync();
        }
    }

    /// The header menu: connection, listen only, replay, view and about.
    fn app_menu(&mut self, ui: &mut Ui) {
        let resp = ui.menu_button(RichText::new(icon::LIST).font(font(18.0)), |ui| self.menu_contents(ui)).response;
        if std::mem::take(&mut self.dev_open_menu) {
            egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&resp));
        }
        resp.on_hover_text("Menu");
    }

    fn menu_contents(&mut self, ui: &mut Ui) {
        ui.set_min_width(260.0);
        ui.spacing_mut().button_padding = vec2(MENU_PAD, 7.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.add_space(4.0);

        section(ui, "Connection");
        let port = self.port_choice.clone().unwrap_or_else(|| "No MIDI port".into());
        ui.menu_button(format!("{}  {}", icon::PLUGS_CONNECTED, elide(ui, &port, font(13.5), 190.0)), |ui| {
            ui.spacing_mut().button_padding = vec2(MENU_PAD, 7.0);
            if self.ports.is_empty() {
                ui.label(RichText::new("No MIDI input ports").color(TEXT_DIM));
            }
            for p in self.ports.clone() {
                let star = if open_stomp::is_ble_port(&p) {
                    format!("{}  ", icon::BLUETOOTH)
                } else if open_stomp::is_stomp_port(&p) {
                    format!("{}  ", icon::GUITAR)
                } else {
                    String::new()
                };
                if ui.selectable_label(self.port_choice.as_ref() == Some(&p), format!("{star}{p}")).clicked() {
                    self.port_choice = Some(p.clone());
                    self.dev.connect(p);
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("MIDI input port, or the pedal over Bluetooth (Wireless on); picking one connects to it");
        if matches!(self.status, Status::Connected { .. }) {
            if ui.button(format!("{}  Disconnect", icon::PLUGS)).clicked() {
                self.dev.disconnect();
            }
            if ui
                .button(format!("{}  Re-sync from pedal", icon::ARROWS_CLOCKWISE))
                .on_hover_text("Re-read presets and state from the pedal")
                .clicked()
            {
                self.dev.resync();
            }
        } else if let Some(port) = self.port_choice.clone()
            && ui.button(format!("{}  Connect", icon::PLUGS_CONNECTED)).clicked()
        {
            self.dev.connect(port);
        }
        let check = if self.listen_only { icon::CHECK } else { "" };
        if ui
            .add(egui::Button::new(format!("{}  Listen only", icon::EAR)).right_text(check))
            .on_hover_text(
                "When off, the app syncs presets and state from the pedal and you can edit it from here. \
                 When on, it never sends anything to the pedal.",
            )
            .clicked()
        {
            self.set_listen_only(!self.listen_only);
        }

        menu_separator(ui);
        section(ui, "User slots");
        if ui
            .button(format!("{}  Import NAM model…", icon::FILE_ARROW_UP))
            .on_hover_text("Convert a .nam file and upload it to a user AMP slot")
            .clicked()
        {
            self.open_slot_import(Kind::Amp, None);
            ui.close();
        }
        if ui
            .button(format!("{}  Import IR…", icon::FILE_AUDIO))
            .on_hover_text("Upload an impulse response (WAV) to a user IR slot")
            .clicked()
        {
            self.open_slot_import(Kind::Ir, None);
            ui.close();
        }

        menu_separator(ui);
        section(ui, "Replay");
        if matches!(self.status, Status::Replaying { .. }) {
            if ui.button(format!("{}  Stop replay", icon::STOP)).clicked() {
                self.dev.stop_replay();
            }
        } else {
            ui.menu_button(format!("{}  Replay capture…", icon::FILM_STRIP), |ui| {
                ui.set_min_width(360.0);
                egui::Frame::new().inner_margin(egui::Margin::same(MENU_PAD as i8)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.label(RichText::new("Play back a capture log from captures/logs/").color(TEXT_DIM));
                    ui.add(egui::TextEdit::singleline(&mut self.replay_path).hint_text("path/to/capture.log").desired_width(f32::INFINITY));
                    ui.horizontal(|ui| {
                        ui.label("Speed");
                        ui.add(egui::Slider::new(&mut self.replay_speed, 0.25..=50.0).logarithmic(true).suffix("×"));
                    });
                    let start = egui::Button::new(RichText::new(format!("{}  Start", icon::PLAY)).color(BG)).fill(ACCENT);
                    if ui.add(start).clicked() && !self.replay_path.is_empty() {
                        self.dev.replay(&self.replay_path, self.replay_speed);
                        ui.close();
                    }
                });
            });
        }

        menu_separator(ui);
        section(ui, "View");
        let shortcut = ui.ctx().format_shortcut(&LOG_SHORTCUT);
        let log_label = if self.show_log { "Hide activity log" } else { "Show activity log" };
        if ui.add(egui::Button::new(format!("{}  {log_label}", icon::PULSE)).shortcut_text(shortcut)).clicked() {
            self.show_log = !self.show_log;
            ui.close();
        }

        menu_separator(ui);
        ui.horizontal(|ui| {
            ui.add_space(MENU_PAD);
            ui.label(
                RichText::new(format!("Open Stomp Manager {}\nopen-stomp library {}", crate::version_long(), crate::LIB_VERSION))
                    .font(font(11.5))
                    .color(TEXT_FAINT),
            );
        });
        ui.add_space(4.0);
    }

    // ───────────────────────────── preset list ─────────────────────────────

    fn preset_list(&mut self, ui: &mut Ui, now: f64) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(icon::LIST).font(font(14.0)).color(TEXT_FAINT));
            ui.label(RichText::new("Presets").font(font_sb(13.0)).color(TEXT_DIM));
            let known = self.state.presets.iter().filter(|p| p.is_some()).count();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{known}/99")).font(font(11.0)).color(TEXT_FAINT));
            });
        });
        ui.add_space(6.0);

        let search_id = egui::Id::new("preset_search");
        if ui.input_mut(|i| i.consume_shortcut(&SEARCH_SHORTCUT)) {
            ui.memory_mut(|m| m.request_focus(search_id));
        }
        let shortcut = ui.ctx().format_shortcut(&SEARCH_SHORTCUT);
        let search = ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(icon::MAGNIFYING_GLASS).color(TEXT_FAINT));
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.preset_search)
                    .id(search_id)
                    .hint_text(format!("Search presets ({shortcut})"))
                    .desired_width(f32::INFINITY),
            );
            if !self.preset_search.is_empty() && icon_button(ui, icon::X, false, "Clear the search").clicked() {
                self.preset_search.clear();
            }
            r
        });
        let search_resp = search.inner;
        let enter = search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if search_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.preset_search.clear();
        }
        ui.add_space(6.0);

        let query = self.preset_search.clone();
        let shown: Vec<u8> = (0..99u8).filter(|&i| preset_matches(i, self.state.preset_name(i), &query)).collect();
        let current = self.state.current;
        // follow the active preset (not while searching; clearing the search re-centres on it)
        let searching = !query.trim().is_empty();
        let scroll_now = !searching && current.is_some() && current != self.scrolled_to;
        let editable = self.editable();
        let can_save = self.can_edit_sound() && current.is_some();
        let mut select = None;
        let mut save_here = None;
        if enter && editable && let Some(&first) = shown.first() && current != Some(first) {
            select = Some(first);
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if shown.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(RichText::new("No presets match").color(TEXT_FAINT));
                });
            }
            for &i in &shown {
                let active = current == Some(i);
                let sense = if editable { Sense::click() } else { Sense::hover() };
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), sense);
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
                    if active { font_sb(12.0) } else { font(12.0) },
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
                if editable {
                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() && !active {
                        select = Some(i);
                    }
                    resp.context_menu(|ui| {
                        if ui.button(format!("{}  Select {}", icon::PLAY, preset_label(i))).clicked() {
                            select = Some(i);
                        }
                        let save = ui.add_enabled(
                            can_save,
                            egui::Button::new(format!("{}  Save current sound here…", icon::FLOPPY_DISK)),
                        );
                        if save.clicked() {
                            save_here = Some(i);
                        }
                    });
                }
            }
        });
        if searching {
            // forget the scroll target so clearing the search scrolls back to the active preset
            self.scrolled_to = None;
        } else if scroll_now {
            self.scrolled_to = current;
        }
        if let Some(i) = select {
            if self.state.modified() {
                self.log.push_note(
                    chrono::Local::now(),
                    RowKind::Info,
                    format!("Unsaved edits to {} were discarded", preset_label(current.unwrap_or(0))),
                );
            }
            self.send(host::program(i));
        }
        if let Some(i) = save_here {
            self.open_save_dialog(i);
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
                    p.rect_filled(r, CornerRadius::same(12), BG);
                    p.text(r.center() + vec2(0.0, 2.0), Align2::CENTER_CENTER, "888", font_b(54.0), with_alpha(ACCENT, 0.05));
                    p.text(
                        r.center() + vec2(0.0, 2.0),
                        Align2::CENTER_CENTER,
                        format!("{:02}", cur as u16 + 1),
                        font_b(54.0),
                        mix(ACCENT, Color32::WHITE, 0.3 * fl),
                    );
                    p.text(r.left_top() + vec2(10.0, 8.0), Align2::LEFT_TOP, "Preset", font_sb(10.5), TEXT_FAINT);
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
                                chip(ui, "● Edited", BG, ACCENT).on_hover_text("The live sound differs from the saved preset");
                            } else if self.state.live_known {
                                chip(ui, "Saved", GOOD, with_alpha(GOOD, 0.14));
                            }
                            if let Some(page) = self.state.page {
                                chip(ui, &format!("Pedal shows {}", module_title(page)), TEXT_DIM, CARD_HI);
                            }
                            let on = Module::EFFECTS.iter().filter(|m| self.state.live.modules[m.index()].on).count();
                            chip(ui, &format!("{on}/8 blocks on"), TEXT_DIM, CARD_HI);
                        });
                        if self.can_edit_sound() {
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| self.save_buttons(ui, cur));
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.volume_meter(ui);
                    });
                });
            });
    }

    fn save_buttons(&mut self, ui: &mut Ui, cur: u8) {
        let modified = self.state.modified();
        let save = egui::Button::new(
            RichText::new(format!("{}  Save", icon::FLOPPY_DISK)).color(if modified { BG } else { TEXT }),
        )
        .fill(if modified { ACCENT } else { CARD_HI });
        if ui.add(save).on_hover_text(format!("Store the live sound in {}", preset_label(cur))).clicked() {
            let name = self.state.live.name.clone();
            self.save_preset(cur, &name);
        }
        let save_as = egui::Button::new(format!("{}  Save as…", icon::PENCIL_SIMPLE));
        if ui.add(save_as).on_hover_text("Rename and/or store in another slot").clicked() {
            self.open_save_dialog(cur);
        }
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
        let editable = self.can_edit_sound();
        let sense = if editable { Sense::click_and_drag() } else { Sense::hover() };
        let (r, resp) = ui.allocate_exact_size(vec2(64.0, 92.0), sense);
        if editable {
            if let Some(n) = drag_input(ui, &resp, self.state.live.volume, 100) {
                self.send(host::volume(n));
            }
            resp.on_hover_text("Preset volume: drag or scroll (Shift = fine)")
                .on_hover_cursor(egui::CursorIcon::ResizeVertical);
        }
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
        p.text(c, Align2::CENTER_CENTER, txt, font_b(16.0), TEXT);
        p.text(pos2(r.center().x, r.bottom() - 8.0), Align2::CENTER_CENTER, "Volume", font_sb(11.0), TEXT_DIM);
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
        let mut toggle = None;
        let can_toggle = self.can_edit_sound();
        card(ui, icon::FLOW_ARROW, "Signal chain", |ui| {
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
                // only the selected block gets an outline
                let stroke = if selected { Stroke::new(2.0, mix(col, Color32::WHITE, 0.35)) } else { Stroke::NONE };
                p.rect(rect, CornerRadius::same(12), fill, stroke, StrokeKind::Inside);
                if resp.hovered() {
                    p.rect_filled(rect, CornerRadius::same(12), with_alpha(Color32::WHITE, 0.03));
                }
                let led_c = rect.left_top() + vec2(14.0, 14.0);
                let led_hit = egui::Rect::from_center_size(led_c, vec2(26.0, 26.0));
                let power = can_toggle && m.is_effect() && resp.hover_pos().is_some_and(|p| led_hit.contains(p));
                if power {
                    p.circle_stroke(led_c, 8.0, Stroke::new(1.5, mix(TEXT_DIM, col, 0.5)));
                }
                led(p, led_c, 4.0, on, col);
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
                    module_title(*m),
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
                let status = if on { "On" } else { "Off" };
                p.text(
                    pos2(rect.center().x, rect.bottom() - 14.0),
                    Align2::CENTER_CENTER,
                    status,
                    font_sb(11.0),
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
                let hint = if can_toggle && m.is_effect() {
                    "Click to edit this block · click the LED to switch it on/off"
                } else {
                    "Click to pin this block in the detail view"
                };
                let resp = resp
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(format!("{} · {model}\n{hint}", module_title(*m)));
                if resp.clicked() {
                    if power {
                        toggle = Some(*m);
                    } else {
                        clicked = Some(*m);
                    }
                }
            }
        });
        if let Some(m) = toggle {
            let st = self.state.live.modules[m.index()];
            self.set_head(m, st.model, !st.on);
        }
        if let Some(m) = clicked {
            self.pinned = if self.pinned == Some(m) { None } else { Some(m) };
            // like the official app, opening a block shows its page on the pedal
            if self.pinned.is_some() && self.editable() {
                self.send(host::page(m));
            }
        }
    }

    fn detail(&mut self, ui: &mut Ui, now: f64) {
        let m = self.shown_module();
        let col = module_color(m);
        let pinned = self.pinned.is_some();
        let mut unpin = false;
        card_with(
            ui,
            module_icon(m),
            module_title(m),
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
        let editable = self.can_edit_sound();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("#{:02}", st.model + 1)).font(font_sb(13.0)).color(col));
            let name = self.state.model_name(m, st.model);
            if editable && m.models().len() > 1 {
                egui::ComboBox::from_id_salt(("model", m as u8))
                    .selected_text(RichText::new(name).font(font_b(18.0)).color(TEXT))
                    .height(420.0)
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for i in 0..m.models().len() as u8 {
                            let label = format!("{:02}  {}", i + 1, self.state.model_name(m, i));
                            if ui.selectable_label(i == st.model, label).clicked() && i != st.model {
                                self.set_head(m, i, st.on);
                            }
                        }
                    });
            } else {
                ui.label(RichText::new(name).font(font_b(22.0)).color(TEXT));
            }
            ui.add_space(8.0);
            if editable {
                let (txt, fg, bg) = if st.on { ("On", BG, col) } else { ("Bypassed", TEXT_DIM, CARD_HI) };
                let b = egui::Button::new(RichText::new(format!("{}  {txt}", icon::POWER)).font(font_b(11.0)).color(fg))
                    .fill(bg)
                    .corner_radius(CornerRadius::same(6));
                if ui.add(b).on_hover_text("Switch this block on/off").clicked() {
                    self.set_head(m, st.model, !st.on);
                }
            } else if st.on {
                chip(ui, "On", BG, col);
            } else {
                chip(ui, "Bypassed", TEXT_DIM, CARD_HI);
            }
            if !editable {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let models = m.models().len();
                    ui.label(RichText::new(format!("{} models", models)).font(font(11.5)).color(TEXT_FAINT));
                });
            }
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
                let mut next = None;
                match pd.kind {
                    ParamKind::Enum(opts) => {
                        let resp = choice(ui, &label_case(pd.name), &txt, col, fl, editable);
                        if editable {
                            let n = opts.len().max(1) as u8;
                            if resp.clicked() {
                                next = Some((v + 1) % n);
                            }
                            resp.on_hover_text("Click for the next option, right-click for the list").context_menu(
                                |ui| {
                                    for (i, o) in opts.iter().enumerate() {
                                        if ui.selectable_label(i == v as usize, *o).clicked() {
                                            next = Some(i as u8);
                                        }
                                    }
                                },
                            );
                        }
                    }
                    ParamKind::Knob | ParamKind::Bipolar => {
                        let bipolar = pd.kind == ParamKind::Bipolar;
                        let resp = knob(ui, &label_case(pd.name), &txt, v as f32 / 100.0, bipolar, col, fl, editable);
                        if editable {
                            next = drag_input(ui, &resp, v, 100);
                            let resp = resp.on_hover_text("Drag or scroll · Shift = fine · double-click = centre");
                            if resp.double_clicked() {
                                next = Some(50);
                            }
                        }
                    }
                }
                if let Some(n) = next.filter(|n| *n != v)
                    && let Some(b) = host::param(m, slot, n)
                {
                    self.send(b);
                }
            }
        });
    }

    fn tuner_card(&self, ui: &mut Ui) {
        let t = &self.state.tuner;
        card(ui, icon::MUSIC_NOTES, "Tuner", |ui| {
            ui.set_min_height(150.0);
            let live = t.detected && t.note.is_some();
            ui.horizontal(|ui| {
                let note = if live { open_stomp::catalog::NOTES[t.note.unwrap() as usize] } else { "—" };
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
                    ui.label(RichText::new(cents).font(font_b(18.0)).color(col));
                    let status = if t.active || live { "listening" } else { "tuner off" };
                    ui.label(RichText::new(status).font(font(11.5)).color(TEXT_FAINT));
                });
            });
            cents_meter(ui, t.cents_offset(), live);
        });
    }

    fn looper_card(&self, ui: &mut Ui, now: f64) {
        card(ui, icon::CASSETTE_TAPE, "Looper", |ui| {
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
                ui.label(RichText::new(l.state_name()).font(font_b(20.0)).color(TEXT));
                let layers = match l.layers {
                    1 => "overdub layer · undo available",
                    2 => "base layer",
                    _ => "no layers",
                };
                ui.label(RichText::new(layers).font(font(11.5)).color(TEXT_FAINT));
            });
        });
        let editable = self.editable();
        if editable {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let (main, main_tip) = match l.state {
                    0 | 4 => (format!("{}  Rec", icon::RECORD), "Start recording"),
                    1 => (format!("{}  Play", icon::PLAY), "Close the loop and play"),
                    2 => (format!("{}  Dub", icon::RECORD), "Overdub"),
                    _ => (format!("{}  Play", icon::PLAY), "Stop overdubbing"),
                };
                let buttons = [
                    (main, main_tip, host::LOOP_REC_PLAY_DUB),
                    (icon::STOP.to_string(), "Stop", host::LOOP_STOP),
                    (icon::ARROW_COUNTER_CLOCKWISE.to_string(), "Undo / redo the last overdub", host::LOOP_UNDO_REDO),
                    (icon::TRASH.to_string(), "Clear the loop", host::LOOP_CLEAR),
                ];
                for (txt, tip, b) in buttons {
                    if ui.add(egui::Button::new(txt).min_size(vec2(34.0, 26.0))).on_hover_text(tip).clicked() {
                        self.send(host::looper_button(b));
                    }
                }
            });
        }
        ui.add_space(4.0);
        let lc = module_color(Module::Loop);
        let num = |v: u8| v.to_string();
        if let Some(n) = level_slider(ui, "Rec level", l.rec_level, 100, &num, lc, editable) {
            self.send(host::cc(0x42, n));
        }
        if let Some(n) = level_slider(ui, "Playback", l.play_level, 100, &num, lc, editable) {
            self.send(host::cc(0x43, n));
        }
        ui.horizontal(|ui| {
            let sync = match l.sync {
                Some(true) => "Sync on",
                Some(false) => "Sync off",
                None => "Sync —",
            };
            let mode = match l.mode {
                Some(0) => "Mode NR",
                Some(_) => "Mode AR",
                None => "Mode —",
            };
            if editable {
                if ui.button(sync).on_hover_text("Loop sync on/off").clicked() {
                    self.send(host::cc(0x49, !l.sync.unwrap_or(false) as u8));
                }
                if ui.button(mode).on_hover_text("Loop mode NR / AR").clicked() {
                    self.send(host::cc(0x4A, (l.mode.unwrap_or(0) == 0) as u8));
                }
            } else {
                chip(ui, sync, TEXT_DIM, CARD_HI);
                chip(ui, mode, TEXT_DIM, CARD_HI);
            }
        });
    }

    fn drum_card(&self, ui: &mut Ui) {
        card(ui, icon::METRONOME, "Drum machine", |ui| {
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
                ui.label(RichText::new(bpm).font(font_b(30.0)).color(TEXT));
            });
        });
        let editable = self.editable();
        if editable {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let (txt, tip) = if d.playing {
                    (format!("{}  Stop", icon::STOP), "Stop the drums")
                } else {
                    (format!("{}  Play", icon::PLAY), "Start the drums")
                };
                if ui.add(egui::Button::new(txt).min_size(vec2(70.0, 26.0))).on_hover_text(tip).clicked() {
                    self.send(host::cc(0x3F, !d.playing as u8));
                }
                let pat = d.pattern.unwrap_or(0);
                ui.menu_button(format!("{}  Pattern", icon::LIST), |ui| {
                    for (si, style) in DRUM_STYLES.iter().enumerate() {
                        ui.menu_button(*style, |ui| {
                            for k in 0..10u8 {
                                let v = si as u8 * 10 + k;
                                if ui.selectable_label(v == pat, drum_pattern_name(v)).clicked() {
                                    self.send(host::cc(0x40, v));
                                    ui.close();
                                }
                            }
                        });
                    }
                });
            });
        }
        ui.add_space(4.0);
        if let Some(n) = level_slider(ui, "Level", d.level, 100, &|v| v.to_string(), col, editable) {
            self.send(host::cc(0x41, n));
        }
    }

    fn device_card(&mut self, ui: &mut Ui) {
        card(ui, icon::GEAR, "Device and system settings", |ui| {
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
        if self.editable() && self.state.settings.is_some() {
            return self.settings_editor(ui);
        }
        let s = &self.state;
        let kv = |ui: &mut Ui, k: &str, v: Option<String>| {
            ui.horizontal(|ui| {
                ui.add_sized([86.0, 18.0], egui::Label::new(RichText::new(k).font(font_sb(11.0)).color(TEXT_DIM)));
                let v = v.unwrap_or_else(|| "—".into());
                ui.label(RichText::new(elide(ui, &v, font(12.5), ui.available_width())).font(font(12.5)).color(TEXT));
            });
        };
        let set = s.settings.as_ref();
        let onoff = |b: bool| if b { "On".to_owned() } else { "Off".to_owned() };
        kv(ui, "Product", s.product.clone());
        kv(ui, "Firmware", s.firmware.clone());
        kv(ui, "Model ID", s.model_id.clone());
        kv(ui, "Input trim", set.map(|x| format!("{:+} dB", x.input_trim_db())));
        kv(ui, "Wireless", set.map(|x| onoff(x.wireless)));
        kv(ui, "Loopback", set.map(|x| onoff(x.loopback)));
    }

    /// Product info plus the editable system settings (read-modify-write of the whole block).
    fn settings_editor(&self, ui: &mut Ui) {
        let s = &self.state;
        let Some(set) = &s.settings else { return };
        let kv = |ui: &mut Ui, k: &str, v: Option<String>| {
            ui.horizontal(|ui| {
                ui.add_sized([86.0, 18.0], egui::Label::new(RichText::new(k).font(font_sb(11.0)).color(TEXT_DIM)));
                let v = v.unwrap_or_else(|| "—".into());
                ui.label(RichText::new(elide(ui, &v, font(12.5), ui.available_width())).font(font(12.5)).color(TEXT));
            });
        };
        kv(ui, "Product", s.product.clone());
        kv(ui, "Firmware", s.firmware.clone());
        let db = |v: u8| format!("{:+} dB", v as i16 - 10);
        if let Some(n) = level_slider(ui, "Input trim", Some(set.input_trim), 20, &db, ACCENT, true) {
            self.set_settings(|s| s.input_trim = n);
        }
        ui.horizontal(|ui| {
            ui.add_sized([86.0, 18.0], egui::Label::new(RichText::new("Options").font(font_sb(11.0)).color(TEXT_DIM)));
            let (mut w, mut l) = (set.wireless, set.loopback);
            if ui.toggle_value(&mut w, "Wireless").changed() {
                self.set_settings(|s| s.wireless = w);
            }
            if ui.toggle_value(&mut l, "Loopback").on_hover_text("USB audio loopback").changed() {
                self.set_settings(|s| s.loopback = l);
            }
        });
    }

    fn global_eq(&self, ui: &mut Ui) {
        let set = self.state.settings.as_ref();
        let editable = self.editable() && set.is_some();
        ui.label(RichText::new("Global EQ").font(font_sb(12.0)).color(TEXT_DIM));
        let eq = module_color(Module::Eq);
        let centred = |v: u8| format!("{:+}", v as i16 - 50);
        if let Some(n) = level_slider(ui, "Bass", set.map(|x| x.eq_bass), 100, &centred, eq, editable) {
            self.set_settings(|s| s.eq_bass = n);
        }
        if let Some(n) = level_slider(ui, "Mid", set.map(|x| x.eq_mid), 100, &centred, eq, editable) {
            self.set_settings(|s| s.eq_mid = n);
        }
        if let Some(n) = level_slider(ui, "Treble", set.map(|x| x.eq_treble), 100, &centred, eq, editable) {
            self.set_settings(|s| s.eq_treble = n);
        }
        ui.label(
            RichText::new("Global EQ changed on the pedal itself is not reported over USB, so this may be stale.")
                .font(font(11.0))
                .color(TEXT_FAINT),
        );
    }

    fn user_slots(&mut self, ui: &mut Ui) {
        let editable = self.editable();
        let s = &self.state;
        let mut import: Option<(Kind, Option<u8>)> = None;
        let mut rename: Option<(Kind, u8)> = None;
        ui.label(RichText::new("User slots").font(font_sb(12.0)).color(TEXT_DIM));
        let name_w = ((ui.available_width() - 40.0) / 2.0).max(60.0);
        egui::Grid::new("user_slots").num_columns(3).spacing([10.0, 3.0]).show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new("Amp (NAM)").font(font_sb(11.0)).color(TEXT_FAINT));
            ui.label(RichText::new("Cab (IR)").font(font_sb(11.0)).color(TEXT_FAINT));
            ui.end_row();
            let name = |n: &Option<String>| match n.as_deref() {
                Some(n) if !n.is_empty() && !n.starts_with("--") => (n.to_owned(), TEXT),
                Some(_) => ("empty".to_owned(), TEXT_FAINT),
                None => ("—".to_owned(), TEXT_FAINT),
            };
            for i in 0..10 {
                ui.label(RichText::new(format!("{:02}", i + 1)).font(font(11.0)).color(TEXT_FAINT));
                for (col, (txt, c)) in [name(&s.user_amps[i]), name(&s.user_irs[i])].into_iter().enumerate() {
                    let r = ui.add(
                        egui::Label::new(RichText::new(elide(ui, &txt, font(11.5), name_w)).font(font(11.5)).color(c))
                            .sense(Sense::click()),
                    );
                    let (kind, label) = if col == 0 {
                        (Kind::Amp, format!("{}  Import NAM model here…", icon::FILE_ARROW_UP))
                    } else {
                        (Kind::Ir, format!("{}  Import IR here…", icon::FILE_AUDIO))
                    };
                    let current = if col == 0 { &s.user_amps[i] } else { &s.user_irs[i] };
                    let in_use = crate::slot_import::slot_in_use(current.as_deref()).is_some();
                    r.context_menu(|ui| {
                        if ui.button(label).clicked() {
                            import = Some((kind, Some(i as u8)));
                            ui.close();
                        }
                        let b = ui
                            .add_enabled(editable && in_use, egui::Button::new(format!("{}  Rename…", icon::PENCIL_SIMPLE)))
                            .on_disabled_hover_text(if in_use { "Connect, with Listen only off" } else { "The slot is empty" });
                        if b.clicked() {
                            rename = Some((kind, i as u8));
                            ui.close();
                        }
                    });
                }
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .button(format!("{}  Import NAM model…", icon::FILE_ARROW_UP))
                .on_hover_text("Convert a .nam file and upload it to a user AMP slot (right-click a slot to pick it)")
                .clicked()
            {
                import = Some((Kind::Amp, None));
            }
            if ui
                .button(format!("{}  Import IR…", icon::FILE_AUDIO))
                .on_hover_text("Upload an impulse response (WAV) to a user IR slot (right-click a slot to pick it)")
                .clicked()
            {
                import = Some((Kind::Ir, None));
            }
        });
        if let Some((kind, slot)) = import {
            self.open_slot_import(kind, slot);
        }
        if let Some((kind, slot)) = rename {
            let names = if kind == Kind::Amp { &self.state.user_amps } else { &self.state.user_irs };
            let name = names[slot as usize].clone().unwrap_or_default();
            self.rename_dialog = Some(RenameDialog { kind, slot, name });
        }
    }

    fn rename_window(&mut self, ctx: &egui::Context) {
        let editable = self.editable();
        let Some(dlg) = &mut self.rename_dialog else { return };
        let mut open = true;
        let (mut rename, mut cancel) = (false, false);
        let what = if dlg.kind == Kind::Amp { "USER AMP" } else { "USER IR" };
        egui::Window::new(format!("{}  Rename {what} {:02}", icon::PENCIL_SIMPLE, dlg.slot + 1))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(340.0);
                egui::Grid::new("rename_grid").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                    ui.label("Name");
                    let r = ui.add(egui::TextEdit::singleline(&mut dlg.name).char_limit(16).desired_width(220.0));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        rename = true;
                    }
                    ui.end_row();
                });
                ui.label(
                    RichText::new("Only the name changes; the slot's sound stays as it is. Up to 16 characters.")
                        .font(font(11.0))
                        .color(TEXT_FAINT),
                );
                if !dlg.name.is_ascii() {
                    ui.label(RichText::new("Only ASCII characters can be stored; others become “?”.").color(BAD));
                }
                if !editable {
                    ui.label(RichText::new("Connect to the pedal, with Listen only off, to rename.").color(BAD));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    let b = egui::Button::new(RichText::new(format!("{}  Rename", icon::PENCIL_SIMPLE)).color(BG))
                        .fill(ACCENT);
                    if ui.add_enabled(editable && !dlg.name.trim().is_empty(), b).clicked() {
                        rename = true;
                    }
                });
            });
        let rename = rename && editable && !dlg.name.trim().is_empty();
        if rename {
            let name = dlg.name.trim();
            let _ = match dlg.kind {
                Kind::Amp => self.dev.rename_user_amp(dlg.slot, name),
                Kind::Ir => self.dev.rename_user_ir(dlg.slot, name),
            };
        }
        if rename || cancel || !open {
            self.rename_dialog = None;
        }
    }

    fn save_window(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.save_dialog else { return };
        let mut open = true;
        let (mut save, mut cancel) = (false, false);
        let cur = self.state.current;
        egui::Window::new(format!("{}  Save preset", icon::FLOPPY_DISK))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(340.0);
                egui::Grid::new("save_grid").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                    ui.label("Name");
                    let r = ui.add(egui::TextEdit::singleline(&mut dlg.name).char_limit(16).desired_width(220.0));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        save = true;
                    }
                    ui.end_row();
                    ui.label("Slot");
                    egui::ComboBox::from_id_salt("save_slot")
                        .selected_text(preset_label(dlg.slot))
                        .height(360.0)
                        .show_ui(ui, |ui| {
                            for i in 0..99u8 {
                                let name = self.state.preset_name(i).unwrap_or("—");
                                ui.selectable_value(&mut dlg.slot, i, format!("{}  {name}", preset_label(i)));
                            }
                        });
                    ui.end_row();
                });
                if !dlg.name.is_ascii() {
                    ui.label(RichText::new("Only ASCII characters can be stored; others become “?”.").color(BAD));
                }
                if Some(dlg.slot) != cur {
                    let old = self.state.preset_name(dlg.slot).unwrap_or("?");
                    ui.label(
                        RichText::new(format!("{}  This overwrites {} “{old}”.", icon::WARNING, preset_label(dlg.slot)))
                            .color(ACCENT),
                    );
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    let b = egui::Button::new(RichText::new(format!("{}  Save", icon::FLOPPY_DISK)).color(BG)).fill(ACCENT);
                    if ui.add(b).clicked() {
                        save = true;
                    }
                });
            });
        if save {
            let (slot, name) = (dlg.slot, dlg.name.clone());
            self.save_preset(slot, &name);
        }
        if save || cancel || !open {
            self.save_dialog = None;
        }
    }

    fn slot_import_window(&mut self, ctx: &egui::Context) {
        let editable = self.editable();
        let Some(dlg) = &mut self.slot_import else { return };
        let names = match dlg.kind() {
            Kind::Amp => &self.state.user_amps,
            Kind::Ir => &self.state.user_irs,
        };
        let (open, upload) = dlg.show(ctx, names, editable);
        if let Some(u) = upload {
            let sent = match u.kind {
                Kind::Amp => self.dev.import_user_amp(u.slot, &u.name, &u.data, u.factor),
                Kind::Ir => self.dev.import_user_ir(u.slot, &u.name, &u.data),
            };
            if let Err(e) = sent {
                dlg.failed(format!("Upload failed: {e}"));
            }
        }
        if !open {
            self.slot_import = None;
        }
    }

    // ───────────────────────────── activity log ─────────────────────────────

    fn log_panel(&mut self, ui: &mut Ui, now: f64) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::PULSE).font(font(14.0)).color(TEXT_FAINT));
            ui.label(RichText::new("Activity").font(font_sb(13.0)).color(TEXT_DIM));
            ui.label(RichText::new(format!("{}", self.view_cache.2.len())).font(font(11.0)).color(TEXT_FAINT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, icon::X, false, "Hide the activity log").clicked() {
                    self.show_log = false;
                }
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
    p.text(pos2(x, y), Align2::LEFT_CENTER, r.at.format("%H:%M:%S%.3f").to_string(), font(11.0), TEXT_FAINT);
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
                Origin::Pedal => ("Pedal", ACCENT),
                Origin::Host => ("Host", HOST),
            };
            let badge = egui::Rect::from_min_size(pos2(x, y - 8.0), vec2(42.0, 16.0));
            p.rect_filled(badge, CornerRadius::same(4), with_alpha(wc, 0.13));
            p.text(badge.center(), Align2::CENTER_CENTER, who, font_sb(10.5), wc);
            x += 50.0;
            // category
            p.circle_filled(pos2(x + 3.0, y), 3.5, col);
            p.text(pos2(x + 12.0, y), Align2::LEFT_CENTER, category_title(r.category), font_sb(11.5), col);
            x += 78.0;
            // subject
            let subject = elide(ui, &r.subject, font(12.5), (rect.right() - x - 60.0).max(40.0));
            let sub = p.layout_no_wrap(subject, font(12.5), if r.is_plumbing() { TEXT_DIM } else { TEXT });
            let sub_w = sub.size().x;
            p.galley(pos2(x, y - sub.size().y / 2.0), sub, TEXT);
            x += sub_w + 10.0;
            // value (with sweep start)
            if let Some(from) = &r.from {
                let g = p.layout_no_wrap(format!("{from} →"), font(12.0), TEXT_FAINT);
                let w = g.size().x;
                p.galley(pos2(x, y - g.size().y / 2.0), g, TEXT_FAINT);
                x += w + 6.0;
            }
            let room = rect.right() - x - 8.0;
            let value = elide(ui, &r.value, font_sb(12.5), room.max(24.0));
            let g = p.layout_no_wrap(value, font_sb(12.5), mix(TEXT, col, 0.35));
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
            open_stomp::midi::hex(&r.raw),
            font(10.5),
            TEXT_FAINT,
        );
    }
    if !r.raw.is_empty() || r.from.is_some() {
        resp.on_hover_ui(|ui| {
            ui.set_max_width(420.0);
            ui.label(RichText::new(format!("{} · {}", category_title(r.category), r.subject)).font(font_b(13.0)));
            if let Some(f) = &r.from {
                ui.label(format!("{f} → {}  ({} updates)", r.value, r.count));
            } else {
                ui.label(&r.value);
            }
            ui.label(RichText::new(r.at.format("%Y-%m-%d %H:%M:%S%.3f").to_string()).color(TEXT_FAINT));
            if !r.raw.is_empty() {
                ui.label(RichText::new(open_stomp::midi::hex(&r.raw[..r.raw.len().min(96)])).font(font(11.0)).color(TEXT_DIM));
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
        if self.held_since.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(SETTLE_SECS / 2.0));
        }
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
        if ui.input_mut(|i| i.consume_shortcut(&LOG_SHORTCUT)) {
            self.show_log = !self.show_log;
        }
        if cfg!(debug_assertions) && ui.input(|i| i.key_pressed(egui::Key::F12)) {
            self.dev_inspector = !self.dev_inspector;
        }
        if self.dev_inspector {
            let ctx = ui.ctx().clone();
            egui::Window::new("Style & inspection (F12)")
                .open(&mut self.dev_inspector)
                .default_size([380.0, 600.0])
                .vscroll(true)
                .show(&ctx, |ui| {
                    ui.collapsing("Style", |ui| ctx.style_ui(ui, ctx.theme()));
                    ui.collapsing("Inspection", |ui| ctx.inspection_ui(ui));
                });
        }
        if self.show_log {
            egui::Panel::right("log")
                .resizable(true)
                .default_size((full_w * 0.34).clamp(380.0, 600.0))
                .size_range(340.0..=1000.0)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 4)))
                .show(ui, |ui| self.log_panel(ui, now));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(18)))
            .show(ui, |ui| self.main_view(ui, now));
        if !self.can_edit_sound() {
            self.save_dialog = None;
        }
        self.save_window(ui.ctx());
        self.slot_import_window(ui.ctx());
        self.rename_window(ui.ctx());

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

#[cfg(test)]
mod tests {
    use super::preset_matches;

    #[test]
    fn preset_search() {
        assert!(preset_matches(4, Some("90 Redfire"), ""));
        assert!(preset_matches(4, Some("90 Redfire"), "red"));
        assert!(preset_matches(4, Some("90 Redfire"), "  REDFIRE "));
        assert!(preset_matches(4, Some("90 Redfire"), "5"));
        assert!(preset_matches(4, Some("90 Redfire"), "p05"));
        assert!(preset_matches(4, Some("90 Redfire"), "P5"));
        assert!(!preset_matches(4, Some("90 Redfire"), "lead"));
        assert!(!preset_matches(4, None, "red"));
        // "8" is preset P08 and any label containing 8 (P18, P28…), not P80 by number
        assert!(preset_matches(7, None, "8"));
        assert!(preset_matches(17, None, "8"));
    }
}
