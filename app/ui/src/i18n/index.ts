// UI translations. Texts are written in English in the code and wrapped in t("…"); the
// English text is the key. Missing translations fall back to English.
//
//   t("Loading {name}", { name })        placeholders in braces
//   t("{n} session(s)", { n })           plural: see plural() below
//
// The language is fixed for the lifetime of the page: main.tsx sets it before any other
// module loads (so module-level constants are translated as well), and switching the language
// reloads the UI.
import { de } from "./de";
import { zhCN } from "./zh-CN";

export type Lang = "en" | "de" | "zh-CN";
export type LangPref = "system" | Lang;

let lang: Lang = "en";
let dict: Record<string, string> = {};

export function setLang(l: Lang) {
  lang = l;
  const dictionaries: Record<Lang, Record<string, string>> = { en: {}, de, "zh-CN": zhCN };
  dict = dictionaries[l];
  document.documentElement.lang = l;
}

export const currentLang = () => lang;
export const currentLocale = () => ({ en: "en-US", de: "de-DE", "zh-CN": "zh-CN" })[lang];

/** Translate an English UI text; `{name}` placeholders are filled from `vars`. */
export function t(en: string, vars?: Record<string, string | number>): string {
  const s = dict[en] ?? en;
  return vars ? s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m)) : s;
}

/** Pick singular or plural (English source texts for both), then translate. */
export function plural(n: number, one: string, many: string, vars?: Record<string, string | number>): string {
  return t(n === 1 ? one : many, { n: fmtNum(n), ...vars });
}

/** Numbers in the UI language (1,234 / 1.234). */
export function fmtNum(n: number): string {
  return n.toLocaleString(currentLocale());
}
