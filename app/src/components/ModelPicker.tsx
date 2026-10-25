import { CaretDownIcon } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import "./ModelPicker.css";

interface Props {
  models: string[];
  value: number;
  disabled: boolean;
  onPick: (model: number) => void;
}

/** The model name as a button that opens the list of models. */
export default function ModelPicker({ models, value, disabled, onPick }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const selected = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    selected.current?.scrollIntoView({ block: "center" });
    const close = (e: MouseEvent) => !root.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  return (
    <div className="picker" ref={root}>
      <button className="picker-button" disabled={disabled} onClick={() => setOpen(!open)}>
        {models[value] ?? `Model ${value + 1}`}
        {!disabled && <CaretDownIcon size={14} weight="bold" />}
      </button>
      {open && (
        <ul className="picker-list" role="listbox">
          {models.map((name, i) => (
            <li key={i}>
              <button
                ref={i === value ? selected : undefined}
                role="option"
                aria-selected={i === value}
                className={i === value ? "picker-on" : ""}
                onClick={() => {
                  setOpen(false);
                  if (i !== value) onPick(i);
                }}
              >
                <span className="picker-number">{String(i + 1).padStart(2, "0")}</span>
                {name}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
