import {
  CheckCircleIcon,
  FileArrowUpIcon,
  FilmStripIcon,
  FloppyDiskIcon,
  FolderOpenIcon,
  PencilSimpleIcon,
  UploadSimpleIcon,
  WarningIcon,
  XCircleIcon,
  XIcon,
} from "@phosphor-icons/react";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { send, useStore } from "../store";
import type { SlotKind } from "../types";
import { closeDialog, useUi, type Dialog } from "../ui";
import "./Dialogs.css";

const NAME_LEN = 16;
const prefix = (k: SlotKind) => (k === "amp" ? "USER AMP" : "USER IR");
const slotLabel = (k: SlotKind, i: number) => `${prefix(k)} ${String(i + 1).padStart(2, "0")}`;
const isAscii = (s: string) => /^[\x00-\x7f]*$/.test(s);

function Modal({ title, icon, onClose, children }: { title: string; icon: React.ReactNode; onClose: () => void; children: React.ReactNode }) {
  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", esc);
    return () => document.removeEventListener("keydown", esc);
  }, [onClose]);
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={title}>
        <header className="modal-head">
          {icon}
          <h2>{title}</h2>
          <button className="modal-close" onClick={onClose} title="Close">
            <XIcon size={16} />
          </button>
        </header>
        {children}
      </div>
    </div>
  );
}

function Message({ tone, children }: { tone: "warn" | "bad" | "good" | "dim"; children: React.ReactNode }) {
  const Icon = { warn: WarningIcon, bad: XCircleIcon, good: CheckCircleIcon, dim: null }[tone];
  return (
    <p className={`message message-${tone}`}>
      {Icon && <Icon size={16} weight="bold" />}
      {children}
    </p>
  );
}

function SaveDialog({ dialog }: { dialog: Extract<Dialog, { kind: "save" }> }) {
  const presets = useStore((s) => s.sections.presets);
  const canSave = useStore((s) => s.sections.conn?.canEditSound ?? false);
  const [name, setName] = useState(dialog.name);
  const [slot, setSlot] = useState(dialog.slot);
  const current = presets?.current ?? null;
  const target = presets?.items[slot];
  const save = () => {
    if (!canSave) return;
    send({ type: "savePreset", slot, name });
    closeDialog();
  };
  return (
    <Modal title="Save preset" icon={<FloppyDiskIcon size={18} />} onClose={closeDialog}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <label>
          Name
          <input autoFocus maxLength={NAME_LEN} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <label>
          Slot
          <select value={slot} onChange={(e) => setSlot(Number(e.target.value))}>
            {presets?.items.map((p) => (
              <option key={p.index} value={p.index}>
                {p.label} {p.name ?? "—"}
              </option>
            ))}
          </select>
        </label>
        {!isAscii(name) && <Message tone="bad">Only ASCII characters can be stored; others become “?”.</Message>}
        {slot !== current && target && (
          <Message tone="warn">
            This overwrites {target.label} “{target.name ?? "?"}”.
          </Message>
        )}
        {!canSave && <Message tone="dim">Connect to the pedal, with Listen only off, to save.</Message>}
        <footer>
          <button type="button" className="button" onClick={closeDialog}>
            Cancel
          </button>
          <button type="submit" className="button button-primary" disabled={!canSave}>
            <FloppyDiskIcon size={15} /> Save
          </button>
        </footer>
      </form>
    </Modal>
  );
}

function RenameDialog({ dialog }: { dialog: Extract<Dialog, { kind: "rename" }> }) {
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  const [name, setName] = useState(dialog.name);
  const ok = editable && name.trim() !== "";
  return (
    <Modal title={`Rename ${slotLabel(dialog.slotKind, dialog.slot)}`} icon={<PencilSimpleIcon size={18} />} onClose={closeDialog}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!ok) return;
          send({ type: "renameSlot", kind: dialog.slotKind, slot: dialog.slot, name });
          closeDialog();
        }}
      >
        <label>
          Name
          <input autoFocus maxLength={NAME_LEN} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <Message tone="dim">Only the name changes; the slot's sound stays as it is. Up to 16 characters.</Message>
        {!isAscii(name) && <Message tone="bad">Only ASCII characters can be stored; others become “?”.</Message>}
        {!editable && <Message tone="dim">Connect to the pedal, with Listen only off, to rename.</Message>}
        <footer>
          <button type="button" className="button" onClick={closeDialog}>
            Cancel
          </button>
          <button type="submit" className="button button-primary" disabled={!ok}>
            <PencilSimpleIcon size={15} /> Rename
          </button>
        </footer>
      </form>
    </Modal>
  );
}

