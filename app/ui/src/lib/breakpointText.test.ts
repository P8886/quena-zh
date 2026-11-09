import { afterEach, expect, it } from "vitest";
import { setLang } from "../i18n";
import { breakpointText } from "./breakpointText";

(globalThis as { document?: unknown }).document ??= { documentElement: {} };
afterEach(() => setLang("en"));

it("localizes backend breakpoint descriptions and preserves executable conditions", () => {
  setLang("zh-CN");
  expect(breakpointText("before requests")).toBe("请求发送前暂停");
  expect(breakpointText("after responses")).toBe("响应返回前暂停");
  for (const condition of ["bpu https://example.test/after responses", "bps 404", "bpv POST", "bpllm model=claude tool=mcp__jira__* tokens=50000"])
    expect(breakpointText(condition)).toBe(condition);
  setLang("en");
  expect(breakpointText("before requests")).toBe("Break before requests");
});
