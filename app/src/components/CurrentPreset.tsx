import { FloppyDiskIcon, PencilSimpleIcon, PlugsIcon } from "@phosphor-icons/react";
import { send, useStore } from "../store";
import { openDialog } from "../ui";
import Slider from "./Slider";
import "./CurrentPreset.css";

function Empty() {
  const status = useStore((s) => s.sections.conn?.status.kind ?? "disconnected");
  const [title, body] =
    status === "connected"
      ? ["Waiting for the pedal…", "Change a preset on the pedal, or wait for the sync to finish, to see the active preset."]
      : status === "replaying"
        ? ["Replaying…", "The active preset appears at the first preset change."]
        : ["No pedal connected", "Plug in the Dual Stomp over USB and it connects by itself. Or use “Replay capture” to play back a recorded session."];
  return (
    <section className="card current current-empty">
      <span className="current-icon">
        <PlugsIcon size={36} />
      </span>
      <div>
        <h1>{title}</h1>
        <p>{body}</p>
      </div>
    </section>
  );
}

export default function CurrentPreset() {
  const presets = useStore((s) => s.sections.presets);
  const live = useStore((s) => s.sections.live);
  const canEdit = useStore((s) => s.sections.conn?.canEditSound ?? false);
  const flash = useStore((s) => s.flashes.preset ?? 0);
  const current = presets?.current ?? null;

  if (current == null || !presets) return <Empty />;

  const item = presets.items[current];
  const name = live?.known && live.name ? live.name : (item?.name ?? "");
  const saveName = live?.name || item?.name || "";

  return (
    <section className="card current">
      {flash > 0 && <span key={flash} className="glow" />}
      <div className="current-label">{item?.label}</div>
      <div className="current-main">
        <div className="current-name">
          <h1>{name || "Untitled"}</h1>
        </div>
        {live?.known && (
          <div className="current-chips">
            {presets.modified ? <span className="chip chip-modified">● Edited</span> : <span className="chip">Saved</span>}
            <span className="chip">
              {live.blocks.filter((b) => b.effect && b.on).length}/{live.blocks.filter((b) => b.effect).length} blocks on
            </span>
            {live.page != null && <span className="chip">Pedal shows {live.blocks[live.page]?.title}</span>}
          </div>
        )}
        {canEdit && (
          <div className="current-actions">
            <button
              className={`button ${presets.modified ? "button-primary" : ""}`}
              title={`Store the live sound in ${item?.label}`}
              onClick={() => send({ type: "savePreset", slot: current, name: saveName })}
            >
              <FloppyDiskIcon size={15} /> Save
            </button>
            <button className="button" title="Rename and/or store in another slot" onClick={() => openDialog({ kind: "save", slot: current, name: saveName })}>
              <PencilSimpleIcon size={15} /> Save as…
            </button>
          </div>
        )}
      </div>
      <div className="current-volume">
        <Slider label="Volume" value={live?.known ? live.volume : null} disabled={!canEdit} onChange={(v) => send({ type: "setVolume", value: v })} />
      </div>
    </section>
  );
}
