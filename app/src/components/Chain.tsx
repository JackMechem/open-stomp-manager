import { FlowArrowIcon, PowerIcon } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { flashKey, send, useStore } from "../store";
import type { Block } from "../types";
import "./Chain.css";

interface Props {
  shown: number;
  pinned: number | null;
  onPick: (module: number) => void;
}

function ChainBlock({ block, selected, canToggle, onPick }: { block: Block; selected: boolean; canToggle: boolean; onPick: () => void }) {
  const flash = useStore((s) => s.flashes[flashKey(block.module)] ?? 0);
  const toggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    send({ type: "setHead", module: block.module, model: block.model, on: !block.on });
  };
  return (
    <div
      role="button"
      tabIndex={0}
      data-module={block.key}
      className={`block ${block.on ? "block-on" : ""} ${selected ? "block-selected" : ""}`}
      onClick={onPick}
      onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onPick()}
    >
      {flash > 0 && <span key={flash} className="glow" />}
      {block.effect && canToggle ? (
        <button className="block-led block-led-button" onClick={toggle} title={block.on ? "Bypass" : "Switch on"}>
          <PowerIcon size={11} weight="bold" />
        </button>
      ) : (
        <span className="block-led" />
      )}
      <span className="block-title">{block.title}</span>
      <span className="block-summary">{block.summary}</span>
    </div>
  );
}

const BLOCK_MIN = 96; // keep in step with Chain.css
const GAP = 14;

/** How many blocks fit on one row of the chain grid, so wires don't cross a line break. */
function useColumns(ready: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  const [cols, setCols] = useState(99);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setCols(Math.max(1, Math.floor((el.clientWidth + GAP) / (BLOCK_MIN + GAP)))));
    ro.observe(el);
    return () => ro.disconnect();
  }, [ready]);
  return [ref, cols] as const;
}

export default function Chain({ shown, pinned, onPick }: Props) {
  const live = useStore((s) => s.sections.live);
  const canToggle = useStore((s) => s.sections.conn?.canEditSound ?? false);
  const [ref, cols] = useColumns(live != null);
  if (!live) return null;
  // the looper and drum sit after the chain, not in it
  const wired = (i: number) => i % cols !== 0 && i < live.chain.length - 2;

  return (
    <section className="card">
      <h2 className="card-title">
        <FlowArrowIcon size={15} /> Signal chain
        {pinned != null && <span className="chip">Pinned · click again to follow the pedal</span>}
      </h2>
      <div className="chain" ref={ref}>
        {live.chain.map((m, i) => (
          <div key={m} className={`chain-slot ${wired(i) ? "chain-wired" : ""}`}>
            <ChainBlock block={live.blocks[m]} selected={m === shown} canToggle={canToggle} onPick={() => onPick(m)} />
          </div>
        ))}
      </div>
    </section>
  );
}
