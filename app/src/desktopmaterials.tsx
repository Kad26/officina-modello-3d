import { useId, useState, type CSSProperties, type ReactNode } from "react";
import type { MaterialInfo, Vec3 } from "./engine";
import { tt } from "./i18n";

export type MaterialValues = {
  color: Vec3;
  roughness: number;
  metalness: number;
  glass: boolean;
  emission: number;
};

const COLOR_NAMES: Record<string, [english: string, italian: string]> = {
  "#f2f0e8": ["Chalk", "Gesso"], "#eddbb8": ["Light oak", "Rovere chiaro"], "#f5ccb3": ["Peach", "Pesca"],
  "#d1e0cc": ["Pale sage", "Salvia chiara"], "#c7d9ed": ["Sky blue", "Azzurro cielo"], "#ded1eb": ["Lilac", "Lilla"],
  "#faed9e": ["Butter yellow", "Giallo burro"], "#c7c7c2": ["Pebble grey", "Grigio pietra"], "#b8292e": ["Brick red", "Rosso mattone"],
  "#73472e": ["Walnut", "Noce"], "#6b8ca8": ["Slate blue", "Blu ardesia"], "#6b995c": ["Leaf green", "Verde foglia"],
  "#8c9199": ["Steel grey", "Grigio acciaio"], "#525761": ["Graphite", "Grafite"], "#292b33": ["Charcoal", "Antracite"],
  "#f7f7f2": ["Ivory", "Avorio"], "#d98c6b": ["Terracotta", "Terracotta"], "#99a880": ["Olive green", "Verde oliva"],
};

function toHex(color: Vec3) {
  return "#" + color.map((n) => Math.round(Math.min(1, Math.max(0, n)) * 255).toString(16).padStart(2, "0")).join("");
}

function toColor(hex: string): Vec3 {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255) as Vec3;
}

function colorName(hex: string) {
  const names = COLOR_NAMES[hex];
  return names ? tt(...names) : `${tt("Color", "Colore")} ${hex.toUpperCase()}`;
}

function materialName(material: MaterialInfo) {
  return COLOR_NAMES[material.name] ? colorName(material.name) : material.name;
}

function MaterialCube({ color, image }: { color: string; image?: string }) {
  const style = {
    "--material-color": color,
    "--material-picture": image?.startsWith("data:image/") ? `url("${image}")` : undefined,
  } as CSSProperties;
  return (
    <span className="material-cube" style={style} aria-hidden="true">
      <i className="material-cube-top" />
      <i className="material-cube-left" />
      <i className="material-cube-right" />
    </span>
  );
}

