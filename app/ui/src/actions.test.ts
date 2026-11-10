// @vitest-environment happy-dom
import { beforeEach, expect, it, vi } from "vitest";
import { api, type Settings, type Status } from "./api";
import { actions } from "./actions";
import { grid, rowCache } from "./grid/SessionGrid";
import { DEFAULT_LAYOUT, get, set } from "./store";

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
    status: null, captureBusy: null, systemProxyBusy: null,
  });
});

it("switches layout through the menu, persists it and keeps the selected session and unsent edits", async () => {
  vi.useFakeTimers();
  try {
    const save = vi.spyOn(api, "saveUiPrefs").mockResolvedValue(undefined);
    const selection = new Set([42]);
    const messageDrafts = { "42:request": { head: null, body: "unsent request edits", file: null } };
    set({
      layout: { ...DEFAULT_LAYOUT, listHeight: 0.36, requestTab: "raw", responseTab: "json" },
      selection, focusId: 42, focusIndex: 3, messageDrafts, unlockedSessions: new Set([42]),
    });
    await actions.menu("view.layout-wide");
    expect(get().layout).toMatchObject({ preset: "wide", stacked: false, listHeight: 0.36, requestTab: "raw", responseTab: "json" });
    expect(get()).toMatchObject({ selection, focusId: 42, focusIndex: 3, messageDrafts });
    expect(get().unlockedSessions.has(42)).toBe(true);
    await vi.advanceTimersByTimeAsync(400);
    expect(save).toHaveBeenCalledWith({ layout: get().layout });
    await actions.menu("view.layout-classic");
    expect(get().layout).toMatchObject({ preset: "classic", stacked: true });
    await actions.menu("view.layout-quena");
    expect(get().layout).toMatchObject({ preset: "quena", stacked: true });
    expect(get().messageDrafts).toBe(messageDrafts);
  } finally {
    vi.clearAllTimers();
    vi.useRealTimers();
    set({ layout: DEFAULT_LAYOUT });
  }
});

it.each([true, false])("F12 toggles local capture to %s and refreshes the saved preference without stopping the listener", async (on) => {
  const status = { engine: { capturing: true, systemProxy: on } } as Status;
  const settings = { proxy: { actAsSystemProxy: on } } as Settings;
  set({ status: { engine: { capturing: true, systemProxy: !on } } as Status });
  const toggle = vi.spyOn(api, "toggleSystemProxy").mockResolvedValue(on);
  const stopListener = vi.spyOn(api, "toggleCapture");
  vi.spyOn(api, "status").mockResolvedValue(status);
  vi.spyOn(api, "settingsGet").mockResolvedValue(settings);

  await actions.menu("capture.system-proxy");

  expect(toggle).toHaveBeenCalledOnce();
  expect(stopListener).not.toHaveBeenCalled();
  expect(get()).toMatchObject({ status, settings, systemProxyBusy: null });
});

it("ignores repeated F12 and listener toggles while the OS proxy change is pending", async () => {
  let finish!: (on: boolean) => void;
  const toggle = vi.spyOn(api, "toggleSystemProxy").mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const stopListener = vi.spyOn(api, "toggleCapture");
  vi.spyOn(api, "status").mockResolvedValue({ engine: { capturing: true, systemProxy: false } } as Status);
  vi.spyOn(api, "settingsGet").mockResolvedValue({ proxy: { actAsSystemProxy: false } } as Settings);
  set({ status: { engine: { capturing: true, systemProxy: true } } as Status });

  const first = actions.toggleSystemProxy();
  expect(get().systemProxyBusy).toBe("stopping");
  await actions.toggleSystemProxy();
  await actions.toggleCapture();
  expect(toggle).toHaveBeenCalledOnce();
  expect(stopListener).not.toHaveBeenCalled();
  finish(false);
  await first;
  expect(get().systemProxyBusy).toBeNull();
});

it("shows an OS proxy failure and refreshes actual state even when the preference was saved", async () => {
  const status = { engine: { capturing: true, systemProxy: true, error: "restore denied" } } as Status;
  const settings = { proxy: { actAsSystemProxy: false } } as Settings;
  vi.spyOn(api, "toggleSystemProxy").mockRejectedValue(new Error("restore denied"));
  vi.spyOn(api, "status").mockResolvedValue(status);
  vi.spyOn(api, "settingsGet").mockResolvedValue(settings);
  set({ status: { engine: { capturing: true, systemProxy: true } } as Status });

  await actions.toggleSystemProxy();

  expect(get()).toMatchObject({ status, settings, systemProxyBusy: null });
  expect(get().message).toMatchObject({ kind: "error", text: "Error: restore denied" });
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
