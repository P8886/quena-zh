import { useEffect, useState } from "react";
import { api, type BreakpointRule } from "../api";
import { say, set } from "../store";
import { t } from "../i18n";
import { breakpointRuleError } from "../lib/breakpointText";

const blankRule = (): BreakpointRule => ({ id: crypto.randomUUID(), name: "", enabled: true, phase: "request", clientIp: "", host: "", url: "", method: "", status: null });
const plain = { spellCheck: false, autoCorrect: "off", autoCapitalize: "off" } as const;

export function BreakpointRulesPanel({ session }: { session?: number }) {
  const [rules, setRules] = useState<BreakpointRule[]>([]);
  const [selected, setSelected] = useState("");
  const [ready, setReady] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [legacy, setLegacy] = useState(false);
  const [tunnel, setTunnel] = useState(false);
  const [onlyRules, setOnlyRules] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    void Promise.all([api.bpGet(), session == null ? Promise.resolve(null) : api.detail(session)]).then(([state, d]) => {
      if (!alive) return;
      const entries = [...(state.rules ?? [])];
      if (d) {
        const isTunnel = d.summary.kind === "tunnel" || d.request.method === "CONNECT";
        const url = new URL(isTunnel ? `https://${d.request.url}` : d.request.url);
        const rule = { ...blankRule(), name: isTunnel ? url.host : `${d.request.method} ${url.pathname}`, host: url.host, url: isTunnel ? "" : url.pathname, method: isTunnel ? "" : d.request.method, clientIp: d.summary.clientIp };
        setTunnel(isTunnel);
        entries.push(rule);
        setSelected(rule.id);
        setDirty(true);
      } else setSelected(entries[0]?.id ?? "");
      setRules(entries);
      setLegacy(!!(state.allRequests || state.allResponses || state.requestUrl || state.responseUrl || state.method || state.status || state.llm));
      setReady(true);
    }).catch((e) => alive && setError(String(e)));
    return () => { alive = false; };
  }, [session]);

  const change = (id: string, patch: Partial<BreakpointRule>) => {
    setRules((all) => all.map((r) => r.id === id ? { ...r, ...patch } : r));
    setDirty(true);
  };
  const current = rules.find((r) => r.id === selected);
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.bpRulesSet(rules, onlyRules);
      set({ status: await api.status() });
      setDirty(false);
      if (onlyRules) setLegacy(false);
      setOnlyRules(false);
      say(t("Breakpoint rules saved."));
    } catch (e) {
      setError(breakpointRuleError(e));
    } finally { setBusy(false); }
  };
  if (!ready) return <p className={error ? "err" : "muted"}>{error ?? t("Loading…")}</p>;
  return <div className="bp-rules">
    <p className="muted small">{t("All filled conditions in a rule must match. Any enabled rule can pause traffic. Empty fields match anything. Rules are saved across restarts.")}</p>
    {tunnel && <p className="muted small">{t("This is a CONNECT tunnel. The rule applies to the HTTP messages inside it after HTTPS decryption is enabled.")}</p>}
    {legacy && <div className="banner warn">
      <p>{t("Global or command breakpoints are also active and can pause traffic outside these rules.")}</p>
      <label className="f-check"><input type="checkbox" disabled={busy} checked={onlyRules} onChange={(e) => { setOnlyRules(e.target.checked); setDirty(true); }} />{t("On save, turn off global and command breakpoints and use only these rules")}</label>
    </div>}
    <div className="bp-rules-split">
      <div className="bp-rule-list">
        {rules.map((r, i) => <div key={r.id} className={`bp-rule-row ${selected === r.id ? "sel" : ""}`}>
          <input type="checkbox" disabled={busy} checked={r.enabled} aria-label={t("Enable rule {n}", { n: i + 1 })} onChange={(e) => change(r.id, { enabled: e.target.checked })} />
          <button disabled={busy} onClick={() => setSelected(r.id)}>{r.name || t("Rule {n}", { n: i + 1 })}</button>
        </div>)}
        <button disabled={busy} onClick={() => {
          const r = blankRule();
          setRules([...rules, r]);
          setSelected(r.id);
          setDirty(true);
        }}>{t("Add Rule")}</button>
      </div>
      {current ? <fieldset disabled={busy} className="bp-rule-fields">
        <label className="f-row"><span>{t("Rule name")}</span><input value={current.name} onChange={(e) => change(current.id, { name: e.target.value })} /></label>
        <label className="f-row"><span>{t("Pause at")}</span><select value={current.phase} onChange={(e) => change(current.id, { phase: e.target.value as BreakpointRule["phase"], status: null })}>
          <option value="request">{t("Before Requests")}</option><option value="response">{t("After Responses")}</option>
        </select></label>
        <label className="f-row"><span>{t("Client IP (exact)")}</span><input {...plain} value={current.clientIp} placeholder="192.168.1.10" onChange={(e) => change(current.id, { clientIp: e.target.value })} /></label>
        <label className="f-row"><span>{t("Host (wildcards allowed)")}</span><input {...plain} value={current.host} placeholder="*.example.com" onChange={(e) => change(current.id, { host: e.target.value })} /></label>
        <label className="f-row"><span>{t("URL or path contains")}</span><input {...plain} value={current.url} placeholder="/api/orders" onChange={(e) => change(current.id, { url: e.target.value })} /></label>
        <label className="f-row"><span>{t("Request method")}</span><input {...plain} value={current.method} list="bp-methods" placeholder="POST" onChange={(e) => change(current.id, { method: e.target.value.toUpperCase() })} /></label>
        <datalist id="bp-methods">{["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].map((m) => <option key={m} value={m} />)}</datalist>
        <label className="f-row"><span>{t("Response status")}</span><input type="number" min={100} max={599} disabled={current.phase !== "response"} value={current.status ?? ""} placeholder="500" onChange={(e) => change(current.id, { status: e.target.value ? Number(e.target.value) : null })} /></label>
        <button className="danger" onClick={() => {
          const rest = rules.filter((r) => r.id !== current.id);
          setRules(rest);
          setSelected(rest[0]?.id ?? "");
          setDirty(true);
        }}>{t("Remove Rule")}</button>
      </fieldset> : <p className="muted">{t("Add a rule to pause only the traffic you need.")}</p>}
    </div>
    {error && <p className="err">{error}</p>}
    <div className="dialog-actions"><button className="primary" disabled={!dirty || busy} onClick={() => void save()}>{t("Save Rules")}</button></div>
  </div>;
}
