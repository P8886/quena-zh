import { api, type BpState } from "./api";
import { say, set } from "./store";
import { plural, t } from "./i18n";

export async function goAll() {
  const n = await api.bpGo();
  say(plural(n, "Resumed {n} session", "Resumed {n} sessions"));
}

/** Break before every LLM request, or no longer (conditions: `bpllm` in the command field). */
export async function toggleLlm() {
  const b = await api.bpGet();
  // With conditions (bpllm …): every LLM request; else on / off.
  const conditional = !!b.llm && (b.llm.model || b.llm.tool || b.llm.minTokens > 0);
  b.llm = b.llm && !conditional ? null : { model: "", tool: "", minTokens: 0 };
  await api.bpSet(b);
  say(t("Break before LLM requests: {state}", { state: b.llm ? t("on") : t("off") }));
}

type AutoMode = "before" | "after" | "off";

// Read and write together: fast clicks / menu shortcuts must not overwrite one another.
let autoChange = Promise.resolve();
function changeAuto(choose: (b: BpState) => AutoMode) {
  const change = autoChange.then(async () => {
    const b = await api.bpGet();
    const mode = choose(b);
    b.allRequests = mode === "before";
    b.allResponses = mode === "after";
    if (mode === "off") b.llm = null;
    await api.bpSet(b);
    set({ status: await api.status() });
    say(mode === "off" ? t("Automatic breakpoints disabled") : mode === "before" ? t("Break before requests: {state}", { state: t("on") }) : t("Break after responses: {state}", { state: t("on") }));
  });
  autoChange = change.catch(() => {});
  return change;
}

export function setAuto(mode: AutoMode) {
  return changeAuto((b) => ((mode === "before" && b.allRequests) || (mode === "after" && b.allResponses)) ? "off" : mode);
}

/** Fiddler-style status button: off → before requests → after responses → off. */
export function cycleAuto() {
  return changeAuto((b) => b.allResponses ? "off" : b.allRequests ? "after" : "before");
}
