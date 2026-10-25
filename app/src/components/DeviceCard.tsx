import { FileArrowUpIcon, FileAudioIcon, GearIcon, PencilSimpleIcon } from "@phosphor-icons/react";
import { send, useStore } from "../store";
import type { Slot, SlotKind } from "../types";
import { openDialog } from "../ui";
import Slider from "./Slider";
import "./DeviceCard.css";

const centred = (v: number) => `${v - 50 > 0 ? "+" : ""}${v - 50}`;
const trimDb = (v: number) => `${v - 10 > 0 ? "+" : ""}${v - 10} dB`;

function Info({ label, value }: { label: string; value: string | null | undefined }) {
  return (
    <div className="info">
      <span>{label}</span>
      <span className={value ? "" : "faint"}>{value || "—"}</span>
    </div>
  );
}

function Toggle({ label, on, disabled, title, onChange }: { label: string; on: boolean; disabled: boolean; title?: string; onChange: (on: boolean) => void }) {
  return (
    <button className={`switch ${on ? "switch-on" : ""}`} role="switch" aria-checked={on} disabled={disabled} title={title} onClick={() => onChange(!on)}>
      <span className="switch-track">
        <span className="switch-knob" />
      </span>
      {label}
    </button>
  );
}

function System() {
  const d = useStore((s) => s.sections.device);
  const editable = useStore((s) => (s.sections.conn?.editable ?? false) && s.sections.device?.settings != null);
  const set = d?.settings;
  const version = useStore((s) => s.sections.catalog?.appVersion);
  const lib = useStore((s) => s.sections.catalog?.libVersion);
  return (
    <div className="device-col">
      <h3>Device</h3>
      <Info label="Product" value={d?.product} />
      <Info label="Firmware" value={d?.firmware} />
      <Info label="Model ID" value={d?.modelId} />
      <Info label="App" value={version && `Open Stomp Manager ${version} · open-stomp ${lib}`} />
      <Slider label="Input trim" value={set?.inputTrim ?? null} max={20} centred format={trimDb} disabled={!editable} onChange={(v) => send({ type: "setSettings", inputTrim: v })} />
      <div className="switches">
        <Toggle label="Wireless" on={set?.wireless ?? false} disabled={!editable} title="Bluetooth MIDI and audio" onChange={(on) => send({ type: "setSettings", wireless: on })} />
        <Toggle label="Loopback" on={set?.loopback ?? false} disabled={!editable} title="USB audio loopback" onChange={(on) => send({ type: "setSettings", loopback: on })} />
      </div>
    </div>
  );
}

function GlobalEq() {
  const set = useStore((s) => s.sections.device?.settings);
  const editable = useStore((s) => (s.sections.conn?.editable ?? false) && s.sections.device?.settings != null);
  return (
    <div className="device-col" data-module="eq">
      <h3>Global EQ</h3>
      <Slider label="Bass" value={set?.eqBass ?? null} centred format={centred} disabled={!editable} onChange={(v) => send({ type: "setSettings", eqBass: v })} />
      <Slider label="Mid" value={set?.eqMid ?? null} centred format={centred} disabled={!editable} onChange={(v) => send({ type: "setSettings", eqMid: v })} />
      <Slider label="Treble" value={set?.eqTreble ?? null} centred format={centred} disabled={!editable} onChange={(v) => send({ type: "setSettings", eqTreble: v })} />
      <p className="note">Global EQ changed on the pedal itself isn't reported over USB, so this may be out of date.</p>
    </div>
  );
}

function SlotCell({ kind, index, slot, editable }: { kind: SlotKind; index: number; slot: Slot; editable: boolean }) {
  const text = slot.inUse ?? (slot.name == null ? "—" : "empty");
  return (
    <div className={`slot ${slot.inUse ? "" : "slot-empty"}`}>
      <span className="slot-name" title={slot.inUse ?? undefined}>
        {text}
      </span>
      <span className="slot-actions">
        <button title={kind === "amp" ? "Import a NAM model here" : "Import an IR here"} onClick={() => openDialog({ kind: "import", slotKind: kind, slot: index })}>
          {kind === "amp" ? <FileArrowUpIcon size={14} /> : <FileAudioIcon size={14} />}
        </button>
        <button
          title={!slot.inUse ? "The slot is empty" : editable ? "Rename" : "Connect, with Listen only off"}
          disabled={!slot.inUse || !editable}
          onClick={() => openDialog({ kind: "rename", slotKind: kind, slot: index, name: slot.inUse ?? "" })}
        >
          <PencilSimpleIcon size={14} />
        </button>
      </span>
    </div>
  );
}

function UserSlots() {
  const d = useStore((s) => s.sections.device);
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  if (!d) return null;
  return (
    <div className="device-col device-slots">
      <h3>User slots</h3>
      <div className="slots">
        <span />
        <span className="slots-head">Amp (NAM)</span>
        <span className="slots-head">Cab (IR)</span>
        {d.userAmps.map((amp, i) => (
          <div className="slots-row" key={i}>
            <span className="slot-number">{String(i + 1).padStart(2, "0")}</span>
            <SlotCell kind="amp" index={i} slot={amp} editable={editable} />
            <SlotCell kind="ir" index={i} slot={d.userIrs[i]} editable={editable} />
          </div>
        ))}
      </div>
      <div className="extra-buttons">
        <button className="pill" onClick={() => openDialog({ kind: "import", slotKind: "amp", slot: null })}>
          <FileArrowUpIcon size={14} /> Import NAM model…
        </button>
        <button className="pill" onClick={() => openDialog({ kind: "import", slotKind: "ir", slot: null })}>
          <FileAudioIcon size={14} /> Import IR…
        </button>
      </div>
    </div>
  );
}

export default function DeviceCard() {
  return (
    <section className="card">
      <h2 className="card-title">
        <GearIcon size={15} /> Device and system settings
      </h2>
      <div className="device">
        <System />
        <GlobalEq />
        <UserSlots />
      </div>
    </section>
  );
}
