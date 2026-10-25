import { ArrowsClockwiseIcon, BluetoothIcon, EarIcon, FilmStripIcon, PulseIcon, StopIcon, UsbIcon } from "@phosphor-icons/react";
import { send, useStore } from "../store";
import type { Conn } from "../types";
import { openDialog, setShowLog, useUi } from "../ui";
import "./TopBar.css";

function statusText(c: Conn): { tone: "good" | "busy" | "off"; text: string } {
  const s = c.status;
  if (s.kind === "replaying") return { tone: "busy", text: `Replaying · ${Math.round(s.progress * 100)}%` };
  if (s.kind === "disconnected") return { tone: "off", text: "Searching for pedal…" };
  if (c.sync.total > 0 && c.sync.done < c.sync.total) return { tone: "busy", text: "Reading the pedal…" };
  if (!s.output) return { tone: "busy", text: "Listening (no output)" };
  return { tone: "good", text: c.listenOnly ? "Connected · listen only" : "Connected" };
}

export default function TopBar() {
  const conn = useStore((s) => s.sections.conn);
  const showLog = useUi((s) => s.showLog);
  const rx = useStore((s) => s.flashes.rx ?? 0);
  const tx = useStore((s) => s.flashes.tx ?? 0);
  if (!conn) return <header className="topbar" />;

  const { tone, text } = statusText(conn);
  const s = conn.status;
  const syncing = conn.sync.total > 0 && conn.sync.done < conn.sync.total;
  const port = s.kind === "connected" ? s.port : "";

  return (
    <header className="topbar">
      <div className="brand">
        <span className="brand-mark" />
        <span className="brand-name">Open Stomp Manager</span>
      </div>

      <div className={`status status-${tone}`} title={s.kind === "connected" ? s.port : undefined}>
        <span className="status-dot" />
        {s.kind === "connected" && (s.bluetooth ? <BluetoothIcon size={14} /> : <UsbIcon size={14} />)}
        {text}
        {conn.firmware && s.kind === "connected" && <span className="status-sub">{conn.firmware}</span>}
        <span className="lights" title="Messages from (In) and to (Out) the pedal">
          <span key={`rx${rx}`} className={`light ${rx ? "light-in" : ""}`}>In</span>
          <span key={`tx${tx}`} className={`light ${tx ? "light-out" : ""}`}>Out</span>
        </span>
        {syncing && (
          <span className="sync-bar">
            <span style={{ width: `${(conn.sync.done / conn.sync.total) * 100}%` }} />
          </span>
        )}
      </div>

      <div className="topbar-actions">
        <select
          className="port-select"
          value={port}
          onChange={(e) => (e.target.value ? send({ type: "connect", port: e.target.value }) : send({ type: "disconnect" }))}
        >
          <option value="">Not connected</option>
          {conn.ports.map((p) => (
            <option key={p} value={p}>
              {p}
            </option>
          ))}
        </select>

        {s.kind === "connected" && (
          <button className="toggle" onClick={() => send({ type: "resync" })} title="Read everything from the pedal again">
            <ArrowsClockwiseIcon size={16} />
          </button>
        )}

        <button
          className={`toggle ${conn.listenOnly ? "toggle-on" : ""}`}
          onClick={() => send({ type: "setListenOnly", on: !conn.listenOnly })}
          title="Never send anything to the pedal"
        >
          <EarIcon size={16} /> <span className="toggle-label">Listen only</span>
        </button>

        {s.kind === "replaying" ? (
          <button className="toggle toggle-on" onClick={() => send({ type: "stopReplay" })} title="Stop the replay">
            <StopIcon size={16} weight="fill" /> <span className="toggle-label">Stop replay</span>
          </button>
        ) : (
          <button className="toggle" onClick={() => openDialog({ kind: "replay" })} title="Play back a recorded session instead of the pedal">
            <FilmStripIcon size={16} /> <span className="toggle-label">Replay capture…</span>
          </button>
        )}

        <button className={`toggle ${showLog ? "toggle-on" : ""}`} onClick={() => setShowLog(!showLog)} title="Everything happening on the pedal">
          <PulseIcon size={16} /> <span className="toggle-label">Activity</span>
        </button>
      </div>
    </header>
  );
}
