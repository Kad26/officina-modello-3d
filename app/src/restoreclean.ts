// What the Restore dialog shows, kept apart from the window code so it can be checked (npm run check).

export interface Item {
  kind: "backup" | "unsaved" | "damaged";
  name: string;
  secs: number;
  size: number;
  label: string;
}

/** "3 min ago", "2 h ago", "yesterday"… for a time in seconds since 1970. */
export function ago(secs: number, now = Date.now() / 1000): string {
  const d = Math.max(0, now - secs);
  if (d < 90) return "just now";
  if (d < 3600) return `${Math.round(d / 60)} min ago`;
  if (d < 86400) return `${Math.round(d / 3600)} h ago`;
  if (d < 172800) return "yesterday";
  return `${Math.round(d / 86400)} days ago`;
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

/** Keep only well-formed rows (the list comes from the engine, but a row is data, never markup). */
export function cleanItems(raw: unknown): Item[] {
  if (!Array.isArray(raw)) return [];
  const out: Item[] = [];
  for (const r of raw.slice(0, 500)) {
    if (!r || typeof r !== "object") continue;
    const o = r as Record<string, unknown>;
    if (!["backup", "unsaved", "damaged"].includes(String(o.kind)) || typeof o.name !== "string" || typeof o.label !== "string") continue;
    out.push({ kind: o.kind as Item["kind"], name: o.name.slice(0, 200), secs: Number(o.secs) || 0, size: Number(o.size) || 0, label: o.label.slice(0, 80) });
  }
  return out;
}

