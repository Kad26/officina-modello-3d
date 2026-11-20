// The keyboard shortcuts sheet.
import { tr, tt } from "./i18n";
import { Sheet } from "./sheet";
import { copyModifier, desktopShortcut, isWindows, optionModifier, redoShortcut, sketchUpWindowsKeys } from "./shortcut-labels";

import { TOOL_INFO } from "./workspace";

export { desktopShortcut } from "./shortcut-labels";

type ShortcutSection = [title: string, rows: [keys: string, action: string][]];

function sections(): ShortcutSection[] {
  const copy = copyModifier();
  const option = optionModifier();
  return [
  [
    "Tools",
    [
      ...([
        ["select", "Select"], ["lasso", "Lasso Select"], ["line", "Line"], ["arc", "2-Point Arc"],
        ["rect", "Rectangle"], ["circle", "Circle"], ["polygon", "Polygon"], ["push", "Push/Pull"],
        ["offset", "Offset"], ["followme", "Follow Me"], ["move", "Move"], ["rotate", "Rotate"],
        ["scale", "Scale"], ["tape", "Tape Measure"], ["dimension", "Dimension"], ["text", "Text"],
        ["paint", "Paint Bucket"], ["erase", "Eraser"], ["section", "Section Plane"],
      ] satisfies [keyof typeof TOOL_INFO, string][]).flatMap(([id, title]): [string, string][] =>
        TOOL_INFO[id].key ? [[TOOL_INFO[id].key, title]] : [],
      ),
    ],
  ],
  [
    "Camera",
    [
      ["O", "Orbit (or middle-drag)"],
      ["H", "Pan (or Shift + middle-drag)"],
      ["Z", "Zoom"],
      ["⇧Z", "Zoom Extents"],
      ...(sketchUpWindowsKeys ? [] : [["⌘1 – ⌘6", "Iso, Top, Front, Right, Back, Left"] satisfies [string, string]]),
      ["K", "Back Edges"],
      [TOOL_INFO.igloo.key, "Image Igloo"],
    ],
  ],
  [
    "Edit",
    [
      ["⌘Z", "Undo"],
      [redoShortcut, "Redo"],
      ["⌘X  ⌘C  ⌘V", "Cut, Copy, Paste"],
      ["⌘A", "Select All"],
      ["⌘T", "Select None"],
      ["⌫", "Delete"],
      ["G", "Make Component"],
      ["⇧S", "Search Commands…"],
      ["⌘N  ⌘O  ⌘S  ⇧⌘S", "New, Open, Save, Save As"],
      ["⌘I", "Import"],
    ],
  ],
  [
    "While using a tool",
    [
      ["Type + Enter", "Exact length, size, angle, or 24s for sides"],
      ["→  ←  ↑", "Lock to the red, green, blue axis"],
      ["Hold ⇧", "Lock the direction you're inferring"],
      ["Esc", "Cancel, or close the group you're in"],
      [copy, "Move and Rotate: copy"],
      ["x5 after a copy", "Make 5 copies in a row"],
      ["Double-click", "Push/Pull: repeat the last distance; Select: open a group"],
    ],
  ],
  [
    "Selecting",
    [
      ["⇧ click", "Add or remove"],
      [`${copy}+click`, "Add"],
      [`Shift+${copy}+click`, "Remove"],
      [`${option}+click`, "Paint Bucket: sample a material"],
      ["⇧ click", "Paint Bucket: paint every matching face"],
      [`${copy}+click`, "Eraser: soften (add Shift to unsoften)"],
      ["⇧ click", "Eraser: hide"],
    ],
  ],
  ...(isWindows() ? [[
    "Menus (Vertical tools layout)",
    [
      ["F10", "Focus the menu bar"],
      [tt("Alt+F, Alt+E, Alt+V, Alt+C", "Alt+F, Alt+M, Alt+V, Alt+T"), tt("Open File, Edit, View or Camera", "Apri File, Modifica, Visualizza o Telecamera")],
      [tt("Alt+D, Alt+T, Alt+W, Alt+H", "Alt+D, Alt+S, Alt+N, Alt+G"), tt("Open Draw, Tools, Window or Help", "Apri Disegno, Strumenti, Finestra o Guida")],
      ["← →", "Switch menus"],
      ["↑ ↓ / Enter / Esc", "Move through commands, run a command, or close"],
    ],
  ] satisfies ShortcutSection] : []),
  ];
}

export function ShortcutSheet({ onClose }: { onClose: () => void }) {
  return (
    <Sheet onClose={onClose} label={tt("Keyboard shortcuts","Scorciatoie da tastiera")}>
      <div className="sheet-head">
        <b>{tt("Keyboard shortcuts","Scorciatoie da tastiera")}</b>
        <span className="muted">{isWindows() ? tt("Windows shortcuts", "Scorciatoie Windows") : tt("Modeling shortcuts", "Scorciatoie di modellazione")}</span>
        <button className="btn small ghost" onClick={onClose}>
          {tt("Close","Chiudi")}
        </button>
      </div>
      <div className="sheet-cols">
        {sections().map(([title, rows]) => (
          <div key={title} className="sheet-sec">
            <div className="section-title">{tr(title)}</div>
            {rows.map(([k, what]) => (
              <div key={k + what} className="sheet-row">
                <kbd>{desktopShortcut(k)}</kbd>
                <span>{tr(what)}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </Sheet>
  );
}