function MaterialEditor({ color, material, onColor, onEditMaterial }: {
  color: Vec3;
  material?: MaterialInfo;
  onColor: (value: Vec3) => void;
  onEditMaterial?: (name: string, values: MaterialValues) => Promise<boolean>;
}) {
  const [hex, setHex] = useState(toHex(color));
  const [roughness, setRoughness] = useState(material?.roughness ?? 0.5);
  const [metalness, setMetalness] = useState(material?.metalness ?? 0);
  const [glass, setGlass] = useState(!!material?.glass);
  const [emission, setEmission] = useState(material?.emission ?? 0);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  const namedEditor = !!material && !!onEditMaterial;
  const save = async () => {
    const nextColor = toColor(hex);
    if (!material || !onEditMaterial) {
      onColor(nextColor);
      return;
    }
    setSaving(true);
    setFailed(false);
    try {
      setFailed(!(await onEditMaterial(material.name, { color: nextColor, roughness, metalness, glass, emission })));
    } catch {
      setFailed(true);
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="desktop-material-editor">
      <label className="desktop-material-color-field">
        <span>{tt("Color", "Colore")}</span>
        <input type="color" value={hex} onChange={(e) => setHex(e.target.value)} />
        <span className="desktop-material-value" aria-hidden="true">{hex.toUpperCase()}</span>
      </label>
      {namedEditor && (
        <>
          <label>
            <span>{tt("Roughness", "Rugosità")} <span className="desktop-material-value" aria-hidden="true">{Math.round(100 * roughness)}%</span></span>
            <input type="range" min="0" max="1" step="0.01" value={roughness} aria-valuetext={`${Math.round(100 * roughness)}%`} onChange={(e) => setRoughness(e.target.valueAsNumber)} />
          </label>
          <label>
            <span>{tt("Metalness", "Metallicità")} <span className="desktop-material-value" aria-hidden="true">{Math.round(100 * metalness)}%</span></span>
            <input type="range" min="0" max="1" step="0.01" value={metalness} aria-valuetext={`${Math.round(100 * metalness)}%`} onChange={(e) => setMetalness(e.target.valueAsNumber)} />
          </label>
          <label>
            <span>{tt("Emission", "Emissione")} <span className="desktop-material-value" aria-hidden="true">{emission}</span></span>
            <input type="range" min="0" max="100" step="0.1" value={emission} onChange={(e) => setEmission(e.target.valueAsNumber)} />
          </label>
          <label className="desktop-material-check">
            <input type="checkbox" checked={glass} onChange={(e) => setGlass(e.target.checked)} />
            <span>{tt("Glass", "Vetro")}</span>
          </label>
        </>
      )}
      <button className="btn small" disabled={saving} onClick={save}>
        {namedEditor ? tt("Apply to material", "Applica al materiale") : tt("Use this color", "Usa questo colore")}
      </button>
      {failed && <span role="alert">{tt("Could not change this material.", "Modifica del materiale non riuscita.")}</span>}
    </div>
  );
}

export function DesktopMaterials({
  colors, color, paintMaterial, onColor, materials, onMaterial, onEditMaterial, children,
}: {
  colors: Vec3[];
  color: Vec3;
  paintMaterial: string | null;
  onColor: (value: Vec3) => void;
  materials?: MaterialInfo[];
  onMaterial?: (name: string) => void;
  onEditMaterial?: (name: string, values: MaterialValues) => Promise<boolean>;
  children: ReactNode;
}) {
  const id = useId();
  const [tab, setTab] = useState<"select" | "edit">("select");
  const [source, setSource] = useState<"colors" | "model">("colors");
  const selected = materials?.find((m) => m.name === paintMaterial);
  const current = selected?.color ?? color;
  const currentHex = toHex(current);
  const title = selected ? materialName(selected) : paintMaterial ?? colorName(currentHex);
  const catalog = !!materials && !!onMaterial;
  const modelMaterials = materials ?? [];
  return (
    <div className="desktop-materials" data-material-tab={tab} data-material-catalog={catalog}>
      <header className="desktop-material-preview">
        <MaterialCube color={currentHex} image={selected?.texture?.data} />
        <div>
          <div className="muted small">{tt("Active material", "Materiale attivo")}</div>
          <strong title={title}>{title}</strong>
          <span className="muted small">{currentHex.toUpperCase()}</span>
        </div>
      </header>
      <div className="desktop-material-tabs" role="tablist" aria-label={tt("Materials", "Materiali")}>
        {(["select", "edit"] as const).map((name) => (
          <button
            key={name} type="button" id={`${id}-${name}`} role="tab" aria-selected={tab === name}
            tabIndex={tab === name ? 0 : -1} aria-controls={`${id}-panel`}
            onClick={() => setTab(name)}
            onKeyDown={(e) => {
              if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
              const next = name === "select" ? "edit" : "select";
              e.preventDefault();
              e.stopPropagation();
              setTab(next);
              document.getElementById(`${id}-${next}`)?.focus();
            }}
          >
            {name === "select" ? tt("Select", "Seleziona") : tt("Edit", "Modifica")}
          </button>
        ))}
      </div>
      <div id={`${id}-panel`} role="tabpanel" aria-labelledby={`${id}-${tab}`}>
        {tab === "select" ? (
          <>
            {catalog && (
              <label className="desktop-material-source">
                <span>{tt("Collection", "Raccolta")}</span>
                <select value={source} onChange={(e) => setSource(e.target.value === "model" ? "model" : "colors")}>
                  <option value="colors">{tt("Colors", "Colori")}</option>
                  <option value="model">{tt("In model", "Nel modello")}</option>
                </select>
              </label>
            )}
            <div className="desktop-material-grid" aria-label={tt("Material swatches", "Campioni materiali")}>
              {source === "model" && catalog ? (
                modelMaterials.length > 0 ? modelMaterials.map((m) => {
                  const name = materialName(m);
                  return (
                    <button key={m.name} className="desktop-material-swatch" aria-pressed={m.name === paintMaterial} title={name} aria-label={name} onClick={() => onMaterial?.(m.name)}>
                      <MaterialCube color={toHex(m.color)} image={m.texture?.data} />
                    </button>
                  );
                }) : <p className="muted small">{tt("There are no materials in this model yet.", "Questo modello non contiene ancora materiali.")}</p>
              ) : colors.map((value) => {
                const hex = toHex(value);
                const name = colorName(hex);
                return (
                  <button key={hex} className="desktop-material-swatch" aria-pressed={!paintMaterial && hex === currentHex} title={name} aria-label={name} onClick={() => onColor(value)}>
                    <MaterialCube color={hex} />
                  </button>
                );
              })}
            </div>
          </>
        ) : (
          <MaterialEditor
            key={[paintMaterial, currentHex, selected?.roughness, selected?.metalness, selected?.glass, selected?.emission].join(":")}
            color={current} material={selected} onColor={onColor} onEditMaterial={onEditMaterial}
          />
        )}
        {children}
      </div>
    </div>
  );
}
