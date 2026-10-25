import { MagnifyingGlassIcon } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { send, useStore } from "../store";
import { openDialog } from "../ui";
import "./PresetList.css";

/** "5", "p5", "P05" and "05" find P05; anything else searches the names. */
function matches(label: string, name: string | null, query: string) {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const num = q.replace(/^p/, "");
  if (/^\d+$/.test(num)) return Number(num) === Number(label.slice(1));
  return (name ?? "").toLowerCase().includes(q);
}

export default function PresetList() {
  const presets = useStore((s) => s.sections.presets);
  const editable = useStore((s) => s.sections.conn?.editable ?? false);
  const canSave = useStore((s) => s.sections.conn?.canEditSound ?? false);
  const liveName = useStore((s) => s.sections.live?.name ?? "");
  const [query, setQuery] = useState("");
  const currentRef = useRef<HTMLButtonElement>(null);
  const search = useRef<HTMLInputElement>(null);

  // Ctrl+F (⌘F) searches the presets
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        search.current?.focus();
        search.current?.select();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);
  const current = presets?.current ?? null;

  // keep the active preset in view when it changes on the pedal
  useEffect(() => {
    currentRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [current]);

  const items = presets?.items.filter((p) => matches(p.label, p.name, query)) ?? [];

  return (
    <aside className="presets">
      <label className="preset-search">
        <MagnifyingGlassIcon size={15} />
        <input
          ref={search}
          placeholder="Search presets"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setQuery("");
            if (e.key === "Enter" && items[0] && editable && items[0].index !== current) {
              send({ type: "selectPreset", index: items[0].index });
            }
          }}
        />
      </label>
      <ol className="preset-items">
        {items.map((p) => {
          const active = p.index === current;
          return (
            <li key={p.index}>
              <button
                ref={active ? currentRef : undefined}
                className={`preset ${active ? "preset-active" : ""}`}
                disabled={!editable}
                onClick={() => !active && send({ type: "selectPreset", index: p.index })}
                onContextMenu={(e) => {
                  e.preventDefault();
                  if (canSave) openDialog({ kind: "save", slot: p.index, name: liveName || (p.name ?? "") });
                }}
                title={canSave ? "Right-click to save the current sound here" : undefined}
              >
                <span className="preset-label">{p.label}</span>
                <span className={`preset-name ${p.name ? "" : "preset-unknown"}`}>{p.name || "—"}</span>
                {active && presets?.modified && <span className="preset-modified" title="Edited, not saved" />}
              </button>
            </li>
          );
        })}
      </ol>
    </aside>
  );
}
