//! "Import NAM model" and "Import IR": convert a `.nam` or WAV file on a worker thread, then hand
//! the result to the hub for upload to a user AMP or IR slot. Both default to open-stomp's
//! optimised conversions, with the official app's algorithms as the alternative. (The same steps
//! as the egui app's `slot_import.rs`, without its UI.)

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

use open_stomp::nam::convert::{self, Measure};
use serde::{Deserialize, Serialize};

/// Optimiser steps the library reports (for the progress bar before its first report).
const STEPS: usize = 300;

/// Which kind of user slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// USER AMP 01–10: `.nam` models.
    Amp,
    /// USER IR 01–10: impulse responses (WAV).
    Ir,
}

impl Kind {
    pub fn slot_label(self, slot: u8) -> String {
        match self {
            Kind::Amp => format!("USER AMP {:02}", slot + 1),
            Kind::Ir => format!("USER IR {:02}", slot + 1),
        }
    }
}

/// What the frontend asks to import.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub kind: Kind,
    pub slot: u8,
    pub path: String,
    pub optimised: bool,
    /// NAM: scale every model to the same output level, like NAM plugins.
    pub normalise: bool,
}

/// The 2048 floats and the slot-info factor (NAM only).
pub type Converted = Result<(Vec<f32>, f32), String>;

/// A conversion running on a worker thread.
pub struct Job {
    pub kind: Kind,
    pub slot: u8,
    pub name: String,
    pub optimised: bool,
    /// Convert only, never upload (`STOMP_DEBUG_IMPORT`).
    pub dry_run: bool,
    done: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Converted>,
    started_ms: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    kind: Kind,
    slot: u8,
    name: String,
    optimised: bool,
    done: usize,
    total: usize,
    /// Unix time in ms, so the frontend can show the elapsed time without updates from here.
    started_ms: u64,
}

/// The slot name the official app would use: the file name without its extension (IRs: the first
/// 16 characters).
pub fn slot_name(kind: Kind, path: &Path) -> String {
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

fn convert_file(
    kind: Kind,
    path: &Path,
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

impl Job {
    /// Start converting; `wake` runs after every progress step and at the end.
    pub fn start(req: Request, dry_run: bool, wake: impl Fn() + Send + 'static) -> Job {
        let (tx, rx) = mpsc::channel();
        let done = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(STEPS));
        let cancel = Arc::new(AtomicBool::new(false));
        let path = PathBuf::from(&req.path);
        let name = slot_name(req.kind, &path);
        let (kind, optimised, normalise) = (req.kind, req.optimised, req.normalise);
        let (d, t, c) = (done.clone(), total.clone(), cancel.clone());
        std::thread::Builder::new()
            .name("slot-import".into())
            .spawn(move || {
                let result = convert_file(kind, &path, optimised, normalise, &mut |step, steps| {
                    d.store(step, Ordering::Relaxed);
                    t.store(steps, Ordering::Relaxed);
                    wake();
                    !c.load(Ordering::Relaxed)
                });
                let _ = tx.send(result);
                wake();
            })
            .expect("spawn the import thread");
        let started_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        Job { kind, slot: req.slot, name, optimised, dry_run, done, total, cancel, rx, started_ms }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The result, once the conversion has finished.
    pub fn finished(&self) -> Option<Converted> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("the conversion stopped unexpectedly".into())),
        }
    }

    pub fn view(&self) -> JobView {
        JobView {
            kind: self.kind,
            slot: self.slot,
            name: self.name.clone(),
            optimised: self.optimised,
            done: self.done.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed).max(1),
            started_ms: self.started_ms,
        }
    }

    /// How long it took, for the outcome message.
    pub fn elapsed_secs(&self) -> u64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        now.saturating_sub(self.started_ms) / 1000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(slot_name(Kind::Amp, Path::new("/x/Jcm800 Crunch.nam")), "Jcm800 Crunch");
        assert_eq!(slot_name(Kind::Ir, Path::new("/x/Celestion V30 SM57 edge.wav")), "Celestion V30 SM");
        assert_eq!(slot_in_use(Some("USER IR 07")), None);
        assert_eq!(slot_in_use(Some("Jcm800")), Some("Jcm800"));
        assert_eq!((slot_in_use(Some("--")), slot_in_use(Some("")), slot_in_use(None)), (None, None, None));
    }
}
