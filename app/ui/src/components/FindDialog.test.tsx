// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, type Status } from "../api";
import { actions } from "../actions";
import { get, set } from "../store";
import { setLang } from "../i18n";
import { FindDialog } from "./FindDialog";
import { StatusBar } from "./StatusBar";

vi.mock("../actions", () => ({ actions: { selectIds: vi.fn(async () => {}) } }));

let root: Root;
let host: HTMLDivElement;

beforeEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  setLang("en");
  localStorage.clear();
  localStorage.setItem("quena.find.options", JSON.stringify({ text: "/tracked" }));
  set({ selection: new Set([42]), focusId: 42, focusIndex: 0, status: null, message: null });
  vi.spyOn(api, "findSessions").mockResolvedValue(1);
  vi.spyOn(api, "findResult").mockResolvedValue({ ids: [7, 8], examined: 2, total: 2, done: true });
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  localStorage.clear();
  set({ status: null });
  vi.unstubAllGlobals();
});

async function renderFind() {
  const done = vi.fn();
  await act(async () => root.render(<FindDialog onDone={done} />));
  return done;
}

function checkbox(text: string) {
  return [...host.querySelectorAll("label")].find((label) => label.textContent?.includes(text))!.querySelector("input")!;
}

async function runFind() {
  await act(async () => host.querySelector<HTMLButtonElement>("button.primary")!.click());
}

it("defaults to following later traffic while selecting only the initial results once", async () => {
  const done = await renderFind();
  expect(checkbox("Keep matching new sessions").checked).toBe(true);
  await runFind();
  expect(api.findSessions).toHaveBeenCalledWith(expect.objectContaining({ text: "/tracked", mark: "gold", continuous: true, ids: [] }));
  expect(actions.selectIds).toHaveBeenCalledExactlyOnceWith([7, 8]);
  expect(done).toHaveBeenCalledOnce();
});

it("remembers opting out of continuous matching", async () => {
  await renderFind();
  await act(async () => checkbox("Keep matching new sessions").click());
  await runFind();
  expect(api.findSessions).toHaveBeenCalledWith(expect.objectContaining({ continuous: false }));
  expect(JSON.parse(localStorage.getItem("quena.find.options")!).continuous).toBe(false);
});

it("limits selected-only searches to the current selection without following new traffic", async () => {
  await renderFind();
  await act(async () => checkbox("Selected sessions only").click());
  expect(checkbox("Keep matching new sessions").disabled).toBe(true);
  expect(checkbox("Keep matching new sessions").checked).toBe(false);
  await runFind();
  expect(api.findSessions).toHaveBeenCalledWith(expect.objectContaining({ continuous: false, ids: [42] }));
});

it("makes select-only searches one-shot when no highlight colour is chosen", async () => {
  localStorage.setItem("quena.find.options", JSON.stringify({ text: "/tracked", mark: null, continuous: true }));
  await renderFind();
  expect(checkbox("Keep matching new sessions").disabled).toBe(true);
  expect(checkbox("Keep matching new sessions").checked).toBe(false);
  await runFind();
  expect(api.findSessions).toHaveBeenCalledWith(expect.objectContaining({ continuous: false, mark: null }));
});

it("stops from the status bar without changing the selected session or unsent edits", async () => {
  const stop = vi.spyOn(api, "findStop").mockResolvedValue(undefined);
  const selection = get().selection;
  const messageDrafts = { "42:request": { head: null, body: "unsent", file: null } };
  set({ status: { liveFind: { text: "/tracked", matches: 3 }, sessions: 10 } as Status, messageDrafts });
  await act(async () => root.render(<StatusBar />));
  const button = [...host.querySelectorAll("button")].find((b) => b.textContent?.includes("Live find:"))!;
  expect(button.textContent).toContain("3 matches");
  await act(async () => button.click());
  expect(stop).toHaveBeenCalledOnce();
  expect(get().status?.liveFind).toBeNull();
  expect(get().selection).toBe(selection);
  expect(get().focusId).toBe(42);
  expect(get().messageDrafts).toBe(messageDrafts);
});
