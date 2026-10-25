import { useEffect, useRef, useState } from "react";
import "./Knob.css";

interface Props {
  label: string;
  /** 0–100. */
  value: number;
  /** The value as the pedal shows it; used unless the knob is being turned here. */
  display: string;
  /** 50 is the centre (EQ bands). */
  bipolar: boolean;
  disabled: boolean;
  /** Bumped when the pedal changed this value, to play the highlight. */
  flash: number;
  onChange: (value: number) => void;
}

const SWEEP = 270; // degrees of travel
const START = -135; // angle of 0, clockwise from 12 o'clock
const R = 26;
const C = 32; // centre of the 64×64 view box

function polar(deg: number) {
  const a = ((deg - 90) * Math.PI) / 180;
  return [C + R * Math.cos(a), C + R * Math.sin(a)];
}

function arc(from: number, to: number) {
  const [a0, a1] = from <= to ? [from, to] : [to, from];
  const [x0, y0] = polar(a0);
  const [x1, y1] = polar(a1);
  return `M ${x0} ${y0} A ${R} ${R} 0 ${a1 - a0 > 180 ? 1 : 0} 1 ${x1} ${y1}`;
}

const clamp = (v: number) => Math.max(0, Math.min(100, Math.round(v)));

/**
 * A rotary knob. Drag up/down (Shift = fine), scroll, arrow keys, or double-click for the centre.
 * While turning it shows its own value, and it sends at most one change per frame.
 */
export default function Knob({ label, value, display, bipolar, disabled, flash, onChange }: Props) {
  // the value under the user's hand; null when the pedal's value is shown
  const [local, setLocal] = useState<number | null>(null);
  const drag = useRef<{ y: number; v: number } | null>(null);
  const pending = useRef<number | null>(null);
  const frame = useRef(0);
  const settle = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => (cancelAnimationFrame(frame.current), clearTimeout(settle.current)), []);

  const shown = local ?? value;

  const change = (v: number) => {
    v = clamp(v);
    setLocal(v);
    clearTimeout(settle.current);
    pending.current = v;
    if (!frame.current) {
      frame.current = requestAnimationFrame(() => {
        frame.current = 0;
        if (pending.current != null) onChange(pending.current);
        pending.current = null;
      });
    }
  };

  // give the pedal's echo a moment to arrive before showing its value again
  const release = () => {
    clearTimeout(settle.current);
    settle.current = setTimeout(() => setLocal(null), 300);
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (disabled || e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { y: e.clientY, v: shown };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const perPixel = e.shiftKey ? 0.1 : 0.5;
    const v = d.v + (d.y - e.clientY) * perPixel;
    if (clamp(v) !== shown) change(v);
  };
  const onPointerUp = () => {
    if (drag.current) release();
    drag.current = null;
  };
  // scrolling turns the knob instead of the page, which needs a non-passive listener
  const el = useRef<HTMLDivElement>(null);
  const onWheel = useRef<(e: WheelEvent) => void>(undefined);
  onWheel.current = (e) => {
    if (disabled) return;
    e.preventDefault();
    change(shown + (e.deltaY < 0 ? 1 : -1) * (e.shiftKey ? 1 : 2));
    release();
  };
  useEffect(() => {
    const node = el.current;
    const handler = (e: WheelEvent) => onWheel.current?.(e);
    node?.addEventListener("wheel", handler, { passive: false });
    return () => node?.removeEventListener("wheel", handler);
  }, []);
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) return;
    const step = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1, PageUp: 10, PageDown: -10 }[e.key];
    if (step != null) {
      e.preventDefault();
      change(shown + step);
      release();
    }
  };

  const angle = START + (shown / 100) * SWEEP;
  const origin = bipolar ? 0 : START;
  const text = local == null ? display : bipolar ? (local - 50 > 0 ? `+${local - 50}` : `${local - 50}`) : `${local}`;

  return (
    <div
      ref={el}
      className={`knob ${disabled ? "knob-disabled" : ""} ${local != null ? "knob-active" : ""}`}
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={shown}
      aria-valuetext={text}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onKeyDown={onKeyDown}
      onDoubleClick={() => !disabled && (change(50), release())}
    >
      <div className="knob-dial">
        {flash > 0 && <span key={flash} className="glow" />}
        <svg viewBox="0 0 64 64">
          <path className="knob-track" d={arc(START, START + SWEEP)} />
          <path className="knob-value" d={arc(origin, angle)} />
          <circle className="knob-cap" cx={C} cy={C} r={18} />
          <line className="knob-pointer" x1={C} y1={C} x2={C} y2={C - 14} transform={`rotate(${angle} ${C} ${C})`} />
        </svg>
      </div>
      <div className="knob-text">{text}</div>
      <div className="knob-label">{label}</div>
    </div>
  );
}
