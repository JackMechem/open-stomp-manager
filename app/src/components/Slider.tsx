import { useEffect, useRef, useState } from "react";
import "./Slider.css";

interface Props {
  label: string;
  /** null while unknown (not read from the pedal yet). */
  value: number | null;
  max?: number;
  format?: (v: number) => string;
  /** Fill from the middle (EQ, trim). */
  centred?: boolean;
  disabled: boolean;
  onChange: (value: number) => void;
}

/**
 * A horizontal level slider: click or drag along the track, scroll, or use the arrow keys.
 * Like the knob, it shows its own value while in use and sends at most one change per frame.
 */
export default function Slider({ label, value, max = 100, format = String, centred = false, disabled, onChange }: Props) {
  const [local, setLocal] = useState<number | null>(null);
  const track = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const pending = useRef<number | null>(null);
  const frame = useRef(0);
  const settle = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => (cancelAnimationFrame(frame.current), clearTimeout(settle.current)), []);

  const shown = local ?? value;
  const off = disabled || value == null;

  const change = (v: number) => {
    v = Math.max(0, Math.min(max, Math.round(v)));
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
  const release = () => {
    clearTimeout(settle.current);
    settle.current = setTimeout(() => setLocal(null), 300);
  };
  const at = (clientX: number) => {
    const r = track.current!.getBoundingClientRect();
    return ((clientX - r.left) / r.width) * max;
  };

  // scrolling moves the slider instead of the page, which needs a non-passive listener
  const onWheel = useRef<(e: WheelEvent) => void>(undefined);
  onWheel.current = (e) => {
    if (off || shown == null) return;
    e.preventDefault();
    change(shown + (e.deltaY < 0 ? 1 : -1));
    release();
  };
  useEffect(() => {
    const node = track.current;
    const handler = (e: WheelEvent) => onWheel.current?.(e);
    node?.addEventListener("wheel", handler, { passive: false });
    return () => node?.removeEventListener("wheel", handler);
  }, []);

  const frac = shown == null ? 0 : shown / max;
  const fill = centred
    ? { left: `${Math.min(frac, 0.5) * 100}%`, width: `${Math.abs(frac - 0.5) * 100}%` }
    : { left: "0%", width: `${frac * 100}%` };

  return (
    <div className={`slider ${off ? "slider-off" : ""} ${local != null ? "slider-active" : ""}`}>
      <span className="slider-label">{label}</span>
      <div
        ref={track}
        className="slider-track"
        role="slider"
        tabIndex={off ? -1 : 0}
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={shown ?? undefined}
        onPointerDown={(e) => {
          if (off || e.button !== 0) return;
          e.currentTarget.setPointerCapture(e.pointerId);
          dragging.current = true;
          change(at(e.clientX));
        }}
        onPointerMove={(e) => dragging.current && change(at(e.clientX))}
        onPointerUp={() => {
          if (dragging.current) release();
          dragging.current = false;
        }}
        onKeyDown={(e) => {
          const step = { ArrowRight: 1, ArrowUp: 1, ArrowLeft: -1, ArrowDown: -1 }[e.key];
          if (off || shown == null || step == null) return;
          e.preventDefault();
          change(shown + step);
          release();
        }}
      >
        <span className="slider-fill" style={fill} />
        {shown != null && <span className="slider-thumb" style={{ left: `${frac * 100}%` }} />}
      </div>
      <span className="slider-value">{shown == null ? "—" : format(shown)}</span>
    </div>
  );
}
