// The Render dialog: quality, size, sun and light settings, and the progressive render.
import { useEffect, useRef, useState } from "react";
import { tt } from "./i18n";
import { exec, type EmbeddedVrayLighting, type Vec3 } from "./engine";
import { savedSettings } from "./skins";
import { pickSave, pickVrayScene } from "./files";
import { areaLight, lightTooClose, RenderLightControls, useRenderLight } from "./render-light";
import type { Cam } from "./Viewport";
import { SheetHead, useEscapeToClose } from "./sheet";

/** Render settings past quality and sun (the dialog's "More settings"). */
interface RenderSettings {
  size: string;
  width: number;
  height: number;
  ev: number;
  sun: number;
  shadowAngle: number | null;
  sky: number;
  warmth: number;
  bounces: number;
  backdrop: "sky" | "color";
  backdropColor: string;
  ground: boolean;
  groundColor: string;
  seamlessGround: boolean;
  denoise: boolean;
}
const RENDER_DEFAULTS: RenderSettings = { size: "quality", width: 1920, height: 1080, ev: 0, sun: 2.4, shadowAngle: null, sky: 1, warmth: 0, bounces: 6, backdrop: "sky", backdropColor: "#ffffff", ground: true, groundColor: "#94948c", seamlessGround: false, denoise: true };
const LIGHTING = [
  { id: "daylight", label: tt("Daylight", "Luce diurna"), altitude: 35, settings: RENDER_DEFAULTS },
  {
    id: "studio", label: tt("Catalog studio", "Studio catalogo"), altitude: 36,
    settings: { ...RENDER_DEFAULTS, size: "catalog", sun: 2.0, shadowAngle: 12, sky: 1.2, ev: 0.1,
      backdrop: "color", backdropColor: "#eae6df", groundColor: "#afa89c", seamlessGround: true },
  },
  {
    id: "interior", label: tt("Warm interior", "Interno caldo"), altitude: 26,
    settings: { ...RENDER_DEFAULTS, size: "catalog", sun: 1.8, shadowAngle: 5, sky: 0.75, ev: 0.3,
      warmth: 0.5, bounces: 8, backdrop: "color", backdropColor: "#d9c8b8", groundColor: "#b5a395", seamlessGround: true },
  },
] as const;
const QUALITY = [
  { id: "draft", label: tt("Draft", "Bozza"), samples: 16, long: 960, about: "16 samples per pixel for checking light and framing" },
  { id: "review", label: tt("Review", "Revisione"), samples: 64, long: 1600, about: "64 samples per pixel for comparing materials and lighting" },
  { id: "production", label: tt("Production", "Produzione"), samples: 256, long: 2400, about: "256 samples per pixel for the finished image" },
] as const;
const RENDER_SIZES: { id: string; label: string; w?: number; h?: number }[] = [
  { id: "quality", label: tt("This view's shape, by quality", "Proporzioni della vista, in base alla qualità") },
  { id: "catalog", label: tt("Catalog portrait 4:5, by quality", "Catalogo verticale 4:5, in base alla qualità") },
  { id: "hd", label: "HD 1920 × 1080", w: 1920, h: 1080 },
  { id: "4k", label: "4K 3840 × 2160", w: 3840, h: 2160 },
  { id: "square", label: tt("Square 2048 × 2048", "Quadrato 2048 × 2048"), w: 2048, h: 2048 },
  { id: "portrait", label: tt("Portrait 1080 × 1350", "Verticale 1080 × 1350"), w: 1080, h: 1350 },
  { id: "custom", label: tt("Custom…", "Personalizzato…") },
];
const clampSize = (n: number) => Math.max(16, Math.min(4096, Math.round(Number.isFinite(n) ? n : 1024)));
const MAX_AREA_LIGHTS = 8;

type ImportedAreaLight = ReturnType<typeof areaLight> & { u_axis?: Vec3; uniform_emission?: boolean };
type VrayReport = {
  filename: string;
  lights: ImportedAreaLight[];
  skipped: [kind: string, count: number][];
  warnings: string[];
  materials: number | null;
};

