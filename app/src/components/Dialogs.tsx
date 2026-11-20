import {
  CheckCircleIcon,
  FileArrowUpIcon,
  FilmStripIcon,
  FloppyDiskIcon,
  FolderOpenIcon,
  PencilSimpleIcon,
  TrayArrowDownIcon,
  TrayArrowUpIcon,
  UploadSimpleIcon,
  WarningIcon,
  XCircleIcon,
  XIcon,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { send, useStore } from "../store";
import type { ExportResult, Plan, PresetItem, SlotKind } from "../types";
import { closeDialog, useUi, type Dialog } from "../ui";
import "./Dialogs.css";

const NAME_LEN = 16;
const prefix = (k: SlotKind) => (k === "amp" ? "USER AMP" : "USER IR");
const slotLabel = (k: SlotKind, i: number) => `${prefix(k)} ${String(i + 1).padStart(2, "0")}`;
const isAscii = (s: string) => /^[\x00-\x7f]*$/.test(s);
const PRESET_EXT = "ospreset";
const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
const fileOf = (path: string) => path.split(/[\\/]/).pop() ?? path;

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


/** A file name from a preset name, without characters file systems refuse. */
const safeFileName = (s: string) => s.replace(/[\\/:*?"<>|]+/g, " ").replace(/\s+/g, " ").trim() || "preset";

function ExportDialog({ dialog }: { dialog: Extract<Dialog, { kind: "export" }> }) {
  const presets = useStore((s) => s.sections.presets);
  const device = useStore((s) => s.sections.device);
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  const reading = useStore((s) => s.sections.export ?? null);
  // null: everything
  const [which, setWhich] = useState<number | null>(dialog.preset);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ error: boolean; text: string } | null>(null);
  const items = presets?.items ?? [];
  const ready = items.length > 0 && items.every((p) => p.name != null) && device != null && [...device.userAmps, ...device.userIrs].every((s) => s.name != null);

  // the user slots that go into the file
  const rows: { kind: SlotKind; slot: number; name: string }[] = [];
  if (device) {
    const used = (kind: SlotKind, slot: number) => {
      if (which == null) return true;
      const p = items[which];
      return (kind === "amp" ? p?.userAmp : p?.userIr) === slot;
    };
    for (const kind of ["amp", "ir"] as SlotKind[]) {
      (kind === "amp" ? device.userAmps : device.userIrs).forEach((s, slot) => s.inUse && used(kind, slot) && rows.push({ kind, slot, name: s.inUse }));
    }
  }

  const doExport = async () => {
    const p = which == null ? null : items[which];
    const base = p ? `${p.label} ${p.name ?? ""}` : `Open Stomp backup ${new Date().toISOString().slice(0, 10)}`;
    const path = await save({
      title: p ? "Export preset" : "Back up the pedal",
      defaultPath: `${safeFileName(base)}.${PRESET_EXT}`,
      filters: [{ name: "Open Stomp presets", extensions: [PRESET_EXT] }],
    });
    if (!path) return;
    setBusy(true);
    setResult(null);
    try {
      const r = await invoke<ExportResult>("export_presets", { preset: which, path });
      const byName = r.missing.length ? ` ${plural(r.missing.length, "model")} went in by name only: ${r.missing.join(", ")}.` : "";
      setResult({ error: false, text: `Saved ${plural(r.presets, "preset")} and ${plural(r.models, "model")} to ${fileOf(path)}.${byName}` });
    } catch (e) {
      setResult({ error: true, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title="Export presets" icon={<TrayArrowUpIcon size={18} />} onClose={closeDialog}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (ready && !busy) doExport();
        }}
      >
        <label>
          Export
          <select
            value={which ?? -1}
            disabled={busy}
            onChange={(e) => {
              setWhich(Number(e.target.value) < 0 ? null : Number(e.target.value));
              setResult(null);
            }}
          >
            <option value={-1}>Everything: all presets, user slots and settings</option>
            {items.map((p) => (
              <option key={p.index} value={p.index}>
                {p.label} {p.name ?? "—"}
              </option>
            ))}
          </select>
        </label>
        {rows.length > 0 && (
          <div className="file-slots">
            {rows.map(({ kind, slot, name }) => (
              <div className="file-slot" key={`${kind}${slot}`}>
                <span className="file-slot-label">{slotLabel(kind, slot)}</span>
                <span className="file-slot-name" title={name}>
                  {name}
                </span>
              </div>
            ))}
          </div>
        )}
        {rows.length > 0 && editable && !busy && (
          <Message tone="dim">The models are read from the pedal and saved in the file, so they can be put back on any pedal exactly as they are.</Message>
        )}
        {rows.length > 0 && !editable && (
          <Message tone="warn">Connect to the pedal (with Listen only off) to include the models; otherwise they go in by name only.</Message>
        )}
        {busy && reading && reading.total > 0 && (
          <>
            <div className="progress">
              <span style={{ width: `${(reading.done / reading.total) * 100}%` }} />
            </div>
            <Message tone="dim">Reading the models from the pedal…</Message>
          </>
        )}
        {!ready && <Message tone="dim">Connect to the pedal and wait for “Reading the pedal…” to finish.</Message>}
        {result && <Message tone={result.error ? "bad" : "good"}>{result.text}</Message>}
        <footer>
          <button type="button" className="button" onClick={closeDialog}>
            {result && !result.error ? "Done" : "Cancel"}
          </button>
          <button type="submit" className="button button-primary" disabled={!ready || busy}>
            <TrayArrowUpIcon size={15} /> Export…
          </button>
        </footer>
      </form>
    </Modal>
  );
}

/** What a model's slot choice says about the pedal slot. */
function targetLabel(kind: SlotKind, slot: number, found: number | null, inUse: string | null | undefined) {
  if (slot === found) return `${slotLabel(kind, slot)} · already there`;
  return `${slotLabel(kind, slot)} · ${inUse ? `replaces ${inUse}` : "empty"}`;
}

function RestoreDialog({ dialog }: { dialog: Extract<Dialog, { kind: "restore" }> }) {
  const pedalPresets = useStore((s) => s.sections.presets);
  const device = useStore((s) => s.sections.device);
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  const restore = useStore((s) => s.sections.restore);
  const job = restore?.job ?? null;
  const elapsed = useElapsed(job?.startedMs);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<boolean[]>([]);
  const [to, setTo] = useState(0);
  const [targets, setTargets] = useState<(number | null)[]>([]);
  const [settings, setSettings] = useState(false);

  const load = async (picked: string) => {
    send({ type: "clearRestoreOutcome" });
    try {
      const p = await invoke<Plan>("read_preset_file", { path: picked });
      setPlan(p);
      setError(null);
      setSelected(p.presets.map(() => true));
      setTo(p.presets[0]?.index ?? 0);
      setTargets(p.models.map((m) => m.target));
      setSettings(false);
    } catch (e) {
      setPlan(null);
      setError(String(e));
    }
  };
  const choose = async () => {
    const picked = await open({ title: "Import presets", multiple: false, directory: false, filters: [{ name: "Open Stomp presets", extensions: [PRESET_EXT, "json"] }] });
    if (typeof picked === "string") load(picked);
  };
  useEffect(() => {
    if (dialog.path) load(dialog.path);
  }, [dialog.path]);

  if (job) {
    const writing = job.stage === "writing";
    // a NAM is measured before its first optimiser step
    const measuring = !writing && job.done === 0;
    return (
      <Modal title="Import presets" icon={<TrayArrowDownIcon size={18} />} onClose={closeDialog}>
        <div className="form">
          <p>{job.text}</p>
          <div className={`progress ${measuring ? "progress-busy" : ""}`}>
            <span style={measuring ? undefined : { width: `${(job.done / job.total) * 100}%` }} />
          </div>
          <Message tone="dim">
            {writing ? `${job.done} of ${job.total} writes confirmed` : measuring ? "Measuring the model…" : `Step ${job.done} of ${job.total}`} · {elapsed}
          </Message>
          {!writing && <Message tone="dim">Models without a converted copy are converted first, about a minute each. Nothing is written until they're done.</Message>}
          {writing && <Message tone="dim">Keep the pedal connected. Afterwards the app reads everything back from the pedal.</Message>}
          <footer>
            {writing ? (
              <button className="button" onClick={closeDialog}>
                Hide
              </button>
            ) : (
              <button className="button" onClick={() => send({ type: "cancelRestore" })}>
                Cancel import
              </button>
            )}
          </footer>
        </div>
      </Modal>
    );
  }

  const single = plan?.presets.length === 1;
  const chosen = plan ? plan.presets.map((p, entry) => ({ entry, to: single ? to : p.index })).filter((_, i) => single || selected[i]) : [];
  const models = plan?.models ?? [];
  const pedalSlot = (kind: SlotKind, slot: number) => (kind === "amp" ? device?.userAmps : device?.userIrs)?.[slot];
  // models that would go into the same slot
  const clash = models.find((m, i) => targets[i] != null && models.some((n, j) => j < i && n.kind === m.kind && targets[j] === targets[i]));
  // chosen presets that use a model that stays behind
  const orphans = chosen.filter((c) => plan?.presets[c.entry].models.some((m) => targets[m] == null)).length;
  const replaced = models.flatMap((m, i) => {
    const t = targets[i];
    const there = t == null ? null : pedalSlot(m.kind, t)?.inUse;
    return t != null && t !== m.found && there ? [`“${there}” in ${slotLabel(m.kind, t)}`] : [];
  });
  const converting = models.filter((m, i) => targets[i] != null && targets[i] !== m.found && !m.converted).length;
  const pedalRead = (pedalPresets?.items ?? []).every((p) => p.name != null) && device != null;
  const nothing = chosen.length === 0 && targets.every((t, i) => t == null || t === models[i]?.found) && !settings;
  const ok = plan != null && editable && pedalRead && !clash && !nothing;
  const target: PresetItem | undefined = pedalPresets?.items[to];

  const start = () => {
    if (!plan || !ok) return;
    send({ type: "startRestore", request: { path: plan.path, presets: chosen, models: targets, settings } });
  };

  return (
    <Modal title="Import presets" icon={<TrayArrowDownIcon size={18} />} onClose={closeDialog}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          start();
        }}
      >
        <label>
          File
          <span className="file">
            <button type="button" className="button" onClick={choose}>
              <FolderOpenIcon size={15} /> Choose file…
            </button>
            <span className={plan ? "" : "faint"} title={plan?.path}>
              {plan ? fileOf(plan.path) : "no file chosen"}
            </span>
          </span>
        </label>
        {error && <Message tone="bad">{error}</Message>}
        {plan && (
          <Message tone="dim">
            {plan.createdBy}
            {plan.created && `, ${new Date(plan.created).toLocaleString()}`}
          </Message>
        )}

        {plan && single && (
          <label>
            Save to
            <select value={to} onChange={(e) => setTo(Number(e.target.value))}>
              {pedalPresets?.items.map((p) => (
                <option key={p.index} value={p.index}>
                  {p.label} {p.name ?? "—"}
                </option>
              ))}
            </select>
          </label>
        )}
        {plan && single && target && (
          <Message tone="warn">
            “{plan.presets[0].name}” replaces {target.label} “{target.name ?? "?"}”.
          </Message>
        )}

        {plan && !single && plan.presets.length > 0 && (
          <div className="file-section">
            <div className="file-section-head">
              <span>
                Presets · {chosen.length} of {plan.presets.length}
              </span>
              <button type="button" className="pill" onClick={() => setSelected(plan.presets.map(() => true))}>
                All
              </button>
              <button type="button" className="pill" onClick={() => setSelected(plan.presets.map(() => false))}>
                None
              </button>
            </div>
            <div className="file-presets">
              {plan.presets.map((p, i) => (
                <label key={i} className="radio">
                  <input type="checkbox" checked={selected[i] ?? false} onChange={(e) => setSelected(selected.map((v, j) => (j === i ? e.target.checked : v)))} />
                  <span className="preset-label">P{String(p.index + 1).padStart(2, "0")}</span>
                  {p.name || "—"}
                </label>
              ))}
            </div>
            <Message tone="dim">Each preset goes back to its own number, replacing what's there now.</Message>
          </div>
        )}

        {models.length > 0 && (
          <div className="file-section">
            <div className="file-section-head">
              <span>Models</span>
            </div>
            {models.map((m, i) => {
              const usable = m.hasData || m.found != null;
              return (
                <div className="file-model" key={i}>
                  <span className="file-slot-label">{m.kind === "amp" ? "NAM" : "IR"}</span>
                  <span className="file-slot-name" title={m.name}>
                    {m.name}
                  </span>
                  <select
                    value={targets[i] ?? -1}
                    disabled={!usable}
                    onChange={(e) => setTargets(targets.map((t, j) => (j === i ? (Number(e.target.value) < 0 ? null : Number(e.target.value)) : t)))}
                  >
                    {Array.from({ length: 10 }, (_, k) => k)
                      .filter((k) => m.hasData || k === m.found)
                      .map((k) => (
                        <option key={k} value={k}>
                          {targetLabel(m.kind, k, m.found, pedalSlot(m.kind, k)?.inUse)}
                        </option>
                      ))}
                    <option value={-1}>{usable ? "Don't import" : "Not available: no copy in the file"}</option>
                  </select>
                </div>
              );
            })}
            <Message tone="dim">Models the pedal already has (same name) stay where they are. Pick a slot for the others; the presets follow them.</Message>
          </div>
        )}

        {plan?.hasSettings && (
          <label className="radio">
            <input type="checkbox" checked={settings} onChange={(e) => setSettings(e.target.checked)} />
            Also restore the system settings (input trim, global EQ, wireless, loopback)
          </label>
        )}

        {clash && <Message tone="bad">Two models can't both go into {slotLabel(clash.kind, targets[models.indexOf(clash)] ?? 0)}.</Message>}
        {replaced.length > 0 && <Message tone="warn">This replaces {replaced.join(", ")}. Presets on the pedal that use {replaced.length === 1 ? "it" : "them"} will play the new model.</Message>}
        {orphans > 0 && (
          <Message tone="warn">
            {plural(orphans, "preset")} use{orphans === 1 ? "s" : ""} a model that won't be imported, and will play whatever is in that slot.
          </Message>
        )}
        {converting > 0 && <Message tone="dim">{plural(converting, "model")} will be converted first (about a minute each for NAM).</Message>}
        {plan && !editable && <Message tone="dim">Connect to the pedal (with Listen only off) to import.</Message>}
        {plan && editable && !pedalRead && <Message tone="dim">Wait for “Reading the pedal…” to finish.</Message>}
        {restore?.outcome && <Message tone={restore.outcome.error ? "bad" : "good"}>{restore.outcome.text}</Message>}
        <footer>
          <button type="button" className="button" onClick={closeDialog}>
            Close
          </button>
          <button type="submit" className="button button-primary" disabled={!ok}>
            <TrayArrowDownIcon size={15} /> Import
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
    case "export":
      return <ExportDialog key="export" dialog={dialog} />;
    case "restore":
      return <RestoreDialog key="restore" dialog={dialog} />;
    case "replay":
      return <ReplayDialog key="replay" />;
  }
}
