// @vitest-environment happy-dom
import { beforeEach, expect, it, vi } from "vitest";
import { api, type Settings } from "./api";
import { actions } from "./actions";
import { grid, rowCache } from "./grid/SessionGrid";
import { get, set } from "./store";

vi.mock("./grid/SessionGrid", () => ({
  grid: { scrollToIndex: vi.fn(), visibleCount: vi.fn(() => 10) },
  rowCache: { clear: vi.fn() },
  idAtIndex: vi.fn(),
}));

beforeEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
  set({
    selection: new Set(), focusId: null, focusIndex: null, anchorIndex: null, activeTab: "inspectors",
    settings: null, listCount: 0, dialog: null, messageDrafts: {}, unlockedSessions: new Set(),
  });
});

it("keeps the manually selected session and current tab when new requests pause", async () => {
  const position = vi.spyOn(api, "positionOf").mockResolvedValue(100);
  const selection = new Set([42]);
  set({ selection, focusId: 42, focusIndex: 7, anchorIndex: 7, activeTab: "composer" });
  await Promise.all([43, 44, 45].map(id => actions.revealBreakpoint(id)));
  expect(get()).toMatchObject({ selection, focusId: 42, focusIndex: 7, anchorIndex: 7, activeTab: "composer" });
  expect(get().selection).toBe(selection);
  expect(position).not.toHaveBeenCalled();
  expect(grid.scrollToIndex).not.toHaveBeenCalled();
});

it("preserves multiple selections and a focus whose row was toggled out", async () => {
  const position = vi.spyOn(api, "positionOf").mockResolvedValue(10);
  set({ selection: new Set([3, 5]), focusId: 5 });
  await actions.revealBreakpoint(8);
  expect([...get().selection]).toEqual([3, 5]);
  set({ selection: new Set(), focusId: 5 });
  await actions.revealBreakpoint(9);
  expect(get().focusId).toBe(5);
  expect(position).not.toHaveBeenCalled();
});

it("previews only the first of many concurrent pauses without changing tabs", async () => {
  let finish!: (position: number) => void;
  const position = vi.spyOn(api, "positionOf").mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  set({ activeTab: "filters" });
  const first = actions.revealBreakpoint(100);
  await Promise.all(Array.from({ length: 30 }, (_, i) => actions.revealBreakpoint(101 + i)));
  expect(get().focusId).toBe(100);
  expect([...get().selection]).toEqual([100]);
  expect(position).toHaveBeenCalledTimes(1);
  finish(6);
  await first;
  expect(get()).toMatchObject({ focusId: 100, focusIndex: 6, anchorIndex: 6, activeTab: "filters" });
  expect(grid.scrollToIndex).toHaveBeenCalledExactlyOnceWith(6, "center");
});

it("does not scroll or overwrite a manual choice made during the position lookup", async () => {
  let finish!: (position: number) => void;
  vi.spyOn(api, "positionOf").mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const reveal = actions.revealBreakpoint(100);
  set({ selection: new Set([20]), focusId: 20, focusIndex: 2, anchorIndex: 2 });
  finish(99);
  await reveal;
  expect(get()).toMatchObject({ focusId: 20, focusIndex: 2, anchorIndex: 2 });
  expect([...get().selection]).toEqual([20]);
  expect(grid.scrollToIndex).not.toHaveBeenCalled();
});

it("does not resurrect a selection cleared while a breakpoint lookup is pending", async () => {
  let finish!: (position: number) => void;
  vi.spyOn(api, "positionOf").mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const reveal = actions.revealBreakpoint(100);
  actions.clearSelection();
  finish(99);
  await reveal;
  expect(get()).toMatchObject({ focusId: null, focusIndex: null });
  expect(get().selection.size).toBe(0);
  expect(grid.scrollToIndex).not.toHaveBeenCalled();
});

