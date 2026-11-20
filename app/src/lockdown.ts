// The lockdown that runs first inside every plugin worker (JavaScript plugins
// and the SketchUp Ruby runtime): no network, no more code, and no way to
// start another worker (a new worker would begin without any of this).
// Kept as a string because it runs inside the worker; checks/run.mjs runs it
// against a pretend global to prove it does what it says.

export const LOCKDOWN_SOURCE = `
for (const k of ["fetch", "XMLHttpRequest", "WebSocket", "WebSocketStream", "WebTransport", "EventSource", "importScripts", "indexedDB", "caches", "Worker", "SharedWorker", "BroadcastChannel", "Request", "RTCPeerConnection", "fetchLater", "FontFace", "FontFaceSet", "Notification", "PushManager", "BackgroundFetchManager", "Cache", "CacheStorage", "FileSystemHandle", "FileSystemDirectoryHandle", "FileSystemFileHandle", "WebTransport", "RTCDataChannel", "RTCSessionDescription"]) {
  try { Object.defineProperty(self, k, { value: undefined, writable: false, configurable: false }); } catch {}
}
try { Object.defineProperty(self.navigator, "sendBeacon", { value: undefined, configurable: false }); } catch {}
// storage a plugin could use to keep data beyond its run (and cache/streaming loads)
for (const k of ["storage", "serviceWorker", "locks", "usb", "bluetooth", "hid", "serial"]) {
  try { Object.defineProperty(self.navigator, k, { value: undefined, configurable: false }); } catch {}
}
for (const k of ["instantiateStreaming", "compileStreaming"]) {
  try { Object.defineProperty(self.WebAssembly, k, { value: undefined, configurable: false }); } catch {}
}
// Dynamic import() is part of the language and can't be removed: the window's
// content security policy stops it (and eval), and cspcheck.ts proves that holds before a plugin runs.
`;
