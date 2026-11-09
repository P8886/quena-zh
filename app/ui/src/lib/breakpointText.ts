import { t } from "../i18n";

// BreakpointState::labels() emits two descriptions and otherwise executable conditions.
// Translate the descriptions; URLs, methods and command syntax must remain unchanged.
export function breakpointText(label: string): string {
  if (label === "before requests") return t("Break before requests");
  if (label === "after responses") return t("Break after responses");
  return label;
}
