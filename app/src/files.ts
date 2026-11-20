// File pickers: native dialogs in the desktop app, typed paths in a browser.
import { open, save } from "@tauri-apps/plugin-dialog";
import { tt } from "./i18n";

const inTauri = "__TAURI_INTERNALS__" in window;

export async function pickOpen(): Promise<string | null> {
  if (!inTauri) return window.prompt("Open which file? (full path to .sketchcraft, .skp, .dwg, .dxf, .fbx, .obj, or a .ply / .las / .xyz scan)");
  const r = await open({
    multiple: false,
    filters: [
      { name: "Models, drawings and scans", extensions: ["sketchcraft", "skp", "dwg", "dxf", "ifc", "fbx", "obj", "ply", "las", "xyz", "pts"] },
      { name: "SketchCraft", extensions: ["sketchcraft"] },
      { name: "SketchUp", extensions: ["skp"] },
      { name: "AutoCAD (DWG, DXF)", extensions: ["dwg", "dxf"] },
      { name: "IFC (Revit, ArchiCAD, BIM)", extensions: ["ifc"] },
      { name: "FBX (binary)", extensions: ["fbx"] },
      { name: "OBJ", extensions: ["obj"] },
      { name: "Point clouds (PLY, LAS, XYZ, PTS, TXT)", extensions: ["ply", "las", "laz", "xyz", "pts", "txt"] },
    ],
  });
  return typeof r === "string" ? r : null;
}

export async function pickSave(defaultName: string, ext: string, label: string): Promise<string | null> {
  if (!inTauri) return window.prompt(`Save to which path? (.${ext})`, defaultName);
  const r = await save({ defaultPath: defaultName, filters: [{ name: label, extensions: [ext] }] });
  return r ?? null;
}

export async function pickVrayScene(): Promise<string | null> {
  if (!inTauri) {
    return window.prompt(tt("Full local path to the matching .vrscene file", "Percorso locale completo del file .vrscene abbinato"))?.trim() || null;
  }
  const currentFile = rememberedFile();
  const directory = currentFile?.match(/^.*[/\\]/)?.[0];
  const path = await open({
    title: tt("Choose the matching V-Ray scene", "Scegli la scena V-Ray abbinata"),
    multiple: false,
    defaultPath: directory,
    filters: [{ name: "V-Ray (.vrscene)", extensions: ["vrscene"] }],
  });
  return typeof path === "string" ? path : null;
}

const KEY = "sketchcraft.currentFile";

export function rememberedFile(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

export function rememberFile(path: string | null) {
  try {
    if (path) localStorage.setItem(KEY, path);
    else localStorage.removeItem(KEY);
  } catch {
    /* storage unavailable: the title just won't persist */
  }
}
