// UI-only state: which dialog is open and whether the activity log shows. Pedal state lives in
// store.ts.

import { useSyncExternalStore } from "react";
import type { SlotKind } from "./types";

export type Dialog =
  | { kind: "save"; slot: number; name: string }
  | { kind: "rename"; slotKind: SlotKind; slot: number; name: string }
  | { kind: "import"; slotKind: SlotKind; slot: number | null }
  | { kind: "replay" };

interface UiState {
  dialog: Dialog | null;
  showLog: boolean;
}

const LOG_KEY = "open-stomp.showLog";

function storedShowLog() {
  try {
    return localStorage.getItem(LOG_KEY) === "1";
  } catch {
    return false;
  }
}

let state: UiState = { dialog: null, showLog: storedShowLog() };
const listeners = new Set<() => void>();

function set(patch: Partial<UiState>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

export function useUi<T>(select: (s: UiState) => T): T {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => select(state),
  );
}

export const openDialog = (dialog: Dialog) => set({ dialog });
export const closeDialog = () => set({ dialog: null });

export function setShowLog(showLog: boolean) {
  set({ showLog });
  try {
    localStorage.setItem(LOG_KEY, showLog ? "1" : "0");
  } catch {
    // private window or blocked storage: the choice just isn't remembered
  }
}
