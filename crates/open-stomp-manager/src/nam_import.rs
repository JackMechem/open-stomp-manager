//! "Import NAM…": convert a `.nam` file on a worker thread and hand the result back for upload to
//! a user AMP slot. The default is open-stomp's optimised conversion (about a minute, closest to
//! the model on hardware); the official app's algorithm is the fast alternative.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use egui::{Align2, RichText, Ui, vec2};
use egui_phosphor::regular as icon;
use open_stomp::nam::convert::{self, Converted, Measure};

use crate::theme::*;

/// Optimiser steps reported by the library (for the progress bar before the first report).
const STEPS: usize = 300;

/// A conversion running on a worker thread.
struct Job {
    slot: u8,
    name: String,
    optimised: bool,
    done: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<Converted, String>>,
    started: Instant,
}

/// What the dialog asks the app to do.
pub struct Upload {
    pub slot: u8,
    pub name: String,
    pub data: Vec<f32>,
    pub factor: f32,
}

pub struct ImportDialog {
    slot: u8,
    path: Option<PathBuf>,
    optimised: bool,
    job: Option<Job>,
    /// The outcome of the last import: text and whether it is an error.
    outcome: Option<(String, bool)>,
}

/// The slot name the official app would use: the file name without `.nam`.
fn slot_name(path: &std::path::Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A user AMP slot's name, or `None` when the slot is empty (no name, `--…`, or the pedal's own
/// default `USER AMP NN`).
pub fn amp_name(name: Option<&str>) -> Option<&str> {
    let n = name?.trim();
    let default = n.strip_prefix("USER AMP ").is_some_and(|d| d.len() == 2 && d.bytes().all(|b| b.is_ascii_digit()));
    (!n.is_empty() && !n.starts_with("--") && !default).then_some(n)
}

fn slot_label(i: u8, names: &[Option<String>; 10]) -> String {
    let n = names[i as usize].as_deref();
    let name = match (n, amp_name(n)) {
        (_, Some(n)) => n,
        (Some(_), None) => "empty",
        (None, _) => "—",
    };
    format!("USER AMP {:02}  ·  {name}", i + 1)
}

impl ImportDialog {
    pub fn new(slot: u8) -> Self {
        ImportDialog { slot, path: None, optimised: true, job: None, outcome: None }
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
        let optimised = self.optimised;
        let (d, t, c, ctx2) = (done.clone(), total.clone(), cancel.clone(), ctx.clone());
        std::thread::Builder::new()
            .name("nam-import".into())
            .spawn(move || {
                let result = (|| {
                    let mut model = open_stomp::nam::Model::load(&path).map_err(|e| e.to_string())?;
                    let x = convert::excitation();
                    Ok(if optimised {
                        convert::convert_optimised(&mut model, &x, &mut |step, steps| {
                            d.store(step, Ordering::Relaxed);
                            t.store(steps, Ordering::Relaxed);
                            ctx2.request_repaint();
                            !c.load(Ordering::Relaxed)
                        })
                    } else {
                        convert::convert_measured(&mut model, &x, Measure::App)
                    })
                })();
                let _ = tx.send(result);
                ctx2.request_repaint();
            })
            .expect("spawn the import thread");
        self.outcome = None;
        self.job = Some(Job {
            slot: self.slot,
            name: slot_name(self.path.as_deref().unwrap_or(std::path::Path::new(""))),
            optimised,
            done,
            total,
            cancel,
            rx,
            started: Instant::now(),
        });
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
        match result {
            Err(e) => {
                self.outcome = Some((format!("Couldn't convert the model: {e}"), true));
                None
            }
            Ok(_) if !can_upload => {
                self.outcome = Some(("Converted, but the pedal is no longer connected: nothing was uploaded.".into(), true));
                None
            }
            Ok(c) => {
                let how = if job.optimised { "optimised" } else { "official algorithm" };
                self.outcome = Some((
                    format!("Imported “{}” into USER AMP {:02} ({how}, {}:{:02}).", job.name, job.slot + 1, secs / 60, secs % 60),
                    false,
                ));
                Some(Upload { slot: job.slot, name: job.name, data: c.data, factor: c.factor })
            }
        }
    }

    /// Draw the dialog. Returns whether it stays open, and an upload to perform.
    pub fn show(&mut self, ctx: &egui::Context, user_amps: &[Option<String>; 10], can_upload: bool) -> (bool, Option<Upload>) {
        let upload = self.poll(can_upload);
        let mut open = true;
        let mut close = false;
        egui::Window::new(format!("{}  Import NAM model", icon::FILE_ARROW_UP))
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
                self.form(ui, user_amps, can_upload, &mut close);
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
        ui.label(RichText::new(format!("Converting “{}” for USER AMP {:02}", job.name, job.slot + 1)).color(TEXT));
        ui.add_space(6.0);
        let (frac, text) = if !job.optimised {
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
        if job.optimised {
            ui.label(RichText::new("About a minute in total. The app stays usable meanwhile.").font(font(11.5)).color(TEXT_FAINT));
        }
    }

    fn form(&mut self, ui: &mut Ui, user_amps: &[Option<String>; 10], can_upload: bool, close: &mut bool) {
        egui::Grid::new("nam_import_grid").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
            ui.label("Model");
            ui.horizontal(|ui| {
                if ui.button(format!("{}  Choose file…", icon::FOLDER_OPEN)).clicked()
                    && let Some(p) = rfd::FileDialog::new().add_filter("NAM model", &["nam"]).set_title("Import NAM model").pick_file()
                {
                    self.path = Some(p);
                    self.outcome = None;
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
            egui::ComboBox::from_id_salt("nam_import_slot").selected_text(slot_label(self.slot, user_amps)).width(300.0).show_ui(
                ui,
                |ui| {
                    for i in 0..10u8 {
                        ui.selectable_value(&mut self.slot, i, slot_label(i, user_amps));
                    }
                },
            );
            ui.end_row();

            ui.label("Conversion");
            ui.vertical(|ui| {
                ui.radio_value(&mut self.optimised, true, "Optimised: closest to the model (about a minute)");
                ui.radio_value(&mut self.optimised, false, "Official app's algorithm (a few seconds)");
            });
            ui.end_row();
        });

        if let Some(n) = amp_name(user_amps[self.slot as usize].as_deref()) {
            ui.label(RichText::new(format!("{}  This replaces “{n}” in USER AMP {:02}.", icon::WARNING, self.slot + 1)).color(ACCENT));
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
        assert_eq!(slot_name(std::path::Path::new("/x/Jcm800 Crunch.nam")), "Jcm800 Crunch");
        let mut names: [Option<String>; 10] = Default::default();
        names[0] = Some("Jcm800".into());
        names[1] = Some("USER AMP 02".into());
        assert_eq!(slot_label(0, &names), "USER AMP 01  ·  Jcm800");
        assert_eq!(slot_label(1, &names), "USER AMP 02  ·  empty");
        assert_eq!(slot_label(2, &names), "USER AMP 03  ·  —");
        assert_eq!((amp_name(Some("--")), amp_name(Some("USER AMP 1X")), amp_name(Some(""))), (None, Some("USER AMP 1X"), None));
    }
}
