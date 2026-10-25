// The JSON the Rust side sends (src-tauri/src/view.rs and hub.rs). Keep the two in step.

export type ConnStatus =
  | { kind: "disconnected" }
  | { kind: "connected"; port: string; output: boolean; bluetooth: boolean }
  | { kind: "replaying"; file: string; progress: number };

export interface Conn {
  status: ConnStatus;
  ports: string[];
  sync: { done: number; total: number };
  listenOnly: boolean;
  /** Connected and not listen-only: the app may send to the pedal. */
  editable: boolean;
  /** Also knows the live preset, so effect edits are allowed. */
  canEditSound: boolean;
  firmware: string | null;
  product: string | null;
  replayPath: string | null;
}

export interface PresetItem {
  index: number;
  label: string;
  /** null until the startup sync has read it. */
  name: string | null;
}

export interface Presets {
  current: number | null;
  modified: boolean;
  items: PresetItem[];
}

export type ParamKind = "knob" | "bipolar" | "enum";

export interface Param {
  slot: number;
  name: string;
  kind: ParamKind;
  options: string[];
  value: number;
  display: string;
}

export type ModuleKey = "odfx" | "amp" | "cab" | "mod" | "delay" | "reverb" | "gate" | "eq" | "loop" | "drum";

export interface Block {
  module: number;
  key: ModuleKey;
  title: string;
  effect: boolean;
  on: boolean;
  model: number;
  summary: string;
  params: Param[];
}

export interface Live {
  known: boolean;
  name: string;
  volume: number;
  /** Module numbers in signal order. */
  chain: number[];
  /** Indexed by module number. */
  blocks: Block[];
  followed: number;
  /** The module page the pedal's display shows. */
  page: number | null;
}

export interface Extras {
  tuner: { active: boolean; detected: boolean; note: string | null; cents: number | null };
  looper: {
    /** 0 empty, 1 recording, 2 playing, 3 overdubbing, 4 stopped. */
    state: number;
    stateName: string;
    /** 0 none, 1 an overdub (undo available), 2 the base layer only. */
    layers: number;
    recLevel: number | null;
    playLevel: number | null;
    sync: boolean | null;
    /** 0 NR, else AR. */
    mode: number | null;
  };
  drum: { playing: boolean; pattern: number | null; patternName: string | null; level: number | null; bpm: number | null };
}

export interface Settings {
  /** 0–20 = −10…+10 dB. */
  inputTrim: number;
  eqBass: number;
  eqMid: number;
  eqTreble: number;
  wireless: boolean;
  loopback: boolean;
}

export interface Slot {
  /** null until the sync has read it. */
  name: string | null;
  /** The name when the slot holds an import; null when it's empty. */
  inUse: string | null;
}

export interface DeviceInfo {
  product: string | null;
  firmware: string | null;
  modelId: string | null;
  settings: Settings | null;
  userAmps: Slot[];
  userIrs: Slot[];
}

export interface LogSummary {
  generation: number;
  count: number;
  total: number;
}

export type SlotKind = "amp" | "ir";

export interface ImportState {
  job: {
    kind: SlotKind;
    slot: number;
    name: string;
    optimised: boolean;
    done: number;
    total: number;
    startedMs: number;
  } | null;
  outcome: { text: string; error: boolean } | null;
}

export interface Catalog {
  /** [style, [[pattern value, name], …]] */
  drumStyles: [string, [number, string][]][];
  devFlags: string[];
  appVersion: string;
  libVersion: string;
}

export interface Sections {
  conn?: Conn;
  presets?: Presets;
  live?: Live;
  /** Model names per module number. */
  models?: string[][];
  extras?: Extras;
  device?: DeviceInfo;
  log?: LogSummary;
  import?: ImportState;
  catalog?: Catalog;
}

export interface LogRow {
  id: number;
  kind: "event" | "mark" | "info" | "error";
  time: string;
  origin: "pedal" | "host";
  /** A module key for effects, else the category ("preset", "tuner", …). */
  category: string;
  categoryTitle: string;
  subject: string;
  value: string;
  from: string | null;
  count: number;
  plumbing: boolean;
  raw: string;
}

export interface LogFilter {
  presets: boolean;
  effects: boolean;
  looper: boolean;
  drum: boolean;
  tuner: boolean;
  device: boolean;
  sysex: boolean;
  verbose: boolean;
  pedal: boolean;
  host: boolean;
  search: string;
}

export type Activity =
  | { kind: "module"; module: number; slot: number | null; host: boolean }
  | { kind: "preset"; host: boolean };

export interface Patch {
  seq: number;
  sections: Sections;
  activity: Activity[];
  /** Messages received from / sent to the pedal since the last patch. */
  rx: number;
  tx: number;
}

export interface Snapshot {
  seq: number;
  sections: Sections;
}

/** What the frontend can ask the Rust side for (hub::Cmd). */
export type Cmd =
  | { type: "selectPreset"; index: number }
  | { type: "setHead"; module: number; model: number; on: boolean }
  | { type: "setParam"; module: number; slot: number; value: number }
  | { type: "setVolume"; value: number }
  | { type: "showPage"; module: number }
  | { type: "savePreset"; slot: number; name: string }
  | { type: "looperButton"; button: "recPlayDub" | "stop" | "undoRedo" | "clear" }
  | { type: "setLooper"; recLevel?: number; playLevel?: number; sync?: boolean; arMode?: boolean }
  | { type: "setDrum"; playing?: boolean; pattern?: number; level?: number }
  | ({ type: "setSettings" } & Partial<Settings>)
  | { type: "renameSlot"; kind: SlotKind; slot: number; name: string }
  | {
      type: "startImport";
      request: { kind: SlotKind; slot: number; path: string; optimised: boolean; normalise: boolean };
    }
  | { type: "cancelImport" }
  | { type: "clearImportOutcome" }
  | { type: "setLogFilter"; filter: LogFilter }
  | { type: "clearLog" }
  | { type: "connect"; port: string }
  | { type: "disconnect" }
  | { type: "resync" }
  | { type: "setListenOnly"; on: boolean }
  | { type: "replay"; path?: string; speed?: number }
  | { type: "stopReplay" };
