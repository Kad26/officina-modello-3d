// What to do next, for every tool and every step of it. The viewport shows
// this beside the model, so a tool explains itself while you use it.

import type { Tool } from "./Viewport";
import { copyModifier, desktopShortcut, optionModifier, sketchUpWindowsKeys } from "./shortcut-labels";
import { tr, tt } from "./i18n";

export interface Guide {
  /** the tool's name */
  title: string;
  /** the one thing to do now */
  step: string;
  /** keys that matter right now: [key, what it does] */
  keys: [string, string][];
}

export interface GuideCtx {
  /** the step in progress (Op.t), and a detail for multi-click tools */
  op: string | null;
  stage?: number;
  selected: number;
  placing?: string | null;
  tapeMode?: TapeMode;
}

export type TapeMode = "line" | "point" | "measure";

export function tapeModeLabel(mode: TapeMode): string {
  if (mode === "line") return tt("Guide Line", "Linea guida");
  if (mode === "point") return tt("Guide Point", "Punto guida");
  return tt("Measure", "Misura");
}

const TYPE: [string, string] = ["Type", "an exact size, then Enter"];
const LOCK: [string, string] = ["→ ← ↑", "lock to red / green / blue"];
const SHIFT: [string, string] = ["Hold ⇧", "lock this direction"];
const ESC: [string, string] = ["Esc", "cancel"];