function useElapsed(startedMs: number | undefined) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (startedMs == null) return;
    const t = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(t);
  }, [startedMs]);
  if (startedMs == null) return "";
  const s = Math.max(0, Math.floor((now - startedMs) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

function ImportDialog({ dialog }: { dialog: Extract<Dialog, { kind: "import" }> }) {
  const kind = dialog.slotKind;
  const slots = useStore((s) => (kind === "amp" ? s.sections.device?.userAmps : s.sections.device?.userIrs) ?? []);
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  const imp = useStore((s) => s.sections.import);
  const job = imp?.job ?? null;
  // the requested slot, else the first empty one
  const [slot, setSlot] = useState(() => dialog.slot ?? Math.max(0, slots.findIndex((s) => s.name != null && !s.inUse)));
  const [path, setPath] = useState<string | null>(null);
  const [optimised, setOptimised] = useState(true);
  const [normalise, setNormalise] = useState(true);
  const elapsed = useElapsed(job?.startedMs);

  const choose = async () => {
    const picked = await open({
      title: kind === "amp" ? "Import NAM model" : "Import IR",
      multiple: false,
      directory: false,
      filters: [kind === "amp" ? { name: "NAM model", extensions: ["nam"] } : { name: "Impulse response (WAV)", extensions: ["wav", "wave"] }],
    });
    if (typeof picked === "string") {
      setPath(picked);
      send({ type: "clearImportOutcome" });
    }
  };

  const title = kind === "amp" ? "Import NAM model" : "Import IR";
  const fileName = path?.split(/[\\/]/).pop();
  const replacing = slots[slot]?.inUse;

  if (job) {
    const fitting = job.kind === "amp" && job.optimised && job.done > 0;
    const text =
      job.kind === "ir"
        ? "Converting the IR…"
        : !job.optimised
          ? "Measuring the model…"
          : job.done === 0
            ? "Measuring the model and rendering the test signal…"
            : `Fitting the pedal to the model: step ${job.done} of ${job.total}`;
    return (
      <Modal title={title} icon={<FileArrowUpIcon size={18} />} onClose={closeDialog}>
        <div className="form">
          <p>
            Converting “{job.name}” for {slotLabel(job.kind, job.slot)}
          </p>
          <div className={`progress ${fitting ? "" : "progress-busy"}`}>
            <span style={fitting ? { width: `${(job.done / job.total) * 100}%` } : undefined} />
          </div>
          <Message tone="dim">
            {text} · {elapsed}
          </Message>
          {job.kind === "amp" && job.optimised && <Message tone="dim">About a minute in total. You can close this; it keeps going.</Message>}
          <footer>
            <button className="button" onClick={() => send({ type: "cancelImport" })}>
              Cancel import
            </button>
          </footer>
        </div>
      </Modal>
    );
  }

  return (
    <Modal title={title} icon={<FileArrowUpIcon size={18} />} onClose={closeDialog}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (path && editable) send({ type: "startImport", request: { kind, slot, path, optimised, normalise } });
        }}
      >
        <label>
          {kind === "amp" ? "Model" : "IR file"}
          <span className="file">
            <button type="button" className="button" onClick={choose}>
              <FolderOpenIcon size={15} /> Choose file…
            </button>
            <span className={fileName ? "" : "faint"} title={path ?? undefined}>
              {fileName ?? "no file chosen"}
            </span>
          </span>
        </label>
        <label>
          Slot
          <select value={slot} onChange={(e) => setSlot(Number(e.target.value))}>
            {Array.from({ length: 10 }, (_, i) => (
              <option key={i} value={i}>
                {slotLabel(kind, i)} · {slots[i]?.inUse ?? (slots[i]?.name == null ? "—" : "empty")}
              </option>
            ))}
          </select>
        </label>
        <fieldset>
          <legend>Conversion</legend>
          <label className="radio">
            <input type="radio" checked={optimised} onChange={() => setOptimised(true)} />
            {kind === "amp" ? "Optimised: closest to the model (about a minute)" : "Optimised: fitted to the 8 ms the pedal plays"}
          </label>
          <label className="radio">
            <input type="radio" checked={!optimised} onChange={() => setOptimised(false)} />
            {kind === "amp" ? "Official app's algorithm (a few seconds)" : "Official app's algorithm (the IR's first 42.7 ms)"}
          </label>
        </fieldset>
        {kind === "amp" && (
          <label className="radio" title="Every model plays at the same volume, whatever level it was captured at. Only the volume changes, not the tone.">
            <input type="checkbox" checked={normalise} onChange={(e) => setNormalise(e.target.checked)} />
            Normalise the level, like NAM plugins
          </label>
        )}
        {kind === "ir" && <Message tone="dim">Any WAV IR (first channel, resampled to 48 kHz).</Message>}
        {replacing && (
          <Message tone="warn">
            This replaces “{replacing}” in {slotLabel(kind, slot)}.
          </Message>
        )}
        {!editable && <Message tone="dim">Connect to the pedal (with Listen only off) to import.</Message>}
        {imp?.outcome && <Message tone={imp.outcome.error ? "bad" : "good"}>{imp.outcome.text}</Message>}
        <footer>
          <button type="button" className="button" onClick={closeDialog}>
            Close
          </button>
          <button type="submit" className="button button-primary" disabled={!path || !editable}>
            <UploadSimpleIcon size={15} /> Import
          </button>
        </footer>
      </form>
    </Modal>
  );
}

