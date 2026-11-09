import { beforeEach, expect, it, vi } from "vitest";
import { api, type BpState } from "./api";
import { cycleAuto, setAuto } from "./breakpoints";

vi.mock("./api", () => ({ api: { bpGet: vi.fn(), bpSet: vi.fn(), status: vi.fn(), bpGo: vi.fn() } }));
vi.mock("./store", () => ({ say: vi.fn(), set: vi.fn() }));

let state: BpState;
beforeEach(() => {
  vi.clearAllMocks();
  state = { allRequests: false, allResponses: false, requestUrl: "/api", responseUrl: null, method: "POST", status: 500, timeoutS: 0, llm: null };
  vi.mocked(api.bpGet).mockImplementation(async () => structuredClone(state));
  vi.mocked(api.bpSet).mockImplementation(async (b) => { state = structuredClone(b); });
  vi.mocked(api.status).mockResolvedValue({} as Awaited<ReturnType<typeof api.status>>);
});

it("serializes three fast clicks into request, response, off without dropping conditions or releasing traffic", async () => {
  await Promise.all([cycleAuto(), cycleAuto(), cycleAuto()]);
  expect(vi.mocked(api.bpSet).mock.calls.map(([b]) => [b.allRequests, b.allResponses])).toEqual([[true, false], [false, true], [false, false]]);
  expect(state).toMatchObject({ requestUrl: "/api", method: "POST", status: 500, timeoutS: 0 });
  expect(api.bpGo).not.toHaveBeenCalled();
  expect(api.status).toHaveBeenCalledTimes(3);
});

it("keeps menu modes exclusive, and toggles the selected mode off", async () => {
  await setAuto("before");
  await setAuto("after");
  expect(state).toMatchObject({ allRequests: false, allResponses: true });
  await setAuto("after");
  expect(state).toMatchObject({ allRequests: false, allResponses: false });
});

it("can retry after a failed update, using the backend state", async () => {
  vi.mocked(api.bpSet).mockRejectedValueOnce(new Error("failed"));
  await expect(cycleAuto()).rejects.toThrow("failed");
  await cycleAuto();
  expect(state).toMatchObject({ allRequests: true, allResponses: false });
});

it("turns both modes and LLM breakpoints off without clearing specific conditions", async () => {
  state.allRequests = state.allResponses = true;
  state.llm = { model: "", tool: "", minTokens: 0 };
  await cycleAuto();
  expect(state).toMatchObject({ allRequests: false, allResponses: false, llm: null, requestUrl: "/api", status: 500 });
});
