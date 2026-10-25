//! The activity log, shared between the hub (which adds rows) and the `log_rows` command (which
//! reads the rows on screen). Rows, knob-sweep merging and filtering live in [`crate::logbook`].

use chrono::{DateTime, Local};
use open_stomp::{Category, Event, Origin};
use serde::{Deserialize, Serialize};

pub use crate::logbook::RowKind;
use crate::logbook::{Filter, LogBook, Row};

use crate::view;

#[derive(Default)]
pub struct Log {
    book: LogBook,
    filter: Filter,
    /// Indices of the rows the filter accepts, for (book generation, filter).
    visible: (u64, Filter, Vec<usize>),
}

/// The filter as the frontend sends it (`logbook::Filter`).
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FilterArgs {
    presets: bool,
    effects: bool,
    looper: bool,
    drum: bool,
    tuner: bool,
    device: bool,
    sysex: bool,
    verbose: bool,
    pedal: bool,
    host: bool,
    search: String,
}

/// What the frontend needs to lay out the list: the row count, and a number that changes whenever
/// rows do, to fetch the visible ones again.
#[derive(Serialize, PartialEq)]
pub struct Summary {
    generation: u64,
    count: usize,
    total: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowView {
    /// Changes when a merged row takes a new value, so the frontend can highlight it.
    id: u64,
    kind: &'static str,
    time: String,
    origin: &'static str,
    category: &'static str,
    category_title: &'static str,
    subject: String,
    value: String,
    from: Option<String>,
    count: u32,
    plumbing: bool,
    raw: String,
}

/// CSS-friendly name of a category (a module key for effects), for colours.
fn category_key(c: Category) -> &'static str {
    match c {
        Category::Effect(m) => view::key(m),
        Category::Preset => "preset",
        Category::Volume => "volume",
        Category::Navigation => "navigation",
        Category::Looper => "loop",
        Category::Drum => "drum",
        Category::Tuner => "tuner",
        Category::Settings => "settings",
        Category::Device => "device",
        Category::Protocol => "protocol",
        Category::Unknown => "unknown",
    }
}

fn category_title(c: Category) -> &'static str {
    match c {
        Category::Effect(m) => view::title(m),
        Category::Preset => "Preset",
        Category::Volume => "Volume",
        Category::Navigation => "Page",
        Category::Looper => "Looper",
        Category::Drum => "Drum",
        Category::Tuner => "Tuner",
        Category::Settings => "Settings",
        Category::Device => "Device",
        Category::Protocol => "SysEx",
        Category::Unknown => "Unknown",
    }
}

fn row_view(r: &Row) -> RowView {
    RowView {
        id: r.id,
        kind: match r.kind {
            RowKind::Event => "event",
            RowKind::Mark => "mark",
            RowKind::Info => "info",
            RowKind::Error => "error",
        },
        time: r.at.format("%H:%M:%S%.3f").to_string(),
        origin: match r.origin {
            Origin::Pedal => "pedal",
            Origin::Host => "host",
        },
        category: category_key(r.category),
        category_title: category_title(r.category),
        subject: r.subject.clone(),
        value: r.value.clone(),
        from: r.from.clone(),
        count: r.count,
        plumbing: r.is_plumbing(),
        raw: open_stomp::midi::hex(&r.raw),
    }
}

impl Log {
    pub fn push_event(&mut self, at: DateTime<Local>, ev: Event) {
        self.book.push_event(at, ev);
    }

    pub fn push_note(&mut self, at: DateTime<Local>, kind: RowKind, text: String) {
        self.book.push_note(at, kind, text);
    }

    pub fn clear(&mut self) {
        self.book.clear();
    }

    pub fn set_filter(&mut self, f: FilterArgs) {
        self.filter = Filter {
            presets: f.presets,
            effects: f.effects,
            looper: f.looper,
            drum: f.drum,
            tuner: f.tuner,
            device: f.device,
            sysex: f.sysex,
            verbose: f.verbose,
            pedal: f.pedal,
            host: f.host,
            search: f.search,
        };
    }

    fn visible(&mut self) -> &[usize] {
        if self.visible.0 != self.book.generation || self.visible.1 != self.filter {
            let idx = self.book.rows.iter().enumerate().filter(|(_, r)| self.filter.accepts(r)).map(|(i, _)| i).collect();
            self.visible = (self.book.generation, self.filter.clone(), idx);
        }
        &self.visible.2
    }

    pub fn summary(&mut self) -> Summary {
        let count = self.visible().len();
        Summary { generation: self.book.generation, count, total: self.book.rows.len() }
    }

    /// Rows `start..start + count` of the filtered list.
    pub fn rows(&mut self, start: usize, count: usize) -> Vec<RowView> {
        let idx: Vec<usize> = self.visible().iter().skip(start).take(count).copied().collect();
        idx.into_iter().map(|i| row_view(&self.book.rows[i])).collect()
    }

    /// The filtered log as text, for the clipboard.
    pub fn export(&self) -> String {
        self.book.export_text(&self.filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_stomp::DeviceState;

    #[test]
    fn filter_and_rows() {
        let mut st = DeviceState::default();
        let mut log = Log::default();
        let now = Local::now();
        log.push_event(now, st.apply(&[0xC0, 4], Origin::Pedal));
        log.push_event(now, st.apply(&[0xB0, 0x12, 64], Origin::Pedal));
        log.push_note(now, RowKind::Info, "connected".into());
        assert_eq!(log.summary().count, 3);

        let only_presets: FilterArgs = serde_json::from_value(serde_json::json!({
            "presets": true, "effects": false, "looper": true, "drum": true, "tuner": true,
            "device": true, "sysex": false, "verbose": false, "pedal": true, "host": true, "search": ""
        }))
        .unwrap();
        log.set_filter(only_presets);
        let rows = log.rows(0, 10);
        assert_eq!(rows.len(), 2, "the AMP knob is filtered out, the note stays");
        assert_eq!((rows[0].kind, rows[0].category, rows[0].origin), ("event", "preset", "pedal"));
        assert_eq!(rows[1].kind, "info");
    }
}
