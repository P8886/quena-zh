// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { EditorView } from "@codemirror/view";
import { undo } from "@codemirror/commands";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CodeView } from "./CodeViewImpl";
import { useMessageEdits } from "./Tamper";
import { api } from "../api";
import { get, set } from "../store";
import { saveSessionEdits } from "../sessionEdits";
import { startReplay } from "../replay";

vi.mock("../grid/SessionGrid", () => ({ grid: {}, rowCache: {}, idAtIndex: vi.fn() }));

let host: HTMLDivElement;
let root: Root;
const original = '{"title":"断点测试商品","quantity":1}';
const edited = '{"title":"修改后重发111111","quantity":5}';

beforeEach(() => {
  vi.restoreAllMocks();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  set({ messageDrafts: {}, unlockedSessions: new Set([1]), gridNonce: 0 });
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(() => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});

function editor() {
  return EditorView.findFromDOM(host.querySelector(".cm-editor")!)!;
}

function typeBody(text: string) {
  const view = editor();
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, userEvent: "input.type" });
}

// Detail refreshes asynchronously after a save. Until then the editor's fallback is
// the previous recorded body, exactly as in the F2 inspector.
function DraftEditor() {
  const { edits, setEdits } = useMessageEdits(1, "request");
  return createElement(CodeView, { text: edits.body ?? original, editable: true, highlight: false, onChange: (body) => setEdits({ ...edits, body }) });
}

it("refreshes external text without reporting a user edit or adding an undo step", async () => {
  const first = vi.fn();
  const latest = vi.fn();
  await act(() => root.render(createElement(CodeView, { text: original, editable: true, highlight: false, onChange: first })));
  await act(() => root.render(createElement(CodeView, { text: edited, editable: true, highlight: false, onChange: latest })));
  expect(editor().state.doc.toString()).toBe(edited);
  expect(first).not.toHaveBeenCalled();
  expect(latest).not.toHaveBeenCalled();
  expect(undo(editor())).toBe(false);
  await act(() => typeBody("real keyboard input"));
  expect(first).not.toHaveBeenCalled();
  expect(latest).toHaveBeenCalledExactlyOnceWith("real keyboard input");
});

it("does not save the previous body over F2 edits when the saved draft is cleared before replay", async () => {
  let saved = original;
  const save = vi.spyOn(api, "editSession").mockImplementation(async (_, edit) => { saved = edit.request!.bodyText!; });
  const replay = vi.spyOn(api, "replay").mockImplementation(async () => { expect(saved).toBe(edited); return 1; });
  await act(() => root.render(createElement(DraftEditor)));
  await act(() => typeBody(edited));
  expect(get().messageDrafts["1:request"].body).toBe(edited);
  await act(async () => { expect(await saveSessionEdits(1)).toBe(true); });
  // Clearing a saved draft synchronizes old detail into CodeMirror. That refresh must
  // not manufacture another draft that Shift+R then saves over the edited request.
  expect(get().messageDrafts).toEqual({});
  await act(() => startReplay([1], { count: 1 }));
  expect(save).toHaveBeenCalledTimes(1);
  expect(replay).toHaveBeenCalledExactlyOnceWith([1], { count: 1 });
  expect(get().messageDrafts).toEqual({});
});

it("keeps real input made while a save is pending", async () => {
  let finish!: () => void;
  vi.spyOn(api, "editSession").mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  await act(() => root.render(createElement(DraftEditor)));
  await act(() => typeBody(edited));
  const saving = saveSessionEdits(1);
  await act(() => typeBody('{"title":"保存期间继续输入"}'));
  await act(async () => { finish(); expect(await saving).toBe(false); });
  expect(get().messageDrafts["1:request"].body).toBe('{"title":"保存期间继续输入"}');
  expect(editor().state.doc.toString()).toBe('{"title":"保存期间继续输入"}');
});
