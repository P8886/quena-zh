import { describe, expect, it } from "vitest";
import { Flags, type SessionSummary } from "../api";
import { rowStyle, stateMark, statusPill, type Palette } from "./style";

const palette: Palette = {
  bg: "background", bgAlt: "alternate", fg: "text", muted: "muted",
  red: "error", blue: "blue", green: "green", purple: "purple", gray: "gray", amber: "paused",
  breakpointRequest: "request-dot", breakpointResponse: "response-dot",
  selBg: "selected", selInactiveBg: "selected-inactive", selFg: "selected-text", focus: "focus", grid: "grid",
  marks: { orange: "pause-background", purple: "match-background" },
  markFg: { orange: "pause-text", purple: "match-text" },
  tones: {
    ok: { fg: "ok-text", bg: "ok" }, info: { fg: "info-text", bg: "info" },
    warn: { fg: "warn-text", bg: "warn" }, err: { fg: "error-text", bg: "error" },
    muted: { fg: "muted-text", bg: "muted" }, violet: { fg: "violet-text", bg: "violet" },
  },
};

const session: SessionSummary = {
  id: 1, kind: "http", state: "done", flags: 0, color: null, method: "GET", protocol: "HTTPS",
  host: "example.test", url: "/test", status: 200, requestBodyLen: 0, responseBodyLen: 100,
  contentType: "application/json", caching: "", process: "", comment: "", custom: "",
  startedAt: 0, durationMs: 100, clientIp: "127.0.0.1", conn: 1,
};

describe("session row colour priority", () => {
  it.each(["breakpointRequest", "breakpointResponse"] as const)("keeps the search background during %s and removes the dot after resuming", (state) => {
    const paused = { ...session, state, color: "purple" as const };
    const breakpointDot = state === "breakpointRequest" ? palette.breakpointRequest : palette.breakpointResponse;
    expect(rowStyle(paused, palette)).toMatchObject({
      bg: palette.marks.purple, fg: palette.markFg.purple, italic: false,
      markBar: palette.markFg.purple, breakpointDot,
    });
    expect(rowStyle({ ...paused, color: null }, palette)).toMatchObject({
      bg: null, fg: palette.fg, bold: false, italic: false, breakpointDot,
    });
    const resumed = rowStyle({ ...paused, state: "done", flags: Flags.BREAKPOINTED }, palette);
    expect(resumed).toMatchObject({ bg: palette.marks.purple, markBar: palette.markFg.purple, breakpointDot: null });
    expect(stateMark(paused, palette)).toBeNull();
  });

  it.each(["active", "inactive"] as const)("keeps %s selection readable over errors, pauses and search marks", (selection) => {
    for (const state of ["aborted", "breakpointRequest", "breakpointResponse", "done"] as const) {
      for (const color of [null, "purple"] as const) {
        expect(rowStyle({ ...session, state, color }, palette, selection)).toMatchObject({
          bg: selection === "active" ? palette.selBg : palette.selInactiveBg,
          fg: palette.selFg,
          markBar: color ? palette.markFg[color] : null,
          breakpointDot: state === "breakpointRequest" ? palette.breakpointRequest
            : state === "breakpointResponse" ? palette.breakpointResponse : null,
        });
      }
    }
  });

  it("does not show a dot for a resumed session even if its breakpoint history flag remains", () => {
    for (const state of ["requestHeaders", "sendingRequest", "awaitingResponse", "receivingResponse", "done", "aborted"] as const) {
      expect(rowStyle({ ...session, state, flags: Flags.BREAKPOINTED }, palette).breakpointDot).toBeNull();
    }
  });

  it("retains an error indicator on a selected, search-marked session", () => {
    const failed = { ...session, state: "aborted" as const, status: 0, color: "purple" as const };
    expect(rowStyle(failed, palette, "inactive").fg).toBe(palette.selFg);
    expect(stateMark(failed, palette)).toBe(palette.red);
    expect(statusPill(failed)).toEqual({ text: "ERR", tone: "err" });
  });
});

describe("breakpoint status badges", () => {
  it("leaves an unknown HTTP status to the renderer instead of adding another pause badge", () => {
    expect(statusPill({ ...session, status: 0, state: "breakpointRequest" })).toBeNull();
    expect(statusPill({ ...session, status: 0, state: "awaitingResponse" })).toBeNull();
  });

  it.each([200, 404, 502])("keeps HTTP status %i visible when its response is paused", (status) => {
    expect(statusPill({ ...session, status, state: "breakpointResponse" })).toEqual(statusPill({ ...session, status }));
  });
});
