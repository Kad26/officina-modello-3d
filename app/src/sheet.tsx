// The frame every dialog shares: a dimmed backdrop (a click on it closes) around a sheet.
import { useEffect, useRef, type ReactNode } from "react";

/** The dialog's room on narrow windows: `width` pixels, never wider than the window. */
const sheetWidth = (width: number) => `min(${width}px, calc(100vw - 32px))`;

/** The open dialogs, newest last: Escape closes only the top one. */
const open: object[] = [];

/** Escape closes the dialog or menu on top (and nothing underneath). Every dialog frame and menu uses it. */
export function useEscapeToClose(onClose: () => void, active = true) {
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    if (!active) return;
    const me = {};
    open.push(me);
    // in the capture phase, so the drawing area's own Escape (cancel, close a group) doesn't also run
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || open[open.length - 1] !== me) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      close.current();
    };
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("keydown", key, true);
      open.splice(open.indexOf(me), 1);
    };
  }, [active]);
}

export function Sheet({ onClose, label, className, width, children }: { onClose: () => void; label: string; className?: string; width?: number; children: ReactNode }) {
  useEscapeToClose(onClose);
  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div className={className ? `sheet ${className}` : "sheet"} style={width ? { width: sheetWidth(width) } : undefined} onClick={(e) => e.stopPropagation()} role="dialog" aria-label={label}>
        {children}
      </div>
    </div>
  );
}

/** The title line of a dialog: a name and what it is for. */
export function SheetHead({ title, note }: { title: string; note: string }) {
  return (
    <div className="sheet-head">
      <b>{title}</b>
      <span className="muted">{note}</span>
    </div>
  );
}
