// Simplified Chinese UI translations, grouped by area.
import { agents } from "./zh-CN/agents";
import { components } from "./zh-CN/components";
import { core } from "./zh-CN/core";
import { diagnostics } from "./zh-CN/diagnostics";
import { inspectors } from "./zh-CN/inspectors";
import { mocks } from "./zh-CN/mocks";
import { panels } from "./zh-CN/panels";
import { sanitize } from "./zh-CN/sanitize";

export const zhCN: Record<string, string> = {
  ...agents,
  ...components,
  ...core,
  ...diagnostics,
  ...inspectors,
  ...mocks,
  ...panels,
  ...sanitize,
};
