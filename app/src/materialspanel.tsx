// The Materials panel: finishes, pictures and the model's materials.
import { sketchUpWindowsKeys } from "./shortcut-labels";
import { confirm } from "./ask";
import { type Scene } from "./engine";
import { Icons } from "./icons";
import { tt } from "./i18n";
import { type Sel } from "./selection";
import { prepareImage } from "./textures";
import { useRef } from "react";
import type { Cam } from "./Viewport";
import { LengthInput, type Run } from "./panelkit";

/** Image materials: add one from a photo, paint with it, size it, project it. */
/** How a surface reflects: SketchUp-style presets over physically based values. */
const FINISHES: { label: string; it: string; roughness?: number; metalness?: number; glass?: boolean; emission?: number }[] = [
  { label: "Matte", it: "Opaco", roughness: 0.9, metalness: 0 },
  { label: "Satin", it: "Satinato", roughness: 0.5, metalness: 0 },
  { label: "Gloss", it: "Lucido", roughness: 0.12, metalness: 0 },
  { label: "Metal", it: "Metallo", roughness: 0.25, metalness: 1 },
  { label: "Brushed metal", it: "Metallo spazzolato", roughness: 0.55, metalness: 1 },
  { label: "Glass", it: "Vetro", roughness: 0.02, glass: true },
  { label: "Glow", it: "Luminoso", roughness: 0.9, emission: 3 },
];

