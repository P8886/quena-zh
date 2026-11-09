import { beforeEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { actions } from "./actions";
import { grid } from "./grid/SessionGrid";
import { get, set } from "./store";

vi.mock("./grid/SessionGrid", () => ({ grid: { scrollToIndex: vi.fn() }, rowCache: {}, idAtIndex: vi.fn() }));

beforeEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
  set({ selection: new Set(), focusId: null, focusIndex: null, anchorIndex: null, activeTab: "inspectors" });
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
