import { t } from "../i18n";

/** Keep expected editor errors readable; preserve diagnostic details for other failures. */
export function editingError(error: unknown): string {
  const message = String(error).replace(/^Error: /, "");
  switch (message) {
    case "the body is incomplete or its recording was truncated":
      return t("The body is incomplete or its recording was truncated. Replace it with a complete file to edit it.");
    case "the decoded body exceeds the 8 MiB editing limit; replace it with a file":
      return t("The decoded body exceeds 8 MiB. Replace it with a file to edit it.");
    case "not enough storage to save the edited body":
      return t("There is not enough storage to save the edited body.");
    case "only finished HTTP sessions can be unlocked for editing":
      return t("Only finished HTTP sessions can be unlocked for editing.");
    case "the request URL must start with http:// or https://":
      return t("The request URL must start with http:// or https://.");
    case "no response to edit":
      return t("This session has no response to edit.");
    case "invalid request start line; use METHOD URL HTTP/1.1":
      return t("Invalid request start line. Use METHOD URL HTTP/1.1.");
    case "invalid response start line; use HTTP/1.1 200 OK":
      return t("Invalid response start line. Use HTTP/1.1 200 OK.");
  }
  if (message.startsWith("unsupported Content-Encoding:")) return t("This Content-Encoding cannot be decoded for editing: {encoding}", { encoding: message.slice(message.indexOf(":") + 1).trim() });
  if (message.startsWith("could not decode the complete body:")) return t("The compressed body could not be decoded completely: {error}", { error: message.slice(message.indexOf(":") + 1).trim() });
  return message;
}
