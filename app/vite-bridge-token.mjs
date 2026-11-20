// Dev-only Vite plugin: hands the dev page the bridge's per-launch token.
// Plain JS (not part of the app's type-checked sources: it uses Node APIs).
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

/** Where `sketchcraft bridge` writes its token (beside the live model). */
function tokenFile() {
  if (process.env.SKETCHCRAFT_BRIDGE_TOKEN_FILE) return process.env.SKETCHCRAFT_BRIDGE_TOKEN_FILE;
  const doc = process.env.SKETCHCRAFT_DOC ?? path.join(os.homedir(), "Documents/SketchCraft/live.sketchcraft");
  return path.join(path.dirname(doc), ".bridge-token");
}

/**
 * Is the caller a process of the same user as this dev server? The token file
 * is readable only by its owner; this endpoint must not widen that. The peer of
 * a loopback connection is found by its port (lsof); a caller that can't be seen
 * (another user's process isn't listed to us) or isn't ours is refused. On
 * Windows there is no such check: the dev server is for a single-user machine.
 */
function sameUser(req) {
  if (process.platform === "win32") return true;
  const port = req.socket.remotePort;
  const addr = req.socket.remoteAddress ?? "";
  if (!port || !(addr === "127.0.0.1" || addr === "::1" || addr === "::ffff:127.0.0.1")) return false;
  try {
    const out = execFileSync("lsof", ["-nP", "-F", "pu", `-iTCP:${port}`], { timeout: 3000, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
    let pid = 0;
    let other = 0;
    for (const line of out.split("\n")) {
      if (line.startsWith("p")) pid = Number(line.slice(1));
      else if (line.startsWith("u")) {
        const mine = Number(line.slice(1)) === process.getuid();
        if (!mine) return false;
        if (pid !== process.pid) other++;
      }
    }
    return other > 0; // the caller's own process was seen, and it is ours
  } catch {
    return false;
  }
}

/**
 * The bridge refuses requests without its token. The token file is readable
 * only by you; the page reads it again if the bridge is restarted. Only
 * same-origin browser requests from the dev page itself (the page marks them
 * with X-Sketchcraft-Dev, which another site can't send without a preflight
 * this server never answers), made by a process of the same user, are answered.
 */
export default function bridgeToken(port) {
  return {
    name: "sketchcraft-bridge-token",
    configureServer(server) {
      server.middlewares.use("/__bridge-token", (req, res) => {
        const host = String(req.headers.host ?? "");
        const site = req.headers["sec-fetch-site"];
        const ok = (host === `127.0.0.1:${port}` || host === `localhost:${port}`) && site === "same-origin" && req.headers["x-sketchcraft-dev"] === "1" && sameUser(req);
        if (!ok) {
          res.statusCode = 403;
          return res.end();
        }
        let token = process.env.SKETCHCRAFT_BRIDGE_TOKEN ?? "";
        if (!token) {
          try {
            token = fs.readFileSync(tokenFile(), "utf8").trim();
          } catch {
            /* the bridge isn't running yet */
          }
        }
        res.setHeader("Cache-Control", "no-store");
        res.setHeader("Content-Type", "text/plain");
        res.end(token);
      });
    },
  };
}

/**
 * The dev page gets the same kind of content security policy the desktop app
 * has (app/src-tauri/tauri.conf.json), so a plugin behaves the same in both:
 * it can't load code or reach any address but this server and the bridge.
 * (The Vite client needs inline scripts and a websocket back to this server.)
 */
export function devCsp(port, bridgePort) {
  const policy = [
    "default-src 'self'",
    "script-src 'self' 'unsafe-inline'",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: blob:",
    "media-src 'self' data: blob:",
    "font-src 'self' data:",
    `connect-src 'self' ws://127.0.0.1:${port} ws://localhost:${port} http://127.0.0.1:${bridgePort}`,
    "worker-src 'self' blob:",
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'none'",
  ].join("; ");
  return {
    name: "sketchcraft-dev-csp",
    apply: "serve",
    transformIndexHtml: () => [{ tag: "meta", attrs: { "http-equiv": "Content-Security-Policy", content: policy }, injectTo: "head-prepend" }],
  };
}
