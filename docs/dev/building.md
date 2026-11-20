# Building, running and testing

## You need

- Rust (stable)
- Node.js 22
- On Linux, the Tauri system packages (WebKitGTK and friends); see the
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Build

```sh
cargo build --release -p sketchcraft-core      # the engine and CLI: target/release/sketchcraft
cd app && npm ci && npm run tauri build         # the desktop app and its installers
```

## Develop the app in a browser

The quickest loop: the engine as a local bridge, the interface from Vite.

```sh
cargo build --release && ./target/release/sketchcraft bridge   # 127.0.0.1:5175
cd app && npm run dev                                          # 127.0.0.1:5174
```

Open http://127.0.0.1:5174. Both listen on localhost only. After changing the
engine, rebuild and restart the bridge.

**The bridge token.** Every bridge request needs a random per-launch token
(`X-Sketchcraft-Token`). The bridge writes it to `.bridge-token` beside the live
model (readable only by you; `--token-file FILE` or `SKETCHCRAFT_BRIDGE_TOKEN`
change that), and the dev server hands it to the page at `/__bridge-token`, reading
the file again if the bridge was restarted, so there's nothing to copy. The dev
server answers that only for a request from its own page (same origin, the
`X-Sketchcraft-Dev` header, which another site can't send) that comes from a process
of **your own user** (it looks up the caller's port with `lsof`; elsewhere, such as
Windows, that check isn't made, so use the dev server on a machine only you use).
`curl` won't get the token from the dev server: read the file, as below. Scripts
that call the bridge send the file's contents as the header (see
`tools/demos/lib.mjs`; `curl -H "X-Sketchcraft-Token: $(cat ~/Documents/SketchCraft/.bridge-token)" ...`).
The dev page also gets the desktop app's content security policy.

**A throwaway instance** (for tests; never touches your real model): pick other
ports and a scratch model, and give the bridge the page's origin.

```sh
export SKETCHCRAFT_DOC=/tmp/scratch/live.sketchcraft SKETCHCRAFT_DEV_PORT=5195 VITE_BRIDGE_PORT=5196
./target/release/sketchcraft bridge --port 5196 --origin http://127.0.0.1:5195 &
(cd app && npx vite) &
node tools/ruby-tests/run.mjs && node tools/security-tests/plugin-sandbox.mjs
``` Dev builds expose the tool state as
`window.__sketchcraft` and the view as `window.__sketchcraftView` for debugging.

## Tests

```sh
cargo test --release -p sketchcraft-core    # modeling, importers (fuzzed), plugins
cd app && npx tsc --noEmit -p .             # the interface's types
cd app && npm run check                     # skin values, plugin lockdown and data checks
```

The importers are fuzzed (`core/tests/fuzz.rs`): thousands of corrupted files
each, no panics, with parse deadlines.

## Demo GIFs

The GIFs in the docs and the in-app Learn center are recorded from the real
app, so they stay true:

```sh
node tools/demos/capture.mjs --list          # what there is
node tools/demos/capture.mjs all             # record every one
node tools/demos/capture.mjs first-model     # or some
```

Each demo is a short script in `tools/demos/demos/`: a starting model, then
clicks, keys and captions. It writes `docs/demos/<name>.gif` and `.mp4`, and
`app/public/learn/<name>.gif` plus `demos.json` for the Learn center.

## Releasing

Bump the version in `core/Cargo.toml`, `app/src-tauri/Cargo.toml`,
`app/src-tauri/tauri.conf.json` and `app/package.json`, move the CHANGELOG's
Unreleased notes under the new version, then tag:

```sh
git tag -a v0.3.6 -m "SketchCraft 0.3.6" && git push origin v0.3.6
```

The Release workflow builds every installer, the portable Windows ZIP and the
CLI into a draft release, with signed update files (`latest.json`). Publish the
draft and installed copies offer the update. Signing: [SIGNING.md](../SIGNING.md).
