// The frontend's copy of the pedal state. The Rust hub emits `state` events with the sections that
// changed; this store keeps the newest of each and lets components subscribe to just what they read.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSyncExternalStore } from "react";
import type { Activity, Cmd, Patch, Sections, Snapshot } from "./types";

interface StoreState {
  sections: Sections;
  /** Bumped on every highlight, keyed by `flashKey`; components remount a glow element on change. */
  flashes: Record<string, number>;
}

let state: StoreState = { sections: {}, flashes: {} };
/** Event number that last set each section, so a late snapshot can't overwrite newer data. */
const seqs: Record<string, number> = {};
const listeners = new Set<() => void>();

function setState(next: StoreState) {
  state = next;
  // for tools/dev/webdriver.py
  if (import.meta.env.DEV) Object.assign(window, { __stomp: state });
  listeners.forEach((l) => l());
}

function merge(seq: number, incoming: Sections) {
  const sections: Sections = { ...state.sections };
  let changed = false;
  for (const [name, value] of Object.entries(incoming)) {
    if ((seqs[name] ?? -1) >= seq) continue;
    seqs[name] = seq;
    (sections as Record<string, unknown>)[name] = value;
    changed = true;
  }
  return changed ? sections : state.sections;
}

export function flashKey(module: number, slot?: number | null) {
  return slot == null ? `m${module}` : `m${module}.${slot}`;
}

function bumpFlashes(activity: Activity[], rx: number, tx: number) {
  if (activity.length === 0 && !rx && !tx) return state.flashes;
  const flashes = { ...state.flashes };
  const bump = (k: string) => (flashes[k] = (flashes[k] ?? 0) + 1);
  if (rx) bump("rx");
  if (tx) bump("tx");
  for (const a of activity) {
    if (a.kind === "preset") {
      bump("preset");
    } else {
      bump(flashKey(a.module));
      // a knob turned in the app already shows its value; only light up the pedal's changes
      if (a.slot != null && !a.host) bump(flashKey(a.module, a.slot));
    }
  }
  return flashes;
}

/** Subscribe to the hub, then fetch the current state. */
export async function connectStore() {
  await listen<Patch>("state", ({ payload }) => {
    setState({ sections: merge(payload.seq, payload.sections), flashes: bumpFlashes(payload.activity, payload.rx, payload.tx) });
  });
  const snap = await invoke<Snapshot>("snapshot");
  setState({ ...state, sections: merge(snap.seq, snap.sections) });
}

export function useStore<T>(select: (s: StoreState) => T): T {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => select(state),
  );
}

export function send(cmd: Cmd) {
  invoke("command", { cmd }).catch((e) => console.error("command failed", cmd, e));
}
