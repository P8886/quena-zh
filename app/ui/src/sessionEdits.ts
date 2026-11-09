// Drafts belong to messages, so changing the selection or inspector tab never discards them.
import type { Part, Resume, SessionId } from "./api";
import { set } from "./store";

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
