import { PowerIcon, PushPinSlashIcon } from "@phosphor-icons/react";
import { flashKey, send, useStore } from "../store";
import type { Block, Param } from "../types";
import { DrumBody, LooperBody } from "./Extras";
import Knob from "./Knob";
import ModelPicker from "./ModelPicker";
import "./Detail.css";

function ParamControl({ block, param, editable }: { block: Block; param: Param; editable: boolean }) {
  const flash = useStore((s) => s.flashes[flashKey(block.module, param.slot)] ?? 0);
  const set = (value: number) => send({ type: "setParam", module: block.module, slot: param.slot, value });

  if (param.kind === "enum") {
    return (
      <div className="choice">
        <div className="choice-options" role="radiogroup" aria-label={param.name}>
          {flash > 0 && <span key={flash} className="glow" />}
          {param.options.map((o, i) => (
            <button
              key={o}
              role="radio"
              aria-checked={i === param.value}
              className={i === param.value ? "choice-on" : ""}
              disabled={!editable}
              onClick={() => i !== param.value && set(i)}
            >
              {o}
            </button>
          ))}
        </div>
        <div className="knob-label">{param.name}</div>
      </div>
    );
  }
  return (
    <Knob
      label={param.name}
      value={param.value}
      display={param.display}
      bipolar={param.kind === "bipolar"}
      disabled={!editable}
      flash={flash}
      onChange={set}
    />
  );
}

interface Props {
  module: number;
  pinned: boolean;
  onUnpin: () => void;
}

export default function Detail({ module, pinned, onUnpin }: Props) {
  const block = useStore((s) => s.sections.live?.blocks[module]);
  const models = useStore((s) => s.sections.models?.[module]);
  const editable = useStore((s) => s.sections.conn?.canEditSound ?? false);
  if (!block) return null;

  if (!block.effect) {
    return (
      <section className="card detail" data-module={block.key}>
        <header className="detail-head">
          <span className="detail-module">{block.title}</span>
          {pinned && (
            <button className="unpin" onClick={onUnpin} title="Follow the pedal again">
              <PushPinSlashIcon size={14} /> Follow pedal
            </button>
          )}
        </header>
        {block.key === "loop" ? <LooperBody /> : <DrumBody patterns />}
      </section>
    );
  }

  const setHead = (model: number, on: boolean) => send({ type: "setHead", module: block.module, model, on });

  return (
    <section className="card detail" data-module={block.key}>
      <header className="detail-head">
        <span className="detail-module">{block.title}</span>
        <span className="detail-number">#{String(block.model + 1).padStart(2, "0")}</span>
        {models && models.length > 1 ? (
          <ModelPicker models={models} value={block.model} disabled={!editable} onPick={(m) => setHead(m, block.on)} />
        ) : (
          <span className="detail-model">{block.summary}</span>
        )}
        <button
          className={`power ${block.on ? "power-on" : ""}`}
          disabled={!editable}
          onClick={() => setHead(block.model, !block.on)}
        >
          <PowerIcon size={14} weight="bold" /> {block.on ? "On" : "Bypassed"}
        </button>
        {pinned && (
          <button className="unpin" onClick={onUnpin} title="Follow the pedal again">
            <PushPinSlashIcon size={14} /> Follow pedal
          </button>
        )}
      </header>
      {block.params.length === 0 ? (
        <p className="detail-empty">No parameters</p>
      ) : (
        <div className="params">
          {block.params.map((p) => (
            <ParamControl key={p.slot} block={block} param={p} editable={editable} />
          ))}
        </div>
      )}
    </section>
  );
}
