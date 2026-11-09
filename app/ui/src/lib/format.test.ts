import { afterEach, describe, expect, it } from "vitest";
import { currentLocale, plural, setLang, t } from "../i18n";
import { fmtInt } from "./format";

(globalThis as { document?: unknown }).document ??= { documentElement: {} };

describe("fmtInt", () => {
  afterEach(() => setLang("en"));
  it("follows the UI language after the first use", () => {
    setLang("en");
    expect(fmtInt(12345)).toBe("12,345");
    setLang("de");
    expect(fmtInt(12345)).toBe("12.345");
    setLang("en");
    expect(fmtInt(12345)).toBe("12,345");
  });
});


describe("Simplified Chinese UI", () => {
  afterEach(() => setLang("en"));
  it("switches translations, interpolates values and formats numbers", () => {
    setLang("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(currentLocale()).toBe("zh-CN");
    expect(t("Language")).toBe("语言");
    expect(fmtInt(12345)).toBe("12,345");
    expect(plural(1234, "{n} session", "{n} sessions")).toContain("1,234");
    expect(t("Unknown text {value}", { value: "ok" })).toBe("Unknown text ok");
    setLang("en");
    expect(t("Language")).toBe("Language");
  });
  it("uses the storage, import and HTTP meanings of ambiguous UI terms", () => {
    setLang("zh-CN");
    expect(t("{size} free", { size: "350.4 GB" })).toBe("可用空间 350.4 GB");
    expect(t("Ask")).toBe("每次询问");
    expect(t("Cookies")).toBe("Cookie");
    expect(t("Console")).toBe("控制台");
    expect(t("Disabled")).toBe("已禁用");
    expect(t("Comment (M)")).toBe("备注（M）");
    expect(t("as sent")).toBe("跟随客户端");
    expect(t("{n} hit(s)", { n: 2 })).toBe("命中 2 次");
    expect(t("OK")).toBe("确定");
  });
});