const VRAY_SKIP_REASONS: Record<string, [english: string, italian: string]> = {
  disabled: ["Disabled lights", "Luci disattivate"],
  other_light: ["Other light types", "Altri tipi di luce"],
  scene_space: ["Unsupported scene coordinates", "Coordinate della scena non supportate"],
  unsupported_rectangle: ["Unsupported rectangular lights", "Luci rettangolari non supportate"],
  photometric_units: ["Unsupported photometric units", "Unità fotometriche non supportate"],
  invalid_rectangle: ["Invalid rectangles", "Luci rettangolari non valide"],
  light_limit: ["Above the renderer limit", "Oltre il limite del motore"],
  include: ["External references", "Riferimenti esterni"],
};

function vraySkipReason(kind: string) {
  const reason = Object.hasOwn(VRAY_SKIP_REASONS, kind) ? VRAY_SKIP_REASONS[kind] : undefined;
  return reason ? tt(...reason) : kind;
}

function readVrayReport(path: string, reply: Awaited<ReturnType<typeof exec>>): VrayReport {
  if (!reply.ok) throw new Error(reply.error ?? tt("Could not read the V-Ray scene.", "Impossibile leggere la scena V-Ray."));
  const vec3 = (v: unknown) => Array.isArray(v) && v.length === 3 && v.every(Number.isFinite);
  if (!Array.isArray(reply.area_lights) || !reply.area_lights.every((l: Partial<ImportedAreaLight> | null) =>
    l && vec3(l.position) && vec3(l.target) && vec3(l.color) && (l.u_axis == null || vec3(l.u_axis))
    && (l.uniform_emission == null || typeof l.uniform_emission === "boolean")
    && Number.isFinite(l.width) && Number.isFinite(l.height) && Number.isFinite(l.intensity))) {
    throw new Error(tt("The scene returned invalid light data.", "La scena ha restituito dati di illuminazione non validi."));
  }
  const skipped = reply.skipped && typeof reply.skipped === "object" && !Array.isArray(reply.skipped)
    ? Object.entries(reply.skipped).filter((pair): pair is [string, number] => Number.isSafeInteger(pair[1]) && pair[1] > 0)
    : [];
  return {
    filename: path.split(/[/\\]/).pop() ?? path,
    lights: reply.area_lights,
    skipped,
    warnings: Array.isArray(reply.warnings) ? reply.warnings.filter((w): w is string => typeof w === "string") : [],
    materials: typeof reply.materials === "number" && Number.isSafeInteger(reply.materials) ? reply.materials : null,
  };
}

