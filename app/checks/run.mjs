// Checks for the interface's security-relevant pure code: the skin value
// grammar, what plugins may hand the window, and the plugin worker lockdown.
//   cd app && npm run check
// Loads the real source through Vite (no server, no port).
import vm from "node:vm";
import { createServer } from "vite";

const server = await createServer({ configFile: false, server: { middlewareMode: true, hmr: false, ws: false }, appType: "custom", logLevel: "error", optimizeDeps: { noDiscovery: true } });
let failed = 0;
const check = (ok, what) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failed++;
};
const load = (p) => server.ssrLoadModule(p);

try {
  // ---- skins: each token takes a plain color, length or font list ----------------
  const { skinValueOk } = await load("/src/skinvalue.ts");
  const good = [
    ["bg", "#fff"], ["bg", "#12345678"], ["accent", "rgb(10, 20, 30)"], ["glass", "rgba(32,18,63,0.95)"], ["accent", "hsl(200 50% 40%)"], ["text", "white"],
    ["radius", "4px"], ["radius", "0"], ["radius", "0.5rem"], ["radius", "4px 8px"],
    ["font", `"Bricolage Grotesque Variable", system-ui, sans-serif`], ["font", "Georgia, serif"], ["font", `'IBM Plex Mono', monospace`],
  ];
  for (const [t, v] of good) check(skinValueOk(t, v), `skin ${t} accepts ${v}`);
  const bad = [
    ["bg", `image-set("https://example.com/x.png" 1x)`], ["bg", `-webkit-image-set("https://example.com/x.png" 1x)`], ["bg", "url(https://example.com/x.png)"], ["bg", "u\\72l(https://x/y)"],
    ["bg", "red; background: url(x)"], ["bg", "var(--x)"], ["bg", "expression(alert(1))"], ["bg", "#12"], ["bg", "rgb(1,2,3) url(x)"], ["bg", ""], ["bg", 5],
    ["radius", "calc(1px + 2px)"], ["radius", "4px url(x)"], ["radius", "red"],
    ["font", `"a", url(x)`], ["font", `"a\\"b"`], ["font", "a; b"], ["font", "a{b}"], ["font", `"x", image-set("https://x/y" 1x)`], ["accent", "a".repeat(300)],
  ];
  for (const [t, v] of bad) check(!skinValueOk(t, v), `skin ${t} refuses ${String(v).slice(0, 50)}`);
  const { SKINS, readSkin } = await load("/src/skins.ts");
  for (const s of SKINS) check(Object.entries(s.tokens).every(([k, v]) => skinValueOk(k, v)), `built-in skin ${s.name} passes the grammar`);
  const sk = readSkin({ name: "x", tokens: { bg: `image-set("https://example.com/x.png" 1x)`, text: "#000" } }, "x");
  check(sk && sk.tokens.bg === SKINS[0].tokens.bg && sk.tokens.text === "#000", "a shared skin's bad value falls back, good ones stay");

  // ---- the restore list: rows are data, never markup ---------------------------------
  const { cleanItems, ago, bytes } = await load("/src/restoreclean.ts");
  const rows = cleanItems([{ kind: "backup", name: "live-1.sketchcraft", secs: 5, size: 9, label: "Timed backup" }, { kind: "evil", name: "x", label: "x" }, { kind: "backup", name: 5, label: "x" }, null, "x", { kind: "unsaved", name: "a".repeat(999), secs: "7", size: "1e3", label: "y".repeat(999) }]);
  check(rows.length === 2 && rows[1].name.length === 200 && rows[1].label.length === 80 && rows[1].secs === 7, "restore rows: unknown kinds and malformed rows are dropped, text is capped");
  check(cleanItems("no").length === 0 && cleanItems(Array(100000).fill({ kind: "backup", name: "a", label: "b" })).length === 500, "restore rows: not a list is empty, a long list is capped");
  check(ago(1000, 1010) === "just now" && ago(1000, 1000 + 600) === "10 min ago" && ago(1000, 1000 + 7200) === "2 h ago" && ago(1000, 1000 + 90000) === "yesterday" && ago(1000, 1000 + 5 * 86400) === "5 days ago", "restore times read plainly");
  check(bytes(10) === "10 B" && bytes(2048) === "2 KB" && bytes(5 * 1024 * 1024) === "5.0 MB", "restore sizes read plainly");

  // ---- what plugins hand the window ------------------------------------------------
  const { cleanResult, cleanForm } = await load("/src/pluginclean.ts");
  const hostile = [null, 5, "x", [], { title: {} }, { table: { columns: 1, rows: "x" } }, { table: { columns: ["a"], rows: [1, null, [{}], ["x".repeat(10000)]] } }, { text: { toString: 1 } }];
  for (const h of hostile) {
    const r = cleanResult(h);
    const ok = (r.title === undefined || typeof r.title === "string") && (r.text === undefined || typeof r.text === "string") && (!r.table || (r.table.columns.every((c) => typeof c === "string") && r.table.rows.every((row) => Array.isArray(row) && row.every((c) => typeof c === "string" || typeof c === "number"))));
    check(ok, `a result shaped ${JSON.stringify(h)?.slice(0, 40)} is cleaned to a safe shape`);
  }
  const big = cleanResult({ table: { columns: Array(500).fill("c"), rows: Array(100000).fill(Array(500).fill("cell")) } });
  check(big.table.rows.length <= 2000 && big.table.columns.length <= 40 && big.table.rows[0].length <= 40, "a huge table is cut down");
  const f = cleanForm({ title: { x: 1 }, fields: [{ id: "a", label: 5, type: "bogus", options: "no" }, { id: 7 }, null, { id: "b", type: "choice", options: ["x", 3], value: {} }] });
  check(f.fields.length === 2 && f.fields.every((x) => typeof x.label === "string" && ["text", "choice", "length", "number", "check"].includes(x.type)), "a form with odd fields is cleaned");

  // tool previews and anchors: only well-formed, finite, bounded points reach the 3D code
  const { cleanPreview, cleanPoint } = await load("/src/pluginclean.ts");
  const pv = cleanPreview([[5, 6], [[0, 0, 0], [1, 1, 1]], [[0, 0, "x"], [1, 1, 1]], [[0, 0, NaN], [1, 1, 1]], null, [[1, 2, 3]], [[1e300, -1e300, 0], [0, 0, 0]], { length: 2 }, Array(1000000).fill(1)]);
  check(pv.length === 2 && pv.every((l) => l.length === 2 && l.flat().every((n) => Number.isFinite(n) && Math.abs(n) <= 1e9)), "a hostile tool preview is cut down to finite, bounded segments");
  check(cleanPreview("no").length === 0 && cleanPreview(Array(100000).fill([[0, 0, 0], [1, 1, 1]])).length === 5000, "a preview that isn't a list is empty, a long one is capped");
  check(cleanPoint([1, 2, 3]) !== null && cleanPoint([1, 2]) === null && cleanPoint([1, 2, Infinity]) === null && cleanPoint("abc") === null && cleanPoint(null) === null, "an anchor must be three finite numbers");

  // ---- the plugin's command check is a first filter only ----------------------------
  globalThis.window ??= {}; // engine.ts asks whether it is inside Tauri
  const { allowed, csvCell } = await load("/src/plugins.tsx");
  // spreadsheet formulas in a saved table are neutralized; ordinary numbers and text are not touched
  check(csvCell("=HYPERLINK(\"http://x\")").startsWith(`"'=`) && csvCell("+1+2") === "'+1+2" && csvCell("@SUM(A1)") === "'@SUM(A1)" && csvCell("-1+cmd") === "'-1+cmd" && csvCell("\tx").startsWith("'\t"), "CSV cells that start like a formula get a quote");
  check(csvCell(-5) === "-5" && csvCell("-5") === "-5" && csvCell("3.5") === "3.5" && csvCell("a,b") === '"a,b"' && csvCell("hello") === "hello", "numbers, plain text and commas are as before");
  check(allowed({ cmd: "rect" }) === null && allowed({}) !== null && allowed(null) !== null && allowed([]) !== null, "allowed() checks the shape of a command");

  // ---- the worker lockdown does what it says ----------------------------------------
  const { SANDBOX_PREFACE } = await load("/src/sandbox.ts");
  const names = ["fetch", "XMLHttpRequest", "WebSocket", "EventSource", "importScripts", "indexedDB", "caches", "Worker", "SharedWorker", "BroadcastChannel", "WebTransport", "FontFace", "FontFaceSet", "Notification", "Request"];
  const self = { navigator: { sendBeacon: () => true, storage: {}, serviceWorker: {} }, WebAssembly: { instantiateStreaming: () => 1, compileStreaming: () => 1 } };
  for (const n of names) self[n] = function () {};
  self.self = self;
  const ctx = vm.createContext(self);
  vm.runInContext(SANDBOX_PREFACE, ctx);
  check(names.every((n) => ctx[n] === undefined), "fetch, XHR, WebSocket, importScripts and every kind of Worker are gone");
  let reached = [];
  for (const n of names) {
    try {
      vm.runInContext(`${n} = () => 1`, ctx);
    } catch {
      /* strict mode refuses */
    }
    try {
      vm.runInContext(`Object.defineProperty(self, "${n}", { value: () => 1 })`, ctx);
    } catch {
      /* not configurable */
    }
    if (ctx[n] !== undefined) reached.push(n);
  }
  check(reached.length === 0, `a plugin can't put them back${reached.length ? ` (${reached})` : ""}`);
  check(ctx.navigator.sendBeacon === undefined && ctx.navigator.storage === undefined && ctx.navigator.serviceWorker === undefined, "sendBeacon, navigator.storage and serviceWorker are gone");
  check(ctx.WebAssembly.instantiateStreaming === undefined && ctx.WebAssembly.compileStreaming === undefined, "WebAssembly can't be streamed in from a URL");
} finally {
  await server.close();
}
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
process.exit(failed ? 1 : 0);
