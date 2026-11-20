import { Fragment, useEffect, useRef, useState, type KeyboardEvent as ReactKeyEvent, type ReactNode } from "react";
import { useEscapeToClose } from "./sheet";
import { desktopShortcut, isWindows } from "./shortcut-labels";
import { italian, tr } from "./i18n";
import { verticalToolIcon } from "./vertical-tool-icons";
import { TOOL_INFO } from "./workspace";

export interface DesktopAction {
  label: string;
  run: () => unknown;
  key?: string;
  icon?: ReactNode;
  checked?: boolean;
  disabled?: boolean;
}

export interface DesktopMenu {
  name: string;
  sections: DesktopAction[][];
}

type ToolId = keyof typeof TOOL_INFO;

export function desktopTool(id: ToolId, label: string, choose: (id: ToolId) => void): DesktopAction {
  return { label, key: TOOL_INFO[id].key || undefined, icon: verticalToolIcon(id), run: () => choose(id) };
}

const TRIGGER = "button[data-menu-access]";
const OPTION = ".dropdown-menu button:not(:disabled)";
const IT_KEYS: Readonly<Record<string, string>> = {
  Edit: "m", View: "v", Camera: "t", Draw: "d", Tools: "s", Window: "n", Extensions: "e", Help: "g",
};

function focusPopup(trigger: HTMLButtonElement, last = false) {
  trigger.focus();
  if (trigger.getAttribute("aria-expanded") !== "true") trigger.click();
  // The popup renders after the state update queued by the click.
  requestAnimationFrame(() => {
    if (trigger.getAttribute("aria-expanded") !== "true") return;
    const items = trigger.parentElement?.querySelectorAll<HTMLButtonElement>(OPTION);
    if (items?.length) items[last ? items.length - 1 : 0].focus();
  });
}

export function DesktopMenus({ file, menus }: { file: ReactNode; menus: DesktopMenu[] }) {
  const [active, setActive] = useState<string | null>(null);
  const bar = useRef<HTMLElement>(null);
  const activeMenu = useRef<HTMLDivElement>(null);
  const windows = isWindows();
  const close = (returnFocus = false) => {
    if (returnFocus) activeMenu.current?.querySelector("button")?.focus();
    setActive(null);
  };
  useEscapeToClose(() => close(true), active != null);
  useEffect(() => {
    if (!active) return;
    const outside = (e: PointerEvent) => {
      if (e.target instanceof Node && !activeMenu.current?.contains(e.target)) setActive(null);
    };
    document.addEventListener("pointerdown", outside, true);
    return () => document.removeEventListener("pointerdown", outside, true);
  }, [active]);
  useEffect(() => {
    if (!windows) return;
    const keyboard = (e: KeyboardEvent) => {
      if (!bar.current || e.defaultPrevented || e.repeat || e.isComposing || e.ctrlKey || e.metaKey || e.shiftKey) return;
      if (e.target instanceof Element && e.target.closest("input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='dialog']")) return;
      if (document.querySelector(".sheet-backdrop")) return;
      const trigger = e.key === "F10" && !e.altKey
        ? bar.current.querySelector<HTMLButtonElement>(TRIGGER)
        : e.altKey && /^[a-z]$/i.test(e.key)
          ? bar.current.querySelector<HTMLButtonElement>(`${TRIGGER}[data-menu-access="${e.key.toLowerCase()}"]`)
          : null;
      if (!trigger) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "F10") trigger.focus();
      else focusPopup(trigger);
    };
    window.addEventListener("keydown", keyboard, true);
    return () => window.removeEventListener("keydown", keyboard, true);
  }, [windows]);

  const navigate = (e: ReactKeyEvent<HTMLElement>) => {
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey || !(e.target instanceof Element)) return;
    const dropdown = e.target.closest(".dropdown");
    const trigger = dropdown?.querySelector<HTMLButtonElement>(TRIGGER);
    if (!trigger) return;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      const triggers = [...e.currentTarget.querySelectorAll<HTMLButtonElement>(TRIGGER)];
      const direction = e.key === "ArrowRight" ? 1 : -1;
      const next = triggers[(triggers.indexOf(trigger) + direction + triggers.length) % triggers.length];
      e.preventDefault();
      e.stopPropagation();
      if (trigger.getAttribute("aria-expanded") === "true") focusPopup(next);
      else next.focus();
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
    const items = [...(dropdown?.querySelectorAll<HTMLButtonElement>(OPTION) ?? [])];
    if (!items.length && (e.target !== trigger || !e.key.startsWith("Arrow"))) return;
    e.preventDefault();
    e.stopPropagation();
    const at = items.indexOf(e.target as HTMLButtonElement);
    if (at < 0) {
      focusPopup(trigger, e.key === "ArrowUp" || e.key === "End");
      return;
    }
    const direction = e.key === "ArrowDown" ? 1 : -1;
    const next = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : (at + direction + items.length) % items.length;
    items[next].focus();
  };

  return (
    <nav className="desktop-menus" aria-label={tr("Main menus")} ref={bar} onKeyDown={navigate}>
      {file}
      {menus.map(({ name, sections }) => {
        const label = tr(name);
        const access = (italian ? IT_KEYS[name] : name === "Extensions" ? "x" : undefined) ?? label[0].toLowerCase();
        const at = label.toLowerCase().indexOf(access);
        return (
        <div
          className="dropdown"
          key={name}
          ref={active === name ? activeMenu : undefined}
          onBlur={(e) => {
            if (active === name && !e.currentTarget.contains(e.relatedTarget)) close();
          }}
        >
          <button
            className="desktop-menu-trigger"
            aria-expanded={active === name}
            aria-keyshortcuts={windows ? `Alt+${access.toUpperCase()}` : undefined}
            data-menu-access={access}
            onClick={() => setActive(active === name ? null : name)}
          >
            {windows && at >= 0 ? <>{label.slice(0, at)}<u>{label[at]}</u>{label.slice(at + 1)}</> : label}
          </button>
          {active === name && (
            <div
              className="dropdown-menu desktop-menu"
              role="group"
              aria-label={label}
            >
              {sections.filter((items) => items.length).map((items, index) => (
                <Fragment key={items[0].label}>
                  {index > 0 && <hr />}
                  {items.map((item) => (
                    <button
                      key={item.label}
                      aria-pressed={item.checked}
                      disabled={item.disabled}
                      onClick={() => {
                        close(true);
                        void item.run();
                      }}
                    >
                      <span className="desktop-menu-label">
                        <span className="desktop-menu-icon" aria-hidden="true">{item.checked ? "✓" : item.icon}</span>
                        {tr(item.label)}
                      </span>
                      {item.key && <kbd>{desktopShortcut(item.key)}</kbd>}
                    </button>
                  ))}
                </Fragment>
              ))}
            </div>
          )}
        </div>
        );
      })}
    </nav>
  );
}
