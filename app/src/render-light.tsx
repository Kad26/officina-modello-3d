import { useEffect, useState } from "react";
import { Vector3 } from "three";
import type { Vec3 } from "./engine";
import { tt } from "./i18n";
import { savedSettings, store } from "./skins";
import type { Cam } from "./Viewport";

const SIDES = ["left", "right", "above", "manual"] as const;
type Side = typeof SIDES[number];
type Light = {
  enabled: boolean;
  side: Side;
  width: number;
  height: number;
  distance: number;
  intensity: number;
  color: string;
  x: number;
  y: number;
  z: number;
};

const DEFAULT: Light = {
  enabled: false, side: "left", width: 1.4, height: 1.8, distance: 2.3,
  intensity: 7, color: "#fff1dd", x: 0, y: 0, z: 3,
};
const KEY = "officina.render.rectangle";
const EPSILON = 0.05;

export function useRenderLight() {
  const [light, setLight] = useState(() => savedSettings(KEY, DEFAULT, { side: SIDES }));
  useEffect(() => store.set(KEY, JSON.stringify(light)), [light]);
  const update = (patch: Partial<Light>) => setLight((current) => ({ ...current, ...patch }));
  return { light, update, studio: () => update({ ...DEFAULT, enabled: true }) };
}

export function areaLight(light: Light, camera: Cam) {
  const target = new Vector3(...camera.target);
  const fromCamera = new Vector3(...camera.eye).sub(target).setZ(0);
  if (fromCamera.lengthSq() < 1e-6) fromCamera.set(0, -1, 0);
  fromCamera.normalize();
  const right = new Vector3(0, 0, 1).cross(fromCamera);
  const offset = light.side === "left" ? -0.75 : light.side === "right" ? 0.75 : 0;
  const overhead = light.side === "above";
  const position = light.side === "manual" ? new Vector3(light.x, light.y, light.z) : target.clone()
    .addScaledVector(fromCamera, light.distance * (overhead ? 0.35 : 1))
    .addScaledVector(right, light.distance * offset)
    .add(new Vector3(0, 0, light.distance * (overhead ? 1.2 : 0.55)));
  const color = /^#[0-9a-f]{6}$/i.test(light.color) ? light.color : DEFAULT.color;
  return {
    position: position.toArray() as Vec3,
    target: camera.target,
    width: light.width,
    height: light.height,
    intensity: light.intensity,
    color: [1, 3, 5].map((i) => parseInt(color.slice(i, i + 2), 16) / 255) as Vec3,
  };
}

export function lightTooClose(light: ReturnType<typeof areaLight>) {
  return new Vector3(...light.position).distanceToSquared(new Vector3(...light.target)) < EPSILON * EPSILON;
}

function NumericField({ label, value, set, min, max, step = 0.1 }: {
  label: string; value: number; set: (value: number) => void; min: number; max: number; step?: number;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  return <label className="field">
    <span>{label}</span>
    <input type="number" min={min} max={max} step={step} value={draft ?? value}
      onChange={(e) => setDraft(e.currentTarget.value)}
      onBlur={(e) => {
        const n = e.currentTarget.valueAsNumber;
        if (draft != null && Number.isFinite(n) && n >= min && n <= max) set(n);
        setDraft(null);
      }}
      onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }} />
  </label>;
}

export function RenderLightControls({ model, camera, busy, invalid }: {
  model: ReturnType<typeof useRenderLight>; camera: Cam; busy: boolean; invalid: boolean;
}) {
  const { light, update } = model;
  return (
    <fieldset className="render-more render-light" disabled={busy}>
      <label className="field check">
        <input type="checkbox" checked={light.enabled} onChange={(e) => update({ enabled: e.currentTarget.checked })} />
        <span>{tt("Rectangular light (softbox)", "Luce rettangolare (softbox)")}</span>
      </label>
      {light.enabled && <>
        <label className="field">
          <span>{tt("Position relative to the view", "Posizione rispetto alla vista")}</span>
          <select value={light.side} onChange={(e) => {
            const side = e.currentTarget.value as Side;
            const [x, y, z] = areaLight(light, camera).position;
            update(side === "manual" ? { side, x, y, z } : { side });
          }}>
            <option value="left">{tt("Upper left", "In alto a sinistra")}</option>
            <option value="right">{tt("Upper right", "In alto a destra")}</option>
            <option value="above">{tt("Overhead", "Dall’alto")}</option>
            <option value="manual">{tt("Manual coordinates", "Coordinate manuali")}</option>
          </select>
        </label>
        <NumericField label={tt("Light width (m)", "Larghezza luce (m)")} value={light.width} min={0.05} max={30} set={(width) => update({ width })} />
        <NumericField label={tt("Light height (m)", "Altezza luce (m)")} value={light.height} min={0.05} max={30} set={(height) => update({ height })} />
        <NumericField label={tt("Light intensity", "Intensità luce")} value={light.intensity} min={0} max={100} set={(intensity) => update({ intensity })} />
        <label className="field">
          <span>{tt("Light color", "Colore luce")}</span>
          <input type="color" value={light.color} onChange={(e) => update({ color: e.currentTarget.value })} />
        </label>
        {light.side === "manual" ? (
          (["x", "y", "z"] as const).map((axis) => <NumericField key={axis} label={`${axis.toUpperCase()} (m)`} value={Math.round(light[axis] * 10000) / 10000}
            step={0.1} min={-1e6} max={1e6} set={(n) => update({ [axis]: n })} />)
        ) : <NumericField label={tt("Light distance (m)", "Distanza luce (m)")} value={light.distance} min={0.15} max={1000} set={(distance) => update({ distance })} />}
        {invalid && <p className="warn" role="alert">{tt("Move the light away from the center of the view.", "Sposta la luce lontano dal centro dell’inquadratura.")}</p>}
      </>}
    </fieldset>
  );
}
