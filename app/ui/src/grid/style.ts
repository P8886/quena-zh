// Row colouring, method/status badges and the state bar.
import { Flags, type SessionSummary } from "../api";

export interface Palette {
  bg: string;
  bgAlt: string;
  fg: string;
  muted: string;
  red: string;
  blue: string;
  green: string;
  purple: string;
  gray: string;
  amber: string;
  breakpointRequest: string;
  breakpointResponse: string;
  selBg: string;
  selFg: string;
  selInactiveBg: string;
  focus: string;
  grid: string;
  marks: Record<string, string>;
  markFg: Record<string, string>;
  tones: Record<Tone, { fg: string; bg: string }>;
}

export function readPalette(el: HTMLElement): Palette {
  const cs = getComputedStyle(el);
  const v = (n: string) => cs.getPropertyValue(n).trim();
  return {
    bg: v("--grid-bg"),
    bgAlt: v("--grid-bg-alt"),
    fg: v("--fg"),
    muted: v("--muted"),
    red: v("--row-red"),
    blue: v("--row-blue"),
    green: v("--row-green"),
    purple: v("--row-purple"),
    gray: v("--row-gray"),
    amber: v("--row-amber"),
    breakpointRequest: v("--breakpoint-request"),
    breakpointResponse: v("--breakpoint-response"),
    selBg: v("--sel-bg"),
    selFg: v("--sel-fg"),
    selInactiveBg: v("--sel-inactive-bg"),
    focus: v("--focus"),
    grid: v("--grid-line"),
    marks: {
      red: v("--mark-red"),
      blue: v("--mark-blue"),
      gold: v("--mark-gold"),
      green: v("--mark-green"),
      orange: v("--mark-orange"),
      purple: v("--mark-purple"),
    },
    markFg: {
      red: v("--markfg-red"),
      blue: v("--markfg-blue"),
      gold: v("--markfg-gold"),
      green: v("--markfg-green"),
      orange: v("--markfg-orange"),
      purple: v("--markfg-purple"),
    },
    tones: Object.fromEntries(
      (["ok", "info", "warn", "err", "muted", "violet"] as const).map((t) => [t, { fg: v(`--pill-${t}-fg`), bg: v(`--pill-${t}-bg`) }]),
    ) as Record<Tone, { fg: string; bg: string }>,
  };
}

export interface RowStyle {
  fg: string;
  bold: boolean;
  bg: string | null;
  italic: boolean;
  /** The user's mark stays visible when selection covers its background. */
  markBar: string | null;
  breakpointDot: string | null;
}

export function rowStyle(r: SessionSummary, p: Palette, selection: "none" | "active" | "inactive" = "none"): RowStyle {
  let fg = p.fg;
  let bold = false;
  let italic = false;
  // The outcome is shown by the status badge; only aborted sessions and tunnels tint the row.
  if (r.state === "aborted") fg = p.red;
  else if (r.kind === "tunnel") fg = p.gray;
  if (r.state !== "done" && r.state !== "aborted") italic = true;
  let bg: string | null = null;
  if (r.color) {
    bg = p.marks[r.color];
    fg = p.markFg[r.color] || fg;
    bold = true;
  }
  // Only a currently paused session has a dot; the BREAKPOINTED history flag is not enough.
  const breakpointDot = r.state === "breakpointRequest" ? p.breakpointRequest
    : r.state === "breakpointResponse" ? p.breakpointResponse : null;
  if (breakpointDot) italic = false;
  // Breakpoints use the dot so selection and search highlights can keep their backgrounds.
  if (selection !== "none") {
    bg = selection === "active" ? p.selBg : p.selInactiveBg;
    fg = p.selFg;
  }
  return { fg, bold, bg, italic, markBar: r.color ? p.markFg[r.color] : null, breakpointDot };
}

export type Tone = "ok" | "info" | "warn" | "err" | "muted" | "violet";

export interface Pill {
  text: string;
  tone: Tone;
}

/** Method badge: the verb's colour tells reads, writes and deletes apart at a glance. */
export function methodPill(r: SessionSummary): Pill | null {
  const m = r.method.toUpperCase();
  if (!m) return null;
  switch (m) {
    case "GET":
      return { text: m, tone: "info" };
    case "POST":
      return { text: m, tone: "ok" };
    case "PUT":
    case "PATCH":
      return { text: m, tone: "warn" };
    case "DELETE":
      return { text: m, tone: "err" };
    case "CONNECT":
    case "HEAD":
    case "OPTIONS":
      return { text: m, tone: "muted" };
    default:
      return { text: m, tone: "violet" };
  }
}

/** Status badge; null while no status is known yet. */
export function statusPill(r: SessionSummary): Pill | null {
  if (r.state === "aborted" && !r.status) return { text: "ERR", tone: "err" };
  const s = r.status;
  if (!s) return null;
  const text = String(s);
  if (s >= 500) return { text, tone: "err" };
  if (s >= 400) return { text, tone: "warn" };
  if (s >= 300) return { text, tone: "muted" };
  if (s >= 200) return { text, tone: "ok" };
  return { text, tone: "info" };
}

/** Colour of the thin state bar at the row's left edge, or null for a finished session. */
export function stateMark(r: SessionSummary, p: Palette): string | null {
  switch (r.state) {
    case "breakpointRequest":
    case "breakpointResponse":
      return null;
    case "aborted":
      return p.red;
    case "requestHeaders":
    case "sendingRequest":
    case "awaitingResponse":
    case "receivingResponse":
      return p.focus;
  }
  if (r.flags & Flags.AUTO_RESPONDED) return p.purple;
  if (r.kind === "webSocket") return p.blue;
  return null;
}
