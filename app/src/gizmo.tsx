// The axis gizmo in the model's top-right corner: red X, green Y and blue Z,
// turning with the camera. Drag it to orbit; click an axis to look down it
// (click the one facing you again to look from the other side).
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import * as THREE from "three";

const SIZE = 104;
const R = 34; // axis length in pixels
const AXES: { key: string; dir: [number, number, number]; color: string; label: string }[] = [
  { key: "+x", dir: [1, 0, 0], color: "#e5484d", label: "X" },
  { key: "+y", dir: [0, 1, 0], color: "#30a46c", label: "Y" },
  { key: "+z", dir: [0, 0, 1], color: "#3e63dd", label: "Z" },
  { key: "-x", dir: [-1, 0, 0], color: "#e5484d", label: "" },
  { key: "-y", dir: [0, -1, 0], color: "#30a46c", label: "" },
  { key: "-z", dir: [0, 0, -1], color: "#3e63dd", label: "" },
];
const NAMES: Record<string, string> = { "+x": "Right", "-x": "Left", "+y": "Back", "-y": "Front", "+z": "Top", "-z": "Bottom" };

export function AxisGizmo({
  camera,
  onOrbit,
  onLook,
  onDrag,
}: {
  /** the camera in use right now */
  camera: () => THREE.Camera | null;
  /** turn the view by this many pixels of drag */
  onOrbit: (dx: number, dy: number) => void;
  /** look at the model from this direction (a unit axis) */
  onLook: (dir: [number, number, number]) => void;
  /** a drag started (true) or ended (false) */
  onDrag: (on: boolean) => void;
}) {
  const svg = useRef<SVGSVGElement>(null);
  const [hover, setHover] = useState<string | null>(null);
  const drag = useRef<{ x: number; y: number; moved: boolean; id: number } | null>(null);
  // where each axis end is on screen, refreshed every frame
  const ends = useRef<Record<string, { x: number; y: number; z: number }>>({});

  useLayoutEffect(() => {
    const viewport = svg.current?.parentElement;
    if (!viewport) return;
    const viewBar = viewport.parentElement?.querySelector<HTMLElement>(":scope > .views");
    // The floating bar may wrap. In Classic its whole row is outside the viewport.
    const position = () => {
      const view = viewport.getBoundingClientRect();
      const bar = viewBar?.getBoundingClientRect();
      const top = bar && bar.bottom > view.top && bar.top < view.bottom ? Math.max(10, bar.bottom - view.top + 8) : 10;
      viewport.style.setProperty("--below-views", `${Math.ceil(top)}px`);
    };
    position();
    const observer = new ResizeObserver(position);
    observer.observe(viewport);
    if (viewBar) observer.observe(viewBar);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    let raf = 0;
    const m = new THREE.Matrix4();
    const v = new THREE.Vector3();
    const tick = () => {
      raf = requestAnimationFrame(tick);
      const cam = camera();
      const el = svg.current;
      if (!cam || !el) return;
      // the view's rotation only: world axes as the camera sees them
      m.extractRotation(cam.matrixWorldInverse);
      const c = SIZE / 2;
      const order: [string, number][] = [];
      for (const a of AXES) {
        v.set(...a.dir).applyMatrix4(m);
        ends.current[a.key] = { x: c + v.x * R, y: c - v.y * R, z: v.z };
        order.push([a.key, v.z]);
      }
      // nearest last, so it draws on top
      order.sort((p, q) => p[1] - q[1]);
      for (const [k, z] of order) {
        const g = el.querySelector<SVGGElement>(`[data-axis="${k}"]`);
        if (!g) continue;
        const e = ends.current[k];
        g.setAttribute("transform", `translate(${e.x.toFixed(2)} ${e.y.toFixed(2)})`);
        g.style.opacity = String(0.55 + 0.45 * ((z + 1) / 2));
        el.appendChild(g);
        const line = el.querySelector<SVGLineElement>(`[data-line="${k}"]`);
        if (line) {
          line.setAttribute("x2", e.x.toFixed(2));
          line.setAttribute("y2", e.y.toFixed(2));
          line.style.opacity = String(0.4 + 0.6 * ((z + 1) / 2));
        }
      }
    };
    tick();
    return () => cancelAnimationFrame(raf);
  }, [camera]);

  /** the axis end under the pointer, if any */
  const axisAt = (e: React.PointerEvent): string | null => {
    const r = svg.current!.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    let best: string | null = null;
    let bz = -Infinity;
    for (const a of AXES) {
      const p = ends.current[a.key];
      if (!p) continue;
      const rad = a.label ? 10 : 7;
      if (Math.hypot(p.x - x, p.y - y) <= rad + 2 && p.z > bz) {
        best = a.key;
        bz = p.z;
      }
    }
    return best;
  };

  return (
    <svg
      ref={svg}
      className={`axis-gizmo${drag.current ? " dragging" : ""}`}
      width={SIZE}
      height={SIZE}
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      role="img"
      aria-label="Axis gizmo: drag to orbit, click an axis to look along it"
      onPointerDown={(e) => {
        e.stopPropagation();
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
        drag.current = { x: e.clientX, y: e.clientY, moved: false, id: e.pointerId };
      }}
      onPointerMove={(e) => {
        const d = drag.current;
        if (!d) return setHover(axisAt(e));
        const dx = e.clientX - d.x;
        const dy = e.clientY - d.y;
        if (!d.moved && Math.hypot(dx, dy) < 3) return;
        if (!d.moved) onDrag(true);
        d.moved = true;
        d.x = e.clientX;
        d.y = e.clientY;
        onOrbit(dx, dy);
      }}
      onPointerUp={(e) => {
        const d = drag.current;
        drag.current = null;
        if (!d) return;
        if (d.moved) return onDrag(false);
        const k = axisAt(e);
        if (!k) return;
        const a = AXES.find((x) => x.key === k)!;
        // already looking down this axis? look from the other side
        const facing = (ends.current[k]?.z ?? 0) > 0.98;
        onLook(facing ? (a.dir.map((n) => -n) as [number, number, number]) : a.dir);
      }}
      onPointerLeave={() => !drag.current && setHover(null)}
    >
      <title>{hover ? `${NAMES[hover]} view` : "Drag to orbit · click an axis to look along it"}</title>
      <circle className="gizmo-bg" cx={SIZE / 2} cy={SIZE / 2} r={SIZE / 2 - 2} />
      {AXES.filter((a) => a.label).map((a) => (
        <line key={a.key} data-line={a.key} x1={SIZE / 2} y1={SIZE / 2} x2={SIZE / 2} y2={SIZE / 2} stroke={a.color} strokeWidth={2.5} strokeLinecap="round" />
      ))}
      {AXES.map((a) => (
        <g key={a.key} data-axis={a.key} className={`gizmo-end${hover === a.key ? " hover" : ""}`}>
          {a.label ? (
            <>
              <circle r={9} fill={a.color} />
              <text textAnchor="middle" dominantBaseline="central" fill="#fff" fontSize={10} fontWeight={700}>
                {a.label}
              </text>
            </>
          ) : (
            <circle r={6} fill="var(--panel, #fff)" stroke={a.color} strokeWidth={2} />
          )}
        </g>
      ))}
    </svg>
  );
}
