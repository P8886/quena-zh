import { t } from "../i18n";

// BreakpointState::labels() emits two descriptions and otherwise executable conditions.
// Translate the descriptions; URLs, methods and command syntax must remain unchanged.
export function breakpointText(label: string): string {
  if (label === "before requests") return t("Break before requests");
  if (label === "after responses") return t("Break after responses");
  if (/^rules \d+$/.test(label)) return t("Breakpoint rules ({n})", { n: Number(label.slice(6)) });
  return label;
}

export function breakpointRuleError(error: unknown): string {
  const message = String(error).replace(/^Error: /, "");
  if (message.startsWith("invalid breakpoint client IP:")) return t("Enter a valid IPv4 or IPv6 address without a port.");
  if (message.startsWith("invalid breakpoint method:")) return t("Enter a valid HTTP method, such as GET or POST.");
  if (message === "use a hostname such as api.example.com or *.example.com") return t("Enter a hostname such as api.example.com or *.example.com, without a URL scheme or path.");
  if (message === "a response status must be between 100 and 599") return t("The response status must be between 100 and 599.");
  if (message === "a response status requires a response breakpoint") return t("To match a response status, choose After Responses.");
  return message;
}
