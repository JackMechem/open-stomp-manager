import { useEffect, useState } from "react";
import Chain from "./components/Chain";
import CurrentPreset from "./components/CurrentPreset";
import Detail from "./components/Detail";
import DeviceCard from "./components/DeviceCard";
import Dialogs from "./components/Dialogs";
import { DrumCard, LooperCard, TunerCard } from "./components/Extras";
import LogPanel from "./components/LogPanel";
import PresetList from "./components/PresetList";
import TopBar from "./components/TopBar";
import { send, useStore } from "./store";
import { openDialog, setShowLog, useUi } from "./ui";
import "./styles/layout.css";

/** `STOMP_DEV=log,import,import-ir,save,rename` opens those at startup (for screenshots). */
function useDevFlags() {
  const flags = useStore((s) => s.sections.catalog?.devFlags);
  useEffect(() => {
    if (!flags) return;
    const has = (f: string) => flags.includes(f);
    if (has("log")) setShowLog(true);
    if (has("import")) openDialog({ kind: "import", slotKind: "amp", slot: null });
    if (has("import-ir")) openDialog({ kind: "import", slotKind: "ir", slot: null });
    if (has("save")) openDialog({ kind: "save", slot: 0, name: "Big Muff Lead" });
    if (has("rename")) openDialog({ kind: "rename", slotKind: "amp", slot: 0, name: "SVT G10 .5" });
  }, [flags]);
}

export default function App() {
  // a block clicked in the chain stays shown; otherwise the panel follows the pedal
  const [pinned, setPinned] = useState<number | null>(null);
  const followed = useStore((s) => s.sections.live?.followed ?? 1);
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  const showLog = useUi((s) => s.showLog);
  const shown = pinned ?? followed;
  useDevFlags();

  // Ctrl+L (⌘L) shows or hides the activity log
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "l") {
        e.preventDefault();
        setShowLog(!showLog);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [showLog]);

  // like the official app, opening a block also turns the pedal's display to its page
  const pick = (m: number) => {
    setPinned(m === pinned ? null : m);
    if (editable && m !== pinned) send({ type: "showPage", module: m });
  };

  return (
    <div className={`app ${showLog ? "app-log" : ""}`}>
      <TopBar />
      <PresetList />
      <main className="main">
        <CurrentPreset />
        <Chain shown={shown} pinned={pinned} onPick={pick} />
        <Detail module={shown} pinned={pinned != null} onUnpin={() => setPinned(null)} />
        <div className="extras">
          <TunerCard />
          <LooperCard />
          <DrumCard />
        </div>
        <DeviceCard />
      </main>
      {showLog && <LogPanel />}
      <Dialogs />
    </div>
  );
}
