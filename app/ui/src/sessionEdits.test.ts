import { beforeEach, expect, it, vi } from "vitest";
import { api, type Detail } from "./api";
import { get, set } from "./store";
import { discardMessageEdits, lockSession, messageKey, patchMessageEdits, saveSessionEdits, toggleSessionEditor } from "./sessionEdits";
import { startReplay } from "./replay";
import { actions } from "./actions";

vi.mock("./grid/SessionGrid", () => ({ grid: {}, rowCache: {}, idAtIndex: vi.fn() }));

beforeEach(() => {
  vi.restoreAllMocks();
  set({ focusId: 1, selection: new Set([1]), messageDrafts: {}, unlockedSessions: new Set(), gridNonce: 0 });
});

it("keeps each session and message's draft when selection and inspector tabs change", () => {
  patchMessageEdits(1, "request", { body: "edited A" });
  patchMessageEdits(1, "response", { head: "HTTP/1.1 201 Created" });
  set({ focusId: 2, selection: new Set([2]), activeTab: "composer" });
  patchMessageEdits(2, "request", { body: "edited B" });
  set({ focusId: 1, selection: new Set([1]), activeTab: "inspectors" });
  expect(get().messageDrafts).toMatchObject({
    "1:request": { body: "edited A" }, "1:response": { head: "HTTP/1.1 201 Created" }, "2:request": { body: "edited B" },
  });
  discardMessageEdits(1, "request");
  expect(get().messageDrafts["1:request"]).toBeUndefined();
  expect(get().messageDrafts["1:response"].head).toBe("HTTP/1.1 201 Created");
});

it("retains newer input made during a save and keeps the session unlocked", async () => {
  let finish!: () => void;
  const edit = vi.spyOn(api, "editSession").mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  patchMessageEdits(1, "request", { body: "first", charset: "UTF-8" });
  set({ unlockedSessions: new Set([1]) });
  const saving = toggleSessionEditor();
  patchMessageEdits(1, "request", { body: "newer" });
  finish();
  await saving;
  expect(edit).toHaveBeenCalledWith(1, { request: { headText: null, bodyText: "first", bodyFile: null, bodyCharset: "UTF-8" }, response: null });
  expect(get().messageDrafts[messageKey(1, "request")].body).toBe("newer");
  expect(get().unlockedSessions.has(1)).toBe(true);
  lockSession(1);
  expect(get().unlockedSessions.has(1)).toBe(true);
});

it("leaves all drafts available if saving fails", async () => {
  vi.spyOn(api, "editSession").mockRejectedValue(new Error("storage full"));
  patchMessageEdits(1, "request", { body: "keep this" });
  patchMessageEdits(1, "response", { body: "and this" });
  const drafts = get().messageDrafts;
  await expect(saveSessionEdits(1)).rejects.toThrow("storage full");
  expect(get().messageDrafts).toBe(drafts);
  expect(get().gridNonce).toBe(0);
});

it("serializes overlapping saves so an older request cannot overwrite newer input", async () => {
  let finish!: () => void;
  const edit = vi.spyOn(api, "editSession").mockImplementationOnce(() => new Promise(resolve => { finish = resolve; })).mockResolvedValue();
  patchMessageEdits(1, "request", { body: "older" });
  const first = saveSessionEdits(1);
  patchMessageEdits(1, "request", { body: "newer" });
  const second = saveSessionEdits(1);
  expect(edit).toHaveBeenCalledTimes(1);
  finish();
  expect(await first).toBe(false);
  expect(await second).toBe(true);
  expect(edit.mock.calls.map(([, e]) => e.request?.bodyText)).toEqual(["older", "newer"]);
  expect(get().messageDrafts).toEqual({});
});

it("F2 unlocks only finished HTTP and saves both sides before locking", async () => {
  const detail = vi.spyOn(api, "detail").mockResolvedValue({ summary: { kind: "http", state: "done" } } as Detail);
  const edit = vi.spyOn(api, "editSession").mockResolvedValue();
  await toggleSessionEditor();
  expect(get().unlockedSessions.has(1)).toBe(true);
  patchMessageEdits(1, "request", { body: "request" });
  patchMessageEdits(1, "response", { body: "response" });
  await toggleSessionEditor();
  expect(edit).toHaveBeenCalledTimes(1);
  expect(get().messageDrafts).toEqual({});
  expect(get().unlockedSessions.has(1)).toBe(false);
  detail.mockResolvedValue({ summary: { kind: "http", state: "breakpointRequest" } } as Detail);
  await toggleSessionEditor();
  expect(get().unlockedSessions.size).toBe(0);
});

it("saves F2 drafts before issuing a replay, and does not replay after a failed save", async () => {
  const calls: string[] = [];
  vi.spyOn(api, "editSession").mockImplementation(async () => { calls.push("save"); });
  const replay = vi.spyOn(api, "replay").mockImplementation(async () => { calls.push("replay"); return 1; });
  set({ unlockedSessions: new Set([1]) });
  patchMessageEdits(1, "request", { body: "edited" });
  await startReplay([1], { count: 1 });
  expect(calls).toEqual(["save", "replay"]);
  expect(get().messageDrafts).toEqual({});
  patchMessageEdits(1, "request", { body: "new edit" });
  vi.mocked(api.editSession).mockRejectedValueOnce("save failed");
  await startReplay([1], { count: 1 });
  expect(replay).toHaveBeenCalledTimes(1);
  expect(get().messageDrafts["1:request"].body).toBe("new edit");
});

it("command-field go applies the paused message's draft before releasing other sessions", async () => {
  const calls: string[] = [];
  vi.spyOn(api, "bpPaused").mockResolvedValue([{ id: 1, phase: "request", url: "http://example.test/", since: 0 }]);
  const resume = vi.spyOn(api, "bpResume").mockImplementation(async () => { calls.push("edited"); });
  vi.spyOn(api, "bpGo").mockImplementation(async () => { calls.push("rest"); return 2; });
  const quickexec = vi.spyOn(api, "quickexec");
  patchMessageEdits(1, "request", { body: "phone edit" });
  patchMessageEdits(2, "request", { body: "unrelated history" });
  expect(await actions.quickexec(" GO ")).toBe(true);
  expect(calls).toEqual(["edited", "rest"]);
  expect(resume).toHaveBeenCalledWith(1, expect.objectContaining({ bodyText: "phone edit", action: "continue" }));
  expect(quickexec).not.toHaveBeenCalled();
  expect(get().messageDrafts["1:request"]).toBeUndefined();
  expect(get().messageDrafts["2:request"].body).toBe("unrelated history");
});

it("does not release traffic or discard a draft when applying it fails", async () => {
  vi.spyOn(api, "bpPaused").mockResolvedValue([{ id: 1, phase: "response" }] as Awaited<ReturnType<typeof api.bpPaused>>);
  vi.spyOn(api, "bpResume").mockRejectedValue("invalid edit");
  const go = vi.spyOn(api, "bpGo").mockResolvedValue(0);
  patchMessageEdits(1, "response", { body: "keep edit" });
  expect(await actions.quickexec("g")).toBe(false);
  expect(go).not.toHaveBeenCalled();
  expect(get().messageDrafts["1:response"].body).toBe("keep edit");
});
