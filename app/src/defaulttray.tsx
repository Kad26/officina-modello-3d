import { useCallback, useEffect, useState, type ReactNode } from "react";
import { store } from "./skins";
import { tr, tt } from "./i18n";

const PANELS = [
  ["entity", "Entity Info"],
  ["materials", "Materials"],
  ["outliner", "Outliner"],
  ["components", "Components"],
  ["styles", "Styles"],
  ["tags", "Tags"],
  ["scenes", "Scenes"],
  ["model", "Model Info"],
] as const;

export type TrayPanelId = typeof PANELS[number][0];
const DEFAULT_OPEN: TrayPanelId[] = ["entity", "materials", "outliner"];
const OPEN_KEY = "officina.tray.drafting.open";
const COLLAPSED_KEY = "officina.tray.collapsed";
const HIDDEN_KEY = "officina.tray.hidden";

export function useTray() {
  const [open, setOpen] = useState<TrayPanelId[]>(() => {
    try {
      const stored = JSON.parse(store.get(OPEN_KEY) ?? "null");
      if (Array.isArray(stored)) return PANELS.map(([id]) => id).filter((id) => stored.includes(id));
    } catch {
      // A damaged preference gets the standard panel arrangement.
    }
    return DEFAULT_OPEN;
  });
  const [collapsed, setCollapsed] = useState(() => store.get(COLLAPSED_KEY) === "1");
  const [hidden, setHidden] = useState(() => store.get(HIDDEN_KEY) === "1");
  useEffect(() => store.set(OPEN_KEY, JSON.stringify(open)), [open]);
  useEffect(() => store.set(COLLAPSED_KEY, collapsed ? "1" : "0"), [collapsed]);
  useEffect(() => store.set(HIDDEN_KEY, hidden ? "1" : "0"), [hidden]);
  const reveal = useCallback((id: TrayPanelId) => {
    setHidden(false);
    setCollapsed(false);
    setOpen((current) => current.includes(id) ? current : [...current, id]);
    requestAnimationFrame(() => document.getElementById(`tray-header-${id}`)?.scrollIntoView({ block: "nearest" }));
  }, []);
  const toggle = (id: TrayPanelId) => setOpen((current) => current.includes(id) ? current.filter((p) => p !== id) : [...current, id]);
  return { open, collapsed, hidden, reveal, toggle, setCollapsed, setHidden };
}

export function DefaultTray({ tray, panels }: { tray: ReturnType<typeof useTray>; panels: Record<TrayPanelId, ReactNode> }) {
  if (tray.hidden) return null;
  return (
    <aside className="default-tray" aria-label={tr("Default Tray")}>
      <div className="tray-title">
        <button aria-label={tray.collapsed ? tt("Expand Default Tray", "Espandi pannello predefinito") : tt("Collapse Default Tray", "Comprimi pannello predefinito")} aria-expanded={!tray.collapsed}
          onClick={() => tray.setCollapsed(!tray.collapsed)}>
          {tr("Default Tray")}
        </button>
        {!tray.collapsed && <button aria-label={tt("Hide Default Tray", "Nascondi pannello predefinito")} title={tt("Hide Default Tray", "Nascondi pannello predefinito")} onClick={() => tray.setHidden(true)}>×</button>}
      </div>
      {!tray.collapsed && <div className="tray-scroll">
        {PANELS.map(([id, label]) => {
          const expanded = tray.open.includes(id);
          return (
            <section className="tray-panel" key={id}>
              <h2>
                <button id={`tray-header-${id}`} aria-expanded={expanded} aria-controls={`tray-content-${id}`} onClick={() => tray.toggle(id)}>
                  <span aria-hidden="true">{expanded ? "▾" : "▸"}</span>{tr(label)}
                </button>
              </h2>
              <div id={`tray-content-${id}`} hidden={!expanded}>{expanded && panels[id]}</div>
            </section>
          );
        })}
      </div>}
    </aside>
  );
}
