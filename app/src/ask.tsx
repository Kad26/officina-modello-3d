// A small in-app text prompt. window.prompt doesn't show in the desktop
// app's web view, so anything that needs a name or a label asks here.

import { useEffect, useRef, useState } from "react";
import { Sheet } from "./sheet";

interface Ask {
  title: string;
  value: string;
  hint?: string;
  /** a yes / no question: no text box, this on the OK button */
  confirm?: string;
  /** explicit choices: no text box or implicit Enter action */
  options?: { value: string; label: string }[];
  done: (v: string | null) => void;
}

let show: ((a: Ask) => void) | null = null;

/** Ask for a line of text; null if cancelled. */
export function ask(title: string, value = "", hint?: string): Promise<string | null> {
  return new Promise((done) => {
    if (show) show({ title, value, hint, done });
    else done(null);
  });
}

/** Ask a yes / no question (`ok` labels the yes); true if confirmed. */
export function confirm(title: string, hint: string, ok = "OK"): Promise<boolean> {
  return new Promise((done) => {
    if (show) show({ title, value: "", hint, confirm: ok, done: (v) => done(v != null) });
    else done(false);
  });
}

export function choose(title: string, hint: string, options: NonNullable<Ask["options"]>): Promise<string | null> {
  return new Promise((done) => {
    if (show) show({ title, value: "", hint, options, done });
    else done(null);
  });
}

export function AskHost() {
  const [a, setA] = useState<Ask | null>(null);
  const [text, setText] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    show = (x) => {
      setA(x);
      setText(x.value);
    };
    return () => {
      show = null;
    };
  }, []);
  useEffect(() => {
    if (a && !a.confirm && !a.options) setTimeout(() => input.current?.select(), 0);
    if (!a?.confirm) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Enter") (e.preventDefault(), a.done(""), setA(null));
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [a]);
  if (!a) return null;
  const finish = (v: string | null) => {
    a.done(v);
    setA(null);
  };
  return (
    <Sheet onClose={() => finish(null)} label={a.title} className="ask">
      <b>{a.title}</b>
      {a.hint && <p className="muted small" style={{ whiteSpace: "pre-line" }}>{a.hint}</p>}
      {!a.confirm && !a.options && <input
        ref={input}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") finish(text);
          if (e.key === "Escape") finish(null);
        }}
      />}
      <div className={a.options ? "solidChoices" : "row gap"}>
        {a.options ? a.options.map((option) => (
          <button key={option.value} className="btn small" onClick={() => finish(option.value)}>
            {option.label}
          </button>
        )) : <button className="btn small" onClick={() => finish(text)}>
          {a.confirm ?? "OK"}
        </button>}
        <button className="btn small ghost" onClick={() => finish(null)}>
          Cancel
        </button>
      </div>
    </Sheet>
  );
}