it.each([
  { name: "settings have not loaded", settings: null },
  { name: "older settings omit the preference", settings: {} as Settings },
  { name: "confirmation is enabled", settings: { confirmRemoveAll: true } as Settings },
])("asks before removing all when $name, and preserves sessions on cancel", async ({ settings }) => {
  const remove = vi.spyOn(api, "removeAll").mockResolvedValue(undefined);
  const selection = new Set([42]);
  const messageDrafts = { "42:request": { head: null, body: "edited request", file: null } };
  const unlockedSessions = new Set([42]);
  set({ settings, listCount: 3, selection, focusId: 42, focusIndex: 1, messageDrafts, unlockedSessions });
  const pending = actions.removeAll();
  const dialog = get().dialog;
  expect(dialog?.kind).toBe("confirm");
  expect(remove).not.toHaveBeenCalled();
  if (dialog?.kind !== "confirm") throw new Error("Expected removal confirmation");
  dialog.resolve(false);
  await pending;
  expect(remove).not.toHaveBeenCalled();
  expect(rowCache.clear).not.toHaveBeenCalled();
  expect(get()).toMatchObject({ selection, focusId: 42, focusIndex: 1, messageDrafts, unlockedSessions });
});

it("removes all sessions only after confirmation is accepted", async () => {
  const remove = vi.spyOn(api, "removeAll").mockResolvedValue(undefined);
  set({ settings: { confirmRemoveAll: true } as Settings, listCount: 3, selection: new Set([42]), focusId: 42 });
  const pending = actions.removeAll();
  const dialog = get().dialog;
  if (dialog?.kind !== "confirm") throw new Error("Expected removal confirmation");
  expect(remove).not.toHaveBeenCalled();
  dialog.resolve(true);
  await pending;
  expect(remove).toHaveBeenCalledExactlyOnceWith();
  expect(rowCache.clear).toHaveBeenCalledOnce();
  expect(get().selection.size).toBe(0);
  expect(get().focusId).toBeNull();
});

it("clears sessions and their drafts directly with Ctrl+X when confirmation is disabled", async () => {
  const remove = vi.spyOn(api, "removeAll").mockResolvedValue(undefined);
  set({
    settings: { confirmRemoveAll: false } as Settings, listCount: 3,
    selection: new Set([42]), focusId: 42, focusIndex: 1,
    messageDrafts: { "42:request": { head: null, body: "edited request", file: null } }, unlockedSessions: new Set([42]),
  });
  expect(actions.gridKey(new KeyboardEvent("keydown", { key: "x", ctrlKey: true }))).toBe(true);
  await vi.waitFor(() => expect(remove).toHaveBeenCalledExactlyOnceWith());
  expect(get().dialog).toBeNull();
  expect(rowCache.clear).toHaveBeenCalledOnce();
  expect(get()).toMatchObject({
    selection: new Set(), focusId: null, focusIndex: null, messageDrafts: {}, unlockedSessions: new Set(),
  });
});

it.each([true, false])("does nothing for an empty list when confirmation is %s", async (confirmRemoveAll) => {
  const remove = vi.spyOn(api, "removeAll").mockResolvedValue(undefined);
  set({ settings: { confirmRemoveAll } as Settings });
  await actions.removeAll();
  expect(remove).not.toHaveBeenCalled();
  expect(rowCache.clear).not.toHaveBeenCalled();
  expect(get().dialog).toBeNull();
});

it.each(["removeSelected", "removeUnselected"] as const)("still confirms %s when remove-all confirmation is disabled", async (action) => {
  const remove = vi.spyOn(api, "remove").mockResolvedValue(undefined);
  const removeExcept = vi.spyOn(api, "removeExcept").mockResolvedValue(undefined);
  set({ settings: { confirmRemoveAll: false } as Settings, listCount: 3, selection: new Set([42]) });
  const pending = actions[action]();
  const dialog = get().dialog;
  expect(dialog?.kind).toBe("confirm");
  if (dialog?.kind !== "confirm") throw new Error("Expected removal confirmation");
  dialog.resolve(false);
  await pending;
  expect(remove).not.toHaveBeenCalled();
  expect(removeExcept).not.toHaveBeenCalled();
});
