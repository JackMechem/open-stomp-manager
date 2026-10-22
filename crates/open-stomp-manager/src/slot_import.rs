//! "Import NAM model…" and "Import IR…": convert a `.nam` or IR audio file on a worker thread and
//! hand the result back for upload to a user AMP or user IR slot. Both default to open-stomp's
//! optimised conversions (closest to the model / IR on hardware), with the official app's
//! algorithms as the alternative.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use egui::{Align2, RichText, Ui, vec2};
use egui_phosphor::regular as icon;
use open_stomp::nam::convert::{self, Measure};

use crate::theme::*;

/// Optimiser steps reported by the library (for the progress bar before the first report).
const STEPS: usize = 300;

/// Which kind of user slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// USER AMP 01–10: `.nam` models.
    Amp,
    /// USER IR 01–10: impulse responses (WAV).
    Ir,
}

impl Kind {
    fn slot_prefix(self) -> &'static str {
        match self {
            Kind::Amp => "USER AMP",
            Kind::Ir => "USER IR",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Kind::Amp => "Import NAM model",
            Kind::Ir => "Import IR",
        }
    }

    fn file_label(self) -> &'static str {
        match self {
            Kind::Amp => "Model",
            Kind::Ir => "IR file",
        }
    }

    fn filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Kind::Amp => ("NAM model", &["nam"]),
            Kind::Ir => ("Impulse response (WAV)", &["wav", "wave"]),
        }
    }
}

/// A converted file: the 2048 floats and the slot-info factor (NAM only).
type Converted = Result<(Vec<f32>, f32), String>;

/// A conversion running on a worker thread.
struct Job {
    kind: Kind,
    slot: u8,
    name: String,
    optimised: bool,
    done: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Converted>,
    started: Instant,
}

/// What the dialog asks the app to do.
pub struct Upload {
    pub kind: Kind,
    pub slot: u8,
    pub name: String,
    pub data: Vec<f32>,
    pub factor: f32,
}

pub struct ImportDialog {
    kind: Kind,
    slot: u8,
    path: Option<PathBuf>,
    optimised: bool,
    /// NAM: scale every model to the same output level, like NAM plugins.
    normalise: bool,
    job: Option<Job>,
    /// The open file dialog's answer (`None` inside: cancelled). It is awaited on its own thread:
    /// blocking the UI thread stalls the Wayland connection and the compositor drops the app.
    picker: Option<mpsc::Receiver<Option<PathBuf>>>,
    /// The outcome of the last import: text and whether it is an error.
    outcome: Option<(String, bool)>,
}

/// Open the file dialog without blocking the UI thread. The dialog is created here (macOS wants
/// the main thread for that) and awaited on a worker thread, which wakes the UI when it is done.
fn pick_file(ctx: &egui::Context, kind: Kind) -> mpsc::Receiver<Option<PathBuf>> {
    let (tx, rx) = mpsc::channel();
    let (label, exts) = kind.filter();
    let dialog = rfd::AsyncFileDialog::new().add_filter(label, exts).set_title(kind.title()).pick_file();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(pollster::block_on(dialog).map(|f| f.path().to_path_buf()));
        ctx.request_repaint();
    });
    rx
}

/// The slot name the official app would use: the file name without its extension (IRs: the first
/// 16 characters).
fn slot_name(kind: Kind, path: &std::path::Path) -> String {
    match kind {
        Kind::Amp => path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        Kind::Ir => open_stomp::ir::slot_name(path),
    }
}

/// A user slot's name, or `None` when the slot is empty (no name, `--…`, or the pedal's own default
/// `USER AMP NN` / `USER IR NN`).
pub fn slot_in_use(name: Option<&str>) -> Option<&str> {
    let n = name?.trim();
    let default = ["USER AMP ", "USER IR "]
        .iter()
        .any(|p| n.strip_prefix(p).is_some_and(|d| d.len() == 2 && d.bytes().all(|b| b.is_ascii_digit())));
    (!n.is_empty() && !n.starts_with("--") && !default).then_some(n)
}

