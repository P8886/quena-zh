import { t } from "../i18n";

/** Keep expected editor errors readable; preserve diagnostic details for other failures. */
export function editingError(error: unknown): string {
  const message = String(error).replace(/^Error: /, "");
  switch (message) {
    case "the body is incomplete or its recording was truncated":
      return t("The body is incomplete or its recording was truncated. Replace it with a complete file to edit it.");
    case "the decoded body exceeds the 8 MiB editing limit; replace it with a file":
      return t("The decoded body exceeds 8 MiB. Replace it with a file to edit it.");
  }
  if (message.startsWith("unsupported Content-Encoding:")) return t("This Content-Encoding cannot be decoded for editing: {encoding}", { encoding: message.slice(message.indexOf(":") + 1).trim() });
  if (message.startsWith("could not decode the complete body:")) return t("The compressed body could not be decoded completely: {error}", { error: message.slice(message.indexOf(":") + 1).trim() });
  return message;
}
