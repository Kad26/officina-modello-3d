import type { ReactNode } from "react";
import type { TOOL_INFO } from "./workspace";

type ToolId = keyof typeof TOOL_INFO;

// Solid silhouettes for the vertical toolbar. currentColor keeps them legible
// in light, dark, high contrast and selected states.
const glyphs: Record<ToolId, ReactNode> = {
  select: <path d="M5 2.8 19 12l-6.2.5 2.8 6.4-2.5 1.1-2.8-6.3L5 18z" fill="currentColor" stroke="none" />,
  igloo: <path d="M3 6.5h12v11H3zM15 8l6-2v11l-6 2M4 15l3.5-4 3 3 2-2" fill="none" />,
  lasso: <path d="M9 16c-3 1-6-1-6-4 0-4 4-7 9-7 5 0 9 2 9 5 0 4-5 6-10 6 0 2-1 4-3 4-1.5 0-2-1-1-2Z" />,
  line: (
    <>
      <path d="m6 14 10-10 4 4L10 18 4 20z" fill="currentColor" fillOpacity=".2" />
      <path d="m14 6 4 4M6 14l4 4M5.1 17.6l1.3 1.3" />
    </>
  ),
  freehand: <path d="M3.5 16.5c1.6-5.5 3.5-6.6 5.2-2.3 1.7 4.2 3.9 5.2 5.4-2.4 1.2-6 3-5.9 4-1.5.7 3 1.6 3.8 2.4 2.6" />,
  arc: (
    <>
      <path d="M4 17a8 8 0 0 1 16 0" strokeWidth="2" />
      <path d="M4 17h16" strokeDasharray="2 2" />
      <circle cx="4" cy="17" r="1.35" fill="currentColor" stroke="none" />
      <circle cx="20" cy="17" r="1.35" fill="currentColor" stroke="none" />
    </>
  ),
  arccenter: (
    <>
      <path d="M5 17a7 7 0 1 1 14 0" strokeWidth="2" />
      <path d="M12 17h7" strokeDasharray="2 2" />
      <circle cx="12" cy="17" r="1.5" fill="currentColor" stroke="none" />
    </>
  ),
  arc3: (
    <>
      <path d="M4 17c3-12 13-12 16 0" />
      <path d="M4 17h.2m7.7-9.1h.2m7.7 9.1h.2" strokeWidth="3" />
    </>
  ),
  pie: <path d="M12 4a8 8 0 1 0 8 8h-8z" fill="currentColor" fillOpacity=".3" />,
  rect: (
    <>
      <path d="M4 6h16v12H4z" fill="currentColor" fillOpacity=".18" />
      <path d="M4 18 20 6" strokeDasharray="2 2.3" />
      <circle cx="4" cy="18" r="1.35" fill="currentColor" stroke="none" />
      <circle cx="20" cy="6" r="1.35" fill="currentColor" stroke="none" />
    </>
  ),
  rrect: (
    <>
      <path d="m4.2 13 10-8.6 5.6 6.6-10 8.6z" fill="currentColor" fillOpacity=".17" />
      <path d="M4.2 13h7.4" strokeDasharray="1.6 1.8" />
      <circle cx="4.2" cy="13" r="1.4" fill="currentColor" stroke="none" />
    </>
  ),
  circle: (
    <>
      <circle cx="12" cy="12" r="8" fill="currentColor" fillOpacity=".18" />
      <path d="M12 12h8" strokeDasharray="2 2" />
      <circle cx="12" cy="12" r="1.35" fill="currentColor" stroke="none" />
    </>
  ),
  polygon: <path d="m12 3.5 7.4 4.2v8.6L12 20.5l-7.4-4.2V7.7z" fill="currentColor" fillOpacity=".23" />,
  push: (
    <>
      <path d="m3.5 13.2 8.5-3.5 8.5 3.5v4.3L12 21l-8.5-3.5z" fill="currentColor" fillOpacity=".2" />
      <path d="m3.5 13.2 8.5 3.5 8.5-3.5M12 16.7V21" />
      <path d="M11 12V6H8l4-4.5L16 6h-3v6z" fill="currentColor" stroke="none" />
    </>
  ),
  offset: (
    <>
      <path d="M4 4h16v16H4z" fill="currentColor" fillOpacity=".13" />
      <path d="M8 8h8v8H8z" />
      <path d="m15 9 5-5m-5 1v4h4" />
    </>
  ),
  followme: (
    <>
      <path d="M5 18h5a7 7 0 0 0 7-7V4m-3.3 3.3L17 4l3.3 3.3" />
      <path d="M3 15h5v6H3z" fill="currentColor" fillOpacity=".25" />
    </>
  ),
  move: <path d="M12 2 8 6h2.8v4.8H6V8l-4 4 4 4v-2.8h4.8V18H8l4 4 4-4h-2.8v-4.8H18V16l4-4-4-4v2.8h-4.8V6H16z" fill="currentColor" stroke="none" />,
  rotate: (
    <>
      <path d="M3.5 17a8.5 8.5 0 0 1 17 0z" fill="currentColor" fillOpacity=".18" />
      <path d="M12 17v-3m-5.6-3.2 1.4 1.6m9.8-1.6-1.4 1.6M12 5.3V2.5m0 0-2.3 2.3m2.3-2.3 2.3 2.3" />
      <circle cx="12" cy="17" r="1.25" fill="currentColor" stroke="none" />
    </>
  ),
  scale: (
    <>
      <path d="M4 20V8h12v12z" fill="currentColor" fillOpacity=".14" />
      <path d="m9 15 11-11m-5 0h5v5" />
      <path d="M4 8h.1m11.8 12h.1M4 20h.1" strokeWidth="3.3" />
    </>
  ),
  tape: (
    <>
      <path d="M4 9a4 4 0 0 1 4-4h3a3 3 0 0 1 3 3v9H7a3 3 0 0 1-3-3z" fill="currentColor" fillOpacity=".2" />
      <path d="M14 13h6.5v4H14m2.1-4v1.5m2.2-1.5v1.5" />
      <circle cx="9" cy="11" r="1.6" />
    </>
  ),
  protractor: (
    <>
      <path d="M3 17a9 9 0 0 1 18 0z" fill="currentColor" fillOpacity=".17" />
      <path d="m12 17 4-6m-4-3v2m-5.8 1.5 1.4 1.3m10.2-1.3-1.4 1.3" />
    </>
  ),
  axes: <path d="M8 16V4m0 12 12 3m-12-3 10-7M6 6l2-2 2 2m7 11 3 2-3 1m-1-11h2v2" strokeWidth="2" />,
  dimension: (
    <>
      <path d="M4 6v12m16-12v12M4 12h16m-13-2-3 2 3 2m10-4 3 2-3 2" />
      <path d="m10 6 2-2 2 2" />
    </>
  ),
  text: (
    <>
      <path d="M9 5h12v10H9z" fill="currentColor" fillOpacity=".14" />
      <path d="M9 15 4 20m0 0h4m-4 0v-4m8-7h6m-3 0v4" />
    </>
  ),
  paint: (
    <>
      <path d="m9 5 9 9-5.7 5.7a2 2 0 0 1-2.8 0l-5.2-5.2z" fill="currentColor" fillOpacity=".20" />
      <path d="M5 14.5h12m-9.5-11L13 9m6.6 7.7c1.4 1.6 2 2.5 2 3.1a2 2 0 0 1-4 0c0-.6.7-1.5 2-3.1z" />
    </>
  ),
  erase: (
    <>
      <path d="M3.8 15.5 13.7 5.6a2 2 0 0 1 2.8 0l3.9 3.9a2 2 0 0 1 0 2.8l-7.4 7.4H7.9z" fill="currentColor" fillOpacity=".2" />
      <path d="m7.7 11.6 6.9 6.9m-1.6 1.2h8" />
    </>
  ),
  orbit: (
    <>
      <ellipse cx="12" cy="12" rx="9" ry="4.3" transform="rotate(-27 12 12)" />
      <path d="M8 4.7c2-1.4 5.4 1.1 7.6 5.5s2.2 8.9.3 10.1M5 13l-2.1 1.2 2.4.9m13.7-4.2 2.1-1.2-2.4-.9" />
      <circle cx="12" cy="12" r="1.6" fill="currentColor" stroke="none" />
    </>
  ),
  pan: <path d="M8 12.5V6a1 1 0 0 1 2 0v4m0 0V4a1 1 0 0 1 2 0v6m0 0V5a1 1 0 0 1 2 0v5m0 0V7a1 1 0 0 1 2 0v6l1.3-1.2a1.4 1.4 0 0 1 1.9 2l-3 4V21h-6v-2.8l-3.5-4.4a1.4 1.4 0 0 1 1.3-1.3z" fill="currentColor" fillOpacity=".16" />,
  zoom: (
    <>
      <circle cx="10" cy="10" r="6" fill="currentColor" fillOpacity=".14" />
      <path d="m14.5 14.5 5 5M7.5 10h5M10 7.5v5" />
    </>
  ),
  zoomwindow: (
    <>
      <path d="M3.5 3.5h15v15h-15z" strokeDasharray="2.4 1.9" />
      <circle cx="12" cy="12" r="3.5" fill="currentColor" fillOpacity=".19" />
      <path d="m14.7 14.7 5.2 5.2" strokeWidth="2" />
    </>
  ),
  section: (
    <>
      <path d="m4 15 5-7 11 3-5 7z" fill="currentColor" fillOpacity=".18" />
      <path d="M12 4v17m-3-4 3 4 3-4" strokeDasharray="4 2" />
    </>
  ),
  position: (
    <>
      <path d="M3.5 7.5h11v9h-11zm11 3 5.5-3.5v10l-5.5-3.5z" fill="currentColor" fillOpacity=".17" />
      <path d="M9 17v4m-3 0h6" />
    </>
  ),
  lookaround: (
    <>
      <path d="M3 12c5-6 13-6 18 0-5 6-13 6-18 0z" fill="currentColor" fillOpacity=".13" />
      <circle cx="12" cy="12" r="2.3" />
      <path d="M6 5h12m-2-2 2 2-2 2" />
    </>
  ),
  walk: (
    <>
      <circle cx="13" cy="5" r="2" fill="currentColor" stroke="none" />
      <path d="m10 20 2-5 4 5m-4-5 1.5-6m-1.5 1-3 2-3.2-.5m7.6-2.7 3.3 2.8h2.7" strokeWidth="2" />
    </>
  ),
  wall: <path d="M4 20V8l7-4 9 2v12l-7 3zm7-16v12m-7-8 9 2 7-4m-7 4v11" fill="currentColor" fillOpacity=".18" />,
  stairs: <path d="M3.5 20v-5h5v-4.5h5V6h7v14z" fill="currentColor" fillOpacity=".21" />,
  roof: <path d="m3.5 12 8.5-7 8.5 7-2 2-6.5-5.4L5.5 14zm3.5 2v5h10v-5" fill="currentColor" fillOpacity=".19" />,
  column: <path d="M6 4.5h12v3H6zm2.5 3h7v9h-7zM6 16.5h12v3H6z" fill="currentColor" fillOpacity=".2" />,
  smoove: <path d="M3 17c3 0 4-7 7.5-7 3.5 0 4 7 10.5 7m-9-7V4m-3 3 3-3 3 3" />,
  stamp: <path d="m3 17 5-4h8l5 4-3 3H6zm6-4V8h6v5M8 8h8V4H8z" fill="currentColor" fillOpacity=".17" />,
  drape: <path d="M3 5h18M6 7v5m6-5v5m6-5v5m-1-1 1 1 1-1M3 18l6-3 8 2 4-2" />,
  flipedge: <path d="M4 4h16v16H4zm0 0 16 16m0-16-6 6M4 20l6-6" fill="currentColor" fillOpacity=".13" />,
};

export function verticalToolIcon(id: string) {
  if (!Object.hasOwn(glyphs, id)) return undefined;
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {glyphs[id as ToolId]}
    </svg>
  );
}
