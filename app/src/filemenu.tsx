// The File menu in the top bar: files, exports, plugins, and the dialogs the rest of the app opens.
import { tt } from "./i18n";
import { useEffect, useRef, useState, type RefObject } from "react";
import { inTauri } from "./engine";
import { loadPlugins, type Plugin, type PluginList } from "./plugins";
import { checkForUpdatesNow } from "./updater";
import { useEscapeToClose } from "./sheet";
import { desktopShortcut, isWindows } from "./shortcut-labels";
import type { useFileActions } from "./usefiles";

export function FileMenu({ file, section, plugins, livePlugins, setPlugins, runPluginCommand, reloadPlugins, fileRef, open, desktop = false }: {
  file: ReturnType<typeof useFileActions>["file"];
  /** a section cut is showing (it can be exported) */
  section: boolean;
  plugins: PluginList;
  livePlugins: Plugin[];
  setPlugins: (p: PluginList) => void;
  runPluginCommand: (plugin: Plugin, commandId: string) => Promise<void> | undefined;
  reloadPlugins: () => void;
  fileRef: RefObject<HTMLInputElement | null>;
  desktop?: boolean;
  /** the dialogs the menu opens, by what they are */
  open: { restore: () => void; plugins: () => void; sheets: () => void; timeline: () => void; location: () => void; text3d: () => void; keys: () => void };
}) {
  const [fileMenu, setFileMenu] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [pluginsOpen, setPluginsOpen] = useState(false);
  const menu = useRef<HTMLDivElement>(null);
  useEscapeToClose(() => {
    if (desktop) menu.current?.querySelector("button")?.focus();
    setFileMenu(false);
  }, fileMenu);
  useEffect(() => {
    if (!desktop || !fileMenu) return;
    const outside = (e: PointerEvent) => {
      if (e.target instanceof Node && !menu.current?.contains(e.target)) setFileMenu(false);
    };
    document.addEventListener("pointerdown", outside, true);
    return () => document.removeEventListener("pointerdown", outside, true);
  }, [desktop, fileMenu]);
  const menuAccess = desktop && isWindows();
  return (
  <div
    className="dropdown"
    ref={menu}
    onBlur={(e) => {
      if (desktop && !e.currentTarget.contains(e.relatedTarget)) setFileMenu(false);
    }}
  >
    <button
      className={desktop ? "desktop-menu-trigger" : "file-btn"}
      aria-expanded={fileMenu}
      aria-keyshortcuts={menuAccess ? "Alt+F" : undefined}
      data-menu-access={desktop ? "f" : undefined}
      onClick={() => setFileMenu((o) => !o)}
    >
      {menuAccess ? <><u>F</u>ile</> : desktop ? "File" : "File ▾"}
    </button>
    {fileMenu && (
      <>
        {!desktop && <div className="menu-scrim" onPointerDown={() => setFileMenu(false)} />}
        <div className={desktop ? "dropdown-menu file-menu desktop-menu" : "dropdown-menu file-menu"} onClick={() => {
          if (desktop) menu.current?.querySelector("button")?.focus();
          setFileMenu(false);
        }}>
          <button onClick={file.new}>{tt("New", "Nuovo")} <kbd>{desktopShortcut("⌘N")}</kbd></button>
          <button onClick={file.open}>{tt("Open…", "Apri…")} <kbd>{desktopShortcut("⌘O")}</kbd></button>
          <button onClick={() => file.save(false)}>{tt("Save", "Salva")} <kbd>{desktopShortcut("⌘S")}</kbd></button>
          <button onClick={() => file.save(true)}>{tt("Save As…", "Salva con nome…")} <kbd>{desktopShortcut("⇧⌘S")}</kbd></button>
          <button onClick={() => open.restore()}>{tt("Restore from backup…", "Ripristina da backup…")}</button>
          <hr />
          <button onClick={file.import}>{tt("Import SketchUp, CAD or OBJ…", "Importa SketchUp, CAD o OBJ…")} <kbd>{desktopShortcut("⌘I")}</kbd></button>
          <button onClick={() => fileRef.current?.click()}>{tt("Import from browser upload…", "Importa dal browser…")}</button>
          <button
            className="submenu-btn"
            aria-expanded={exportOpen}
            onClick={(e) => {
              e.stopPropagation();
              setExportOpen((o) => !o);
            }}
          >
            {tt("Export", "Esporta")} <span className="caret">{exportOpen ? "▾" : "▸"}</span>
          </button>
          {exportOpen && (
            <div className="submenu">
              <button onClick={() => file.export("glb")}>glTF (.glb)…</button>
              <button onClick={() => file.export("obj")}>OBJ…</button>
              <button onClick={() => file.export("stl")}>STL (mm)…</button>
              <button onClick={() => file.export("dae")}>Collada (.dae)…</button>
              <button onClick={() => file.export("dxf")}>3D DXF (mm)…</button>
              <button onClick={() => file.export("dwg")}>DWG (AutoCAD)…</button>
              <button onClick={() => file.export("ifc")}>IFC (BIM)…</button>
              <button onClick={() => file.export("pdf")}>{tt("PDF (this view)…", "PDF (vista corrente)…")}</button>
              <button onClick={file.render}>{tt("PNG (this view)…", "PNG (vista corrente)…")}</button>
              <button onClick={file.animation}>{tt("Scene tour (MP4)…", "Tour delle scene (MP4)…")}</button>
              {section && (
                <>
                  <button onClick={() => file.section("svg")}>{tt("Section cut (SVG)…", "Sezione (SVG)…")}</button>
                  <button onClick={() => file.section("dxf")}>{tt("Section cut (DXF)…", "Sezione (DXF)…")}</button>
                </>
              )}
            </div>
          )}
          <button
            className="submenu-btn"
            aria-expanded={pluginsOpen}
            onClick={(e) => {
              e.stopPropagation();
              if (!pluginsOpen) loadPlugins().then(setPlugins, () => {});
              setPluginsOpen((o) => !o);
            }}
          >
            {tt("Plugins", "Estensioni")} <span className="caret">{pluginsOpen ? "▾" : "▸"}</span>
          </button>
          {pluginsOpen && (
            <div className="submenu">
              {livePlugins.flatMap((pl) =>
                pl.commands.map((c) => (
                  <button key={`${pl.id}:${c.id}`} title={c.hint ?? pl.description} onClick={() => void runPluginCommand(pl, c.id)}>
                    {c.label}
                  </button>
                )),
              )}
              {plugins.problems.map((p) => (
                <button key={`x${p.id}`} disabled title={p.error}>
                  {p.id}: {p.error}
                </button>
              ))}
              <button onClick={() => (reloadPlugins(), open.plugins())}>{tt("Manage plugins…", "Gestisci estensioni…")}</button>
            </div>
          )}
          <hr />
          <button onClick={file.blueprint}>{tt("Make blueprint (PDF)…", "Crea planimetria (PDF)…")}</button>
          <button onClick={() => open.sheets()}>{tt("Drawing sheets (PDF)…", "Tavole di disegno (PDF)…")}</button>
          <button onClick={() => open.timeline()}>{tt("Timeline (video)…", "Sequenza temporale (video)…")}</button>
          <hr />
          <button onClick={() => open.location()}>{tt("Add location…", "Aggiungi posizione…")}</button>
          <button onClick={() => open.text3d()}>{tt("3D Text…", "Testo 3D…")}</button>
          <button onClick={file.report}>{tt("Generate report (CSV)…", "Genera rapporto (CSV)…")}</button>
          <hr />
          <button onClick={() => open.keys()}>
            {tt("Keyboard shortcuts", "Scorciatoie da tastiera")} <kbd>?</kbd>
          </button>
          {inTauri && import.meta.env.VITE_OFFICINA !== "1" && <button onClick={checkForUpdatesNow}>Check for updates…</button>}
          <div className="menu-version" title="The version of SketchCraft you're running">
            SketchCraft {__APP_VERSION__}
          </div>
        </div>
      </>
    )}
  </div>
  );
}