function useVrayLights() {
  const [report, setReport] = useState<VrayReport | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const attach = async () => {
    setLoading(true);
    setError("");
    try {
      const path = await pickVrayScene();
      if (!path) return;
      if (!/\.vrscene$/i.test(path)) throw new Error(tt("Choose a .vrscene file.", "Scegli un file .vrscene."));
      const next = readVrayReport(path, await exec({ cmd: "import_vray_lights", path }));
      setReport(next);
      setEnabled(next.lights.length > 0);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };
  const remove = () => {
    setReport(null);
    setEnabled(false);
    setError("");
  };
  return { report, enabled, setEnabled, loading, error, attach, remove };
}

function VrayLightControls({ vray, busy, used }: { vray: ReturnType<typeof useVrayLights>; busy: boolean; used: number }) {
  const { report, enabled, setEnabled, loading, error, attach, remove } = vray;
  const skipped = report?.skipped.reduce((total, [, count]) => total + count, 0) ?? 0;
  return (
    <fieldset className="render-vray" disabled={busy || loading}>
      <legend>{tt("V-Ray scene lights (beta)", "Luci della scena V-Ray (beta)")}</legend>
      <p className="muted small">{tt(
        "Choose a .vrscene exported from this model. This reads rectangular lights for the current render dialog. Officina uses the materials of the open model.",
        "Scegli il .vrscene esportato da questo modello. Le luci rettangolari sono disponibili in questa finestra di rendering. Officina usa i materiali del modello aperto.",
      )}</p>
      <div className="row gap">
        <button className="btn small ghost" onClick={attach}>{loading
          ? tt("Reading scene…", "Lettura scena…")
          : report ? tt("Change .vrscene…", "Cambia .vrscene…") : tt("Add V-Ray lights…", "Aggiungi luci V-Ray…")}</button>
        {report && <button className="btn small ghost" onClick={remove}>{tt("Remove", "Rimuovi")}</button>}
      </div>
      {error && <p className="warn small" role="alert">{error}</p>}
      {report && <>
        <div aria-live="polite" className="render-vray-report">
          <strong className="render-vray-name">{report.filename}</strong>
          <p>{tt(
            `${report.lights.length} compatible rectangular lights · ${used} in the render · ${skipped} skipped elements`,
            `${report.lights.length} luci rettangolari compatibili · ${used} nel render · ${skipped} elementi saltati`,
          )}</p>
          {report.materials != null && report.materials > 0 && <p className="muted small">{tt(
            `${report.materials} V-Ray material definitions detected.`,
            `${report.materials} definizioni di materiali V-Ray rilevate.`,
          )}</p>}
          {enabled && used < report.lights.length && <p className="warn small">{tt(
            `The renderer accepts up to ${MAX_AREA_LIGHTS} rectangular lights, including the Officina softbox.`,
            `Il motore accetta fino a ${MAX_AREA_LIGHTS} luci rettangolari, incluso il softbox di Officina.`,
          )}</p>}
        </div>
        <label className="field check">
          <input type="checkbox" checked={enabled} disabled={!report.lights.length} onChange={(e) => setEnabled(e.currentTarget.checked)} />
          {tt("Use imported lights", "Usa luci importate")}
        </label>
        {(report.skipped.length > 0 || report.warnings.length > 0) && (
          <details className="render-vray-details">
            <summary>{tt("Compatibility details", "Dettagli di compatibilità")}</summary>
            <ul>
              {report.skipped.map(([kind, count]) => <li key={kind}>{count} × {vraySkipReason(kind)}</li>)}
              {report.warnings.map((warning, index) => <li key={index}>{warning}</li>)}
            </ul>
          </details>
        )}
      </>}
    </fieldset>
  );
}

function renderSize(adv: RenderSettings, widthByHeight: number, long: number): [number, number] {
  if (adv.size === "custom") return [clampSize(adv.width), clampSize(adv.height)];
  if (adv.size === "catalog") return [clampSize(long * 0.8), long];
  const size = RENDER_SIZES.find((x) => x.id === adv.size);
  if (size?.w && size.h) return [size.w, size.h];
  return widthByHeight >= 1 ? [long, clampSize(long / widthByHeight)] : [clampSize(long * widthByHeight), long];
}

/** Keep the view's horizontal coverage when a portrait output is narrower than the viewport. */
function cameraForOutput(cam: Cam, viewportAspect: number, outputAspect: number): Cam {
  if (outputAspect >= viewportAspect) return cam;
  const half = Math.tan(cam.fov * Math.PI / 360) * viewportAspect / outputAspect;
  return { ...cam, fov: Math.min(160, Math.atan(half) * 360 / Math.PI) };
}

/** Photoreal render of the current view: quality, sun, then save. */
/** More settings: remembered between renders on this computer. */
function useRenderSettings() {
  const [adv, setAdvRaw] = useState<RenderSettings>(() => {
    return savedSettings("sketchcraft.render", RENDER_DEFAULTS, { size: RENDER_SIZES.map((s) => s.id), backdrop: ["sky", "color"] });
  });
  const setAdv = (patch: Partial<RenderSettings>) =>
    setAdvRaw((a) => {
      const n = { ...a, ...patch };
      try {
        localStorage.setItem("sketchcraft.render", JSON.stringify(n));
      } catch {}
      return n;
    });
  return { adv, setAdv };
}

/** The sun for a render: placed for this view, set by hand, or the real sun at a date and time. */
function useSun() {
  const [sunMode, setSunMode] = useState<"view" | "manual" | "real">("view");
  const [az, setAz] = useState(200);
  const [alt, setAlt] = useState(35);
  const [date, setDate] = useState(() => new Date().toISOString().slice(0, 10));
  const [minutes, setMinutes] = useState(15 * 60);
  const [real, setReal] = useState<{ azimuth: number; altitude: number; located: boolean; lon: number } | null>(null);
  // this Mac's clock offset on that date (daylight saving included); used when
  // the site is in roughly the same timezone, else the engine guesses from longitude
  const localOffset = -new Date(`${date}T12:00:00`).getTimezoneOffset() / 60;
  const offsetFor = (lon: number | undefined) => (lon != null && Math.abs(lon / 15 - localOffset) < 1.6 ? { utc_offset: localOffset } : {});
  const clock = `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
  const clock12 = `${((Math.floor(minutes / 60) + 11) % 12) + 1}:${String(minutes % 60).padStart(2, "0")} ${minutes < 720 ? "am" : "pm"}`;
  useEffect(() => {
    if (sunMode !== "real") return;
    let live = true;
    (async () => {
      // first ask where the site is, then where the sun is on its clock
      const probe = await exec({ cmd: "sun", date, time: clock });
      if (!probe.ok) return;
      const r = await exec({ cmd: "sun", date, time: clock, ...offsetFor(probe.lon as number) });
      if (live && r.ok) setReal(r as unknown as { azimuth: number; altitude: number; located: boolean; lon: number });
    })();
    return () => {
      live = false;
    };
  }, [sunMode, date, clock]);
  const sunDown = sunMode === "real" && real != null && real.altitude < 1;
  return { sunMode, setSunMode, az, setAz, alt, setAlt, date, setDate, minutes, setMinutes, real, clock, clock12, sunDown, offsetFor };
}

const compassOf = (a: number) => ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][Math.round(((a % 360) + 360) % 360 / 45) % 8];

/** The Sun choices: which sun, and its direction, height, date or time. */
function SunControls({ sun, busy, artificial }: { sun: ReturnType<typeof useSun>; busy: boolean; artificial: boolean }) {
  const { sunMode, setSunMode, az, setAz, alt, setAlt, date, setDate, minutes, setMinutes, real, clock, clock12, sunDown } = sun;
  const compass = compassOf(az);
  return (
    <>
        <div className="seg" role="group" aria-label={tt("Sun", "Sole")}>
          {([
            ["view", tt("Sun for this view", "Sole per la vista"), tt("Lights the side you're looking at", "Illumina il lato inquadrato")],
            ["manual", tt("Set by hand", "Sole manuale"), tt("Pick the direction and height", "Scegli direzione e altezza")],
            ["real", tt("Real sun", "Sole reale"), tt("Uses the place, date and time", "Usa posizione, data e ora del modello")],
          ] as const).map(([id, label, tip]) => (
            <button key={id} className={sunMode === id ? "on" : ""} aria-pressed={sunMode === id} onClick={() => setSunMode(id)} title={tip} disabled={busy}>
              {label}
            </button>
          ))}
        </div>
        {sunMode === "manual" && (
          <label className="field">
            <span>{tt("Sun from", "Direzione del sole")} {compass}</span>
            <input type="range" min={0} max={359} value={az} disabled={busy} onChange={(e) => setAz(Number(e.target.value))} />
          </label>
        )}
        {sunMode !== "real" && (
          <label className="field">
            <span>{tt("Sun height", "Altezza del sole")} {alt}°</span>
            <input type="range" min={5} max={85} value={alt} disabled={busy} onChange={(e) => setAlt(Number(e.target.value))} />
          </label>
        )}
        {sunMode === "real" && (
          <>
            <label className="field">
              <span>{tt("Date", "Data")}</span>
              <input type="date" value={date} disabled={busy} onChange={(e) => e.target.value && setDate(e.target.value)} />
            </label>
            <label className="field">
              <span>{tt("Time", "Ora")} {tt(clock12, clock)}</span>
              <input type="range" min={0} max={23 * 60 + 45} step={15} value={minutes} disabled={busy} onChange={(e) => setMinutes(Number(e.target.value))} />
            </label>
            <span className={`muted small${sunDown ? " warn" : ""}`}>
              {real == null
                ? tt("Finding the sun…", "Calcolo della posizione del sole…")
                : sunDown
                  ? artificial ? tt("Sun below the horizon. The artificial lights are active.", "Sole sotto l’orizzonte. Le luci artificiali sono attive.")
                    : tt("Sun below the horizon. Change the time or add an artificial light.", "Sole sotto l’orizzonte. Cambia l’ora o aggiungi una luce artificiale.")
                  : tt(`Sun from the ${compassOf(real.azimuth)}, ${Math.round(real.altitude)}° up.`, `Sole da ${compassOf(real.azimuth)}, altezza ${Math.round(real.altitude)}°.`)}
              {real && !real.located && tt(" Default location: central United States.", " Posizione predefinita: Stati Uniti centrali.")}
            </span>
          </>
        )}
    </>
  );
}

/** The "More settings" fieldset: size, light, background and ground. */
function MoreSettings({ adv, setAdv, busy, importedSunOff }: { adv: RenderSettings; setAdv: (patch: Partial<RenderSettings>) => void; busy: boolean; importedSunOff: boolean }) {
  return (
      <fieldset className="render-more" disabled={busy}>
        <label className="field">
          <span>{tt("Size", "Formato")}</span>
          <select value={adv.size} onChange={(e) => setAdv({ size: e.target.value })}>
            {RENDER_SIZES.map((x) => (
              <option key={x.id} value={x.id}>
                {x.label}
              </option>
            ))}
          </select>
        </label>
        {adv.size === "custom" && (
          <div className="row gap">
            <label className="field">
              <span>{tt("Width", "Larghezza")}</span>
              <input type="number" min={16} max={4096} value={adv.width} onChange={(e) => setAdv({ width: Number(e.target.value) })} />
            </label>
            <label className="field">
              <span>{tt("Height", "Altezza")}</span>
              <input type="number" min={16} max={4096} value={adv.height} onChange={(e) => setAdv({ height: Number(e.target.value) })} />
            </label>
          </div>
        )}
        <label className="field" title="Brighter or darker overall, in camera stops">
          <span>{tt("Exposure", "Esposizione")} {adv.ev > 0 ? "+" : ""}{adv.ev.toFixed(1)}</span>
          <input type="range" min={-3} max={3} step={0.1} value={adv.ev} onChange={(e) => setAdv({ ev: Number(e.target.value) })} />
        </label>
        <label className="field" title="How strong direct sunlight is (and how dark the shadows look next to it)">
          <span>{tt("Sunlight", "Luce del sole")} {importedSunOff ? tt("off in V-Ray", "disattivata in V-Ray") : adv.sun.toFixed(1)}</span>
          <input type="range" min={0} max={8} step={0.1} disabled={importedSunOff} value={importedSunOff ? 0 : adv.sun} onChange={(e) => setAdv({ sun: Number(e.target.value) })} />
        </label>
        <label className="field" title="The sky's soft light: it fills in the shadows">
          <span>{tt("Sky light", "Luce del cielo")} {adv.sky.toFixed(1)}</span>
          <input type="range" min={0} max={4} step={0.1} value={adv.sky} onChange={(e) => setAdv({ sky: Number(e.target.value) })} />
        </label>
        <label className="field" title="A wider light makes the edges of shadows softer">
          <span>{tt("Sun shadow softness", "Morbidezza ombre del sole")} {adv.shadowAngle == null ? tt("default", "predefinita") : `${adv.shadowAngle}°`}</span>
          <input type="range" min={0} max={20} step={0.5} value={adv.shadowAngle ?? 0.5} onChange={(e) => setAdv({ shadowAngle: Number(e.target.value) })} />
        </label>
        <label className="field" title="The sun's color: cool overcast to warm golden hour">
          <span>{tt("Sun color", "Colore del sole")} {adv.warmth < -0.15 ? tt("cool", "freddo") : adv.warmth > 0.15 ? tt("warm", "caldo") : tt("daylight", "neutro")}</span>
          <input type="range" min={-1} max={1} step={0.05} value={adv.warmth} onChange={(e) => setAdv({ warmth: Number(e.target.value) })} />
        </label>
        <label className="field" title="How many times light bounces between surfaces. More: brighter, truer interiors; slower">
          <span>{tt("Light bounces", "Rimbalzi della luce")} {adv.bounces}</span>
          <input type="range" min={1} max={12} step={1} value={adv.bounces} onChange={(e) => setAdv({ bounces: Number(e.target.value) })} />
        </label>
        <div className="field">
          <span>{tt("Background", "Sfondo")}</span>
          <div className="row gap">
            <div className="seg">
              <button className={adv.backdrop === "sky" ? "on" : ""} onClick={() => setAdv({ backdrop: "sky" })}>
                {tt("Sky", "Cielo")}
              </button>
              <button className={adv.backdrop === "color" ? "on" : ""} onClick={() => setAdv({ backdrop: "color" })} title="A plain color behind the model (the sky still lights it)">
                {tt("Color", "Colore")}
              </button>
            </div>
            {adv.backdrop === "color" && <input type="color" aria-label="Background color" value={adv.backdropColor} onChange={(e) => setAdv({ backdropColor: e.target.value })} />}
          </div>
        </div>
        <div className="field">
          <span>{tt("Ground", "Terreno")}</span>
          <div className="row gap">
            <label className="field check">
              <input type="checkbox" checked={adv.ground} onChange={(e) => setAdv({ ground: e.target.checked })} /> {tt("Show the ground", "Mostra il terreno")}
            </label>
            {adv.ground && <input type="color" aria-label="Ground color" value={adv.groundColor} onChange={(e) => setAdv({ groundColor: e.target.value })} />}
          </div>
        </div>
        {adv.ground && adv.backdrop === "color" && (
          <label className="field check" title="Blend the far floor into the backdrop to soften the horizon">
            <input type="checkbox" checked={adv.seamlessGround} onChange={(e) => setAdv({ seamlessGround: e.target.checked })} /> {tt("Seamless ground", "Raccorda terreno e sfondo")}
          </label>
        )}
        <label className="field check" title="Smooths the grain while the picture sharpens. Off: the raw samples">
          <input type="checkbox" checked={adv.denoise} onChange={(e) => setAdv({ denoise: e.target.checked })} /> {tt("Remove grain (denoise)", "Riduci il rumore")}
        </label>
        <button className="btn small ghost" onClick={() => setAdv(RENDER_DEFAULTS)}>
          {tt("Reset to defaults", "Ripristina valori predefiniti")}
        </button>
      </fieldset>
  );
}

export function RenderDialog({ camera, aspect, name, embeddedVray, onClose, onToast }: { camera: () => Cam; aspect: number; name: string; embeddedVray?: EmbeddedVrayLighting; onClose: () => void; onToast: (t: string, bad?: boolean) => void }) {
  const [quality, setQuality] = useState<(typeof QUALITY)[number]>(QUALITY[0]);
  const { adv, setAdv } = useRenderSettings();
  const [more, setMore] = useState(false);
  const light = useRenderLight();
  const vray = useVrayLights();
  const studioFeature = import.meta.env.VITE_OFFICINA === "1";
  const [embeddedEnabled, setEmbeddedEnabled] = useState(true);
  const sun = useSun();
  const { sunMode, offsetFor, real, date, clock, az, alt, sunDown } = sun;
  const [busy, setBusy] = useState(false);
  const cam = useRef(camera());
  const stopRef = useRef(false);
  const [picture, setPicture] = useState<{ data: string; key: string; done: number; goal: number; secs: number; w: number; h: number } | null>(null);
  const [w, h] = renderSize(adv, aspect, quality.long);
  const panel = studioFeature && light.light.enabled ? areaLight(light.light, cam.current) : null;
  const invalidLight = panel != null && lightTooClose(panel);
  const imported = studioFeature && vray.enabled
    ? (vray.report?.lights ?? []).slice(0, MAX_AREA_LIGHTS - (panel ? 1 : 0)) : [];
  const areaLights = panel ? [panel, ...imported] : imported;
  const hasEmbedded = studioFeature && !!embeddedVray
    && (embeddedVray.area_lights.length > 0 || !!embeddedVray.sky_color || !!embeddedVray.white_balance || !!embeddedVray.reinhard);
  const useEmbedded = hasEmbedded && embeddedEnabled && !vray.enabled;
  const embeddedLights = useEmbedded ? embeddedVray.area_lights.slice(0, MAX_AREA_LIGHTS - areaLights.length) : [];
  const importedSunOff = useEmbedded && embeddedVray.sun_enabled === false
    && (embeddedLights.length > 0 || !!embeddedVray.sky_color);
  const artificial = [...areaLights, ...embeddedLights].some((l) => l.intensity > 0 && l.color.some((c) => c > 0))
    || (useEmbedded && adv.sky > 0 && !!embeddedVray.sky_color?.some((c) => c > 0));
  const canRender = !vray.loading && !invalidLight && (!sunDown || artificial);
  const renderCamera = cameraForOutput(cam.current, aspect, w / h);
  const sunOptions = sunMode === "real"
    ? { date, time: clock, ...offsetFor(real?.lon) }
    : sunMode === "manual" ? { sun_azimuth: az, sun_altitude: alt } : { sun_altitude: alt };
  const settings = {
    width: w,
    height: h,
    camera: renderCamera,
    camera_up: renderCamera.up,
    parallel: !!renderCamera.parallel,
    ortho_height: renderCamera.ortho_height != null ? renderCamera.ortho_height * Math.max(1, aspect / (w / h)) : undefined,
    ...sunOptions,
    bounces: adv.bounces,
    exposure: Math.pow(2, adv.ev),
    ...(importedSunOff ? { sun_strength: 0 } : { sun_strength: adv.sun }),
    sky_strength: adv.sky,
    ...(adv.shadowAngle == null ? {} : { shadow_angle: adv.shadowAngle }),
    warmth: adv.warmth,
    denoise: adv.denoise,
    ground: adv.ground,
    ground_color: adv.groundColor,
    seamless_ground: adv.seamlessGround,
    ...(areaLights.length ? { area_lights: areaLights } : {}),
    use_embedded_lights: useEmbedded,
    ...(adv.backdrop === "color" ? { background: adv.backdropColor } : {}),
  };
  const key = JSON.stringify(settings);
  const samePicture = picture?.key === key;
  const activeLighting = LIGHTING.find(({ settings, altitude }) =>
    !areaLights.length && !useEmbedded && sunMode === "view" && alt === altitude
    && (Object.keys(settings) as (keyof RenderSettings)[]).every((name) => adv[name] === settings[name]));
  const applyLighting = ({ settings, altitude }: (typeof LIGHTING)[number]) => {
    setAdv(settings);
    sun.setSunMode("view");
    sun.setAlt(altitude);
    light.update({ enabled: false });
  };
  const applySoftbox = () => {
    const studio = LIGHTING[1];
    setAdv({ ...studio.settings, sun: 0, sky: 0.45 });
    sun.setSunMode("view");
    sun.setAlt(studio.altitude);
    light.studio();
  };
  const go = async (more = false) => {
    if (!canRender) return;
    stopRef.current = false;
    setBusy(true);
    const t0 = performance.now();
    let restart = !more || !samePicture;
    let goal = (restart ? 0 : picture?.done ?? 0) + quality.samples;
    let per = 1;
    try {
      while (!stopRef.current) {
        const reply = await exec({ cmd: "photoreal", samples: per, progressive: true, restart, ...settings });
        if (!reply.ok) {
          onToast(reply.error ?? "The render didn't work", true);
          break;
        }
        const done = reply.spp as number;
        // The engine may have switched scenes since the last picture.
        if (!restart && done === per) goal = quality.samples;
        restart = false;
        setPicture({ data: reply.data as string, key, done, goal, secs: (performance.now() - t0) / 1000, w, h });
        if (done >= goal) break;
        per = Math.max(1, Math.min(16, goal - done, Math.max(2, done)));
      }
    } catch (error) {
      onToast(error instanceof Error ? error.message : "The render didn't work", true);
    } finally {
      setBusy(false);
    }
  };
  const save = async () => {
    if (!picture) return;
    const path = await pickSave(`${name} render ${picture.w}x${picture.h}.png`, "png", "PNG image");
    if (!path) return;
    try {
      const r = await exec({ cmd: "save_image", path, data: picture.data });
      if (r.ok) onToast(`Saved ${path.split(/[\\/]/).pop()}`);
      else onToast(r.error ?? "Could not save the PNG", true);
    } catch (error) {
      onToast(error instanceof Error ? error.message : "Could not save the PNG", true);
    }
  };
  const dismiss = () => {
    stopRef.current = true;
    onClose();
  };
  useEscapeToClose(dismiss);
  return (
    <div className="sheet-backdrop" onClick={dismiss}>
      <div className={`sheet render-dlg${more ? " more" : ""}`} onClick={(e) => e.stopPropagation()} role="dialog" aria-label={import.meta.env.VITE_OFFICINA === "1" ? "Officina Render" : "Render"}>
        <SheetHead title={import.meta.env.VITE_OFFICINA === "1" ? "Officina Render" : "Render"} note={tt("Framing follows the current model view.", "L’inquadratura segue la vista corrente del modello.")} />
        <div className="render-view">
          {picture ? <img src={picture.data} alt="Render" /> : <div className="render-empty">{busy ? tt("Starting…", "Avvio…") : tt("Choose lighting and render the current view.", "Scegli l’illuminazione e renderizza la vista corrente.")}</div>}
          {picture && (
            <div className="render-progress" aria-live="polite">
              <i style={{ width: `${Math.min(100, (picture.done / picture.goal) * 100)}%` }} />
              <span>
                {!samePicture ? tt("Previous image", "Immagine precedente") : busy ? tt("Sharpening", "Riduzione del rumore") : picture.done >= picture.goal ? tt("Done", "Completato") : tt("Stopped", "Fermato")} · {tt(`${picture.done} of ${picture.goal} samples`, `${picture.done} di ${picture.goal} campioni`)} · {picture.secs.toFixed(1)} s
              </span>
            </div>
          )}
        </div>
        <div className="render-controls">
          <div className="seg render-looks" role="group" aria-label={tt("Lighting presets", "Preimpostazioni di illuminazione")}>
            {LIGHTING.map((look) => (
              <button key={look.id} className={activeLighting?.id === look.id ? "on" : ""}
                aria-pressed={activeLighting?.id === look.id} onClick={() => applyLighting(look)} disabled={busy}>
                {look.label}
              </button>
            ))}
            {studioFeature && <button className={panel && adv.sun === 0 ? "on" : ""} aria-pressed={!!panel && adv.sun === 0}
              onClick={applySoftbox} disabled={busy}>{tt("Softbox studio", "Studio con softbox")}</button>}
          </div>
          <div className="seg" role="group" aria-label={tt("Quality", "Qualità")}>
            {QUALITY.map((x) => (
              <button key={x.id} className={quality.id === x.id ? "on" : ""} aria-pressed={quality.id === x.id} onClick={() => setQuality(x)} title={x.about} disabled={busy}>
                {x.label}
              </button>
            ))}
          </div>
          <span className="muted small">{w} × {h} px</span>
          <SunControls sun={sun} busy={busy} artificial={artificial} />
          {studioFeature && <RenderLightControls model={light} camera={cam.current} busy={busy} invalid={invalidLight} />}
          {hasEmbedded && (
            <fieldset className="render-vray" disabled={busy || vray.enabled}>
              <label className="row gap">
                <input type="checkbox" checked={useEmbedded} onChange={(e) => setEmbeddedEnabled(e.target.checked)} />
                <span>{tt("Use V-Ray lighting from the SketchUp file", "Usa l’illuminazione V-Ray del file SketchUp")}</span>
              </label>
              <small className="muted">
                {vray.enabled
                  ? tt("The attached V-Ray scene replaces the embedded lighting.", "La scena V-Ray allegata sostituisce l’illuminazione incorporata.")
                  : tt(`${embeddedVray.area_lights.length} rectangular lights · approximate brightness`, `${embeddedVray.area_lights.length} luci rettangolari · intensità approssimata`)}
              </small>
              {Object.keys(embeddedVray.skipped).length > 0 && (
                <details className="render-vray-details">
                  <summary>{tt("Lights omitted", "Luci omesse")}</summary>
                  <ul>{Object.entries(embeddedVray.skipped).map(([kind, count]) => <li key={kind}>{count} × {vraySkipReason(kind)}</li>)}</ul>
                </details>
              )}
            </fieldset>
          )}
          {studioFeature && <VrayLightControls vray={vray} busy={busy} used={imported.length} />}
          <button className="btn small ghost render-more-toggle" onClick={() => setMore((m) => !m)} aria-expanded={more}>
            {more ? "▾" : "▸"} {tt("More settings", "Altre impostazioni")}
          </button>
          {more && <MoreSettings adv={adv} setAdv={setAdv} busy={busy} importedSunOff={importedSunOff} />}
          <div className="row gap">
            {busy ? (
              <button className="btn small" onClick={() => (stopRef.current = true)} title="Keep the picture as it is now">
                {tt("Stop", "Interrompi")}
              </button>
            ) : (
              <>
                <button className="btn small" onClick={() => go(false)} disabled={!canRender}>
                  {picture ? tt("Render again", "Renderizza di nuovo") : tt("Render", "Renderizza")}
                </button>
                {samePicture && (
                  <button className="btn small ghost" onClick={() => go(true)} disabled={!canRender} title="Keep adding samples to this picture: less grain, finer shadows">
                    {tt("Sharpen more", "Riduci ancora il rumore")}
                  </button>
                )}
              </>
            )}
            <button className="btn small ghost" onClick={save} disabled={!picture || busy}>
              {tt("Save PNG…", "Salva PNG…")}
            </button>
            <button
              className="btn small ghost"
              onClick={() => {
                stopRef.current = true;
                onClose();
              }}
            >
              {tt("Close", "Chiudi")}
            </button>

          </div>
        </div>
      </div>
    </div>
  );
}
