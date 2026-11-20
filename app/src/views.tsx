// The Views panel: section planes and saved scenes (cameras with their style).
import { useRef, useState } from "react";
import { ask } from "./ask";
import { type Scene, type SavedView } from "./engine";
import { tt } from "./i18n";
import { Icons } from "./icons";
import type { Cam } from "./Viewport";
import { type Run } from "./panelkit";

/** Section planes: click one to cut with it (again to stop), double-click to rename. */
function SectionList({ scene, run }: { scene: Scene; run: Run }) {
  const list = scene.sections ?? [];
  if (!list.length) return null;
  const rename = async (id: number, name: string) => {
    const n = await ask("Rename the section plane", name);
    if (n?.trim()) await run({ cmd: "section_edit", id, name: n.trim() });
  };
  return (
    <>
      <div className="section-title sub" style={{ marginTop: 12 }}>
        Section planes
      </div>
      <ul className="layers views-list">
        {list.map((p) => {
          const on = scene.activeSection === p.id;
          return (
            <li key={p.id} className={on ? "on" : ""} onClick={() => run(on ? { cmd: "section_off" } : { cmd: "section_edit", id: p.id, active: true })} onDoubleClick={() => rename(p.id, p.name)} title={on ? "Cutting now: click to stop" : "Click to cut with this plane; double-click to rename"}>
              <Icons.section />
              <span className="name">{p.name}</span>
              {on && <span className="muted small">cutting</span>}
              <button
                className="eye"
                title="Delete section plane"
                onClick={(e) => {
                  e.stopPropagation();
                  run({ cmd: "section_edit", id: p.id, delete: true });
                }}
              >
                <Icons.trash />
              </button>
            </li>
          );
        })}
      </ul>
    </>
  );
}

export function Views({ scene, run, camera, goTo, onMatchPhoto, onPhotoModel, onRefImage }: { scene: Scene; run: Run; camera: () => Cam; goTo: (c: SavedView) => void; onMatchPhoto: (f: File) => void; onPhotoModel: () => void; onRefImage: () => void }) {
  const [name, setName] = useState("");
  const photoInput = useRef<HTMLInputElement>(null);
  const save = async () => {
    const n = name.trim() || `View ${scene.views.length + 1}`;
    await run({ cmd: "save_view", name: n, ...camera() });
    setName("");
  };
  return (
    <>
      <div className="row">
        <input className="num wide-input" placeholder={tt("Name this view", "Nome della scena")} value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && save()} />
        <button className="btn small" onClick={save}>
          <Icons.plus /> {tt("Save", "Salva")}
        </button>
      </div>
      <div className="photo-actions" role="group" aria-label="From photos">
        <span className="section-label">From photos</span>
        <button onClick={() => photoInput.current?.click()}>
          <b>Match photo…</b>
          <span>Line a photo up with the axes, then model over it</span>
        </button>
        <input ref={photoInput} type="file" accept="image/*" hidden onChange={(e) => e.target.files?.[0] && (onMatchPhoto(e.target.files[0]), (e.target.value = ""))} />
        <button onClick={onPhotoModel}>
          <b>Model from photos…</b>
          <span>{scene.views.filter((v) => v.photo).length >= 2 ? "Mark the same corners in your matched photos, get 3D" : "Needs two or more matched photos of one building"}</span>
        </button>
        <button onClick={onRefImage}>
          <b>Reference image…</b>
          <span>A see-through front, side or top board to trace</span>
        </button>
      </div>
      <ul className="layers views-list">
        {scene.views.map((v) => (
          <li key={v.name} onClick={() => goTo(v)}>
            {v.photo ? <img className="view-thumb" src={v.photo} alt="" /> : <Icons.camera />}
            <button className="name" type="button" onClick={(e) => { e.stopPropagation(); goTo(v); }}>{v.name}</button>
            <button
              className="eye"
              title="Delete view"
              onClick={(e) => {
                e.stopPropagation();
                run({ cmd: "delete_view", name: v.name });
              }}
            >
              <Icons.trash />
            </button>
          </li>
        ))}
        {!scene.views.length && <li className="empty">{tt("Save camera angles here and click to fly back. Renders can use them by name.", "Salva le inquadrature qui. Seleziona una scena per ripristinarla.")}</li>}
      </ul>
      <SectionList scene={scene} run={run} />
    </>
  );
}