const IT_STEP: Record<string, string> = {
  "Click something to select it, or drag a box around things.": "Fai clic o trascina per selezionare. Maiusc aggiunge o rimuove dalla selezione.",
  "Release to pick. Left-to-right takes what's inside the box; right-to-left, what it touches.": "Rilascia per selezionare. Da sinistra a destra solo gli oggetti interni, da destra a sinistra anche quelli attraversati.",
  "Drag a loop or click an empty point to start a polygon.": "Trascina per tracciare il contorno oppure fai clic in uno spazio vuoto per iniziare un poligono.",
  "Click to add a corner. Double-click to select.": "Fai clic per aggiungere un vertice. Fai doppio clic per selezionare.",
  "Release to select. Clockwise includes only fully enclosed items.": "Rilascia per selezionare. In senso orario selezioni solo gli elementi completamente racchiusi.",
  "Release to select. Counterclockwise includes only fully enclosed items.": "Rilascia per selezionare. In senso antiorario selezioni solo gli elementi completamente racchiusi.",
  "Click where the line starts.": "Fai clic sul punto iniziale della linea.",
  "Click the next point. Close the shape to make a face.": "Indica il punto successivo. Chiudi il contorno per creare una faccia.",
  "Click the opposite corner.": "Indica l’angolo opposto.",
  "Click the first corner, on the ground or on a face.": "Indica il primo angolo sul terreno o su una faccia.",
  "Click to set the radius.": "Fai clic per impostare il raggio.",
  "Click the center, on the ground or on a face.": "Indica il centro sul terreno o su una faccia.",
  "Click to set its size.": "Fai clic per impostare la dimensione.",
  "Click the center.": "Indica il centro.",
  "Click where it starts (that sets the radius).": "Indica il punto iniziale per fissare il raggio.",
  "Swing round and click where it ends.": "Ruota e indica il punto finale.",
  "Click where the arc starts.": "Indica il punto iniziale dell’arco.",
  "Click a point it passes through.": "Indica un punto attraversato dall’arco.",
  "Click where it ends.": "Indica il punto finale.",
  "Click the first corner.": "Indica il primo angolo.",
  "Click the end of the first side (any direction, even up a slope).": "Indica la fine del primo lato, anche in pendenza.",
  "Swing out the other side and click.": "Indica la larghezza del secondo lato.",
  "Click the new origin (a corner of a building set at an angle, say).": "Indica la nuova origine degli assi.",
  "Click along the new red direction.": "Indica la direzione del nuovo asse rosso.",
  "Click toward the new green side.": "Indica la direzione del nuovo asse verde.",
  "Drag to draw. End where you began to close it into a face.": "Trascina per disegnare. Torna al punto iniziale per creare una faccia.",
  "Pull out to bend it, then click.": "Regola la curvatura e fai clic.",
  "Move to pull the face out or push it in, then click.": "Sposta il puntatore per estrudere la faccia e fai clic. Puoi digitare la distanza.",
  "Click a face to pull it into 3D.": "Fai clic su una faccia da estrudere.",
  "Move in or out, then click.": "Spostati verso l’interno o l’esterno, poi fai clic.",
  "Click a face to draw a copy of its outline inside (or outside) it.": "Seleziona la faccia di cui vuoi duplicare il contorno.",
  "Select the path first (edges, or a face to go around), then click the profile face to sweep it along.": "Seleziona il percorso, poi fai clic sulla faccia del profilo da estrudere lungo il percorso.",
  "Click where it goes.": "Indica la destinazione.",
  "Click a point on the selection to pick it up there.": "Indica il punto di presa sulla selezione.",
  "Click something to grab it, or a corner to move just that point; the faces around it follow.": "Seleziona un elemento da spostare oppure un vertice per spostare solo quel punto.",
  "Click the center of rotation (on a face, it turns in that face's plane).": "Indica il centro di rotazione, anche sul piano di una faccia.",
  "Click a point to start the angle from.": "Indica la direzione iniziale.",
  "Swing it around and click.": "Ruota e fai clic per confermare.",
  "Move to resize, then click.": "Ridimensiona e fai clic per confermare.",
  "Click to start resizing the selection.": "Fai clic per ridimensionare la selezione.",
  "Click something to resize it.": "Seleziona un elemento da ridimensionare.",
  "Click to drop a guide here (or read the distance).": "Fai clic per posizionare una guida o leggi la distanza.",
  "Click a point or an edge to measure from.": "Indica il punto o lo spigolo da cui misurare.",
  "Click the first point to measure from.": "Indica il primo punto della quota.",
  "Click the second point.": "Indica il secondo punto.",
  "Pull the dimension out and click to place it.": "Sposta la quota e fai clic per posizionarla.",
  "Click where the label goes.": "Indica dove posizionare l’etichetta.",
  "Click the point the label is about.": "Indica il punto a cui si riferisce l’etichetta.",
  "Click a face or a group to paint it with the color (or texture) on the right.": "Fai clic su una faccia o su un gruppo per applicare il materiale selezionato.",
  "Click an edge, face or group to delete it. Erasing an edge between two faces merges them.": "Fai clic su uno spigolo, una faccia o un gruppo per eliminarlo.",
  "Drag to turn the view around the model.": "Trascina per orbitare intorno al modello.",
  "Drag to slide the view.": "Trascina per spostare la vista.",
  "Drag up to zoom in, down to zoom out.": "Trascina verso l’alto per ingrandire, verso il basso per ridurre.",
  "Drag a box around what you want to fill the view.": "Traccia un riquadro intorno alla zona che vuoi ingrandire.",
  "Click where to stand: you'll be there at eye height, looking the way you were.": "Indica la posizione da cui osservare il modello all’altezza degli occhi.",
  "Drag to turn your head; you stay where you are.": "Trascina per cambiare la direzione dello sguardo.",
  "Drag up to walk forward, down to back up, sideways to turn. Or hold the arrow keys / WASD.": "Trascina per camminare oppure usa le frecce o i tasti WASD.",
  "Click a face: the model is cut just behind it so you can see inside. For floor plans, use Plan in the view bar.": "Seleziona una faccia per posizionare un piano di sezione e vedere l’interno del modello.",
  "Swing to the angle and click: a guide line is laid there (15° steps snap).": "Indica l’angolo. La guida si aggancia a incrementi di 15°.",
  "Click a point on the baseline the angle starts from.": "Indica un punto sulla direzione di partenza.",
  "Click the corner the angle turns around (on a face to measure in its plane).": "Indica il vertice da cui misurare l’angolo.",
  "Click the next corner. Click the first point to close a room.": "Indica l’angolo successivo. Torna al primo per chiudere la stanza.",
  "Click where the wall starts. Thickness and height are in the Library tab.": "Indica l’inizio della parete. Spessore e altezza sono nel pannello Componenti.",
  "Click the direction the stairs climb.": "Indica la direzione di salita.",
  "Click where the bottom step starts.": "Indica l’inizio del primo gradino.",
  "Click a flat top face, like the top of your walls.": "Indica una faccia orizzontale in sommità.",
  "Click to drop a column.": "Fai clic per posizionare la colonna.",
  "Click the terrain to raise it there.": "Seleziona un punto del terreno da sollevare.",
  "Select a footprint face first, then click the terrain to flatten a pad under it.": "Seleziona la faccia di ingombro, poi fai clic sul terreno per spianarlo.",
  "Select lines first, then click the terrain to lay them on it.": "Seleziona le linee, poi fai clic sul terreno per proiettarle.",
  "Click a diagonal between two triangles to swap it.": "Fai clic sulla diagonale tra due triangoli per scambiarla.",
};