/** A picture's size in the model, as people read it. */
function pictureSize(data: string): string {
  const mb = (data.length * 0.75) / (1 << 20);
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.max(1, Math.round((data.length * 0.75) / 1024))} KB`;
}

function textureSummary(count: number, size: string) {
  return tt(
    `${count} ${count === 1 ? "picture" : "pictures"}, ${size} in all.`,
    `${count} ${count === 1 ? "immagine" : "immagini"}, ${size} in totale.`,
  );
}

export function Textures({
  scene,
  paintMaterial,
  choose,
  run,
  camera,
  sel,
  onToast,
}: {
  scene: Scene;
  paintMaterial: string | null;
  choose: (name: string | null) => void;
  run: Run;
  camera: () => Cam;
  sel: Sel;
  onToast: (t: string, bad?: boolean) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const textures = scene.materials.filter((m) => m.texture);
  const active = textures.find((m) => m.name === paintMaterial);
  const add = async (file: File) => {
    try {
      const img = await prepareImage(file);
      let name = file.name.replace(/\.[^.]+$/, "") || "Image";
      while (scene.materials.some((m) => m.name === name)) name += " 2";
      const width = 1;
      const r = await run({ cmd: "texture", name, data: img.data, width, height: width * img.aspect, color: img.color });
      if (r.ok) {
        choose(name);
        onToast(tt(`Added ${name}. Click faces with Paint, or set its size in Edit.`, `Aggiunto ${name}. Usa il secchiello sulle facce o imposta la dimensione in Modifica.`));
      }
    } catch (e) {
      onToast(String(e), true);
    }
  };
  const replacer = useRef<HTMLInputElement>(null);
  // a new picture for a material that exists: every face that uses it shows the new one
  const replace = async (file: File) => {
    if (!active) return;
    try {
      const img = await prepareImage(file);
      const r = await run({ cmd: "texture", name: active.name, data: img.data, color: img.color, width: active.texture!.width, height: active.texture!.width * img.aspect });
      if (r.ok) onToast(tt(`${active.name} now shows ${file.name}. Undo brings the old picture back.`, `${active.name} ora usa ${file.name}. Annulla ripristina l’immagine precedente.`));
    } catch (e) {
      onToast(String(e), true);
    }
  };
  // take a material (and its picture) out of the model; its faces go to another material or back to the plain look
  const remove = async (replaceWith?: string) => {
    if (!active) return;
    const uses = scene.faces.filter((f) => f.material === active.name).length;
    const replacement = replaceWith?.replace(/^[a-z]+:/, "");
    const where = replacement ? tt(`They'll use ${replacement}.`, `Useranno ${replacement}.`) : tt("They go back to the plain look.", "Torneranno all’aspetto predefinito.");
    const usage = tt(`${uses} face${uses === 1 ? " uses" : "s use"} it.`, `È in uso su ${uses} facce.`);
    const size = pictureSize(active.texture!.data);
    const removal = tt(`Its picture (${size}) is removed from the model. Undo brings it back.`, `La texture (${size}) verrà rimossa dal modello. Puoi ripristinarla con Annulla.`);
    if (!(await confirm(tt(`Delete ${active.name}?`, `Eliminare ${active.name}?`), `${usage} ${where} ${removal}`, tt("Delete", "Elimina")))) return;
    const r = await run(replaceWith ? { cmd: "delete_material", name: active.name, replace_with: replaceWith } : { cmd: "delete_material", name: active.name });
    if (r.ok) {
      choose(null);
      onToast(tt(`Deleted ${active.name}.`, `Eliminato ${active.name}.`));
    }
  };
  const purgeUnused = async () => {
    const r = await run({ cmd: "purge_materials" });
    if (!r.ok) return;
    const size = Number(r.freed_mb) ? ` (${r.freed_mb} MB)` : "";
    onToast(Number(r.removed)
      ? tt(`Removed ${r.removed} unused materials${size}. Undo brings them back.`, `Rimossi ${r.removed} materiali inutilizzati${size}. Annulla li ripristina.`)
      : tt("Every material is in use.", "Tutti i materiali sono in uso."));
  };
  const project = async () => {
    if (!active) return;
    const c = camera();
    const dir = [c.target[0] - c.eye[0], c.target[1] - c.eye[1], c.target[2] - c.eye[2]];
    const r = await run({ cmd: "texture", name: active.name, project: dir, origin: c.target });
    if (r.ok) onToast(tt("Projected from this view. Paint a group for a continuous image.", "Proiettata da questa vista. Dipingi un gruppo per applicare un’immagine continua."));
  };
  const ids = [...sel.faces, ...sel.groups];
  return (
    <div className="section">
      <div className="material-finish-block">
        <div className="section-title">{tt("Finish", "Finitura")}</div>
        <div className="finishes" title={ids.length
          ? tt("Applies to the selection, keeping each face's color or image", "Applica alla selezione conservando il colore o la texture delle facce")
          : tt("Select faces or groups first", "Seleziona prima facce o gruppi")}>
          {FINISHES.map(({ label, it, ...values }) => (
            <button key={label} className="btn small ghost" disabled={!ids.length} onClick={async () => {
              const r = await run({ cmd: "finish", ids, label, ...values });
              if (r.ok) onToast(tt(`${label} applied. See it in the render or physically based display.`, `Finitura ${it.toLowerCase()} applicata. È visibile nel render o nello stile a base fisica.`));
            }}>
              {tt(label, it)}
            </button>
          ))}
        </div>
      </div>
      <div className="material-texture-palette">
        <div className="section-title">{tt("Textures", "Texture")}</div>
        <div className="swatches">
          {textures.map((m) => (
            <button
              key={m.name}
              className={`swatch texture ${paintMaterial === m.name ? "on" : ""}`}
              style={{ backgroundImage: `url(${m.texture!.data})` }}
              title={`${m.name} (${pictureSize(m.texture!.data)})`}
              aria-label={m.name}
              onClick={() => {
                choose(m.name);
                const targets = sketchUpWindowsKeys ? [...ids, ...sel.instances] : ids;
                if (targets.length) run({ cmd: sketchUpWindowsKeys ? "paint_objects" : "paint", ids: targets, material: m.name });
              }}
            />
          ))}
          <button className="swatch add" title={tt("Add a texture image", "Aggiungi un’immagine come texture")} aria-label={tt("Add texture", "Aggiungi texture")} onClick={() => input.current?.click()}>
            <Icons.plus />
          </button>
          <input ref={input} type="file" accept="image/*" hidden onChange={(e) => e.target.files?.[0] && add(e.target.files[0])} />
        </div>
        {textures.length > 0 && (
          <p className="muted small">
            {textureSummary(textures.length, pictureSize(textures.map((m) => m.texture!.data).join("")))}{" "}
            {[...textures].sort((a, b) => b.texture!.data.length - a.texture!.data.length).slice(0, 3).map((m) => `${m.name} ${pictureSize(m.texture!.data)}`).join(", ")}
            {textures.length > 3 ? "…" : ""}
          </p>
        )}
        <div className="row gap">
          <button className="btn small ghost" title={tt("Remove every material that no face uses", "Rimuovi i materiali non usati dalle facce")} onClick={purgeUnused}>
            {tt("Remove unused materials", "Rimuovi materiali inutilizzati")}
          </button>
        </div>
      </div>
      {active?.texture && (
        <div className="build-options material-active-texture">
          <p>
            <b>{active.name}</b>{" · "}{active.texture.project
              ? tt("Projected across the faces", "Proiettata sulle facce")
              : tt("Follows each face", "Segue le singole facce")}
          </p>
          <LengthInput
            label={tt("Tile width", "Larghezza texture")}
            value={active.texture.width}
            units={scene.units || "m"}
            onChange={(w) => run({ cmd: "texture", name: active.name, width: w })}
          />
          <label className="field" title={tt("Shows the model through the image", "Rende visibile il modello attraverso l’immagine")}>
            <span>{tt("Transparency", "Trasparenza")} {Math.round((1 - (active.opacity ?? 1)) * 100)}%</span>
            <input type="range" min={0} max={0.9} step={0.05} value={1 - (active.opacity ?? 1)} onChange={(e) => run({ cmd: "opacity", material: active.name, value: 1 - Number(e.target.value) })} />
          </label>
          <div className="row gap">
            <button className="btn small" onClick={project}>
              {tt("Project from this view", "Proietta da questa vista")}
            </button>
            {active.texture.project && (
              <button className="btn small ghost" onClick={() => run({ cmd: "texture", name: active.name, project: null })}>
                {tt("Lay on faces", "Adatta alle facce")}
              </button>
            )}
          </div>
          <div className="row gap">
            <button className="btn small ghost" title={tt("Show another picture on every face that uses this material", "Cambia l’immagine su tutte le facce che usano il materiale")} onClick={() => replacer.current?.click()}>
              {tt("Replace picture…", "Sostituisci immagine…")}
            </button>
            <button className="btn small ghost" title={tt("Take this material and its picture out of the model", "Rimuovi il materiale e la relativa immagine dal modello")} onClick={() => remove()}>
              {tt("Delete", "Elimina")}
            </button>
            <input ref={replacer} type="file" accept="image/*" hidden onChange={(e) => e.target.files?.[0] && replace(e.target.files[0])} />
          </div>
          {scene.materials.some((m) => m.name !== active.name) && (
            <label className="field" title={tt("Delete this material and put another one on the faces that use it", "Elimina il materiale e applicane un altro alle facce che lo usano")}>
              <span>{tt("Replace with", "Sostituisci con")}</span>
              <select value="" onChange={(e) => e.target.value && remove(e.target.value)}>
                <option value="">{tt("Choose a material…", "Scegli un materiale…")}</option>
                {scene.materials.filter((m) => m.name !== active.name).map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.name.replace(/^[a-z]+:/, "")}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
      )}
    </div>
  );
}
