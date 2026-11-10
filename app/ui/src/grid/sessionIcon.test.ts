import { describe, expect, it } from "vitest";
import { Flags, type SessionSummary } from "../api";
import { sessionIcon, sessionIconTitle } from "./sessionIcon";

const base: SessionSummary = {
  id: 1, kind: "http", state: "done", flags: 0, color: null, method: "GET", protocol: "HTTPS",
  host: "example.test", url: "/api/items", status: 200, requestBodyLen: 0, responseBodyLen: 100,
  contentType: "application/json", caching: "", process: "", comment: "", custom: "",
  startedAt: 0, durationMs: 100, clientIp: "127.0.0.1", conn: 1,
};

describe("session type icons", () => {
  it.each([
    ["application/json", "json"], ["Application/Problem+Json; charset=utf-8", "json"],
    ["application/ld+json", "json"], ["application/x-ndjson", "json"], ["application/json-seq", "json"],
    ["text/javascript", "javascript"], ["application/x-javascript", "javascript"],
    ["text/css", "css"], ["text/html", "html"], ["application/xhtml+xml", "html"],
    ["application/soap+xml", "xml"], ["image/svg+xml", "image"], ["image/avif", "image"],
    ["font/woff2", "font"], ["application/font-woff", "font"], ["application/x-font-ttf", "font"],
    ["application/vnd.ms-fontobject", "font"], ["audio/mpeg", "audio"], ["video/mp4", "video"],
    ["application/vnd.apple.mpegurl", "video"], ["text/event-stream; charset=utf-8", "sse"],
    ["application/grpc+proto", "grpc"], ["application/grpc-web-text", "grpc"],
    ["application/x-protobuf", "protobuf"], ["application/proto", "protobuf"],
    ["application/pdf", "pdf"], ["application/zip", "archive"], ["application/x-7z-compressed", "archive"],
    ["application/x-shockwave-flash", "flash"], ["application/x-silverlight-app", "silverlight"],
    ["multipart/form-data; boundary=example", "form"], ["application/x-www-form-urlencoded", "form"],
    ["application/octet-stream", "binary"], ["application/wasm", "binary"], ["text/plain", "text"],
  ])("uses the actual response type %s", (contentType, icon) => {
    expect(sessionIcon({ ...base, contentType })).toBe(icon);
  });

  it("identifies protocols and header-only methods before the response content type", () => {
    expect(sessionIcon({ ...base, kind: "webSocket", method: "CONNECT" })).toBe("websocket");
    expect(sessionIcon({ ...base, kind: "tunnel", url: "/script.js" })).toBe("tunnel");
    expect(sessionIcon({ ...base, method: "CONNECT" })).toBe("tunnel");
    expect(sessionIcon({ ...base, method: "OPTIONS" })).toBe("options");
    expect(sessionIcon({ ...base, method: "HEAD" })).toBe("head");
    expect(sessionIcon({ ...base, method: "POST" })).toBe("json");
  });

  it.each([
    ["/logo.SVG?v=1", "image"], ["/app.js?payload=file.json", "javascript"],
    ["/fonts/regular.woff2", "font"], ["/export/data.json#part", "json"],
    ["/api/items?file=image.png", "request"], ["/path.constructor", "request"],
  ])("only infers a type from the URL path when Content-Type is absent: %s", (url, icon) => {
    const inferred = { ...base, contentType: "", url };
    expect(sessionIcon(inferred)).toBe(icon);
    if (icon !== "request") expect(sessionIconTitle(inferred)).toContain("URL");
    expect(sessionIcon({ ...inferred, contentType: "text/plain" })).toBe("text");
    expect(sessionIcon({ ...inferred, contentType: "application/octet-stream" })).toBe("binary");
  });

  it("keeps an unknown response type generic instead of guessing from its URL or size", () => {
    expect(sessionIcon({ ...base, contentType: "application/custom", url: "/data.json" })).toBe("request");
    expect(sessionIcon({ ...base, contentType: "", responseBodyLen: 0 })).toBe("request");
  });

  it.each(["breakpointRequest", "breakpointResponse"] as const)("hides type icons during %s and restores them after resuming", (state) => {
    expect(sessionIcon({ ...base, state })).toBeNull();
    expect(sessionIcon({ ...base, state, kind: "webSocket" })).toBeNull();
    expect(sessionIcon({ ...base, state: "done", flags: Flags.BREAKPOINTED })).toBe("json");
    expect(sessionIcon({ ...base, state: "aborted", flags: Flags.BREAKPOINTED })).toBe("aborted");
    expect(sessionIconTitle({ ...base, state })).toContain("Paused");
  });

  it.each([
    [301, "redirect"], [302, "redirect"], [303, "redirect"], [307, "redirect"], [308, "redirect"],
    [304, "cached"], [206, "partial"], [204, "empty"], [205, "empty"],
    [401, "auth"], [403, "auth"], [407, "auth"], [404, "error"], [500, "error"], [502, "error"],
  ])("shows the HTTP outcome for status %i even with a JSON response", (status, icon) => {
    const response = { ...base, status };
    expect(sessionIcon(response)).toBe(icon);
    expect(sessionIcon({ ...response, state: "breakpointResponse" })).toBeNull();
    expect(sessionIcon({ ...response, state: "aborted" })).toBe("aborted");
    expect(sessionIconTitle({ ...response, contentType: "" })).not.toContain("inferred");
  });

  it("shows live states and RPC streams without using a resource extension to guess", () => {
    for (const state of ["requestHeaders", "sendingRequest", "awaitingResponse"] as const) {
      expect(sessionIcon({ ...base, state, status: 0 })).toBe("pending");
    }
    expect(sessionIcon({ ...base, state: "receivingResponse" })).toBe("receiving");
    expect(sessionIcon({ ...base, state: "receivingResponse", contentType: "text/event-stream" })).toBe("sse");
    for (const method of ["RPC_IN_DATA", "RPC_OUT_DATA"]) expect(sessionIcon({ ...base, method })).toBe("rpc");
  });

  it("distinguishes generic POST requests while preserving known response types", () => {
    expect(sessionIcon({ ...base, method: "POST", contentType: "" })).toBe("post");
    expect(sessionIcon({ ...base, method: "POST", contentType: "application/custom" })).toBe("post");
    expect(sessionIcon({ ...base, method: "POST" })).toBe("json");
  });

  it("shows the unlocked session icon until editing ends, with breakpoints taking priority", () => {
    expect(sessionIcon(base, true)).toBe("unlocked");
    expect(sessionIcon({ ...base, state: "aborted" }, true)).toBe("unlocked");
    expect(sessionIcon({ ...base, state: "breakpointRequest" }, true)).toBeNull();
    expect(sessionIcon({ ...base, state: "breakpointResponse" }, true)).toBeNull();
    expect(sessionIcon(base, false)).toBe("json");
  });
});