export function guide(tool: Tool, c: GuideCtx): Guide {
  const g = (title: string, step: string, keys: [string, string][] = []): Guide => ({
    title: tr(title),
    step: Object.hasOwn(IT_STEP, step) ? tt(step, IT_STEP[step]) : step,
    keys: keys.map(([key, action]) => [desktopShortcut(key), action]),
  });
  const copyKey = copyModifier();
  const busy = c.op != null;
  switch (tool) {
    case "select":
      if (c.op === "box") return g("Select", "Release to pick. Left-to-right takes what's inside the box; right-to-left, what it touches.", [["⇧", "add or remove"]]);
      return c.selected
        ? g("Select", tt(`${c.selected} selected. M moves, Q rotates, S scales, Delete erases; right-click for more.`, `${c.selected} in selezione. M sposta, Q ruota, S scala, Canc elimina.`), [["⇧ click", "add or remove"], ["Double-click", "open a group"], ["⌘T", "select none"]])
        : g("Select", "Click something to select it, or drag a box around things.", [["⇧ click", "add or remove"], ["Double-click", "open a group"], ["Space", "back here from any tool"]]);
    case "lasso":
      if (!busy) return g("Lasso Select", "Drag a loop or click an empty point to start a polygon.");
      if (c.stage === 2) return g("Lasso Select", "Click to add a corner. Double-click to select.", [ESC]);
      return g("Lasso Select", sketchUpWindowsKeys
        ? "Release to select. Clockwise includes only fully enclosed items."
        : "Release to select. Counterclockwise includes only fully enclosed items.", [ESC]);
    case "line":
      return busy ? g("Line", "Click the next point. Close the shape to make a face.", [TYPE, LOCK, SHIFT, ["Esc", "stop drawing"]]) : g("Line", "Click where the line starts.", [LOCK]);
    case "rect":
      return busy ? g("Rectangle", "Click the opposite corner.", [["Type", "width,depth (12',8') then Enter"], ESC]) : g("Rectangle", "Click the first corner, on the ground or on a face.", []);
    case "circle":
      return busy ? g("Circle", "Click to set the radius.", [["Type", "a radius, or 24s for 24 sides"], ESC]) : g("Circle", "Click the center, on the ground or on a face.", []);
    case "polygon":
      return busy ? g("Polygon", "Click to set its size.", [["Type", "a radius, or 8s for 8 sides"], ESC]) : g("Polygon", "Click the center.", []);
    case "arccenter":
    case "pie": {
      const name = tool === "pie" ? "Pie" : "Arc";
      if (!busy) return g(name, "Click the center, on the ground or on a face.", []);
      return c.stage === 1 ? g(name, "Click where it starts (that sets the radius).", [ESC]) : g(name, "Swing round and click where it ends.", [["Type", "the angle in degrees"], ESC]);
    }
    case "arc3":
      if (!busy) return g("3-Point Arc", "Click where the arc starts.", []);
      return c.stage === 1 ? g("3-Point Arc", "Click a point it passes through.", [LOCK, ESC]) : g("3-Point Arc", "Click where it ends.", [ESC]);
    case "rrect":
      if (!busy) return g("Rotated Rectangle", "Click the first corner.", []);
      return c.stage === 1 ? g("Rotated Rectangle", "Click the end of the first side (any direction, even up a slope).", [LOCK, ESC]) : g("Rotated Rectangle", "Swing out the other side and click.", [["Type", "its length"], ESC]);
    case "axes":
      if (!busy) return g("Axes", "Click the new origin (a corner of a building set at an angle, say).", []);
      return c.stage === 1 ? g("Axes", "Click along the new red direction.", [LOCK, ESC]) : g("Axes", "Click toward the new green side.", [ESC]);
    case "freehand":
      return g("Freehand", "Drag to draw. End where you began to close it into a face.", []);
    case "arc":
      if (!busy) return g("Arc", "Click where the arc starts.", []);
      return c.stage === 1 ? g("Arc", "Click where it ends.", [TYPE, LOCK, ESC]) : g("Arc", "Pull out to bend it, then click.", [["Type", "how far it bows out"], ESC]);
    case "push": {
      const modes: [string, string][] = sketchUpWindowsKeys ? [
        ["Ctrl", tt("keep the starting face", "conserva la faccia di partenza")],
        ["Alt", tt("stretch a preselected face", "stira una faccia preselezionata")],
      ] : [];
      const start = sketchUpWindowsKeys && c.selected === 1
        ? tt("Click anywhere to start on the selected face.", "Fai clic ovunque per iniziare dalla faccia selezionata.")
        : "Click a face to pull it into 3D.";
      return busy ? g("Push/Pull", "Move to pull the face out or push it in, then click.", [["Type", "a distance, then Enter"], ...modes, ESC]) : g("Push/Pull", start, [["Double-click", "repeat the last distance"], ...modes]);
    }
    case "offset":
      return busy ? g("Offset", "Move in or out, then click.", [TYPE, ESC]) : g("Offset", "Click a face to draw a copy of its outline inside (or outside) it.", []);
    case "followme":
      return g("Follow Me", "Select the path first (edges, or a face to go around), then click the profile face to sweep it along.", []);
    case "move":
      if (busy) return g("Move", "Click where it goes.", [["Type", "a distance, then Enter"], [copyKey, "leave a copy (then type x5)"], LOCK, ESC]);
      return g("Move", c.selected ? "Click a point on the selection to pick it up there." : "Click something to grab it, or a corner to move just that point; the faces around it follow.", [[copyKey, "copy instead of move"]]);
    case "rotate":
      if (!busy) return g("Rotate", "Click the center of rotation (on a face, it turns in that face's plane).", [[copyKey, "rotate a copy"]]);
      return c.stage === 1 ? g("Rotate", "Click a point to start the angle from.", [ESC]) : g("Rotate", "Swing it around and click.", [["Type", "degrees, then Enter"], ESC]);
    case "scale":
      return busy ? g("Scale", "Move to resize, then click.", [["Type", "a factor (2 doubles it)"], ESC]) : g("Scale", c.selected ? "Click to start resizing the selection." : "Click something to resize it.", []);
    case "tape": {
      const cycle: [string, string] = ["Ctrl", tt("cycle modes", "cambia modalità")];
      if (c.tapeMode === "line") return busy
        ? g("Tape Measure", tt("Move perpendicular to the edge. Click or enter an offset to place the guide.", "Spostati perpendicolarmente allo spigolo. Fai clic o digita la distanza per posizionare la guida."), [cycle, TYPE, LOCK, ESC])
        : g("Tape Measure", tt("Guide Line: click an edge or a guide to offset it.", "Linea guida: fai clic su uno spigolo o su una guida per crearne una parallela."), [cycle]);
      if (c.tapeMode === "point") return g("Tape Measure", tt("Guide Point: click to place a point.", "Punto guida: fai clic per posizionare un punto."), [cycle]);
      if (c.tapeMode === "measure") return busy
        ? g("Tape Measure", tt("Click the end point to read the distance.", "Fai clic sul punto finale per leggere la distanza."), [cycle, LOCK, ESC])
        : g("Tape Measure", tt("Measure: click the start point. Enter the real length after measuring to resize the model.", "Misura: fai clic sul punto iniziale. Dopo la misura digita la lunghezza reale per ridimensionare il modello."), [cycle]);
      return busy ? g("Tape Measure", "Click to drop a guide here (or read the distance).", [TYPE, LOCK, ESC]) : g("Tape Measure", "Click a point or an edge to measure from.", [["After measuring", "type the real length to resize the model"]]);
    }
    case "dimension":
      if (!busy) return g("Dimension", "Click the first point to measure from.", []);
      return c.stage === 1 ? g("Dimension", "Click the second point.", [LOCK, ESC]) : g("Dimension", "Pull the dimension out and click to place it.", [ESC]);
    case "text":
      return busy ? g("Text", "Click where the label goes.", [ESC]) : g("Text", "Click the point the label is about.", []);
    case "paint":
      if (sketchUpWindowsKeys) return g("Paint Bucket", tt("Click to paint. Alt samples · Ctrl connected · Shift all · Ctrl+Shift same object.", "Fai clic per dipingere. Alt campiona · Ctrl facce connesse · Maiusc tutte · Ctrl+Maiusc stesso oggetto."), []);
      return g("Paint Bucket", "Click a face or a group to paint it with the color (or texture) on the right.", [[`${optionModifier()}+click`, "pick up a color"], ["⇧ click", "paint every matching face"]]);
    case "erase":
      if (sketchUpWindowsKeys) return g("Eraser", tt(
        "Click to erase. Tap a modifier to keep its mode, or hold it for a single click.",
        "Fai clic per cancellare. Premi e rilascia un tasto per mantenere la modalità, oppure tienilo premuto durante il clic."
      ), [["Ctrl", tt("soften", "ammorbidisci")], ["Alt", tt("unsoften", "rimuovi ammorbidimento")], ["⇧", tt("hide", "nascondi")]]);
      return g("Eraser", "Click an edge, face or group to delete it. Erasing an edge between two faces merges them.", [["⇧ click", "hide instead"], [copyKey, "soften the edge"]]);
    case "igloo":
      return g("Image Igloo", tt(
        "Orbit to highlight a photo. Press Enter to open its scene.",
        "Orbita per evidenziare una foto. Premi Invio per aprire la sua scena.",
      ), [["← ↑ ↓ →", tt("change photo", "cambia foto")], ["Shift + ← →", tt("scene order", "ordine delle scene")], [`${copyKey} + ← →`, tt("same viewpoint", "stesso punto di ripresa")], ESC]);
    case "orbit":
      return g("Orbit", "Drag to turn the view around the model.", [["Middle-drag", "orbit from any tool"], ["⇧Z", "see everything"]]);
    case "pan":
      return g("Pan", "Drag to slide the view.", [["Right-drag", "pan from any tool"]]);
    case "zoom":
      return g("Zoom", "Drag up to zoom in, down to zoom out.", [["Scroll", "zoom from any tool"], ["⇧Z", "see everything"], ["Type", "a field of view: 35 (degrees) or 50mm"]]);
    case "protractor":
      if (c.op === "rotate" && c.stage === 2) return g("Protractor", "Swing to the angle and click: a guide line is laid there (15° steps snap).", [["Type", "an exact angle, then Enter"]]);
      if (c.op === "rotate") return g("Protractor", "Click a point on the baseline the angle starts from.", []);
      return g("Protractor", "Click the corner the angle turns around (on a face to measure in its plane).", [["Esc", "start over"]]);
    case "zoomwindow":
      return g("Zoom Window", "Drag a box around what you want to fill the view.", [["⌘[ / ⌘]", "previous / next view"]]);
    case "position":
      return g("Position Camera", "Click where to stand: you'll be there at eye height, looking the way you were.", [["Type", "an eye height (5'6\", 1.7)"]]);
    case "lookaround":
      return g("Look Around", "Drag to turn your head; you stay where you are.", [["Type", "an eye height"], ["⌘[", "back to the previous view"]]);
    case "walk":
      return g("Walk", "Drag up to walk forward, down to back up, sideways to turn. Or hold the arrow keys / WASD.", [["⇧", "run"], [optionModifier(), "walk through walls"], ["Type", "an eye height"]]);
    case "section":
      return g("Section", "Click a face: the model is cut just behind it so you can see inside. For floor plans, use Plan in the view bar.", []);
    case "place":
      return g("Place", tt(`Click to drop ${c.placing ? c.placing.replace(/^file:.*\//, "").replace(/^component:\d+$/, "a copy").replace(/\.\w+$/, "").replace(/_/g, " ") : "it"}. On a wall, doors and windows face out of it.`, "Fai clic per inserire il componente."), [["Esc", "done placing"]]);
    case "wall":
      return busy ? g("Wall", "Click the next corner. Click the first point to close a room.", [["Double-click / Enter", "finish"], TYPE, LOCK, ESC]) : g("Wall", "Click where the wall starts. Thickness and height are in the Library tab.", []);
    case "stairs":
      return busy ? g("Stairs", "Click the direction the stairs climb.", [ESC]) : g("Stairs", "Click where the bottom step starts.", []);
    case "roof":
      return g("Roof", "Click a flat top face, like the top of your walls.", []);
    case "column":
      return g("Column", "Click to drop a column.", []);
    case "smoove":
      return g("Smoove", "Click the terrain to raise it there.", [["⇧ click", "dig instead"]]);
    case "stamp":
      return g("Stamp", "Select a footprint face first, then click the terrain to flatten a pad under it.", []);
    case "drape":
      return g("Drape", "Select lines first, then click the terrain to lay them on it.", []);
    case "flipedge":
      return g("Flip Edge", "Click a diagonal between two triangles to swap it.", []);
  }
  return g(tool, "", []);
}
