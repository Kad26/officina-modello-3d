// Command search (SketchUp's Shift+S): type a few letters of a tool or an
// action, pick it with the arrows and Enter.

import { useEffect, useMemo, useRef, useState } from "react";
import { Sheet } from "./sheet";
import { desktopShortcut } from "./shortcut-labels";
import { tr, tt } from "./i18n";

export interface Command {
  label: string;
  /** what it does, or where it lives */
  hint?: string;
  key?: string;
  run: () => void;
}

/** Every word typed appears in the label or hint (any order); earlier matches first. */
function score(c: Command, q: string): number {
  // punctuation doesn't count: "xray" finds X-Ray, "3d text" finds 3D Text…
  const plain = (t: string) => t.toLowerCase().replace(/[^\p{L}\p{N}\s]/gu, "");
  const label = plain(tr(c.label));
  const hay = plain(`${tr(c.label)} ${c.label} ${c.hint ?? ""}`);
  const words = plain(q).split(/\s+/).filter(Boolean);
  if (!words.length) return 1;
  let s = 0;
  for (const w of words) {
    const i = hay.indexOf(w);
    if (i < 0) return -1;
    s += i === 0 || hay[i - 1] === " " ? 10 : 1; // a word start counts more
  }
  return s + (label.startsWith(words[0]) ? 20 : 0);
}

export function CommandPalette({ commands, onClose }: { commands: Command[]; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const list = useMemo(
    () =>
      commands
        .map((c) => [c, score(c, q)] as const)
        .filter(([, s]) => s >= 0)
        .sort((a, b) => b[1] - a[1])
        .map(([c]) => c)
        .slice(0, 12),
    [commands, q]
  );
  useEffect(() => input.current?.focus(), []);
  useEffect(() => setAt(0), [q]);
  const pick = (c: Command | undefined) => {
    if (!c) return;
    onClose();
    c.run();
  };
  return (
    <Sheet onClose={onClose} label={tt("Search commands", "Cerca comandi")} className="palette">
      <input
        ref={input}
        placeholder={tt("Search tools and commands", "Cerca strumenti e comandi")}
        value={q}
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") (e.preventDefault(), setAt((a) => Math.min(a + 1, list.length - 1)));
          else if (e.key === "ArrowUp") (e.preventDefault(), setAt((a) => Math.max(a - 1, 0)));
          else if (e.key === "Enter") pick(list[at]);
          else if (e.key === "Escape") onClose();
        }}
        aria-activedescendant={list[at] ? `cmd-${at}` : undefined}
      />
      <ul role="listbox">
        {list.map((c, i) => (
          <li key={`${c.label}|${c.hint}`} id={`cmd-${i}`} role="option" aria-selected={i === at} className={i === at ? "on" : ""} onMouseEnter={() => setAt(i)} onClick={() => pick(c)}>
            <span>{tr(c.label)}</span>
            {c.hint && <span className="muted small">{c.hint}</span>}
            {c.key && <kbd>{desktopShortcut(c.key)}</kbd>}
          </li>
        ))}
        {!list.length && <li className="muted">{tt("Nothing matches", "Nessun risultato per")} “{q}”</li>}
      </ul>
    </Sheet>
  );
}
