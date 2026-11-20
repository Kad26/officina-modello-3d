import { useCallback, useEffect, useRef, useState } from "react";
import { UpdateCard, checkForUpdatesNow } from "./updater";
import { ask, AskHost } from "./ask";
import { exec, execPlugin, inTauri, loadScene, sync, type Reply, type Scene } from "./engine";
import { Icons, Logo } from "./icons";
import { BuildTools, Sandbox } from "./buildpanels";
import { ContextMenu, contextItems, solidToolItems } from "./contextmenu";
import { DraftingBars } from "./draftingbars";
import { ShadowClock, useShadowClock } from "./shadow-clock";
import { DesktopMaterials } from "./desktopmaterials";
import { tr, tt } from "./i18n";
import { DesktopMenus, desktopTool, type DesktopAction, type DesktopMenu } from "./desktopmenus";
import { LocationDialog, SheetsDialog } from "./dialogs";
import { EntityInfo } from "./entityinfo";
import { type IssueEdge } from "./solidinspector";
import { Environments, Templates } from "./environments";
import { Library } from "./library";
import { Outliner, Tags } from "./outliner";
import { DefaultTray, useTray, type TrayPanelId } from "./defaulttray";
import { RenderDialog } from "./renderdialog";
import { ShortcutSheet } from "./shortcuts";
import { desktopShortcut, modelingKeysBlocked, redoShortcut, sketchUpWindowsKeys } from "./shortcut-labels";
import { Textures } from "./materialspanel";
import { Views } from "./views";
import { EMPTY, isEmpty, selIds, useSelection } from "./selection";
import { formatLength, parseLength, UNIT_OPTIONS } from "./units";
import { pickSave, rememberedFile, rememberFile } from "./files";
import Viewport, { type Backdrop, type BuildSettings, type FaceStyle, type Styles, type Tool, type ViewportApi } from "./Viewport";
import { matchedViews } from "./viewport/image-igloo";
import { MatchPhoto, type PhotoMatch } from "./matchphoto";
import { prepareImage } from "./textures";
import { savedSettings } from "./skins";
import { useLook, type Tab } from "./uselook";
import { CommandPalette, type Command } from "./palette";
import { TOOL_INFO, WORKSPACES } from "./workspace";
import { AppearanceDialog, WorkspaceDialog } from "./appearance";
import { WorkspaceStrip } from "./wsstrip";
import { Toolbar } from "./toolbar";
import { PluginFormDialog, PluginResultDialog, PluginsDialog, type Plugin } from "./plugins";
import { usePlugins } from "./useplugins";
import { FileMenu } from "./filemenu";
import { readFigure, skippedText, useFileActions, type FigureChoice } from "./usefiles";
import { ErrorBoundary } from "./boundary";
import { PhotoModeler } from "./photomodel";
import { RefImageDialog } from "./refimage";
import { Text3dDialog } from "./text3d";
import { RestoreDialog } from "./restore";
import { TimelinePanel } from "./timeline";
import { LearnCenter, LessonCard, lessonsDone, useLearnCtx } from "./learn";

const DEFAULT_BUILD: BuildSettings = {
  wallThickness: 0.2,
  wallHeight: 2.7,
  stairWidth: 1.0,
  stairRise: 2.7,
  roofKind: "gable",
  roofPitch: 30,
  roofOverhang: 0.4,
  columnSize: 0.3,
  columnHeight: 2.7,
  columnRound: false,
  smooveRadius: 5,
  smooveHeight: 1,
  stampOffset: 1,
  gridSize: 20,
  gridSpacing: 1,
  contourSpacing: 1,
};

const STANDARD_VIEWS = ["iso", "top", "front", "right", "back", "left"] as const;

function loadBuild(): BuildSettings {
  return savedSettings("sketchcraft.build", DEFAULT_BUILD, { roofKind: ["gable", "hip", "shed"] });
}

/** One-click looks, like SketchUp's style collections. */
const STYLE_PRESETS: { name: string; styles: Partial<Styles> }[] = [
  { name: "Default", styles: { face: "textured", background: "default", profiles: false, sketchy: false, extensions: false, ao: false, edges: true } },
  { name: "Architectural", styles: { face: "shaded", background: "default", profiles: true, sketchy: false, extensions: false, ao: true, edges: true } },
  { name: "Sketchy on paper", styles: { face: "hiddenline", background: "paper", profiles: true, sketchy: true, extensions: true, ao: false, edges: true } },
  { name: "Hidden line", styles: { face: "hiddenline", background: "default", profiles: true, sketchy: false, extensions: false, ao: false, edges: true } },
  { name: "Blueprint", styles: { face: "hiddenline", background: "blueprint", profiles: false, sketchy: false, extensions: true, ao: false, edges: true } },
  { name: "Clay", styles: { face: "monochrome", background: "default", profiles: false, sketchy: false, extensions: false, ao: true, edges: false } },
  { name: "Night", styles: { face: "shaded", background: "night", profiles: true, sketchy: false, extensions: false, ao: false, edges: true } },
];

/** The cut's height for a floor-plan cut: type one (4', 14', 1.2 m) to move it. */
function SectionHeight({ api, units, tick }: { api: React.RefObject<ViewportApi | null>; units: string; tick: number }) {
  const plane = api.current?.sectionPlane();
  const flat = plane && Math.abs(plane.normal[2]) > 0.99;
  const [text, setText] = useState<string | null>(null);
  void tick;
  if (!flat) return null;
  const z = plane!.origin[2];
  return (
    <input
      className="section-height"
      title="Cut height (type a height, Enter)"
      value={text ?? formatLength(z, units)}
      onFocus={(e) => (setText(formatLength(z, units)), e.target.select())}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => setText(null)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          const h = parseLength(text ?? "", units);
          if (!Number.isNaN(h)) api.current?.planCut(h);
          (e.target as HTMLInputElement).blur();
        }
        if (e.key === "Escape") (e.target as HTMLInputElement).blur();
      }}
    />
  );
}

const SWATCHES: [number, number, number][] = [
  [0.95, 0.94, 0.91], [0.93, 0.86, 0.72], [0.96, 0.8, 0.7], [0.82, 0.88, 0.8], [0.78, 0.85, 0.93], [0.87, 0.82, 0.92],
  [0.98, 0.93, 0.62], [0.78, 0.78, 0.76], [0.72, 0.16, 0.18], [0.45, 0.28, 0.18], [0.42, 0.55, 0.66], [0.42, 0.6, 0.36],
  [0.55, 0.57, 0.6], [0.32, 0.34, 0.38], [0.16, 0.17, 0.2], [0.97, 0.97, 0.95], [0.85, 0.55, 0.42], [0.6, 0.66, 0.5],
];

