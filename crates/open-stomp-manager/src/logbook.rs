//! The activity log: stores rows, merges knob sweeps into one row, and filters.

use std::time::Duration;

use chrono::{DateTime, Local};
use stomp_proto::{Category, Event, Origin};

const MAX_ROWS: usize = 50_000;
/// Consecutive updates to the same control within this window merge into one row.
const COALESCE_WINDOW: Duration = Duration::from_millis(2500);
/// How far back to look for a row to merge into (skipping hidden plumbing rows).
const COALESCE_LOOKBACK: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Event,
    Mark,
    Info,
    Error,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub kind: RowKind,
    pub at: DateTime<Local>,
    pub last_at: DateTime<Local>,
    pub origin: Origin,
    pub category: Category,
    pub subject: String,
    pub value: String,
    /// Value before a merged sweep started.
    pub from: Option<String>,
    pub count: u32,
    pub coalesce: Option<u64>,
    pub redundant: bool,
    pub raw: Vec<u8>,
    /// Monotonic id, for flash animations.
    pub id: u64,
}

impl Row {
    pub fn is_plumbing(&self) -> bool {
        self.redundant || self.category == Category::Protocol
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub presets: bool,
    pub effects: bool,
    pub looper: bool,
    pub drum: bool,
    pub tuner: bool,
    pub device: bool,
    pub sysex: bool,
    pub verbose: bool,
    pub pedal: bool,
    pub host: bool,
    pub search: String,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            presets: true,
            effects: true,
            looper: true,
            drum: true,
            tuner: true,
            device: true,
            sysex: false,
            verbose: false,
            pedal: true,
            host: true,
            search: String::new(),
        }
    }
}

impl Filter {
    pub fn accepts(&self, r: &Row) -> bool {
        if r.kind != RowKind::Event {
            return self.search_matches(r);
        }
        let origin_ok = match r.origin {
            Origin::Pedal => self.pedal,
            Origin::Host => self.host,
        };
        let cat_ok = match r.category {
            Category::Preset | Category::Volume => self.presets,
            Category::Effect(_) => self.effects,
            Category::Looper => self.looper,
            Category::Drum => self.drum,
            Category::Tuner => self.tuner,
            Category::Settings | Category::Device | Category::Navigation | Category::Unknown => self.device,
            Category::Protocol => self.sysex,
        };
        origin_ok && cat_ok && (self.verbose || !r.redundant) && self.search_matches(r)
    }

    fn search_matches(&self, r: &Row) -> bool {
        let q = self.search.trim();
        if q.is_empty() {
            return true;
        }
        let q = q.to_lowercase();
        r.subject.to_lowercase().contains(&q)
            || r.value.to_lowercase().contains(&q)
            || r.category.label().to_lowercase().contains(&q)
    }
}

#[derive(Default)]
pub struct LogBook {
    pub rows: Vec<Row>,
    next_id: u64,
    /// Bumped whenever rows change, so the view can cache its filtered index.
    pub generation: u64,
}

impl LogBook {
    pub fn clear(&mut self) {
        self.rows.clear();
        self.generation += 1;
    }

    pub fn push_event(&mut self, at: DateTime<Local>, ev: Event) {
        self.generation += 1;
        if let Some(key) = ev.coalesce
            && !ev.redundant
            && let Some(i) = self.merge_target(key, at)
        {
            let id = self.next_id;
            self.next_id += 1;
            let row = &mut self.rows[i];
            if row.from.is_none() {
                row.from = Some(std::mem::take(&mut row.value));
            }
            row.value = ev.value;
            row.subject = ev.subject;
            row.raw = ev.raw;
            row.count += 1;
            row.last_at = at;
            row.id = id;
            return;
        }
        let row = Row {
            kind: RowKind::Event,
            at,
            last_at: at,
            origin: ev.origin,
            category: ev.category,
            subject: ev.subject,
            value: ev.value,
            from: None,
            count: 1,
            coalesce: ev.coalesce,
            redundant: ev.redundant,
            raw: ev.raw,
            id: 0,
        };
        self.push(row);
    }

    pub fn push_note(&mut self, at: DateTime<Local>, kind: RowKind, text: String) {
        self.generation += 1;
        self.push(Row {
            kind,
            at,
            last_at: at,
            origin: Origin::Host,
            category: Category::Device,
            subject: text,
            value: String::new(),
            from: None,
            count: 1,
            coalesce: None,
            redundant: false,
            raw: vec![],
            id: 0,
        });
    }

    fn push(&mut self, mut row: Row) {
        row.id = self.next_id;
        self.next_id += 1;
        self.rows.push(row);
        if self.rows.len() > MAX_ROWS {
            self.rows.drain(..MAX_ROWS / 10);
        }
    }

    fn merge_target(&self, key: u64, at: DateTime<Local>) -> Option<usize> {
        let n = self.rows.len();
        let mut found = None;
        for i in (n.saturating_sub(COALESCE_LOOKBACK)..n).rev() {
            let r = &self.rows[i];
            if r.coalesce == Some(key) && !r.redundant {
                let recent = (at - r.last_at).to_std().map(|d| d < COALESCE_WINDOW).unwrap_or(true);
                if recent {
                    found = Some(i);
                }
                break;
            }
            if !r.is_plumbing() {
                break;
            }
        }
        found
    }

    pub fn export_text(&self, filter: &Filter) -> String {
        let mut s = String::new();
        for r in self.rows.iter().filter(|r| filter.accepts(r)) {
            let t = r.at.format("%H:%M:%S%.3f");
            match r.kind {
                RowKind::Mark => s.push_str(&format!("{t}  ===== {} =====\n", r.subject)),
                RowKind::Info | RowKind::Error => s.push_str(&format!("{t}  -- {}\n", r.subject)),
                RowKind::Event => {
                    let who = if r.origin == Origin::Pedal { "PEDAL" } else { "HOST " };
                    let val = match &r.from {
                        Some(f) => format!("{f} → {} (×{})", r.value, r.count),
                        None => r.value.clone(),
                    };
                    s.push_str(&format!("{t}  {who}  {:9} {}  {}\n", r.category.label(), r.subject, val));
                }
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stomp_proto::DeviceState;

    #[test]
    fn knob_sweep_merges_into_one_row() {
        let mut st = DeviceState::default();
        let mut log = LogBook::default();
        let t = Local::now();
        for (i, v) in [10u8, 20, 30, 40].into_iter().enumerate() {
            let at = t + chrono::Duration::milliseconds(100 * i as i64);
            log.push_event(at, st.apply(&[0xB0, 0x12, v], Origin::Pedal));
        }
        // a different knob starts a new row
        log.push_event(t, st.apply(&[0xB0, 0x13, 5], Origin::Pedal));
        assert_eq!(log.rows.len(), 2);
        assert_eq!(log.rows[0].count, 4);
        assert_eq!(log.rows[0].from.as_deref(), Some("10"));
        assert_eq!(log.rows[0].value, "40");
    }

    #[test]
    fn preset_changes_do_not_merge() {
        let mut st = DeviceState::default();
        let mut log = LogBook::default();
        log.push_event(Local::now(), st.apply(&[0xC0, 1], Origin::Pedal));
        log.push_event(Local::now(), st.apply(&[0xC0, 2], Origin::Pedal));
        assert_eq!(log.rows.len(), 2);
    }
}
