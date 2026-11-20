// The plugin lockdown (lockdown.ts) removes what a worker could use to reach
// the network or load code, but the window's content security policy is what
// really stops `import()`, `FontFace` and `eval`, which can't be deleted. If a
// webview didn't apply that policy to workers, plugins would have network
// access again. So before the first plugin runs, a probe worker (without the
// lockdown) tries a fetch, an `import()` and an `eval` at an address that
// isn't allowed, and the policy must report each one blocked.
// Where a webview has no violation reports at all (no SecurityPolicyViolationEvent)
// the probe says so and plugins run: the policy is all that is left to rely on there.

const PROBE = `
const hits = { fetch: false, import: false, eval: false };
let key = "";
self.addEventListener("securitypolicyviolation", (e) => {
  const d = String(e.effectiveDirective || e.violatedDirective || "");
  if (d.startsWith("connect-src")) hits[key] = true;
  else if (d.startsWith("script-src")) hits[key] = true;
});
const probe = async (name, f) => { key = name; try { await f(); } catch (e) { /* refused, or nothing listening */ } await new Promise((r) => setTimeout(r, 30)); };
(async () => {
  // a webview that can't report violations to workers can't be checked this way
  if (typeof SecurityPolicyViolationEvent === "undefined") return postMessage({ unverifiable: true });
  await probe("fetch", () => fetch("http://127.0.0.1:9/csp-probe"));
  await probe("import", () => import("http://127.0.0.1:9/csp-probe.js"));
  await probe("eval", () => eval("1"));
  postMessage({ hits });
})();
`;

let verdict: Promise<{ ok: boolean; why?: string }> | null = null;

/** Does the window's policy hold inside plugin workers? Checked once. */
export function pluginSandboxHolds(): Promise<{ ok: boolean; why?: string }> {
  return (verdict ??= new Promise((resolve) => {
    let url = "";
    try {
      url = URL.createObjectURL(new Blob([PROBE], { type: "text/javascript" }));
      const w = new Worker(url);
      const done = (v: { ok: boolean; why?: string }) => {
        clearTimeout(timer);
        w.terminate();
        URL.revokeObjectURL(url);
        resolve(v);
      };
      const timer = setTimeout(() => done({ ok: false, why: "the safety check didn't finish" }), 5000);
      w.onerror = () => done({ ok: false, why: "the safety check couldn't run" });
      w.onmessage = (e) => {
        const d = e.data ?? {};
        if (d.unverifiable) return done({ ok: true });
        const h = d.hits ?? {};
        const open = ["fetch", "import", "eval"].filter((k) => !h[k]);
        done(open.length ? { ok: false, why: `this window's security policy doesn't block ${open.join(", ")} inside plugins` } : { ok: true });
      };
    } catch (e) {
      if (url) URL.revokeObjectURL(url);
      resolve({ ok: false, why: `the safety check couldn't start: ${String(e)}` });
    }
  }));
}

/** Test hook: forget the verdict. */
export function resetPluginSandboxCheck() {
  verdict = null;
}
