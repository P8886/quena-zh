// Drafts belong to messages, so changing the selection or inspector tab never discards them.
import { api, type Part, type Resume, type SessionId } from "./api";
import { get, say, set } from "./store";
import { t } from "./i18n";
import { editingError } from "./lib/editingText";

export interface MessageEdits {
  head: string | null;
  body: string | null;
  file: string | null;
  charset?: string | null;
}

export const EMPTY_EDITS: MessageEdits = { head: null, body: null, file: null };
export const messageKey = (id: SessionId, part: Part) => `${id}:${part}`;

export function patchMessageEdits(id: SessionId, part: Part, patch: Partial<MessageEdits>) {
  const key = messageKey(id, part);
  set((s) => ({ messageDrafts: { ...s.messageDrafts, [key]: { ...(s.messageDrafts[key] ?? EMPTY_EDITS), ...patch } } }));
}

export function discardMessageEdits(id: SessionId, part: Part) {
  const key = messageKey(id, part);
  set((s) => {
    const drafts = { ...s.messageDrafts };
    delete drafts[key];
    return { messageDrafts: drafts };
  });
}

export function messageEdit(edits: MessageEdits): Omit<Resume, "action"> {
  return { headText: edits.head, bodyText: edits.body, bodyCharset: edits.charset, bodyFile: edits.file };
}

export const hasSessionEdits = (id: SessionId) => !!(get().messageDrafts[messageKey(id, "request")] || get().messageDrafts[messageKey(id, "response")]);

/** Save before replay too: F2 edits and the next Shift+R use the same request. */
const saves = new Map<SessionId, Promise<boolean>>();
export function saveSessionEdits(id: SessionId): Promise<boolean> {
  const previous = saves.get(id);
  // F2 and Replay can overlap. A slower old save must never overwrite a newer one.
  const saving = previous ? previous.catch(() => {}).then(() => saveDraftSnapshot(id)) : saveDraftSnapshot(id);
  saves.set(id, saving);
  const done = () => { if (saves.get(id) === saving) saves.delete(id); };
  void saving.then(done, done);
  return saving;
}

async function saveDraftSnapshot(id: SessionId) {
  const drafts = get().messageDrafts;
  const req = drafts[messageKey(id, "request")];
  const resp = drafts[messageKey(id, "response")];
  if (!req && !resp) return true;
  await api.editSession(id, { request: req ? messageEdit(req) : null, response: resp ? messageEdit(resp) : null });
  // Keep anything typed while saving; a completed save must not erase a newer draft.
  for (const [part, saved] of [["request", req], ["response", resp]] as const) {
    if (get().messageDrafts[messageKey(id, part)] === saved) discardMessageEdits(id, part);
  }
  set((s) => ({ gridNonce: s.gridNonce + 1 }));
  return !hasSessionEdits(id);
}

export function lockSession(id: SessionId) {
  if (hasSessionEdits(id)) return;
  set((s) => ({ unlockedSessions: new Set([...s.unlockedSessions].filter((n) => n !== id)) }));
}

export async function toggleSessionEditor() {
  const id = get().focusId;
  if (id == null || get().selection.size !== 1) return;
  try {
    if (get().unlockedSessions.has(id)) {
      if (await saveSessionEdits(id)) lockSession(id);
      else say(t("New edits were made while saving. They remain open for review."));
      return;
    }
    const d = await api.detail(id);
    if (!d || d.summary.kind !== "http" || !["done", "aborted"].includes(d.summary.state)) {
      say(t("Only finished HTTP sessions can be unlocked for editing."), "error");
      return;
    }
    set((s) => ({ unlockedSessions: new Set([...s.unlockedSessions, id]), activeTab: "inspectors" }));
  } catch (e) {
    say(editingError(e), "error");
  }
}
