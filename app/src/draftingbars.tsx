import type { ReactNode } from "react";
import { Icons } from "./icons";
import { tr, tt } from "./i18n";
import type { Tool, ViewportApi } from "./Viewport";
import { verticalToolIcon } from "./vertical-tool-icons";
import type { MenuItem } from "./contextmenu";

const TOOL_ROWS: [Tool, string][][] = [
  [["select", "Select"], ["paint", "Paint Bucket"], ["erase", "Eraser"]],
  [["line", "Line"], ["freehand", "Freehand"], ["rect", "Rectangle"], ["rrect", "Rotated Rectangle"], ["circle", "Circle"], ["polygon", "Polygon"], ["arc", "2-Point Arc"], ["arccenter", "Arc from Center"], ["arc3", "3-Point Arc"], ["pie", "Pie"]],
  [["move", "Move"], ["push", "Push/Pull"], ["rotate", "Rotate"], ["followme", "Follow Me"], ["scale", "Scale"], ["offset", "Offset"]],
  [["tape", "Tape Measure"], ["dimension", "Dimension"], ["protractor", "Protractor"], ["text", "Text"], ["axes", "Axes"], ["section", "Section Plane"]],
  [["orbit", "Orbit"], ["pan", "Pan"], ["zoom", "Zoom"], ["zoomwindow", "Zoom Window"], ["position", "Position Camera"], ["lookaround", "Look Around"], ["walk", "Walk"]],
];

const VIEWS: [Parameters<ViewportApi["view"]>[0], string][] = [
  ["iso", "Isometric"], ["top", "Top"], ["front", "Front"], ["right", "Right"], ["back", "Back"], ["left", "Left"],
];

const SOLIDS = [
  ["Outer Shell", "Solid: Outer Shell"],
  ["Union", "Solid: Union"],
  ["Subtract", "Solid: Subtract…"],
  ["Trim", "Solid: Trim…"],
  ["Intersect", "Solid: Intersect"],
  ["Split", "Solid: Split"],
] as const;

function BadgeIcon({ children }: { children: ReactNode }) {
  return <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{children}</svg>;
}

const fileIcon = <BadgeIcon><path d="M6 3h8l4 4v14H6z M14 3v5h4" /></BadgeIcon>;
const folderIcon = <BadgeIcon><path d="M3 7V5h6l3 3h9v11H3z M3 11h18" /></BadgeIcon>;
const saveIcon = <BadgeIcon><path d="M4 4h14l2 2v14H4z M8 4v6h8V4 M8 20v-7h8v7" /></BadgeIcon>;

function ViewIcon({ view }: { view: typeof VIEWS[number][0] }) {
  return view === "iso" ? <Icons.cube /> : (
    <BadgeIcon>
      <path d="M4 4h16v16H4z" />
      {view === "top" && <path d="M4 8h16" strokeWidth="3" />}
      {view === "front" && <path d="M8 20v-8h8v8" />}
      {view === "right" && <path d="M15 5v14" strokeWidth="3" />}
      {view === "back" && <path d="M7 11h10v5H7z" />}
      {view === "left" && <path d="M9 5v14" strokeWidth="3" />}
    </BadgeIcon>
  );
}

function SolidIcon({ variant }: { variant: number }) {
  return (
    <BadgeIcon>
      <path d="M3 4h12v12H3z" strokeDasharray={variant === 2 || variant === 3 ? "2 2" : undefined} />
      <path d="M9 9h12v12H9z" strokeDasharray={variant === 0 ? "2 2" : undefined} />
      {variant === 1 && <path d="M4 5h10v5h6v10H10v-5H4z" fill="currentColor" fillOpacity=".23" stroke="none" />}
      {variant === 4 && <path d="M9 9h6v7H9z" fill="currentColor" />}
      {variant === 5 && <path d="m4 19 15-15" />}
    </BadgeIcon>
  );
}

function BarButton({ label, icon, onClick, pressed, id }: { label: string; icon: ReactNode; onClick?: () => unknown; pressed?: boolean; id?: Tool }) {
  const name = tr(label);
  return <button type="button" className="draft-button" data-tool={id} aria-label={name} title={name} aria-pressed={pressed} disabled={!onClick} onClick={onClick}>{icon}</button>;
}

export function DraftingBars({ tool, choose, api, file, undo, redo, solids, toggles, render, tray, search, shadowClock }: {
  tool: Tool;
  choose: (id: Tool) => void;
  api: React.RefObject<ViewportApi | null>;
  file: { new: () => unknown; open: () => unknown; save: (as: boolean) => unknown };
  undo: () => unknown;
  redo: () => unknown;
  solids: MenuItem[];
  toggles: { label: string; icon: ReactNode; on: boolean; change: () => void }[];
  render: () => void;
  tray: () => void;
  search: () => void;
  shadowClock: ReactNode;
}) {
  return (
    <div className="draft-bars">
      <nav className="draft-row" aria-label={tt("Drawing toolbar", "Barra di disegno")}>
        {TOOL_ROWS.map((group) => (
          <div className="draft-group" key={group[0][0]}>
            {group.map(([id, label]) => <BarButton key={id} id={id} label={label} icon={verticalToolIcon(id)} pressed={tool === id} onClick={() => choose(id)} />)}
          </div>
        ))}
      </nav>
      <nav className="draft-row" aria-label={tt("File, view and solid toolbars", "Barre di file, viste e solidi")}>
        <div className="draft-group">
          <BarButton label={tt("Search commands", "Cerca comandi")} icon={verticalToolIcon("zoom")} onClick={search} />
          <BarButton label={tt("New", "Nuovo")} icon={fileIcon} onClick={file.new} />
          <BarButton label={tt("Open", "Apri")} icon={folderIcon} onClick={file.open} />
          <BarButton label={tt("Save", "Salva")} icon={saveIcon} onClick={() => file.save(false)} />
          <BarButton label="Undo" icon={<Icons.undo />} onClick={undo} />
          <BarButton label="Redo" icon={<Icons.redo />} onClick={redo} />
        </div>
        <div className="draft-group">
          {VIEWS.map(([view, label]) => <BarButton key={view} label={label} icon={<ViewIcon view={view} />} onClick={() => api.current?.view(view)} />)}
          <BarButton label="Zoom Extents" icon={<BadgeIcon><path d="M4 9V4h5m6 0h5v5m0 6v5h-5m-6 0H4v-5" /><circle cx="12" cy="12" r="3" /></BadgeIcon>} onClick={() => api.current?.fit()} />
          <BarButton label="Previous View" icon={<Icons.undo />} onClick={() => api.current?.previousView()} />
        </div>
        <div className="draft-group">
          {toggles.map(({ label, icon, on, change }) => <BarButton key={label} label={label} icon={icon} pressed={on} onClick={change} />)}
        </div>
        <div className="draft-group">
          {SOLIDS.map(([label, action], index) => <BarButton key={label} label={label} icon={<SolidIcon variant={index} />} onClick={solids.find(item => item.label === action)?.run} />)}
        </div>
        <div className="draft-group">
          <BarButton label="Default Tray" icon={<BadgeIcon><path d="M3 4h18v16H3z M14 4v16 M16 8h3m-3 4h3m-3 4h3" /></BadgeIcon>} onClick={tray} />
          <BarButton label="Officina Render" icon={<Icons.camera />} onClick={render} />
        </div>
        {shadowClock}
      </nav>
    </div>
  );
}