fn slot_label(kind: Kind, i: u8, names: &[Option<String>; 10]) -> String {
    let n = names[i as usize].as_deref();
    let name = match (n, slot_in_use(n)) {
        (_, Some(n)) => n,
        (Some(_), None) => "empty",
        (None, _) => "—",
    };
    format!("{} {:02}  ·  {name}", kind.slot_prefix(), i + 1)
}

fn convert_file(
    kind: Kind,
    path: &std::path::Path,
    optimised: bool,
    normalise: bool,
    on_step: &mut dyn FnMut(usize, usize) -> bool,
) -> Converted {
    match kind {
        Kind::Amp => {
            let mut model = open_stomp::nam::Model::load(path).map_err(|e| e.to_string())?;
            let x = convert::excitation();
            let mut c = if optimised {
                convert::convert_optimised(&mut model, &x, on_step)
            } else {
                convert::convert_measured(&mut model, &x, Measure::App)
            };
            if normalise {
                convert::normalise_level(&mut c, &mut model);
            }
            Ok((c.data, c.factor))
        }
        Kind::Ir => {
            let wav = open_stomp::ir::Wav::load(path).map_err(|e| e.to_string())?;
            let method = if optimised { open_stomp::ir::Method::Optimised } else { open_stomp::ir::Method::App };
            Ok((open_stomp::ir::convert_with(&wav, method).data, 1.0))
        }
    }
}

impl ImportDialog {
    pub fn new(kind: Kind, slot: u8) -> Self {
        ImportDialog { kind, slot, path: None, optimised: true, normalise: true, job: None, picker: None, outcome: None }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Start converting `path` right away (`STOMP_DEBUG_IMPORT`, for screenshots of the progress view).
    pub fn debug_start(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.path = Some(path);
        self.start(ctx);
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn start(&mut self, ctx: &egui::Context) {
        let Some(path) = self.path.clone() else { return };
        let (tx, rx) = mpsc::channel();
        let done = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(STEPS));
        let cancel = Arc::new(AtomicBool::new(false));
        let (kind, optimised, normalise) = (self.kind, self.optimised, self.normalise);
        let name = slot_name(kind, &path);
        let (d, t, c, ctx2) = (done.clone(), total.clone(), cancel.clone(), ctx.clone());
        std::thread::Builder::new()
            .name("slot-import".into())
            .spawn(move || {
                let result = convert_file(kind, &path, optimised, normalise, &mut |step, steps| {
                    d.store(step, Ordering::Relaxed);
                    t.store(steps, Ordering::Relaxed);
                    ctx2.request_repaint();
                    !c.load(Ordering::Relaxed)
                });
                let _ = tx.send(result);
                ctx2.request_repaint();
            })
            .expect("spawn the import thread");
        self.outcome = None;
        self.job = Some(Job { kind, slot: self.slot, name, optimised, done, total, cancel, rx, started: Instant::now() });
    }

    /// Replace the outcome with an error (e.g. the upload itself failed).
    pub fn failed(&mut self, text: String) {
        self.outcome = Some((text, true));
    }

    fn cancel(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
            self.outcome = Some(("Import cancelled; the slot was not changed.".into(), true));
        }
    }

