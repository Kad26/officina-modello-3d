import { useEffect, useState } from "react";
import { exec, type Scene } from "./engine";
import { italian, tt } from "./i18n";

type Clock = { year: number; day: number; minutes: number };
const MS_PER_DAY = 86_400_000;
const monthFormat = new Intl.DateTimeFormat(italian ? "it-IT" : "en", { month: "long", timeZone: "UTC" });
const dateFormat = new Intl.DateTimeFormat(italian ? "it-IT" : "en", { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });
const pad = (n: number) => String(n).padStart(2, "0");

function currentClock(): Clock {
  const now = new Date();
  const year = now.getFullYear();
  const day = (Date.UTC(year, now.getMonth(), now.getDate()) - Date.UTC(year, 0, 1)) / MS_PER_DAY + 1;
  return { year, day, minutes: 12 * 60 };
}

export function useShadowClock(active: boolean, location: Scene["location"]) {
  const [clock, setClock] = useState(currentClock);
  const [reading, setReading] = useState<{ key: string; direction: [number, number] | null } | null>(null);
  const date = new Date(Date.UTC(clock.year, 0, clock.day));
  const day = date.toISOString().slice(0, 10);
  const hour = Math.floor(clock.minutes / 60);
  const minute = clock.minutes % 60;
  const time = `${pad(hour)}:${pad(minute)}`;
  const [lat, lon] = [location?.lat, location?.lon];
  const offset = -new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate(), hour, minute).getTimezoneOffset() / 60;
  const localOffset = lon != null && Math.abs(lon / 15 - offset) < 1.6 ? offset : undefined;
  const key = `${day}|${time}|${lat ?? ""}|${lon ?? ""}|${localOffset ?? ""}`;
  useEffect(() => {
    if (!active) return;
    let canceled = false;
    const timer = window.setTimeout(async () => {
      let direction: [number, number] | null = null;
      try {
        const r = await exec({ cmd: "sun", date: day, time, utc_offset: localOffset });
        if (r.ok && typeof r.azimuth === "number" && Number.isFinite(r.azimuth) && typeof r.altitude === "number" && Number.isFinite(r.altitude)) {
          direction = [r.azimuth, r.altitude];
        }
      } catch {
        // Leave the standard light in place if the motor is unavailable.
      }
      if (!canceled) setReading({ key, direction });
    }, 60);
    return () => {
      canceled = true;
      window.clearTimeout(timer);
    };
  }, [active, day, time, localOffset, key]);
  return { clock, setClock, date, time, direction: active ? reading?.direction : null, resolved: reading?.key === key, located: !!location, active };
}

export function ShadowClock({ model }: { model: ReturnType<typeof useShadowClock> }) {
  const { clock, setClock, date, time, located, active, direction, resolved } = model;
  const maxDay = (Date.UTC(clock.year + 1, 0, 1) - Date.UTC(clock.year, 0, 1)) / MS_PER_DAY;
  const dayLabel = dateFormat.format(date);
  const message = !active ? tt("Enable Shadows to preview the sun position", "Attiva Ombre per vedere la posizione del sole")
    : !resolved ? tt("Calculating sun position", "Calcolo della posizione del sole")
    : !direction ? tt("Sun position unavailable", "Posizione del sole non disponibile")
    : `${direction[1] < 0 ? tt("Sun below the horizon", "Sole sotto l’orizzonte") : tt("Sun above the horizon", "Sole sopra l’orizzonte")}${located ? "" : tt(" · Default location", " · Posizione predefinita")}`;
  return (
    <div className="draft-group shadow-clock" role="group" aria-label={tt("Shadow date and time", "Data e ora delle ombre")} title={message}>
      <label className="shadow-range">
        <input type="range" min={1} max={maxDay} step={1} value={clock.day} aria-label={tt("Shadow date", "Data delle ombre")} aria-valuetext={dayLabel}
          onChange={(e) => setClock({ ...clock, day: e.currentTarget.valueAsNumber })} />
        <span className="shadow-months" aria-hidden="true">
          {Array.from({ length: 12 }, (_, i) => <span key={i}>{monthFormat.format(Date.UTC(clock.year, i, 1))[0].toUpperCase()}</span>)}
        </span>
      </label>
      <label className="shadow-range shadow-time">
        <input type="range" min={0} max={1439} step={1} value={clock.minutes} aria-label={tt("Shadow time", "Ora delle ombre")} aria-valuetext={time}
          onChange={(e) => setClock({ ...clock, minutes: e.currentTarget.valueAsNumber })} />
        <span className="shadow-hours" aria-hidden="true"><span>00:00</span><output>{time}</output><span>23:59</span></span>
      </label>
      <output className="shadow-sr-only" aria-live="polite">{message}</output>
    </div>
  );
}
