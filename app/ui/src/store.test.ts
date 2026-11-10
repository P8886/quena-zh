import { describe, expect, it } from "vitest";
import { restoreLayout } from "./store";

describe("restoreLayout", () => {
  it("starts in the Quena layout", () => {
    expect(restoreLayout(undefined).preset).toBe("quena");
    // Only a theme or language saved (e.g. written by hand): still Quena, its columns.
    const l = restoreLayout({ theme: "light" });
    expect(l.preset).toBe("quena");
    expect(l.stacked).toBe(true);
    expect(l.columns.find((c) => c.key === "protocol")?.visible).toBe(false);
  });
  it("keeps the arrangement of layouts from before the presets", () => {
    const cols = restoreLayout(undefined).columns;
    expect(restoreLayout({ columns: cols, stacked: true }).preset).toBe("classic");
    expect(restoreLayout({ columns: cols, stacked: false }).preset).toBe("quena");
    expect(restoreLayout({ preset: "classic" }).preset).toBe("classic");
  });
  it("restores wide layout and both independently resized dividers after saving", () => {
    const saved = JSON.parse(JSON.stringify({ preset: "wide", listHeight: 0.36, inspectorSplit: 0.61 }));
    const layout = restoreLayout(saved);
    expect(layout).toMatchObject({ preset: "wide", listHeight: 0.36, inspectorSplit: 0.61, stacked: false });
    expect(layout.columns.find((c) => c.key === "protocol")?.visible).toBe(true);
  });
  it("adds a usable list height to older settings without changing their arrangement", () => {
    const layout = restoreLayout({ preset: "classic", leftWidth: 0.63, inspectorSplit: 0.39 });
    expect(layout).toMatchObject({ preset: "classic", leftWidth: 0.63, inspectorSplit: 0.39, stacked: true, listHeight: 0.42 });
    expect(restoreLayout({ preset: "wide" }).stacked).toBe(false);
  });
});
