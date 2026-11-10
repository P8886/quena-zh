import type { SessionSummary } from "../api";
import { t } from "../i18n";
import type { Palette, Tone } from "./style";

// Small vector glyphs follow the Lucide style used by the rest of the UI (ISC; see NOTICE).
// Paths use a 24px viewBox and are compiled once, never once per captured session.
const ICONS = {
  aborted: { tone: "err", path: "M8 3h8l5 5v8l-5 5H8l-5-5V8z M9 9l6 6 M15 9l-6 6" },
  error: { tone: "err", path: "m12 3 10 18H2z M12 9v5 M12 17h.01" },
  auth: { tone: "warn", path: "M5 11h14v10H5z M8 11V7a4 4 0 0 1 8 0v4 M12 15v2" },
  redirect: { tone: "info", path: "M4 20v-8a5 5 0 0 1 5-5h11 M15 2l5 5-5 5" },
  cached: { tone: "muted", path: "M4 3h10l5 5v13H4z M14 3v5h5 M8 13l2 2 5-5" },
  partial: { tone: "info", path: "M5 2h9l5 5v15H5z M14 2v6h5 M8 12h8 M8 16h4 M14 16h2" },
  empty: { tone: "muted", path: "M22 12a10 10 0 1 0-20 0 10 10 0 0 0 20 0 M7 12h10" },
  pending: { tone: "muted", path: "M22 12a10 10 0 1 0-20 0 10 10 0 0 0 20 0 M12 6v6l4 2" },
  receiving: { tone: "info", path: "M12 3v12 M7 10l5 5 5-5 M4 17v4h16v-4" },
  unlocked: { tone: "warn", path: "M5 11h14v10H5z M9 11V7a4 4 0 0 1 7-3 M12 15v2" },
  post: { tone: "ok", path: "M4 3h10l5 5v13H4z M14 3v5h5 M7 15h9 M12 11l4 4-4 4" },
  flash: { tone: "warn", path: "m13 2-9 12h7l-1 8 10-13h-7z" },
  silverlight: { tone: "info", path: "m12 2 3 7 7 3-7 3-3 7-3-7-7-3 7-3z" },
  rpc: { tone: "violet", path: "M3 5h6v6H3z M15 13h6v6h-6z M9 8h11l-3-3 M15 16H4l3 3" },
  json: { tone: "violet", path: "M8 3H7a2 2 0 0 0-2 2v5a2 2 0 0 1-2 2 2 2 0 0 1 2 2v5a2 2 0 0 0 2 2h1 M16 21h1a2 2 0 0 0 2-2v-5a2 2 0 0 1 2-2 2 2 0 0 1-2-2V5a2 2 0 0 0-2-2h-1" },
  javascript: { tone: "warn", path: "M4 4h16v16H4z M10 8v7a2 2 0 0 1-4 0 M18 9a2 2 0 0 0-4 0c0 3 4 2 4 5a2 2 0 0 1-4 0" },
  css: { tone: "violet", path: "M4 9h16 M3 15h16 M10 3 8 21 M16 3l-2 18" },
  html: { tone: "info", path: "M3 4h18v16H3z M3 8h18 M6 6h.01 M9 6h.01 M9 11l-3 3 3 3 M15 11l3 3-3 3" },
  xml: { tone: "warn", path: "m18 16 4-4-4-4 M6 8l-4 4 4 4 M14.5 4l-5 16" },
  image: { tone: "ok", path: "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z M11 8a2 2 0 1 0-4 0 2 2 0 0 0 4 0 M21 15l-4-4-10 10" },
  font: { tone: "info", path: "M4 20 12 4l8 16 M7 14h10" },
  options: { tone: "info", path: "M22 12a10 10 0 1 0-20 0 10 10 0 0 0 20 0 M12 11v6 M12 7h.01" },
  head: { tone: "muted", path: "M3 5h18 M3 10h18 M3 15h10 M3 20h7" },
  websocket: { tone: "info", path: "m8 3-4 4 4 4 M4 7h16 M16 21l4-4-4-4 M20 17H4" },
  tunnel: { tone: "muted", path: "M3 6h6v6H3z M15 12h6v6h-6z M9 9h3a6 6 0 0 1 6 3 M6 3v3 M18 18v3" },
  sse: { tone: "violet", path: "M14 12a2 2 0 1 0-4 0 2 2 0 0 0 4 0 M5 5a10 10 0 0 0 0 14 M19 5a10 10 0 0 1 0 14 M8 8a6 6 0 0 0 0 8 M16 8a6 6 0 0 1 0 8" },
  grpc: { tone: "info", path: "M9 2h6v6H9z M2 16h6v6H2z M16 16h6v6h-6z M12 8v4 M5 16v-4h14v4" },
  protobuf: { tone: "warn", path: "m12 3 9 5v8l-9 5-9-5V8z M3 8l9 5 9-5 M12 13v8 M7 5l10 6" },
  form: { tone: "ok", path: "M3 5h4v4H3z M3 15h4v4H3z M11 7h10 M11 17h10" },
  audio: { tone: "info", path: "M9 18V5l12-2v13 M9 18a3 3 0 1 0-6 0 3 3 0 0 0 6 0 M21 16a3 3 0 1 0-6 0 3 3 0 0 0 6 0" },
  video: { tone: "info", path: "M3 5h12v14H3z M15 9l6-4v14l-6-4" },
  pdf: { tone: "warn", path: "M5 2h9l5 5v15H5z M14 2v6h5 M9 18v-7h3a2 2 0 0 1 0 4H9" },
  archive: { tone: "warn", path: "M5 2h9l5 5v15H5z M14 2v6h5 M10 3v2h2v2h-2v2h2v2h-2v2h2v4h-3v-4" },
  text: { tone: "muted", path: "M5 2h9l5 5v15H5z M14 2v6h5 M8 12h8 M8 16h8 M8 19h5" },
  binary: { tone: "muted", path: "M4 6h3v13 M3 19h6 M14 5h4v14h-4z" },
  request: { tone: "info", path: "M4 3h10l5 5v13H4z M14 3v5h5 M7 15h9 M12 11l4 4-4 4" },
} satisfies Record<string, { tone: Tone; path: string }>;

