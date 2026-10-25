// The tuner, looper and drum machine: cards under the chain, and the looper/drum bodies also show
// in the detail panel when their block is selected.

import {
  ArrowCounterClockwiseIcon,
  CassetteTapeIcon,
  MetronomeIcon,
  MusicNotesIcon,
  PlayIcon,
  RecordIcon,
  StopIcon,
  TrashIcon,
} from "@phosphor-icons/react";
import { useState } from "react";
import { send, useStore } from "../store";
import Slider from "./Slider";
import "./Extras.css";

const num = (v: number) => String(v);

export function LooperBody() {
  const l = useStore((s) => s.sections.extras?.looper);
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  if (!l) return null;

  const Icon = { 1: RecordIcon, 2: PlayIcon, 3: RecordIcon, 4: StopIcon }[l.state] ?? CassetteTapeIcon;
  const main =
    l.state === 0 || l.state === 4
      ? { text: "Rec", tip: "Start recording", Icon: RecordIcon }
      : l.state === 1
        ? { text: "Play", tip: "Close the loop and play", Icon: PlayIcon }
        : l.state === 2
          ? { text: "Dub", tip: "Overdub", Icon: RecordIcon }
          : { text: "Play", tip: "Stop overdubbing", Icon: PlayIcon };
  const layers = l.layers === 1 ? "overdub layer · undo available" : l.layers === 2 ? "base layer" : "no layers";

  return (
    <div className="extra" data-module="loop">
      <div className={`extra-state looper-${l.state}`}>
        <Icon size={34} weight="fill" />
        <div>
          <div className="extra-big">{l.stateName}</div>
          <div className="extra-sub">{layers}</div>
        </div>
      </div>
      {editable && (
        <div className="extra-buttons">
          <button className="pill pill-main" title={main.tip} onClick={() => send({ type: "looperButton", button: "recPlayDub" })}>
            <main.Icon size={14} weight="fill" /> {main.text}
          </button>
          <button className="pill" title="Stop" onClick={() => send({ type: "looperButton", button: "stop" })}>
            <StopIcon size={14} weight="fill" />
          </button>
          <button className="pill" title="Undo / redo the last overdub" onClick={() => send({ type: "looperButton", button: "undoRedo" })}>
            <ArrowCounterClockwiseIcon size={14} weight="bold" />
          </button>
          <button className="pill" title="Clear the loop" onClick={() => send({ type: "looperButton", button: "clear" })}>
            <TrashIcon size={14} />
          </button>
        </div>
      )}
      <Slider label="Rec level" value={l.recLevel} disabled={!editable} format={num} onChange={(v) => send({ type: "setLooper", recLevel: v })} />
      <Slider label="Playback" value={l.playLevel} disabled={!editable} format={num} onChange={(v) => send({ type: "setLooper", playLevel: v })} />
      <div className="extra-buttons">
        <button
          className={`pill ${l.sync ? "pill-on" : ""}`}
          disabled={!editable}
          title="Loop sync on/off"
          onClick={() => send({ type: "setLooper", sync: !l.sync })}
        >
          Sync {l.sync == null ? "—" : l.sync ? "on" : "off"}
        </button>
        <button
          className="pill"
          disabled={!editable}
          title="Loop mode NR / AR"
          onClick={() => send({ type: "setLooper", arMode: (l.mode ?? 0) === 0 })}
        >
          Mode {l.mode == null ? "—" : l.mode === 0 ? "NR" : "AR"}
        </button>
      </div>
    </div>
  );
}

function PatternPicker({ value, onPick }: { value: number | null; onPick: (v: number) => void }) {
  const styles = useStore((s) => s.sections.catalog?.drumStyles ?? []);
  const [style, setStyle] = useState(() => Math.floor((value ?? 0) / 10));
  const patterns = styles[style]?.[1] ?? [];
  return (
    <div className="patterns">
      <div className="pattern-styles">
        {styles.map(([name], i) => (
          <button key={name} className={i === style ? "pill pill-on" : "pill"} onClick={() => setStyle(i)}>
            {name}
          </button>
        ))}
      </div>
      <div className="pattern-grid">
        {patterns.map(([v, name]) => (
          <button key={v} className={v === value ? "pattern pattern-on" : "pattern"} onClick={() => onPick(v)}>
            {name}
          </button>
        ))}
      </div>
    </div>
  );
}

export function DrumBody({ patterns = false }: { patterns?: boolean }) {
  const d = useStore((s) => s.sections.extras?.drum);
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  if (!d) return null;
  return (
    <div className="extra" data-module="drum">
      <div className={`extra-state ${d.playing ? "drum-playing" : ""}`}>
        {d.playing ? <PlayIcon size={34} weight="fill" /> : <StopIcon size={34} weight="fill" />}
        <div>
          <div className="extra-big">{d.patternName ?? "—"}</div>
          <div className="extra-sub">{d.playing ? "playing" : "stopped"}</div>
        </div>
        <div className="bpm">
          <span>{d.bpm ?? "—"}</span>
          BPM
        </div>
      </div>
      {editable && (
        <div className="extra-buttons">
          <button className="pill pill-main" onClick={() => send({ type: "setDrum", playing: !d.playing })}>
            {d.playing ? <StopIcon size={14} weight="fill" /> : <PlayIcon size={14} weight="fill" />}
            {d.playing ? "Stop" : "Play"}
          </button>
        </div>
      )}
      <Slider label="Level" value={d.level} disabled={!editable} format={num} onChange={(v) => send({ type: "setDrum", level: v })} />
      {patterns && editable && <PatternPicker value={d.pattern} onPick={(v) => send({ type: "setDrum", pattern: v })} />}
    </div>
  );
}

/** −50…+50 cents as a needle on a scale; green when within ±5. */
function CentsMeter({ cents, live }: { cents: number | null; live: boolean }) {
  const pos = live && cents != null ? 50 + cents : null;
  const tone = cents == null ? "" : Math.abs(cents) <= 5 ? "in-tune" : Math.abs(cents) <= 20 ? "close" : "off";
  return (
    <div className="cents">
      <span className="cents-zero" />
      {pos != null && <span className={`cents-needle ${tone}`} style={{ left: `${pos}%` }} />}
    </div>
  );
}

export function TunerCard() {
  const t = useStore((s) => s.sections.extras?.tuner);
  if (!t) return null;
  const live = t.detected && t.note != null;
  const tone = !live || t.cents == null ? "" : Math.abs(t.cents) <= 5 ? "in-tune" : Math.abs(t.cents) <= 20 ? "close" : "off";
  return (
    <section className="card">
      <h2 className="card-title">
        <MusicNotesIcon size={15} /> Tuner
      </h2>
      <div className={`tuner ${tone} ${live ? "" : "tuner-idle"}`}>
        <span className="tuner-note">{live ? t.note : "—"}</span>
        <div>
          <div className="tuner-cents">{live && t.cents != null ? `${t.cents > 0 ? "+" : ""}${t.cents} ¢` : "—"}</div>
          <div className="extra-sub">{t.active || live ? "listening" : "tuner off"}</div>
        </div>
      </div>
      <CentsMeter cents={t.cents} live={live} />
    </section>
  );
}

export function LooperCard() {
  return (
    <section className="card" data-module="loop">
      <h2 className="card-title">
        <CassetteTapeIcon size={15} /> Looper
      </h2>
      <LooperBody />
    </section>
  );
}

export function DrumCard() {
  return (
    <section className="card" data-module="drum">
      <h2 className="card-title">
        <MetronomeIcon size={15} /> Drum machine
      </h2>
      <DrumBody />
    </section>
  );
}