const hex = (c: number[]) => "#" + c.map((x) => Math.round(Math.min(1, Math.max(0, x)) * 255).toString(16).padStart(2, "0")).join("");
const unhex = (h: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as [number, number, number];


export default function App() {
  const [scene, setScene] = useState<Scene | null>(null);
  const [tool, setTool] = useState<Tool>(import.meta.env.VITE_OFFICINA === "1" ? "select" : "rect");
  const [matchedPhoto, setMatchedPhoto] = useState<string | null>(null);
  const [sel, setSel] = useSelection();
  const [color, setColorState] = useState<[number, number, number]>(SWATCHES[1]);
  const [paintMaterial, setPaintMaterial] = useState<string | null>(null);
  // picking a color stops painting with an image
  const setColor = (c: [number, number, number]) => {
    setColorState(c);
    setPaintMaterial(null);
  };
  const [measure, setMeasure] = useState({ label: "", value: "" });
  const [status, setStatus] = useState("");
  const [typed, setTyped] = useState("");
  const [toast, setToast] = useState<{ text: string; bad?: boolean } | null>(null);
  const [tab, setTab] = useState<Tab>("outliner");
  const [styles, setStyles] = useState<Styles>(() => {
    let help = true;
    try {
      help = localStorage.getItem("sketchcraft.help") !== "off";
    } catch {
      /* private window: help stays on */
    }
    return { xray: false, backEdges: false, shadows: false, edges: true, parallel: false, face: "textured", background: "default", help };
  });
  // help on or off is remembered
  useEffect(() => {
    try {
      localStorage.setItem("sketchcraft.help", styles.help === false ? "off" : "on");
    } catch {
      /* not saved: fine */
    }
  }, [styles.help]);
  const [section, setSection] = useState(false);
  /** what a scene remembers besides the camera: the style and the section cut */
  const sceneProps = () => {
    const sp = api.current?.sectionPlane();
    return { style: styles, section: sp ? [...sp.origin, ...sp.normal] : null };
  };
  /** go to a scene: its tags, hidden things, style and section cut, then its camera */
  const showView = async (v: Omit<Scene["views"][number], "photo"> & { photo?: string | null }) => {
    const r = await run({ cmd: "show_view", name: v.name });
    if (!r.ok) return;
    const parallel = v.photo ? false : v.parallel;
    api.current?.goTo({ eye: v.eye, target: v.target, fov: v.fov, up: v.up, parallel, ortho_height: v.ortho_height, photo: v.photo ?? undefined, photoName: v.name });
    if (r.style != null || parallel != null) {
      setStyles((s) => ({ ...s, ...(r.style as Partial<Styles> | null), ...(parallel == null ? {} : { parallel }) }));
    }
    if (r.style == null) return;
    const sec = r.section as number[] | null | undefined;
    if (sec && sec.length === 6) api.current?.setSection([sec[0], sec[1], sec[2]], [sec[3], sec[4], sec[5]]);
    else {
      api.current?.clearSection();
      setSection(false);
    }
  };
  const [showSkins, setShowSkins] = useState(false);
  const [showPhotoModel, setShowPhotoModel] = useState(false);
  const [showRef, setShowRef] = useState(false);
  const [showText3d, setShowText3d] = useState(false);
  const [showRestore, setShowRestore] = useState(false);
  const [showTimeline, setShowTimeline] = useState(false);
  const [showLearn, setShowLearn] = useState(false);
  const [lesson, setLesson] = useState<string | null>(null);
  const [showWs, setShowWs] = useState(false);
  const [placing, setPlacing] = useState<string | null>(null);
  const [build, setBuildState] = useState<BuildSettings>(loadBuild);
  const setBuild = (b: BuildSettings) => {
    setBuildState(b);
    try {
      localStorage.setItem("sketchcraft.build", JSON.stringify(b));
    } catch {
      /* settings just won't persist */
    }
  };
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [showPlugins, setShowPlugins] = useState(false);
  // who stands at the origin of a new model, for scale
  const [figure, setFigureRaw] = useState<FigureChoice>(() => readFigure());
  const setFigure = (f: FigureChoice) => {
    setFigureRaw(f);
    try {
      localStorage.setItem("sketchcraft.figure", f);
    } catch {}
  };
  const [currentFile, setCurrentFileState] = useState<string | null>(rememberedFile());
  const setCurrentFile = (p: string | null) => {
    setCurrentFileState(p);
    rememberFile(p);
  };
  const api = useRef<ViewportApi | null>(null);
  const locateSolidIssue = useCallback((edge: IssueEdge | null) => api.current?.locateSolidIssue(edge), []);
  const typedRef = useRef<HTMLInputElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  /** ⌘C / ⌘X: what to paste, and whether it was cut */
  const clip = useRef<{ ids: number[]; cut: boolean } | null>(null);
  const [showKeys, setShowKeys] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [showSheets, setShowSheets] = useState(false);
  const [showLocation, setShowLocation] = useState(false);
  const [showRender, setShowRender] = useState(false);
  // bumps when the section moves, so its height box refreshes
  const [sectionTick, setSectionTick] = useState(0);
  /** Match Photo in progress: the photo being lined up */
  const [matching, setMatching] = useState<{ src: string; width: number; height: number } | null>(null);
  const startMatch = async (file: File) => {
    try {
      const img = await prepareImage(file, 1600);
      setMatching({ src: img.data, width: img.width, height: img.height });
    } catch (e) {
      say(String(e), true);
    }
  };
  const finishMatch = async (mp: PhotoMatch) => {
    const m = matching;
    if (!m) return;
    const r = await run({ cmd: "match_photo", name: mp.name, width: m.width, height: m.height, red: mp.red, green: mp.green, origin: mp.origin, distance: mp.distance, photo: m.src, photoName: mp.name, parallel: false });
    if (!r.ok) return; // keep the editor open to fix the lines
    setMatching(null);
    api.current?.goTo({ eye: r.eye as [number, number, number], target: r.target as [number, number, number], fov: r.fov as number, photo: m.src });
    setStyles((s) => ({ ...s, parallel: false }));
    say(`Matched. Model over the photo; orbit to leave, click "${mp.name}" in Views to come back.`);
  };
  const showKeysRef = useRef(false);
  showKeysRef.current = showKeys;

  const say = useCallback((text: string, bad = false) => {
    setToast({ text, bad });
    window.setTimeout(() => setToast((t) => (t?.text === text ? null : t)), bad && text.length > 120 ? 9000 : 3600);
  }, []);

  const refresh = useCallback(async () => setScene(await loadScene()), []);

  const run = useCallback(
    async (cmd: Record<string, unknown>): Promise<Reply> => {
      const r = await exec(cmd);
      if (!r.ok) say(r.error ?? "That didn't work", true);
      else if (r.save_error) say(`Couldn't save your model: ${r.save_error}`, true);
      else if (Array.isArray(r.skipped_pictures) && r.skipped_pictures.length) say(skippedText(r.skipped_pictures as string[]), true);
      await refresh();
      return r;
    },
    [refresh, say]
  );

  // what plugin code asks for (JavaScript or Ruby): the engine only lets it model
  const runForPlugin = useCallback(
    async (cmd: Record<string, unknown>, plugin?: Plugin): Promise<Reply> => {
      const r = await execPlugin(cmd);
      if (!r.ok) say(`${plugin ? `Plugin “${plugin.name}”: ` : ""}${r.error ?? "That didn't work"}`, true);
      await refresh();
      return r;
    },
    [refresh, say]
  );
  // what a plugin says is shown as the plugin's, never in the app's own voice
  const sayAs = useCallback((plugin: Plugin, t: string, bad?: boolean) => say(plugin.builtin ? t : `Plugin “${plugin.name.slice(0, 40)}”: ${t}`, bad), [say]);
  const { plugins, setPlugins, pluginShown, setPluginShown, pluginRuns, toolSession, pluginTool, pluginForm, reloadPlugins, livePlugins, pluginButtons, runPluginCommand } = usePlugins({ sel, scene, tool, setTool, say, sayAs, runForPlugin, refresh });
  const { file, importFile, runTemplate } = useFileActions({ run, say, refresh, api, scene, figure, currentFile, setCurrentFile, setSel });

  // a notice from the engine (a damaged model file was set aside...): say it once
  const lastNotice = useRef("");
  useEffect(() => {
    const n = scene?.notice;
    if (n && n !== lastNotice.current) {
      lastNotice.current = n;
      say(n, true);
    }
  }, [scene, say]);

  useEffect(() => {
    refresh();
    // pick up edits Claude makes over MCP
    const id = window.setInterval(async () => {
      if (await sync()) {
        await refresh();
        say("Model updated by another session");
      }
    }, 700);
    return () => window.clearInterval(id);
  }, [refresh, say]);

  // drop selections that no longer exist
  useEffect(() => {
    if (!scene) return;
    const f = new Set(scene.faces.map((x) => x.id));
    const e = new Set(scene.edgeList.map((x) => x.id));
    const g = new Set(scene.groups.map((x) => x.id));
    const i = new Set(scene.instances.map((x) => x.id));
    setSel((s) => ({ faces: s.faces.filter((x) => f.has(x)), edges: s.edges.filter((x) => e.has(x)), groups: s.groups.filter((x) => g.has(x)), instances: s.instances.filter((x) => i.has(x)) }));
  }, [scene]);

  const chooseTool = useCallback((t: Tool) => {
    setTool(t);
    if (t !== "place") setPlacing(null);
  }, []);
  const photos = matchedViews(scene?.views ?? []);
  const iglooAvailable = tool === "igloo" || (photos.length > 1 && photos.some((photo) => photo.name === matchedPhoto));
  const toggleIgloo = () => {
    if (tool === "igloo") return chooseTool("select");
    if (!iglooAvailable) return say(tt(
      "Image Igloo needs at least two matched photos. Open one of their scenes first.",
      "Igloo immagini richiede almeno due foto abbinate. Apri prima una delle loro scene.",
    ), true);
    chooseTool("igloo");
  };
  const { skin, wearSkin, wsId, layout, setLayout, ws, editWs, switchWs } = useLook({ tool, chooseTool, setStyles, setTab });

  const deleteSelection = useCallback(async () => {
    if (isEmpty(sel)) return;
    await run({ cmd: "delete", ids: selIds(sel) });
    setSel(EMPTY);
  }, [sel, run]);

  const selectAll = () => {
    if (!scene) return;
    const editingComponent = scene.editing != null;
    const parent = editingComponent ? null : scene.activeGroup;
    const visibleEdges = new Set(scene.edgeIds);
    const visibleChild = (object: { parent: number | null; hidden: boolean; tag: string | null }) =>
      !editingComponent && object.parent === parent && !object.hidden && (!object.tag || scene.tags[object.tag] !== false);
    setSel({
      faces: scene.faces.filter((f) => f.group === parent && !f.hidden).map((f) => f.id),
      edges: editingComponent ? [] : scene.edgeList.filter((edge) => edge.group === parent && visibleEdges.has(edge.id)).map((edge) => edge.id),
      groups: scene.groups.filter(visibleChild).map((g) => g.id),
      instances: scene.instances.filter(visibleChild).map((i) => i.id),
    });
  };

  const copySelection = async (cut = false) => {
    if (isEmpty(sel)) return;
    const ids = selIds(sel);
    clip.current = { ids, cut };
    if (cut) {
      await run({ cmd: "hide", ids });
      setSel(EMPTY);
    }
    const shortcut = desktopShortcut("⌘V");
    say(cut ? `Cut. ${shortcut} pastes it where you click.` : `Copied. ${shortcut} pastes, then click to place.`);
  };

  // ⌘V: a copy of the clipboard, selected, with Move ready to place it.
  // ⇧⌘V (Paste in Place): the copy stays exactly where the original was,
  // inside whatever group is open now.
  const paste = async (inPlace = false) => {
    const c = clip.current;
    if (!c) return say(`Nothing to paste: select something and press ${desktopShortcut("⌘C")} first.`, true);
    const d = await run({ cmd: "duplicate", ids: c.ids, by: [0, 0, 0] });
    if (!d.ok) return;
    const ids = d.ids as number[];
    if (c.cut) {
      await run({ cmd: "unhide", ids });
      await run({ cmd: "delete", ids: c.ids });
      clip.current = { ids, cut: false };
    }
    let sc = await loadScene();
    const has = (list: { id: number }[]) => ids.filter((i) => list.some((x) => x.id === i));
    if (inPlace && sc.activeGroup != null) {
      // into the open group (groups and components move in; loose faces stay where they were made)
      const movable = [...has(sc.groups), ...has(sc.instances)].filter((i) => (sc.groups.find((g) => g.id === i)?.parent ?? sc.instances.find((x) => x.id === i)?.parent ?? null) !== sc.activeGroup);
      if (movable.length) {
        await run({ cmd: "set_parent", ids: movable, parent: sc.activeGroup });
        sc = await loadScene();
      }
    }
    setScene(sc);
    setSel({ faces: has(sc.faces), edges: [], groups: has(sc.groups), instances: has(sc.instances) });
    if (inPlace) return say("Pasted in place.");
    chooseTool("move");
    say("Pasted in place. Click a point on it, then click where it goes.");
  };

  // keyboard: tools, undo, delete, groups, typing into the measurement box
  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      if (modelingKeysBlocked(e) || e.altKey) return;
      // SubD mode has its own keys (the viewport handles them)
      if (api.current?.inCage()) {
        if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
          e.preventDefault();
          run({ cmd: e.shiftKey ? "redo" : "undo" });
        }
        return;
      }
      // Tab on a selected subdivision object: shape it (SubD mode)
      if (!e.shiftKey && !e.ctrlKey && !e.metaKey && e.key === "Tab" && sel.groups.length === 1 && scene?.cages?.[String(sel.groups[0])]) {
        e.preventDefault();
        api.current?.editCage(sel.groups[0]);
        return;
      }
      const mod = e.metaKey || e.ctrlKey;
      const k = e.key.toLowerCase();
      if (mod && ["n", "o", "s", "i"].includes(k) && (!e.shiftKey || k === "s")) {
        e.preventDefault();
        const f = fileRef2.current;
        if (k === "n") return void f.new();
        if (k === "o") return void f.open();
        if (k === "s") return void f.save(e.shiftKey);
        if (k === "i") return void f.import();
      }
      if (mod && k === "z") {
        e.preventDefault();
        await run({ cmd: e.shiftKey ? "redo" : "undo" });
        return;
      }
      if (mod && !e.shiftKey && k === "y") {
        // Windows redo
        e.preventDefault();
        await run({ cmd: "redo" });
        return;
      }
      if (mod && !e.shiftKey && k === "t") {
        // Select None
        e.preventDefault();
        setSel(EMPTY);
        return;
      }
      if (mod && ["c", "x", "v"].includes(k) && (!e.shiftKey || k === "v")) {
        e.preventDefault();
        if (k === "v") return void paste(e.shiftKey);
        return void copySelection(k === "x");
      }
      if (mod && !e.shiftKey && !sketchUpWindowsKeys && /^[1-6]$/.test(e.key)) {
        // standard views (SketchUp for Mac): Iso, Top, Front, Right, Back, Left
        e.preventDefault();
        api.current?.view(STANDARD_VIEWS[Number(e.key) - 1]);
        return;
      }
      if (mod && !e.shiftKey && k === "a" && scene) {
        e.preventDefault();
        return selectAll();
      }
      if (mod) return;
      if (e.code === "Space" && (e.shiftKey || sketchUpWindowsKeys)) {
        if (e.target instanceof Element && e.target.closest(".desktop-menus, [role='menu']")) return;
        e.preventDefault();
        if (!e.repeat) chooseTool(e.shiftKey ? "lasso" : "select");
        return;
      }
      if (e.key === "Escape" && showKeysRef.current) return setShowKeys(false);
      if (e.key === "?" || (e.shiftKey && e.key === "/")) {
        e.preventDefault();
        return setShowKeys((v) => !v);
      }
      // Shift+S: search commands (before plain S picks Scale)
      if (e.shiftKey && k === "s") {
        e.preventDefault();
        return setShowPalette(true);
      }
      if (e.shiftKey && k === "z") {
        e.preventDefault();
        return api.current?.fit();
      }
      if (e.shiftKey) return;
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        if (!e.repeat) void deleteSelection();
        return;
      }
      if (k === "g") {
        e.preventDefault();
        if (e.repeat) return;
        const ids = [...sel.faces, ...sel.groups];
        if (!ids.length) return say(tt("Select faces or groups first.", "Seleziona prima le facce o i gruppi."), true);
        const name = await ask(tt("Component name", "Nome componente"), tt("Component", "Componente"));
        if (name) {
          await run({ cmd: "make_component", ids, name });
          setSel(EMPTY);
        }
        return;
      }
      if (k === "k") {
        e.preventDefault();
        if (e.repeat) return;
        setStyles((s) => ({ ...s, backEdges: !s.backEdges, xray: false }));
        return;
      }
      if (/^[0-9.\-x/]$/.test(e.key) && !(k === "x" && tool !== "move")) {
        setTyped(e.key);
        typedRef.current?.focus();
        e.preventDefault();
        return;
      }
      if (k === "i") {
        e.preventDefault();
        if (!e.repeat) toggleIgloo();
        return;
      }
      const match = Object.entries(TOOL_INFO).find(([, info]) => info.key.toLowerCase() === k);
      if (match) {
        e.preventDefault();
        if (!e.repeat) chooseTool(match[0] as Tool);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [scene, sel, tool, run, deleteSelection, chooseTool, iglooAvailable]);

  /** everything Shift+S can find: tools, then actions from the menus and view bar */
  const commands = (): Command[] => {
    const tools: Command[] = (Object.entries(TOOL_INFO) as [Tool, { name: string; key: string }][]).filter(([id]) => id !== "igloo" || iglooAvailable).map(([id, t]) => ({ label: t.name.split(":")[0].split(" (")[0], hint: `Tool${t.name.includes(":") ? ` · ${t.name.split(":")[1].trim()}` : ""}`, key: t.key || undefined, run: id === "igloo" ? toggleIgloo : () => chooseTool(id) }));
    const toggle = (k: keyof Styles, label: string) => ({ label, hint: "View", run: () => setStyles((s) => ({ ...s, [k]: !s[k] })) });
    const act: Command[] = [
      { label: "New model", hint: "File", key: "⌘N", run: file.new },
      { label: "Open…", hint: "File", key: "⌘O", run: file.open },
      { label: "Save", hint: "File", key: "⌘S", run: () => file.save(false) },
      { label: "Save As…", hint: "File", run: () => file.save(true) },
      { label: "Restore from backup…", hint: "File · earlier copies of this model, and work set aside", run: () => setShowRestore(true) },
      { label: "Import…", hint: "File · SketchUp, DWG/DXF, IFC, FBX, OBJ, point clouds", key: "⌘I", run: file.import },
      ...(["glb", "obj", "stl", "dae", "dxf", "dwg", "pdf", "ifc"] as const).map((k) => ({ label: `Export ${k.toUpperCase()}…`, hint: "File", run: () => file.export(k) })),
      { label: "Render…", hint: "Photoreal render", run: () => setShowRender(true) },
      { label: "Timeline (video)…", hint: "Turntables, walkthroughs, sun studies", run: () => setShowTimeline(true) },
      { label: "Drawing sheets (PDF)…", hint: "File", run: () => setShowSheets(true) },
      { label: "Make blueprint (PDF)…", hint: "File", run: file.blueprint },
      { label: "Add location…", hint: "Terrain and map imagery", run: () => setShowLocation(true) },
      { label: "3D Text…", hint: "File", run: () => setShowText3d(true) },
      { label: "Generate report (CSV)…", hint: "File", run: file.report },
      { label: "Themes…", hint: "Colors, layout and icons (skins)", run: () => setShowSkins(true) },
      ...livePlugins.flatMap((pl) => pl.commands.map((c) => ({ label: c.label, hint: `Plugin · ${c.hint ?? pl.name}`, run: () => void runPluginCommand(pl, c.id) }))),
      { label: "Manage plugins…", hint: "Install, turn on or off, remove", run: () => (reloadPlugins(), setShowPlugins(true)) },
      {
        label: ws.dock === "classic" ? "Floating layout" : "Classic layout",
        hint: ws.dock === "classic" ? "SketchCraft's floating tool bars" : "Tool bars docked above the model, like SketchUp",
        run: () => setLayout(ws.dock === "classic" ? "floating" : "classic"),
      },
      { label: "Keyboard shortcuts", hint: "Help", key: "?", run: () => setShowKeys(true) },
      ...(inTauri && import.meta.env.VITE_OFFICINA !== "1" ? [{ label: "Check for updates…", hint: "Help", run: checkForUpdatesNow }] : []),
      { label: "About SketchCraft", hint: `Version ${__APP_VERSION__}`, run: () => say(`SketchCraft ${__APP_VERSION__}`) },
      { label: "Zoom extents", hint: "Camera", key: "⇧Z", run: () => api.current?.fit() },
      ...STANDARD_VIEWS.map((v) => ({ label: `${v[0].toUpperCase()}${v.slice(1)} view`, hint: "Camera", run: () => api.current?.view(v) })),
      { label: "Undo", hint: "Edit", key: "⌘Z", run: () => void run({ cmd: "undo" }) },
      { label: "Redo", hint: "Edit", key: redoShortcut, run: () => void run({ cmd: "redo" }) },
      { label: "Paste", hint: "Edit · then click where it goes", key: "⌘V", run: () => void paste() },
      { label: "Paste in place", hint: "Edit · exactly where it was, into the open group", key: "⇧⌘V", run: () => void paste(true) },
      { label: "Unhide all", hint: "Edit", run: () => void run({ cmd: "unhide" }) },
      { label: "Reset axes", hint: "Drawing axes back to the model's own", run: () => void run({ cmd: "set_axes", reset: true }) },
      {
        label: "Raise walls…",
        hint: "Walls from an imported floor plan (select it first)",
        run: async () => {
          const g = sel.groups[0];
          if (g == null) return say("Select the imported plan first (click it once), then Raise walls.", true);
          const units = scene?.units || "m";
          const raw = await ask("Wall height", formatLength(2.7, units), "The thin bands between wall lines go up this high.");
          if (raw == null) return;
          const height = parseLength(raw, units);
          if (!(height > 0)) return say("That isn't a height", true);
          const r = await run({ cmd: "raise_walls", group: g, height });
          if (r.ok) say(`Raised ${r.walls} walls${r.windows ? ` with ${r.windows} window openings` : ""}`);
        },
      },
      { label: "Clear guides", hint: "Edit", run: () => void run({ cmd: "clear_guides" }) },
      toggle("xray", "X-Ray"),
      toggle("shadows", "Shadows"),
      toggle("edges", "Edges"),
      toggle("backEdges", "Back edges"),
      toggle("parallel", "Parallel projection"),
      toggle("hideRest", "Hide rest of model (while editing)"),
      ...WORKSPACES.map((w) => ({ label: `${w.name} workspace`, hint: w.blurb, run: () => switchWs(w.id) })),
      ...(scene?.views ?? []).map((v) => ({ label: `Go to scene "${v.name}"`, hint: "Scenes", run: () => void showView(v) })),
    ];
    return [...tools, ...act];
  };

  const fileRef2 = useRef(file);
  fileRef2.current = file;

  const learnCtx = useLearnCtx(scene, wsId, section, !!styles.byTag, { theme: skin.id, layout, pluginRuns, figure });
  const docName = currentFile ? currentFile.split(/[/\\]/).pop()!.replace(/\.sketchcraft$/i, "") : tt("Untitled", "Senza titolo");
  const selCount = selIds(sel).length;
  const desktopLayout = layout === "side";
  const officina = import.meta.env.VITE_OFFICINA === "1";
  const showDefaultTray = officina && desktopLayout;
  const shadowClock = useShadowClock(showDefaultTray && styles.shadows && !!scene, scene?.location);
  const tray = useTray();
  const revealTrayPanel = tray.reveal;
  useEffect(() => {
    if (showDefaultTray && tool === "paint") revealTrayPanel("materials");
  }, [showDefaultTray, tool, revealTrayPanel]);
  const editing = scene?.activeGroup != null || scene?.editing != null;
  const fileMenu = (
    <FileMenu
      file={file}
      section={section}
      plugins={plugins}
      livePlugins={livePlugins}
      setPlugins={setPlugins}
      runPluginCommand={runPluginCommand}
      reloadPlugins={reloadPlugins}
      fileRef={fileRef}
      desktop={desktopLayout}
      open={{ restore: () => setShowRestore(true), plugins: () => setShowPlugins(true), sheets: () => setShowSheets(true), timeline: () => setShowTimeline(true), location: () => setShowLocation(true), text3d: () => setShowText3d(true), keys: () => setShowKeys(true) }}
    />
  );
  const outliner = scene && <Outliner scene={scene} sel={sel} setSel={setSel} run={run} byTag={!!styles.byTag} setByTag={(on) => setStyles((s) => ({ ...s, byTag: on }))} showTags={!showDefaultTray} />;
  const library = scene && (
    <>
      <Library
        scene={scene}
        placing={placing}
        run={run}
        onToast={say}
        place={(k) => {
          // components and assets would land inside the open component's own table
          if (k && scene.editing != null) return say("Close the component you're editing first: you can't place into a component while editing one.", true);
          setPlacing(k);
          setTool(k ? "place" : "select");
        }}
      />
      <details className="panel-fold" open>
        <summary>{tt("Build", "Costruzione")}</summary>
        <BuildTools tool={tool} setTool={chooseTool} build={build} setBuild={setBuild} units={scene.units || "m"} />
      </details>
      <details className="panel-fold">
        <summary>Sandbox (terrain)</summary>
        <Sandbox tool={tool} setTool={chooseTool} build={build} setBuild={setBuild} units={scene.units || "m"} run={run} sel={sel} onToast={say} />
      </details>
      <details className="panel-fold" open={!!scene.environment || undefined}>
        <summary>{tt("Environments", "Ambienti")}</summary>
        <Environments scene={scene} run={run} onToast={say} />
      </details>
      <details className="panel-fold">
        <summary>{tt("Templates", "Modelli iniziali")}</summary>
        <Templates onTemplate={runTemplate} />
      </details>
    </>
  );
  const sceneViews = scene && <Views scene={scene} run={run} camera={() => ({ ...api.current!.camera(), ...sceneProps() })} goTo={showView} onMatchPhoto={startMatch} onPhotoModel={() => setShowPhotoModel(true)} onRefImage={() => setShowRef(true)} />;
  const entityInfo = scene && (showDefaultTray || selCount > 0) && <EntityInfo scene={scene} sel={sel} run={run} locateSolidIssue={locateSolidIssue} desktop={showDefaultTray} />;
  const modelInfo = (
    <div className="section">
      {!showDefaultTray && <div className="section-title">Document</div>}
      <div className="kv">
        <span>{tt("Faces", "Facce")}</span>
        <b>{scene?.faces.length ?? 0}</b>
        <span>{tt("Groups", "Gruppi")}</span>
        <b>{scene?.groups.length ?? 0}</b>
        <span>{tt("Components", "Componenti")}</span>
        <b>{scene?.instances.length ?? 0}</b>
        <span>{tt("Units", "Unità")}</span>
        <select className="units-select" value={scene?.units || "m"} onChange={(e) => run({ cmd: "units", units: e.target.value })}>
          {UNIT_OPTIONS.map((u) => (
            <option key={u.id} value={u.id}>
              {u.label}
            </option>
          ))}
        </select>
        <span title="Who stands at the origin of a new model, for a sense of size">{tt("Scale figure", "Figura in scala")}</span>
        <select className="units-select" value={figure} onChange={(e) => setFigure(e.target.value as FigureChoice)} title="For new models (File › New). Add one any time from the Library.">
          <option value="person">Person</option>
          <option value="beaver">Beaver, with clipboard</option>
          <option value="none">None</option>
        </select>
      </div>
    </div>
  );
  const stylePanel = (
    <div className="section">
      {!showDefaultTray && <div className="section-title">Style</div>}
      <div className="kv">
        <span>{tt("Preset", "Preimpostazione")}</span>
        <select
          className="units-select"
          value=""
          onChange={(e) => {
            const p = STYLE_PRESETS.find((x) => x.name === e.target.value);
            if (p) setStyles((s) => ({ ...s, ...p.styles }));
          }}
        >
          <option value="">Choose…</option>
          {STYLE_PRESETS.map((p) => (
            <option key={p.name}>{p.name}</option>
          ))}
        </select>
        <span>{tt("Faces", "Facce")}</span>
        <select className="units-select" value={styles.face ?? "textured"} onChange={(e) => setStyles((s) => ({ ...s, face: e.target.value as FaceStyle }))}>
          <option value="textured">{tt("Shaded with textures", "Ombreggiato con texture")}</option>
          <option value="physical">Physically based (roughness, metal)</option>
          <option value="shaded">{tt("Shaded", "Ombreggiato")}</option>
          <option value="monochrome">{tt("Monochrome", "Monocromatico")}</option>
          <option value="hiddenline">{tt("Hidden line", "Linee nascoste")}</option>
          <option value="wireframe">{tt("Wireframe", "Reticolo")}</option>
        </select>
        <span>{tt("Background", "Sfondo")}</span>
        <select className="units-select" value={styles.background ?? "default"} onChange={(e) => setStyles((s) => ({ ...s, background: e.target.value as Backdrop }))}>
          <option value="default">{tt("Sky and ground", "Cielo e terreno")}</option>
          <option value="paper">{tt("Paper", "Carta")}</option>
          <option value="blueprint">Blueprint</option>
          <option value="night">{tt("Night", "Notte")}</option>
        </select>
      </div>
      <div className="style-checks">
        {(
          [
            ["profiles", "Profiles"],
            ["sketchy", "Sketchy edges"],
            ["extensions", "Extensions"],
            ["ao", "Ambient occlusion"],
          ] as [keyof Styles, string][]
        ).map(([k, label]) => (
          <label key={k} className="field check">
            <input type="checkbox" checked={!!styles[k]} onChange={(e) => setStyles((s) => ({ ...s, [k]: e.target.checked }))} />
            <span>{label}</span>
          </label>
        ))}
      </div>
    </div>
  );
  const paintSelection = (paint: { material: string } | { color: [number, number, number] }) => {
    const ids = [...sel.faces, ...sel.groups, ...(sketchUpWindowsKeys ? sel.instances : [])];
    if (!ids.length) return false;
    void run({ cmd: sketchUpWindowsKeys ? "paint_objects" : "paint", ids, ...paint });
    return true;
  };
  const textures = scene && (
    <Textures
      scene={scene}
      paintMaterial={paintMaterial}
      choose={(n) => {
        setPaintMaterial(n);
        if (n) chooseTool(selCount ? tool : "paint");
      }}
      run={run}
      camera={() => api.current!.camera()}
      sel={sel}
      onToast={say}
    />
  );
  const materials = (
    <>
      <div className="section">
        <div className="section-title">{selCount ? "Fill" : "Paint"}</div>
        <div className="swatches">
          {SWATCHES.map((c) => (
            <button
              key={hex(c)}
              className={`swatch ${!paintMaterial && hex(c) === hex(color) ? "on" : ""}`}
              style={{ background: hex(c) }}
              onClick={() => {
                setColor(c);
                paintSelection({ color: c });
              }}
            />
          ))}
        </div>
      </div>
      {textures}
    </>
  );
  const arrange = selCount > 0 && (
    <div className="section">
      <div className="section-title">{tt("Arrange", "Organizza")}</div>
      <div className="btn-grid">
        {(sel.faces.length > 0 || sel.groups.length + sel.instances.length > 1) && (
          <button className="btn small" title="Groups and components selected go inside the new group" onClick={() => run({ cmd: "group", ids: [...sel.faces, ...sel.groups, ...sel.instances] }).then((r) => r.ok && setSel({ ...EMPTY, groups: [r.group as number] }))}>
            Make group
          </button>
        )}
        {(sel.faces.length > 0 || sel.groups.length > 0) && (
          <button
            className="btn small"
            onClick={async () => {
              const name = await ask(tt("Component name", "Nome componente"), tt("Component", "Componente"));
              if (name) run({ cmd: "make_component", ids: [...sel.faces, ...sel.groups], name }).then(() => setSel(EMPTY));
            }}
          >
            Make component
          </button>
        )}
        {sel.instances.length > 0 && (
          <button className="btn small" onClick={() => Promise.all(sel.instances.map((id) => run({ cmd: "explode", id }))).then(() => setSel(EMPTY))}>
            Explode
          </button>
        )}
        <button className="btn small ghost" onClick={() => run({ cmd: "hide", ids: selIds(sel) }).then(() => setSel(EMPTY))}>
          Hide
        </button>
        <button className="btn small danger" onClick={deleteSelection}>
          Delete
        </button>
      </div>
    </div>
  );
  const trayPanels = showDefaultTray ? {
    entity: <>{entityInfo}{arrange}</>,
    outliner,
    tags: scene && <div className="section"><Tags scene={scene} sel={sel} run={run} byTag={!!styles.byTag} setByTag={(on) => setStyles((s) => ({ ...s, byTag: on }))} showHeading={false} /></div>,
    materials: (
      <DesktopMaterials colors={SWATCHES} color={color} paintMaterial={paintMaterial} materials={scene?.materials}
        onColor={(value) => {
          setColor(value);
          if (!paintSelection({ color: value })) chooseTool("paint");
        }}
        onMaterial={(name) => {
          setPaintMaterial(name);
          if (!paintSelection({ material: name })) chooseTool("paint");
        }}
        onEditMaterial={async (name, values) => (await run({ cmd: "material", name, ...values })).ok}
      >{textures}</DesktopMaterials>
    ),
    components: <div className="section">{library}</div>,
    styles: stylePanel,
    scenes: <div className="section">{sceneViews}</div>,
    model: modelInfo,
  } : null;
  const trayItem = (id: TrayPanelId, label: string): DesktopAction => ({
    label, checked: !tray.hidden && !tray.collapsed && tray.open.includes(id), run: () => tray.reveal(id),
  });
  const desktopMenus = (): DesktopMenu[] => {
    const toolItem = (id: keyof typeof TOOL_INFO, label: string) => desktopTool(id, label, chooseTool);
    const toggleStyle = (id: keyof Styles, label: string, checked = !!styles[id]): DesktopAction => ({
      label, checked, run: () => setStyles((s) => ({ ...s, [id]: !checked })),
    });
    const related = scene ? contextItems(scene, sel, run, setSel, () => api.current?.fit(), api.current?.sectionPlane(), (id) => api.current?.editCage(id), say) : [];
    const sceneTab = (nextTab: Tab, label: string): DesktopAction => ({
      label, checked: ws.showLeft && tab === nextTab, run: () => editWs({ showLeft: true, leftTab: nextTab }),
    });
    const faceModes: [FaceStyle, string][] = [["textured", "Shaded with Textures"], ["shaded", "Shaded"], ["monochrome", "Monochrome"], ["hiddenline", "Hidden Line"], ["wireframe", "Wireframe"]];
    return [
      { name: "Edit", sections: [
        [{ label: "Undo", key: "⌘Z", run: () => run({ cmd: "undo" }) }, { label: "Redo", key: redoShortcut, run: () => run({ cmd: "redo" }) }],
        [
          { label: "Cut", key: "⌘X", disabled: !selCount, run: () => copySelection(true) },
          { label: "Copy", key: "⌘C", disabled: !selCount, run: () => copySelection() },
          { label: "Paste", key: "⌘V", run: () => paste() },
          { label: "Paste in Place", key: "⇧⌘V", run: () => paste(true) },
          { label: "Delete", key: "⌫", disabled: !selCount, run: deleteSelection },
        ],
        [{ label: "Select All", key: "⌘A", disabled: !scene, run: selectAll }, { label: "Deselect All", key: "⌘T", disabled: !selCount, run: () => setSel(EMPTY) }],
        related.filter(({ label }) => /^(Make group|Group these|Make component|Ungroup|Make unique|Edit group|Edit component|Explode$|Close editing)/.test(label)),
        [{ label: "Unhide All", run: () => run({ cmd: "unhide" }) }, { label: "Delete Guides", run: () => run({ cmd: "clear_guides" }) }],
      ] },
      { name: "View", sections: [
        [toggleStyle("edges", "Edges"), toggleStyle("backEdges", "Back Edges"), toggleStyle("xray", "X-Ray"), toggleStyle("shadows", "Shadows")],
        faceModes.map(([face, label]) => ({ label, checked: (styles.face ?? "textured") === face, run: () => setStyles((s) => ({ ...s, face })) })),
        [...(showDefaultTray ? [{ label: "Plan", run: () => { api.current?.planCut(1.22); api.current?.view("top"); } }] : []), toggleStyle("notes", "Dimensions and Labels", styles.notes !== false), toggleStyle("help", "Tool Hints", styles.help !== false), ...(editing ? [toggleStyle("hideRest", "Hide Rest of Model")] : [])],
      ] },
      { name: "Camera", sections: [
        [{ label: "Perspective", checked: !styles.parallel, run: () => setStyles((s) => ({ ...s, parallel: false })) }, { label: "Parallel Projection", checked: styles.parallel, run: () => setStyles((s) => ({ ...s, parallel: true })) }],
        STANDARD_VIEWS.map((view, i) => ({ label: view === "iso" ? "Isometric" : view[0].toUpperCase() + view.slice(1), key: sketchUpWindowsKeys ? undefined : `⌘${i + 1}`, run: () => api.current?.view(view) })),
        [toolItem("orbit", "Orbit"), toolItem("pan", "Pan"), toolItem("zoom", "Zoom"), toolItem("zoomwindow", "Zoom Window"), { label: "Zoom Extents", key: "⇧Z", run: () => api.current?.fit() }],
        [toolItem("position", "Position Camera"), toolItem("walk", "Walk"), toolItem("lookaround", "Look Around"), { label: "Image Igloo", key: TOOL_INFO.igloo.key, checked: tool === "igloo", disabled: !iglooAvailable, run: toggleIgloo }],
        [{ label: "Previous View", run: () => api.current?.previousView() }, { label: "Next View", run: () => api.current?.nextView() }],
      ] },
      { name: "Draw", sections: [
        [toolItem("line", "Line"), toolItem("freehand", "Freehand")],
        [toolItem("arc", "2-Point Arc"), toolItem("arccenter", "Arc from Center"), toolItem("arc3", "3-Point Arc"), toolItem("pie", "Pie")],
        [toolItem("rect", "Rectangle"), toolItem("rrect", "Rotated Rectangle"), toolItem("circle", "Circle"), toolItem("polygon", "Polygon"), { label: "3D Text…", run: () => setShowText3d(true) }],
      ] },
      { name: "Tools", sections: [
        [toolItem("select", "Select"), toolItem("lasso", "Lasso Select"), toolItem("erase", "Eraser"), toolItem("paint", "Paint Bucket")],
        [toolItem("move", "Move"), toolItem("rotate", "Rotate"), toolItem("scale", "Scale"), toolItem("push", "Push/Pull"), toolItem("followme", "Follow Me"), toolItem("offset", "Offset")],
        [toolItem("tape", "Tape Measure"), toolItem("protractor", "Protractor"), toolItem("axes", "Axes"), toolItem("dimension", "Dimension"), toolItem("text", "Text"), toolItem("section", "Section Plane")],
        related.filter(({ label }) => /^(Solid: |Intersect Faces|Reverse faces|Orient faces|Bevel|Round|Slice)/.test(label)),
      ] },
      { name: "Window", sections: [
        ...(showDefaultTray ? [
          [{ label: "Default Tray", checked: !tray.hidden, run: () => tray.setHidden(!tray.hidden) }],
          [trayItem("entity", "Entity Info"), trayItem("outliner", "Outliner"), trayItem("tags", "Tags"), trayItem("materials", "Materials"),
           trayItem("components", "Components"), trayItem("styles", "Styles"), trayItem("scenes", "Scenes"), trayItem("model", "Model Info")],
        ] : [
          [sceneTab("outliner", "Outliner"), sceneTab("library", "Library"), sceneTab("views", "Scenes"), { label: "Properties", checked: ws.showRight, run: () => editWs({ showRight: !ws.showRight }) }],
        ]),
        WORKSPACES.map((w) => ({ label: `${w.name} Workspace`, checked: wsId === w.id, run: () => switchWs(w.id) })),
        [{ label: "Customize Toolbar…", run: () => setShowWs(true) }, { label: "Themes and Layout…", run: () => setShowSkins(true) }, { label: "Manage Plugins…", run: () => (reloadPlugins(), setShowPlugins(true)) }],
      ] },
      ...(showDefaultTray ? [{ name: "Extensions", sections: [
        [{ label: "Manage Plugins…", run: () => (reloadPlugins(), setShowPlugins(true)) }],
        livePlugins.flatMap((p) => p.commands.map((c) => ({ label: c.label, run: () => runPluginCommand(p, c.id) }))),
      ] }] : []),
      { name: "Help", sections: [
        [{ label: "Search Commands…", key: "⇧S", run: () => setShowPalette(true) }, { label: "Keyboard Shortcuts…", key: "?", run: () => setShowKeys(true) }, { label: "Learn SketchCraft…", run: () => setShowLearn(true) }],
        [...(inTauri && !officina ? [{ label: "Check for Updates…", run: checkForUpdatesNow }] : []), { label: officina ? "About Officina Modello 3D" : "About SketchCraft", run: () => say(officina ? `Officina Modello 3D · SketchCraft ${__APP_VERSION__}` : `SketchCraft ${__APP_VERSION__}`) }],
      ] },
    ];
  };

  return (
    <div className={`app ${ws.showLeft ? "" : "no-left"} ${ws.showRight ? "" : "no-right"} ${ws.dock === "left" ? "dock-left" : ws.dock === "classic" ? "dock-classic" : ""} ${layout === "side" ? "panels-right" : ""} ${showDefaultTray ? `has-default-tray ${tray.hidden ? "tray-hidden" : tray.collapsed ? "tray-collapsed" : ""}` : ""}`}>
      <header className="topbar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <Logo />
          <span className="brand-name">{officina ? "Officina Modello 3D" : "SketchCraft"}</span>
          <span className="sep">/</span>
          {!desktopLayout && fileMenu}
          <span className="doc" title={docName}>{docName}</span>
          <span className="live" title={officina ? "The local MCP connection can edit this model directly" : "Claude can edit this model over MCP; changes show up here"}>
            <i /> Live
          </span>
          {(scene?.editing != null || scene?.activeGroup != null) && (
            <button className="editing-pill" onClick={() => run({ cmd: "close" })}>
              Editing {scene?.editing != null ? "component" : "group"} · Done
            </button>
          )}
        </div>
        <div className="ws-switch" role="tablist" aria-label="Workspace">
          {WORKSPACES.map((w) => (
            <button key={w.id} role="tab" aria-selected={wsId === w.id} className={wsId === w.id ? "on" : ""} title={w.blurb} onClick={() => switchWs(w.id)}>
              {w.name}
            </button>
          ))}
          <button title={`Customize the ${ws.name} workspace: tools, toolbar, panels`} onClick={() => setShowWs(true)} aria-label="Customize workspace">
            ⋯
          </button>
        </div>
        <div className="top-actions">
          <button className="btn small ghost learn-btn" title="Lessons and demos: learn SketchCraft by doing" onClick={() => setShowLearn(true)}>
            Learn{lessonsDone().length === 0 && <i className="learn-dot" aria-label="(new)" />}
          </button>
          <button className="btn small ghost skin-btn" title="Themes: SketchCraft's colors, layout and icons" onClick={() => setShowSkins(true)}>
            <Icons.theme />
            Themes
          </button>
          <button className="icon-btn" title={`Undo (${desktopShortcut("⌘Z")})`} onClick={() => run({ cmd: "undo" })}>
            <Icons.undo />
          </button>
          <button className="icon-btn" title={`Redo (${desktopShortcut(redoShortcut)})`} onClick={() => run({ cmd: "redo" })}>
            <Icons.redo />
          </button>
          <input ref={fileRef} type="file" accept=".obj,.skp,.fbx,.dxf" hidden onChange={(e) => e.target.files?.[0] && importFile(e.target.files[0])} />
          <button className="btn primary" onClick={() => setShowRender(true)} title="A photoreal picture of this view">
            <Icons.camera /> {officina ? "Officina Render" : "Render"}
          </button>
        </div>
        {desktopLayout && <DesktopMenus file={fileMenu} menus={desktopMenus()} />}
        {showDefaultTray && <DraftingBars
          tool={tool}
          choose={chooseTool}
          api={api}
          file={file}
          undo={() => run({ cmd: "undo" })}
          redo={() => run({ cmd: "redo" })}
          solids={scene ? solidToolItems(scene, sel, run, setSel) : []}
          toggles={[
            { label: "X-Ray", icon: <Icons.eye />, on: styles.xray, change: () => setStyles((s) => ({ ...s, xray: !s.xray })) },
            { label: "Edges", icon: <Icons.cube />, on: styles.edges, change: () => setStyles((s) => ({ ...s, edges: !s.edges })) },
            { label: "Back Edges", icon: <Icons.layer />, on: styles.backEdges, change: () => setStyles((s) => ({ ...s, backEdges: !s.backEdges })) },
            { label: "Parallel Projection", icon: <Icons.axes />, on: styles.parallel, change: () => setStyles((s) => ({ ...s, parallel: !s.parallel })) },
            { label: "Shadows", icon: <Icons.section />, on: styles.shadows, change: () => setStyles((s) => ({ ...s, shadows: !s.shadows })) },
          ]}
          render={() => setShowRender(true)}
          tray={() => tray.setHidden(!tray.hidden)}
          search={() => setShowPalette(true)}
          shadowClock={<ShadowClock model={shadowClock} />}
        />}
      </header>

      {!showDefaultTray && (
        <aside className="panel left">
          <div className="tabs">
            {(["outliner", "library", "views"] as Tab[]).map((t) => (
              <button key={t} className={tab === t ? "on" : ""} onClick={() => setTab(t)}>
                {t === "outliner" ? "Outliner" : t === "library" ? "Library" : "Views"}
              </button>
            ))}
          </div>
          <div className="section grow">
            {tab === "outliner" ? outliner : tab === "library" ? library : sceneViews}
          </div>
        </aside>
      )}

      <main className="stage">
        <Viewport
          onHideHelp={() => {
            setStyles((s) => ({ ...s, help: false }));
            say("Help hidden. Turn it back on with Help in the view bar.");
          }}
          scene={scene}
          tool={tool}
          setTool={chooseTool}
          sel={sel}
          setSel={setSel}
          color={color}
          setColor={setColor}
          paintMaterial={paintMaterial}
          setPaintMaterial={setPaintMaterial}
          styles={styles}
          drafting={showDefaultTray}
          daylight={shadowClock.direction}
          onStatus={showDefaultTray ? setStatus : undefined}
          placeAsset={placing}
          build={build}
          run={run}
          onMeasure={(label, value) => setMeasure({ label, value })}
          onMatchedPhoto={setMatchedPhoto}
          onActivatePhoto={showView}
          onContext={(x, y) => setMenu({ x, y })}
          onToast={say}
          onSection={(on) => {
            setSection(on);
            setSectionTick((t) => t + 1);
          }}
          api={api}
          pluginTool={
            tool === "plugin" && pluginTool
              ? { ...pluginTool, move: (at) => toolSession.current?.move(at), click: (at) => toolSession.current?.click(at) }
              : null
          }
        />
        {matching && <MatchPhoto src={matching.src} width={matching.width} height={matching.height} defaultName={`Photo ${(scene?.views.filter((v) => v.photo).length ?? 0) + 1}`} onDone={finishMatch} onCancel={() => setMatching(null)} />}

        <Toolbar groups={ws.tools} tool={tool} choose={(id) => chooseTool(id as Tool)} vertical={showDefaultTray || ws.dock === "left"} iconStyle={layout === "side" ? "workshop" : "default"} onCustomize={() => setShowWs(true)} extra={pluginButtons}>
          <label className="chip" style={{ background: hex(color) }} title="Paint color">
            <input type="color" value={hex(color)} onChange={(e) => (setColor(unhex(e.target.value)), chooseTool("paint"))} />
          </label>
        </Toolbar>

        {(!showDefaultTray || section) && <div className="views">
          {(["iso", "top", "front", "right"] as const).map((v) => (
            <button key={v} onClick={() => api.current?.view(v)}>
              {v}
            </button>
          ))}
          <button onClick={() => api.current?.fit()} title={`Zoom extents (${desktopShortcut("⇧Z")})`}>
            fit
          </button>
          <button onClick={() => api.current?.previousView()} title={`Previous view (${desktopShortcut("⌘[")})`} aria-label="Previous view">
            ‹
          </button>
          <button onClick={() => api.current?.nextView()} title={`Next view (${desktopShortcut("⌘]")})`} aria-label="Next view">
            ›
          </button>
          <span className="vsep" />
          {(
            [
              ["xray", "X-ray"],
              ["backEdges", "Back edges"],
              ["shadows", "Shadows"],
              ["edges", "Edges"],
              ["parallel", "Parallel"],
              ["notes", "Notes"],
              ["help", "Help"],
            ] as [keyof Styles, string][]
          ).map(([k, label]) => (
            <button
              key={k}
              className={(k === "notes" || k === "help" ? styles[k] !== false : styles[k]) ? "on" : ""}
              title={k === "notes" ? "Dimensions, labels and door swings" : k === "help" ? "The tool coach, top-left: what each tool needs next" : undefined}
              onClick={() => setStyles((s) => ({ ...s, [k]: k === "notes" || k === "help" ? s[k] === false : !s[k] }))}
            >
              {tr(label)}
            </button>
          ))}
          {scene && (scene.activeGroup != null || scene.editing != null) && (
            <button className={styles.hideRest ? "on" : ""} title="Hide Rest of Model: only what you're editing shows" onClick={() => setStyles((s) => ({ ...s, hideRest: !s.hideRest }))}>
              Hide rest
            </button>
          )}
          {!section && (
            <button
              onClick={() => {
                api.current?.planCut(1.22);
                api.current?.view("top");
              }}
              title="Floor plan: cut the model 4 ft up and look down. Change the height to see other floors."
            >
              Plan
            </button>
          )}
          {section && (
            <span className="section-ctl">
              <button title="Move the cut up 1 ft (or back, for a side cut)" onClick={() => api.current?.moveSection(-0.3048)}>
                ▲
              </button>
              <button title="Move the cut down 1 ft (or in, for a side cut)" onClick={() => api.current?.moveSection(0.3048)}>
                ▼
              </button>
              <SectionHeight api={api} units={scene?.units || "m"} tick={sectionTick} />
              <button
                className="on"
                onClick={() => {
                  api.current?.clearSection();
                  setSection(false);
                }}
                title="Remove the section cut"
              >
                Section ✕
              </button>
            </span>
          )}
        </div>}

        <form
          className="measure"
          onSubmit={(e) => {
            e.preventDefault();
            api.current?.applyTyped(typed);
            setTyped("");
            typedRef.current?.blur();
          }}
        >
          <label htmlFor="measure-value">{tr(measure.label || "Value")}</label>
          <input id="measure-value" ref={typedRef} value={typed} placeholder={measure.value ? tr(measure.value) : showDefaultTray ? "" : tt("type, then Enter", "scrivi, poi premi Invio")} onChange={(e) => setTyped(e.target.value)} onKeyDown={(e) => e.key === "Escape" && (setTyped(""), typedRef.current?.blur())} />
        </form>
        {showDefaultTray && <div className="draft-status" role="status">{status}</div>}

        {scene && !showTimeline && (
          <WorkspaceStrip
            ws={wsId}
            scene={scene}
            sel={sel}
            setSel={setSel}
            run={run}
            api={api}
            section={section}
            setSection={(on) => {
              setSection(on);
              setSectionTick((t) => t + 1);
            }}
            actions={{ blueprint: file.blueprint, sheets: () => setShowSheets(true), location: () => setShowLocation(true), render: () => setShowRender(true), animation: () => setShowTimeline((v) => !v), showView: (v) => showView(v), sceneProps }}
          />
        )}
        {!officina && <UpdateCard />}
        {scene && showTimeline && <TimelinePanel scene={scene} run={run} api={api} setTool={chooseTool} onToast={say} onClose={() => setShowTimeline(false)} />}
        {showLearn && (
          <LearnCenter
            onStart={(id) => {
              setShowLearn(false);
              setLesson(null);
              window.setTimeout(() => setLesson(id), 0);
            }}
            onClose={() => setShowLearn(false)}
          />
        )}
        {lesson && learnCtx && <LessonCard key={lesson} id={lesson} ctx={learnCtx} onClose={() => setLesson(null)} onToast={say} />}
        {showRestore && <RestoreDialog run={run} onClose={() => setShowRestore(false)} onToast={say} />}
        {showText3d && scene && <Text3dDialog scene={scene} run={run} camera={() => api.current!.camera()} onClose={() => setShowText3d(false)} onToast={say} />}
        {showRef && scene && <RefImageDialog scene={scene} run={run} onClose={() => setShowRef(false)} onToast={say} />}
        {showPhotoModel && scene && <PhotoModeler scene={scene} run={run} onClose={() => setShowPhotoModel(false)} onToast={say} />}
        {showSkins && <AppearanceDialog current={skin} onPick={wearSkin} onClose={() => setShowSkins(false)} onToast={say} layout={layout} onLayout={setLayout} />}
        {pluginForm && (
          <ErrorBoundary onError={() => pluginForm.done(null)} fallback={() => null}>
            <PluginFormDialog form={pluginForm.form} plugin={pluginForm.plugin} units={scene?.units || "m"} done={pluginForm.done} />
          </ErrorBoundary>
        )}
        {showPlugins && <PluginsDialog list={plugins} reload={reloadPlugins} onClose={() => setShowPlugins(false)} onToast={say} commandsOf={(p) => livePlugins.find((x) => x.id === p.id)?.commands ?? p.commands} />}
        {pluginShown && (
          <ErrorBoundary onError={() => setPluginShown(null)} fallback={() => null}>
            <PluginResultDialog result={pluginShown.result} plugin={pluginShown.plugin} onClose={() => setPluginShown(null)} onToast={say} />
          </ErrorBoundary>
        )}
        {showWs && <WorkspaceDialog ws={ws} desktopTray={showDefaultTray} onChange={editWs} onReset={() => editWs(null)} onClose={() => setShowWs(false)} />}
        {toast && <div className={`toast ${toast.bad ? "bad" : ""}`}>{toast.text}</div>}
        <AskHost />
        {showRender && api.current && (
          <RenderDialog
            key={currentFile ?? "untitled"}
            embeddedVray={scene?.embeddedVray}
            camera={() => api.current!.camera()}
            aspect={(() => {
              const c = document.querySelector(".viewport canvas") as HTMLCanvasElement | null;
              return c && c.clientHeight ? c.clientWidth / c.clientHeight : 1.6;
            })()}
            name={(currentFile?.split("/").pop() ?? "model").replace(/\.sketchcraft$/, "")}
            onClose={() => setShowRender(false)}
            onToast={say}
          />
        )}
        {showKeys && <ShortcutSheet onClose={() => setShowKeys(false)} />}
        {showPalette && <CommandPalette commands={commands()} onClose={() => setShowPalette(false)} />}
        {showLocation && (
          <LocationDialog
            current={scene?.location}
            onClose={() => setShowLocation(false)}
            onAdd={async (p) => {
              setShowLocation(false);
              say("Fetching the ground and map…");
              const r = await run({ cmd: "add_location", ...p });
              if (r.ok) {
                say(`Location added: ground ${Math.round(r.elevation as number)} m above sea level`);
                api.current?.fitSoon();
              }
            }}
          />
        )}
        {showSheets && scene && (
          <SheetsDialog
            scene={scene}
            onClose={() => setShowSheets(false)}
            onMake={async (spec) => {
              const path = await pickSave(`${spec.project} sheets.pdf`, "pdf", "PDF");
              if (!path) return;
              setShowSheets(false);
              say("Drawing the sheets…");
              const r = await run({ cmd: "sheets", path, ...spec });
              if (r.ok) say(`Made ${r.pages} sheet${r.pages === 1 ? "" : "s"}: ${path.split("/").pop()}`);
            }}
          />
        )}
        {menu && scene && <ContextMenu x={menu.x} y={menu.y} close={() => setMenu(null)} items={contextItems(scene, sel, run, setSel, () => api.current?.fit(), api.current?.sectionPlane(), (id) => api.current?.editCage(id), say)} />}
      </main>

      {trayPanels ? (
        <DefaultTray tray={tray} panels={trayPanels} />
      ) : (
        <aside className="panel right">
          {entityInfo}
          {selCount === 0 && modelInfo}
          {selCount === 0 && stylePanel}
          {materials}
          {arrange}
          <div className="section hint">
            <p>Each tool tells you what to do next, top-left of the model. Press <b>?</b> for every shortcut.</p>
          </div>
        </aside>
      )}
    </div>
  );
}