const SPEEDS = [1, 2, 4, 8, 16];

function ReplayDialog() {
  const defaultPath = useStore((s) => s.sections.conn?.replayPath ?? null);
  const [path, setPath] = useState<string | null>(defaultPath);
  const [speed, setSpeed] = useState(4);
  const choose = async () => {
    const picked = await open({ title: "Replay capture", multiple: false, directory: false, filters: [{ name: "Capture log", extensions: ["log", "txt"] }] });
    if (typeof picked === "string") setPath(picked);
  };
  return (
    <Modal title="Replay capture" icon={<FilmStripIcon size={18} />} onClose={closeDialog}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!path) return;
          send({ type: "replay", path, speed });
          closeDialog();
        }}
      >
        <Message tone="dim">Plays back a recorded session through the engine instead of the pedal, as if it were happening now.</Message>
        <label>
          Capture
          <span className="file">
            <button type="button" className="button" onClick={choose}>
              <FolderOpenIcon size={15} /> Choose file…
            </button>
            <span className={path ? "" : "faint"} title={path ?? undefined}>
              {path?.split(/[\\/]/).pop() ?? "no file chosen"}
            </span>
          </span>
        </label>
        <label>
          Speed
          <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))}>
            {SPEEDS.map((v) => (
              <option key={v} value={v}>
                {v}×
              </option>
            ))}
          </select>
        </label>
        <footer>
          <button type="button" className="button" onClick={closeDialog}>
            Cancel
          </button>
          <button type="submit" className="button button-primary" disabled={!path}>
            <FilmStripIcon size={15} /> Replay
          </button>
        </footer>
      </form>
    </Modal>
  );
}

/** Whichever dialog is open (see ui.ts). */
export default function Dialogs() {
  const dialog = useUi((s) => s.dialog);
  if (!dialog) return null;
  // keyed so reopening starts fresh
  switch (dialog.kind) {
    case "save":
      return <SaveDialog key="save" dialog={dialog} />;
    case "rename":
      return <RenameDialog key={`rename-${dialog.slotKind}-${dialog.slot}`} dialog={dialog} />;
    case "import":
      return <ImportDialog key={`import-${dialog.slotKind}-${dialog.slot}`} dialog={dialog} />;
    case "replay":
      return <ReplayDialog key="replay" />;
  }
}
