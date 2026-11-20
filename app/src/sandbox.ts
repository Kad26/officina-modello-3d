// The sandboxed worker every plugin runs in (JavaScript plugins, plugin tools and the
// SketchUp Ruby runtime): the lockdown first, then the code, in a worker that is
// torn down the same way every time.
import { LOCKDOWN_SOURCE } from "./lockdown";

/** What runs first in every plugin worker: strict mode, then the lockdown (see lockdown.ts). */
export const SANDBOX_PREFACE = `"use strict";\n${LOCKDOWN_SOURCE}`;

export interface SandboxWorker {
  worker: Worker;
  /** end the worker and release its script */
  stop: () => void;
}

/** Start a worker whose script is `parts` joined end to end (callers include SANDBOX_PREFACE). Throws if the browser won't start it. */
export function startWorker(parts: string[]): SandboxWorker {
  const url = URL.createObjectURL(new Blob(parts, { type: "text/javascript" }));
  let worker: Worker;
  try {
    worker = new Worker(url);
  } catch (e) {
    URL.revokeObjectURL(url);
    throw e;
  }
  return {
    worker,
    stop: () => {
      worker.terminate();
      URL.revokeObjectURL(url);
    },
  };
}
