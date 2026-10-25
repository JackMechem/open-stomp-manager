import { ArrowLineDownIcon, BracketsCurlyIcon, CopyIcon, FlagIcon, FunnelIcon, HandTapIcon, InfoIcon, MagnifyingGlassIcon, PulseIcon, TrashIcon, WarningIcon, XIcon } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { send, useStore } from "../store";
import type { LogFilter, LogRow } from "../types";
import { setShowLog } from "../ui";
import "./LogPanel.css";

const OVERSCAN = 20;

const DEFAULT_FILTER: LogFilter = {
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
  search: "",
};

const CHIPS: { key: keyof Omit<LogFilter, "search">; label: string; color: string; title?: string }[] = [
  { key: "presets", label: "Presets", color: "var(--accent)" },
  { key: "effects", label: "Effects", color: "var(--mod-amp)" },
  { key: "looper", label: "Looper", color: "var(--mod-loop)" },
  { key: "drum", label: "Drum", color: "var(--mod-drum)" },
  { key: "tuner", label: "Tuner", color: "#9cf26b" },
  { key: "device", label: "Device", color: "#b8c4d8" },
  { key: "sysex", label: "SysEx", color: "var(--text-dim)", title: "Request/reply plumbing: sync reads, acks, uploads" },
  { key: "verbose", label: "Redundant", color: "var(--text-dim)", title: "Values the pedal re-sends after a preset change that were already known" },
  { key: "pedal", label: "From pedal", color: "var(--accent)" },
  { key: "host", label: "From host", color: "var(--host)" },
];

function Row({ row, fresh, raw }: { row: LogRow; fresh: boolean; raw: boolean }) {
  const cls = `row row-${row.kind} ${fresh ? "row-new" : ""}`;
  if (row.kind === "mark") {
    return (
      <div className={cls}>
        <span className="row-time">{row.time}</span>
        <span className="row-mark">
          <FlagIcon size={13} weight="fill" /> {row.subject}
        </span>
      </div>
    );
  }
  if (row.kind !== "event") {
    return (
      <div className={cls}>
        <span className="row-time">{row.time}</span>
        <span className="row-note">
          {row.kind === "error" ? <WarningIcon size={13} /> : <InfoIcon size={13} />} {row.subject}
        </span>
      </div>
    );
  }
  const title = [row.from != null ? `${row.from} → ${row.value} (${row.count} updates)` : row.value, row.raw.slice(0, 290)].join("\n");
  return (
    <div className={`${cls} ${raw ? "row-with-raw" : ""}`} data-category={row.category} title={title}>
      <span className="row-time">{row.time}</span>
      <span className={`row-origin row-origin-${row.origin}`}>{row.origin === "pedal" ? "Pedal" : "Host"}</span>
      <span className="row-category">{row.categoryTitle}</span>
      <span className={`row-subject ${row.plumbing ? "row-plumbing" : ""}`}>{row.subject}</span>
      {row.from != null && <span className="row-from">{row.from} →</span>}
      <span className="row-value">{row.value}</span>
      {row.count > 1 && <span className="row-count">×{row.count}</span>}
      {raw && row.raw && <span className="row-raw">{row.raw}</span>}
    </div>
  );
}

