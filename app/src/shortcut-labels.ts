/** Key names shown by the current platform. The engine handles the key events. */
export function isWindows(platform = globalThis.navigator?.platform ?? "") {
  return platform.startsWith("Win");
}

function isApple(platform = globalThis.navigator?.platform ?? "") {
  return /Mac|iPhone|iPad/.test(platform);
}

export function desktopShortcut(printed: string, platform?: string) {
  if (isApple(platform)) return printed;
  return printed
    .replace(/⇧\s+click\b/gi, "Shift+click")
    .replace(/⇧(?=\S)/g, "Shift+")
    .replace(/⇧/g, "Shift")
    .replace(/⌘/g, "Ctrl+")
    .replace(/⌫/g, "Delete");
}

/** The conventional modifier for copying entities (and adding to selections). */
export function copyModifier() {
  return isApple() ? "Option" : "Ctrl";
}

/** The Option key is Alt on Windows. */
export function optionModifier() {
  return isApple() ? "Option" : "Alt";
}

export const sketchUpWindowsKeys = import.meta.env.VITE_OFFICINA === "1" && isWindows();
export const redoShortcut = isWindows() ? "Ctrl+Y" : "⇧⌘Z";

export function modelingKeysBlocked(e: KeyboardEvent) {
  if (e.defaultPrevented || e.isComposing || e.getModifierState("AltGraph")) return true;
  if (e.target instanceof Element && e.target.closest(
    "input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='dialog']",
  )) return true;
  return !!document.querySelector(".sheet-backdrop, .matchphoto");
}