    /// Collect a finished conversion. Returns the upload when there is one to do.
    fn poll(&mut self, can_upload: bool) -> Option<Upload> {
        let job = self.job.as_ref()?;
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err("the conversion stopped unexpectedly".into()),
        };
        let job = self.job.take()?;
        let secs = job.started.elapsed().as_secs();
        let what = match job.kind {
            Kind::Amp => "the model",
            Kind::Ir => "the IR",
        };
        match result {
            Err(e) => {
                self.outcome = Some((format!("Couldn't convert {what}: {e}"), true));
                None
            }
            Ok(_) if !can_upload => {
                self.outcome = Some(("Converted, but the pedal isn't connected: nothing was uploaded.".into(), true));
                None
            }
            Ok((data, factor)) => {
                let how = match (job.kind, job.optimised) {
                    (Kind::Amp, true) => format!(" (optimised, {}:{:02})", secs / 60, secs % 60),
                    (Kind::Ir, true) => " (optimised)".to_owned(),
                    (_, false) => " (official algorithm)".to_owned(),
                };
                let target = format!("{} {:02}", job.kind.slot_prefix(), job.slot + 1);
                self.outcome = Some((format!("Imported “{}” into {target}{how}.", job.name), false));
                Some(Upload { kind: job.kind, slot: job.slot, name: job.name, data, factor })
            }
        }
    }

    /// Draw the dialog. `names` are the slot names of this dialog's kind. Returns whether it stays
    /// open, and an upload to perform.
    pub fn show(&mut self, ctx: &egui::Context, names: &[Option<String>; 10], can_upload: bool) -> (bool, Option<Upload>) {
        let upload = self.poll(can_upload);
        let mut open = true;
        let mut close = false;
        egui::Window::new(format!("{}  {}", icon::FILE_ARROW_UP, self.kind.title()))
            .id(egui::Id::new("slot_import"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(440.0);
                if let Some(job) = &self.job {
                    self.progress(ui, job);
                    ui.add_space(8.0);
                    if ui.button("Cancel").clicked() {
                        self.cancel();
                    }
                    return;
                }
                self.form(ui, names, can_upload, &mut close);
            });
        if self.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        if !open {
            self.cancel();
        }
        (open && !close, upload)
    }

    fn progress(&self, ui: &mut Ui, job: &Job) {
        let (done, total) = (job.done.load(Ordering::Relaxed), job.total.load(Ordering::Relaxed).max(1));
        let secs = job.started.elapsed().as_secs();
        let target = format!("{} {:02}", job.kind.slot_prefix(), job.slot + 1);
        ui.label(RichText::new(format!("Converting “{}” for {target}", job.name)).color(TEXT));
        ui.add_space(6.0);
        let (frac, text) = if job.kind == Kind::Ir {
            (None, "Converting the IR…".to_owned())
        } else if !job.optimised {
            (None, "Measuring the model…".to_owned())
        } else if done == 0 {
            (None, "Measuring the model and rendering the test signal…".to_owned())
        } else {
            (Some(done as f32 / total as f32), format!("Fitting the pedal to the model: step {done} of {total}"))
        };
        let bar = match frac {
            Some(f) => egui::ProgressBar::new(f).fill(ACCENT),
            None => egui::ProgressBar::new(0.0).animate(true).fill(ACCENT),
        };
        ui.add(bar.desired_width(440.0));
        ui.label(
            RichText::new(format!("{text}  ·  {}:{:02}", secs / 60, secs % 60)).font(font(11.5)).color(TEXT_DIM),
        );
        if job.optimised && job.kind == Kind::Amp {
            ui.label(RichText::new("About a minute in total. The app stays usable meanwhile.").font(font(11.5)).color(TEXT_FAINT));
        }
    }

    fn form(&mut self, ui: &mut Ui, names: &[Option<String>; 10], can_upload: bool, close: &mut bool) {
        let kind = self.kind;
        egui::Grid::new("slot_import_grid").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
            ui.label(kind.file_label());
            ui.horizontal(|ui| {
                if let Some(rx) = &self.picker {
                    match rx.try_recv() {
                        Ok(picked) => {
                            self.picker = None;
                            if let Some(p) = picked {
                                self.path = Some(p);
                                self.outcome = None;
                            }
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => self.picker = None,
                    }
                }
                let choose = ui.add_enabled(self.picker.is_none(), egui::Button::new(format!("{}  Choose file…", icon::FOLDER_OPEN)));
                if choose.clicked() {
                    self.picker = Some(pick_file(ui.ctx(), kind));
                }
                let name = self.path.as_deref().map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned());
                let (txt, col) = match &name {
                    Some(n) => (n.as_str(), TEXT),
                    None => ("no file chosen", TEXT_FAINT),
                };
                ui.label(RichText::new(crate::widgets::elide(ui, txt, font(12.5), ui.available_width())).font(font(12.5)).color(col));
            });
            ui.end_row();

            ui.label("Slot");
            egui::ComboBox::from_id_salt("slot_import_slot").selected_text(slot_label(kind, self.slot, names)).width(300.0).show_ui(
                ui,
                |ui| {
                    for i in 0..10u8 {
                        ui.selectable_value(&mut self.slot, i, slot_label(kind, i, names));
                    }
                },
            );
            ui.end_row();

            ui.label("Conversion");
            ui.vertical(|ui| match kind {
                Kind::Amp => {
                    ui.radio_value(&mut self.optimised, true, "Optimised: closest to the model (about a minute)");
                    ui.radio_value(&mut self.optimised, false, "Official app's algorithm (a few seconds)");
                }
                Kind::Ir => {
                    ui.radio_value(&mut self.optimised, true, "Optimised: fitted to the 8 ms the pedal plays")
                        .on_hover_text("The pedal plays only the first 384 samples of an IR; this fits them to the whole IR");
                    ui.radio_value(&mut self.optimised, false, "Official app's algorithm (the IR's first 42.7 ms)");
                }
            });
            ui.end_row();

            if kind == Kind::Amp {
                ui.label("Level");
                ui.checkbox(&mut self.normalise, "Normalise, like NAM plugins")
                    .on_hover_text("Every model plays at the same volume, whatever level it was captured at. Only the volume changes, not the tone.");
                ui.end_row();
            }
        });

        if kind == Kind::Ir {
            ui.label(RichText::new("Any WAV IR (first channel, resampled to 48 kHz).").font(font(11.5)).color(TEXT_FAINT));
        }
        if let Some(n) = slot_in_use(names[self.slot as usize].as_deref()) {
            let target = format!("{} {:02}", kind.slot_prefix(), self.slot + 1);
            ui.label(RichText::new(format!("{}  This replaces “{n}” in {target}.", icon::WARNING)).color(ACCENT));
        }
        if !can_upload {
            ui.label(RichText::new("Connect to the pedal (with Listen only off) to import.").color(TEXT_DIM));
        }
        if let Some((text, error)) = &self.outcome {
            let (ic, col) = if *error { (icon::X_CIRCLE, BAD) } else { (icon::CHECK_CIRCLE, GOOD) };
            ui.label(RichText::new(format!("{ic}  {text}")).color(col));
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Close").clicked() {
                *close = true;
            }
            let ready = self.path.is_some() && can_upload;
            let b = egui::Button::new(RichText::new(format!("{}  Import", icon::UPLOAD_SIMPLE)).color(BG)).fill(ACCENT);
            if ui.add_enabled(ready, b).clicked() {
                self.start(ui.ctx());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_labels() {
        assert_eq!(slot_name(Kind::Amp, std::path::Path::new("/x/Jcm800 Crunch.nam")), "Jcm800 Crunch");
        assert_eq!(slot_name(Kind::Ir, std::path::Path::new("/x/Celestion V30 SM57 edge.wav")), "Celestion V30 SM");
        let mut names: [Option<String>; 10] = Default::default();
        names[0] = Some("Jcm800".into());
        names[1] = Some("USER AMP 02".into());
        assert_eq!(slot_label(Kind::Amp, 0, &names), "USER AMP 01  ·  Jcm800");
        assert_eq!(slot_label(Kind::Amp, 1, &names), "USER AMP 02  ·  empty");
        assert_eq!(slot_label(Kind::Ir, 2, &names), "USER IR 03  ·  —");
        assert_eq!(slot_in_use(Some("USER IR 07")), None);
        assert_eq!((slot_in_use(Some("--")), slot_in_use(Some("USER AMP 1X")), slot_in_use(Some(""))), (None, Some("USER AMP 1X"), None));
    }
}
