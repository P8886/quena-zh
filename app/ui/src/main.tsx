import { createRoot } from "react-dom/client";
import { api, isTauri } from "./api";
import { setLang } from "./i18n";
import "./styles.css";

// The language is set before the UI modules load, so their constants are translated too.
async function start() {
  if (isTauri) {
    const language = await api.uiLanguage().catch(() => "en");
    setLang(language === "de" || language === "zh-CN" ? language : "en");
  }
  const [{ App }, { AppCrash, ErrorBoundary }, { installContextMenus }] = await Promise.all([import("./App"), import("./components/ErrorBoundary"), import("./components/contextMenus")]);
  installContextMenus();
  createRoot(document.getElementById("root")!).render(
    <ErrorBoundary name="app" fallback={(e) => <AppCrash error={e} />}>
      <App />
    </ErrorBoundary>,
  );
}
void start();