export type SessionIcon = keyof typeof ICONS;

const EXTENSIONS: Record<string, SessionIcon> = {
  json: "json", jsonl: "json", js: "javascript", mjs: "javascript", cjs: "javascript", css: "css",
  html: "html", htm: "html", xhtml: "html", xml: "xml", xsl: "xml", xslt: "xml",
  png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image", avif: "image", svg: "image", ico: "image", bmp: "image",
  woff: "font", woff2: "font", ttf: "font", otf: "font", eot: "font",
  mp3: "audio", wav: "audio", ogg: "audio", m4a: "audio", flac: "audio",
  mp4: "video", webm: "video", mov: "video", m3u8: "video", pdf: "pdf",
  zip: "archive", gz: "archive", tgz: "archive", tar: "archive", rar: "archive", "7z": "archive",
  txt: "text", csv: "text", log: "text", proto: "protobuf", wasm: "binary", bin: "binary", swf: "flash", xap: "silverlight",
};
const URL_ICONS = new Set(Object.values(EXTENSIONS));

/** Pauses replace icons; outcomes precede response types. URLs are only a fallback. */
export function sessionIcon(r: SessionSummary, unlocked = false): SessionIcon | null {
  if (r.state === "breakpointRequest" || r.state === "breakpointResponse") return null;
  if (unlocked) return "unlocked";
  if (r.state === "aborted") return "aborted";
  if (r.status === 401 || r.status === 403 || r.status === 407) return "auth";
  if (r.status >= 400) return "error";
  if ([301, 302, 303, 307, 308].includes(r.status)) return "redirect";
  if (r.status === 304) return "cached";
  if (r.status === 206) return "partial";
  if (r.kind === "webSocket") return "websocket";
  const method = r.method.toUpperCase();
  if (r.kind === "tunnel" || method === "CONNECT") return "tunnel";
  if (method === "RPC_IN_DATA" || method === "RPC_OUT_DATA") return "rpc";
  if (r.state === "requestHeaders" || r.state === "sendingRequest" || r.state === "awaitingResponse") return "pending";
  if (method === "OPTIONS") return "options";
  if (method === "HEAD") return "head";
  if (r.status === 204 || r.status === 205) return "empty";
  const mime = r.contentType.split(";", 1)[0].trim().toLowerCase();
  if (mime === "text/event-stream") return "sse";
  if (r.state === "receivingResponse") return "receiving";
  if (!mime) {
    const extension = r.url.split(/[?#]/, 1)[0].match(/\.([a-z0-9]+)$/i)?.[1].toLowerCase();
    return extension && Object.prototype.hasOwnProperty.call(EXTENSIONS, extension) ? EXTENSIONS[extension] : method === "POST" ? "post" : "request";
  }
  if (mime.startsWith("image/")) return "image"; // SVG is an image, even with its +xml suffix.
  if (mime.startsWith("font/") || /^(application\/(font-(woff2?|sfnt)|x-(font-\w+|woff)|vnd\.ms-(fontobject|opentype)))$/.test(mime)) return "font";
  if (mime.startsWith("application/grpc")) return "grpc";
  if (mime === "application/x-shockwave-flash") return "flash";
  if (mime === "application/x-silverlight-app") return "silverlight";
  if (/(?:[/+]json|\/x-json)$/.test(mime) || ["application/x-ndjson", "application/ndjson", "application/json-seq"].includes(mime)) return "json";
  if (/^(text|application)\/(x-)?(javascript|ecmascript)$/.test(mime)) return "javascript";
  if (mime === "text/css") return "css";
  if (mime === "text/html" || mime === "application/xhtml+xml") return "html";
  if (/(?:[/+]xml)$/.test(mime)) return "xml";
  if (mime.startsWith("audio/")) return "audio";
  if (mime.startsWith("video/") || mime === "application/vnd.apple.mpegurl" || mime === "application/x-mpegurl") return "video";
  if (mime === "application/pdf") return "pdf";
  if (/^application\/(zip|x-zip-compressed|gzip|x-gzip|x-tar|x-7z-compressed|vnd\.rar|x-rar-compressed)$/.test(mime)) return "archive";
  if (["application/protobuf", "application/x-protobuf", "application/vnd.google.protobuf", "application/proto"].includes(mime)) return "protobuf";
  if (mime === "application/x-www-form-urlencoded" || mime.startsWith("multipart/")) return "form";
  if (mime === "application/octet-stream" || mime === "application/wasm") return "binary";
  if (mime.startsWith("text/")) return "text";
  return method === "POST" ? "post" : "request";
}

function iconLabel(icon: SessionIcon): string {
  const labels: Record<SessionIcon, string> = {
    aborted: t("Session aborted"), error: t("HTTP error response"), auth: t("Authentication required or access denied"),
    redirect: t("Redirect response"), cached: t("304: use the cached response"), partial: t("206: partial content"),
    empty: t("204/205: no response body"), pending: t("Sending request or waiting for response"), receiving: t("Receiving response body"),
    unlocked: t("Session unlocked for editing"), post: t("POST request"),
    flash: t("Flash resource"), silverlight: t("Silverlight resource"), rpc: t("RPC over HTTP tunnel"),
    json: t("JSON response"), javascript: t("JavaScript resource"), css: t("CSS stylesheet"), html: t("HTML page"), xml: t("XML response"),
    image: t("Image resource"), font: t("Font resource"), audio: t("Audio resource"), video: t("Video resource"),
    pdf: t("PDF document"), archive: t("Archive download"), text: t("Text response"), binary: t("Binary response"), form: t("Form data response"),
    options: t("OPTIONS (often a CORS preflight)"), head: t("HEAD (response headers only)"),
    websocket: t("WebSocket connection"), tunnel: t("CONNECT tunnel"), sse: t("Server-Sent Events (SSE)"),
    grpc: t("gRPC response"), protobuf: t("Protobuf response"), request: t("HTTP request (response type unknown)"),
  };
  return labels[icon];
}

export function sessionIconTitle(r: SessionSummary, unlocked = false): string {
  if (r.state === "breakpointRequest") return t("Paused before sending the request");
  if (r.state === "breakpointResponse") return t("Paused before returning the response");
  const icon = sessionIcon(r, unlocked)!;
  if (icon === "request" && r.contentType) return t("Other response type: {type}", { type: r.contentType });
  const label = iconLabel(icon);
  return !r.contentType.trim() && URL_ICONS.has(icon)
    ? t("Type inferred from URL: {type}", { type: label }) : label;
}

const paths = new Map<SessionIcon, Path2D>();

export function drawSessionIcon(ctx: CanvasRenderingContext2D, icon: SessionIcon, x: number, y: number, size: number, palette: Palette, foreground?: string) {
  let path = paths.get(icon);
  if (!path) paths.set(icon, path = new Path2D(ICONS[icon].path));
  ctx.save();
  ctx.translate(x, y);
  ctx.scale(size / 24, size / 24);
  ctx.strokeStyle = foreground ?? palette.tones[ICONS[icon].tone].fg;
  ctx.lineWidth = 2;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.stroke(path);
  ctx.restore();
}
