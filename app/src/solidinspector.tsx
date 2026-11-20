import { useEffect, useState } from "react";
import { tt } from "./i18n";
import { exec, type Scene, type Vec3 } from "./engine";
import { formatVolume } from "./panelkit";

export type IssueEdge = [Vec3, Vec3];

const findings = {
  no_faces: [tt("No visible faces", "Nessuna faccia visibile"), tt("Show the faces or open the assembly and choose a part.", "Mostra le facce oppure apri l’assieme e seleziona un pezzo.")],
  open_edges: [tt("Open edges", "Spigoli aperti"), tt("Close missing faces along the highlighted boundary.", "Chiudi le facce mancanti lungo il contorno evidenziato.")],
  nonmanifold_edges: [tt("Edges shared by more than two faces", "Spigoli condivisi da più di due facce"), tt("Check for a face inside the part or solids that touch along this edge.", "Controlla se ci sono facce interne o solidi a contatto lungo lo spigolo.")],
  loose_edges: [tt("Loose edges", "Spigoli liberi"), tt("Check whether these lines should belong to a face or stay outside the part.", "Controlla se le linee devono appartenere a una faccia o stare fuori dal pezzo.")],
  inconsistent_faces: [tt("Inconsistent face directions", "Orientamento delle facce incoerente"), tt("Open the part and orient the adjoining faces consistently.", "Apri il pezzo e orienta tutte le facce adiacenti in modo coerente.")],
  invalid_shell_orientation: [tt("Invalid shell orientation or volume", "Orientamento o volume del guscio non valido"), tt("Check that outside faces face outward and cavity faces face into the cavity.", "Controlla che le facce esterne puntino verso l’esterno e quelle interne verso la cavità.")],
  attached_objects: [tt("Attached groups or components", "Gruppi o componenti collegati"), tt("Trim keeps these when cutting this group. Use a single solid as the cutter.", "La ferramenta rimane collegata durante il taglio. Usa un solo solido come utensile.")],
  nested_component: [tt("Nested objects in this component", "Oggetti annidati nel componente"), tt("Open the component and choose the individual solid to trim.", "Apri il componente e seleziona il singolo solido da tagliare.")],
} as const;

type Finding = { kind: keyof typeof findings; count: number; edges: IssueEdge[] };
type Inspection = { volume: number | null; findings: Finding[] };

export function SolidInspector({ id, scene, locate, automatic = false }: { id: number; scene: Scene; locate: (edge: IssueEdge | null) => void; automatic?: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const [answer, setAnswer] = useState<{ scene: Scene; report?: Inspection; error?: string } | null>(null);
  const inspect = automatic || expanded;
  useEffect(() => {
    if (!inspect) return;
    let current = true;
    exec({ cmd: "solid_inspect", id }).then((reply) => {
      if (!current) return;
      if (reply.ok) {
        setAnswer({ scene, report: { volume: reply.volume as Inspection["volume"], findings: reply.findings as Finding[] } });
      } else {
        setAnswer({ scene, error: reply.error ?? tt("Could not inspect this part.", "Impossibile ispezionare il pezzo.") });
      }
    }).catch(() => {
      if (current) setAnswer({ scene, error: tt("Could not inspect this part.", "Impossibile ispezionare il pezzo.") });
    });
    return () => {
      current = false;
      locate(null);
    };
  }, [inspect, id, scene, locate]);

  const result = answer?.scene === scene ? answer : null;
  return (
    <div className="solidInspector">
      {automatic && (
        !result ? <p className="muted small" role="status">{tt("Checking solid…", "Verifica del solido…")}</p> :
        result.error ? <p className="attr-err" role="status">{result.error}</p> :
        result.report?.volume != null ? (
          <div className="kv solid-summary" role="status">
            <span>{result.report.findings.length ? tt("Shell volume", "Volume del guscio") : "Volume"}</span>
            <b>{formatVolume(result.report.volume, scene.units)}</b>
          </div>
        ) : <p className="muted small" role="status">{tt("Not a closed solid", "Il solido non è chiuso")}</p>
      )}
      <button className="btn small" aria-expanded={expanded} onClick={() => {
        if (!automatic) setAnswer(null);
        if (expanded) locate(null);
        setExpanded(!expanded);
      }}>
        {expanded ? tt("Close Solid Inspector", "Chiudi ispettore solidi") : tt("Inspect solid", "Ispeziona solido")}
      </button>
      {expanded && (!result ? <p className="muted small" role="status">{tt("Inspecting…", "Ispezione in corso…")}</p> : result.error ? (
        <p className="attr-err" role="alert">{result.error}</p>
      ) : result.report ? (
        <>
          {!automatic && <p className="small" role="status">
            {result.report.volume == null ? tt("Issues found in the visible shell.", "Controlla i problemi del guscio visibile.") : `Closed shell · ${formatVolume(result.report.volume, scene.units)}`}
          </p>}
          {automatic && !result.report.findings.length && <p className="small">{tt("Closed solid", "Solido chiuso")}</p>}
          {result.report.findings.map((finding) => {
            const [label, advice] = findings[finding.kind];
            const showCount = finding.edges.length > 0 || finding.kind === "attached_objects" || finding.kind === "nested_component";
            return (
              <div className="solidFinding" key={finding.kind}>
                <b>{showCount ? `${finding.count} · ` : ""}{label}</b>
                <p className="muted small">{advice}</p>
                {finding.edges.length > 0 && <div className="solidFindingLocations">
                  {finding.edges.map((edge, i) => (
                    <button className="btn small ghost" key={i} onClick={() => locate(edge)}
                      aria-label={`Locate ${label.toLowerCase()}, example ${i + 1}`}>
                      {finding.edges.length === 1 ? tt("Locate", "Mostra") : `Locate ${i + 1}`}
                    </button>
                  ))}
                  {finding.count > finding.edges.length && <span className="muted small">Showing {finding.edges.length} locations.</span>}
                </div>}
              </div>
            );
          })}
        </>
      ) : null)}
    </div>
  );
}
