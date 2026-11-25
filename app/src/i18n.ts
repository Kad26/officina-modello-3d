export const italian = import.meta.env.VITE_OFFICINA === "1" && import.meta.env.VITE_OFFICINA_LOCALE !== "en";

export function tt(english: string, translated: string): string {
  return italian ? translated : english;
}

const LABELS: Readonly<Record<string, string>> = {
  "File": "File", "Edit": "Modifica", "View": "Visualizza", "Camera": "Telecamera",
  "Draw": "Disegno", "Tools": "Strumenti", "Window": "Finestra", "Extensions": "Estensioni", "Help": "Guida",
  "Main menus": "Menu principale", "Default Tray": "Pannello predefinito", "Entity Info": "Informazioni entità",
  "Outliner": "Struttura", "Materials": "Materiali", "Components": "Componenti", "Styles": "Stili", "Tags": "Tag",
  "Scenes": "Scene", "Model Info": "Informazioni modello", "Library": "Libreria", "Properties": "Proprietà",
  "Undo": "Annulla", "Redo": "Ripeti", "Cut": "Taglia", "Copy": "Copia", "Paste": "Incolla",
  "Paste in Place": "Incolla sul posto", "Delete": "Elimina", "Select All": "Seleziona tutto",
  "Deselect All": "Deseleziona tutto", "Unhide All": "Mostra tutto", "Unhide all": "Mostra tutto",
  "Delete Guides": "Elimina guide", "Clear guides": "Elimina guide", "Soften flat seams": "Ammorbidisci spigoli complanari",
  "Edges": "Bordi", "Back Edges": "Bordi posteriori", "Back edges": "Bordi posteriori", "X-Ray": "Raggi X",
  "X-ray": "Raggi X", "Shadows": "Ombre", "Shaded with Textures": "Ombreggiato con texture",
  "Shaded with textures": "Ombreggiato con texture", "Shaded": "Ombreggiato", "Monochrome": "Monocromatico",
  "Hidden Line": "Linee nascoste", "Hidden line": "Linee nascoste", "Wireframe": "Reticolo",
  "Dimensions and Labels": "Quote ed etichette", "Tool Hints": "Istruzioni strumento",
  "Hide Rest of Model": "Nascondi il resto del modello", "Perspective": "Prospettiva",
  "Parallel Projection": "Proiezione parallela", "Parallel": "Parallela", "Notes": "Annotazioni",
  "Isometric": "Isometrica", "Top": "Alto", "Front": "Fronte", "Right": "Destra", "Back": "Retro", "Left": "Sinistra",
  "Zoom Extents": "Zoom estensioni", "Zoom to selection": "Zoom sulla selezione", "Previous View": "Vista precedente", "Next View": "Vista successiva",
  "Previous view": "Vista precedente", "Next view": "Vista successiva", "Plan": "Pianta",
  "Search Commands…": "Cerca comandi…", "Keyboard Shortcuts…": "Scorciatoie da tastiera…",
  "Learn SketchCraft…": "Impara SketchCraft…", "Customize Toolbar…": "Personalizza barre degli strumenti…",
  "Themes and Layout…": "Temi e disposizione…", "Manage Plugins…": "Gestisci estensioni…",
  "Manage plugins…": "Gestisci estensioni…", "About Officina Modello 3D": "Informazioni su Officina Modello 3D",
  "Model Workspace": "Area Modellazione", "Arch Workspace": "Area Architettura",
  "Design Workspace": "Area Progettazione", "Previz Workspace": "Area Presentazione",
  "Select": "Seleziona", "Lasso Select": "Selezione a lazo", "Eraser": "Gomma", "Paint Bucket": "Secchiello", "Line": "Linea",
  "Freehand": "Mano libera", "Rectangle": "Rettangolo", "Rotated Rectangle": "Rettangolo ruotato",
  "Circle": "Cerchio", "Polygon": "Poligono", "2-Point Arc": "Arco a 2 punti", "3-Point Arc": "Arco a 3 punti",
  "Arc from Center": "Arco dal centro", "Arc": "Arco", "Pie": "Settore circolare", "Move": "Sposta",
  "Rotate": "Ruota", "Scale": "Scala", "Push/Pull": "Spingi/tira", "Push / Pull": "Spingi/tira",
  "Follow Me": "Seguimi", "Offset": "Offset", "Tape Measure": "Metro", "Protractor": "Goniometro",
  "Axes": "Assi", "Dimension": "Quota", "Text": "Testo", "Text label": "Etichetta di testo",
  "3D Text…": "Testo 3D…", "Section Plane": "Piano di sezione", "Section cut": "Piano di sezione",
  "Orbit": "Orbita", "Pan": "Panoramica", "Zoom": "Zoom", "Zoom Window": "Finestra zoom",
  "Image Igloo": "Igloo immagini",
  "Position Camera": "Posiziona telecamera", "Look Around": "Guarda intorno", "Walk": "Cammina",
  "Wall": "Parete", "Stairs": "Scale", "Roof": "Tetto", "Column": "Colonna",
  "Make group": "Crea gruppo", "Group these (nest them)": "Raggruppa gli elementi",
  "Make component": "Crea componente", "Make unique": "Rendi unico",
  "Ungroup (keep what's inside)": "Separa gruppo", "Edit group": "Modifica gruppo", "Edit component": "Modifica componente",
  "Explode": "Esplodi", "Close editing": "Chiudi modifica", "Intersect Faces": "Interseca facce",
  "Reverse faces": "Inverti facce", "Orient faces": "Orienta facce", "Hide": "Nascondi", "Show": "Mostra",
  "Solid: Outer Shell": "Solidi: involucro esterno", "Solid: Union": "Solidi: unione", "Solid: Intersect": "Solidi: intersezione",
  "Solid: Subtract…": "Solidi: sottrai…", "Solid: Trim…": "Solidi: ritaglia…", "Solid: Split": "Solidi: dividi",
  "Outer Shell": "Involucro esterno", "Union": "Unione", "Intersect": "Interseca",
  "Subtract": "Sottrai", "Trim": "Ritaglia", "Split": "Dividi", "Value": "Misure",
  "Length": "Lunghezza", "Width": "Larghezza", "Height": "Altezza", "Direction": "Direzione",
  "Angle": "Angolo", "Dimensions": "Dimensioni", "Distance": "Distanza", "Red axis": "Asse rosso",
  "Radius": "Raggio", "Bulge": "Freccia dell’arco", "Area": "Area", "Volume": "Volume",
};

export function tr(label: string): string {
  return italian && Object.hasOwn(LABELS, label) ? LABELS[label] : label;
}
