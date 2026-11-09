import { beforeEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { get, set } from "./store";
import { discardMessageEdits, patchMessageEdits } from "./sessionEdits";
import { actions } from "./actions";

vi.mock("./grid/SessionGrid", () => ({ grid: {}, rowCache: {}, idAtIndex: vi.fn() }));

beforeEach(() => {
  vi.restoreAllMocks();
  set({ focusId: 1, selection: new Set([1]), messageDrafts: {}, gridNonce: 0 });
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