export default function LogPanel() {
  const summary = useStore((s) => s.sections.log);
  const [filter, setFilter] = useState(DEFAULT_FILTER);
  const [raw, setRaw] = useState(false);
  const [follow, setFollow] = useState(true);
  const [view, setView] = useState<{ start: number; rows: LogRow[]; fresh: Set<number> }>({ start: 0, rows: [], fresh: new Set() });
  const [range, setRange] = useState({ top: 0, height: 0 });
  const scroller = useRef<HTMLDivElement>(null);
  const maxSeen = useRef(-1);
  const request = useRef(0);
  const rowH = raw ? 46 : 30;
  const count = summary?.count ?? 0;

  // the search waits for a pause in typing; the chips apply at once
  useEffect(() => {
    const t = setTimeout(() => send({ type: "setLogFilter", filter }), filter.search ? 150 : 0);
    return () => clearTimeout(t);
  }, [filter]);

  // fetch the rows on screen whenever the log, the filter or the scroll position changes
  const first = Math.max(0, Math.floor(range.top / rowH) - OVERSCAN);
  const last = Math.min(count, Math.ceil((range.top + range.height) / rowH) + OVERSCAN);
  useEffect(() => {
    const id = ++request.current;
    invoke<LogRow[]>("log_rows", { start: first, count: last - first }).then((rows) => {
      if (id !== request.current) return;
      const fresh = new Set(rows.filter((r) => r.id > maxSeen.current && maxSeen.current >= 0).map((r) => r.id));
      maxSeen.current = Math.max(maxSeen.current, ...rows.map((r) => r.id), 0);
      setView({ start: first, rows, fresh });
    });
  }, [summary?.generation, count, first, last]);

  // stay at the bottom while following
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && follow) el.scrollTop = el.scrollHeight;
  }, [count, follow, rowH]);

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const measure = () => setRange({ top: el.scrollTop, height: el.clientHeight });
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    measure();
    return () => ro.disconnect();
  }, []);

  const onScroll = () => {
    const el = scroller.current!;
    setRange({ top: el.scrollTop, height: el.clientHeight });
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < rowH;
    if (atBottom !== follow) setFollow(atBottom);
  };

  const copy = async () => {
    const text = await invoke<string>("log_export");
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // older WebKitGTK: the clipboard API needs a fallback
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
    }
  };

  return (
    <aside className="log">
      <header className="log-head">
        <PulseIcon size={15} />
        <span className="log-title">Activity</span>
        <span className="log-count">{count}</span>
        <span className="log-tools">
          <button className={follow ? "on" : ""} title="Follow new events" onClick={() => setFollow(!follow)}>
            <ArrowLineDownIcon size={15} />
          </button>
          <button className={raw ? "on" : ""} title="Show raw MIDI bytes" onClick={() => setRaw(!raw)}>
            <BracketsCurlyIcon size={15} />
          </button>
          <button title="Copy the visible log as text" onClick={copy}>
            <CopyIcon size={15} />
          </button>
          <button title="Clear the log" onClick={() => send({ type: "clearLog" })}>
            <TrashIcon size={15} />
          </button>
          <button title="Hide the activity log" onClick={() => setShowLog(false)}>
            <XIcon size={15} />
          </button>
        </span>
      </header>
      <div className="log-chips">
        {CHIPS.map((c) => (
          <button
            key={c.key}
            className={`log-chip ${filter[c.key] ? "on" : ""}`}
            style={{ "--chip": c.color } as React.CSSProperties}
            title={c.title}
            onClick={() => setFilter({ ...filter, [c.key]: !filter[c.key] })}
          >
            {c.label}
          </button>
        ))}
      </div>
      <label className="log-search">
        <MagnifyingGlassIcon size={14} />
        <input placeholder="Filter… (e.g. gain, looper, P05)" value={filter.search} onChange={(e) => setFilter({ ...filter, search: e.target.value })} />
      </label>
      <div className="log-rows" ref={scroller} onScroll={onScroll}>
        {count === 0 ? (
          <div className="log-empty">
            {(summary?.total ?? 0) > 0 ? (
              <>
                <FunnelIcon size={28} />
                Nothing matches the filter
              </>
            ) : (
              <>
                <HandTapIcon size={28} />
                Nothing yet: press a footswitch or turn a knob
              </>
            )}
          </div>
        ) : (
          <div style={{ height: count * rowH, position: "relative" }}>
            <div style={{ position: "absolute", top: view.start * rowH, left: 0, right: 0 }}>
              {view.rows.map((r, i) => (
                <div key={view.start + i} style={{ height: rowH }} className={(view.start + i) % 2 ? "stripe" : ""}>
                  <Row row={r} fresh={view.fresh.has(r.id)} raw={raw} />
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}
