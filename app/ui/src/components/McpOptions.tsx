import { useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, type McpClient, type McpStatus, type Settings } from "../api";
import { actions } from "../actions";
import { set, useStore } from "../store";
import { t } from "../i18n";
import { isWindows } from "../lib/format";

const CLIENTS = { claudeCode: "Claude Code", codex: "Codex", vsCode: "VS Code", cursor: "Cursor" } as const;

/** Keep copied instructions consistent with the formats written by mcp_setup_client. */
function connectionInstructions(client: McpClient, port: number, token: string): string {
  const url = `http://127.0.0.1:${port}/mcp`;
  const authorization = `Bearer ${token}`;
  if (client === "claudeCode") {
    const header = `Authorization: ${authorization}`;
    const quoted = isWindows ? header.replace(/'/g, "''") : header.replace(/'/g, "'\\''");
    return `claude mcp add --scope user --transport http quena ${url} --header '${quoted}'`;
  }
  if (client === "codex") {
    return `[mcp_servers.quena]\nurl = ${JSON.stringify(url)}\nhttp_headers = { Authorization = ${JSON.stringify(authorization)} }`;
  }
  return JSON.stringify({
    [client === "vsCode" ? "servers" : "mcpServers"]: {
      quena: { ...(client === "vsCode" ? { type: "http" } : {}), url, headers: { Authorization: authorization } },
    },
  }, null, 2);
}

type Feedback = { error: boolean; text: string };

export function McpOptions({ s, up }: { s: Settings; up: (f: (x: Settings) => void) => void }) {
  const [status, setStatus] = useState<McpStatus | null>(null);
  const savedClient = useStore((st) => st.layout.mcpClient);
  const client = savedClient && Object.keys(CLIENTS).includes(savedClient) ? savedClient : "claudeCode";
  const name = CLIENTS[client];
  const [settingUp, setSettingUp] = useState(false);
  const [feedback, setFeedback] = useState<(Feedback & { key: string }) | null>(null);
  const [installingSkill, setInstallingSkill] = useState(false);
  const [skillFeedback, setSkillFeedback] = useState<Feedback | null>(null);
  useEffect(() => {
    api.mcpStatus().then(setStatus, () => setStatus(null));
  }, []);
  const m = s.mcp ?? { enabled: false, port: 8867, access: "readOnly", token: "", includeSecrets: false, filesDir: "" };
  const newToken = async () => {
    const token = await api.mcpNewToken();
    up((x) => (x.mcp = { ...m, ...x.mcp, token }));
  };
  const instructions = m.token ? connectionInstructions(client, m.port, m.token) : "";
  // Results belong to the client and connection settings used when the action started.
  const setupKey = JSON.stringify([client, m.enabled, m.port, m.token]);
  // Clients are set up from the saved settings: not while the dialog holds other ones.
  const savedMcp = useStore((st) => st.settings?.mcp);
  const unsaved = !savedMcp || savedMcp.enabled !== m.enabled || savedMcp.port !== m.port || savedMcp.token !== m.token;
  const setup = async () => {
    setSettingUp(true);
    setFeedback(null);
    try {
      await api.mcpSetupClient(client);
      setFeedback({ key: setupKey, error: false, text: t("{name} configured. Reload its MCP servers or restart the client to connect.", { name }) });
    } catch (e) {
      setFeedback({ key: setupKey, error: true, text: t("Could not configure {name}: {error}", { name, error: String(e) }) });
    } finally {
      setSettingUp(false);
    }
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(instructions);
      setFeedback({ key: setupKey, error: false, text: t("Copied connection instructions for {name}.", { name }) });
    } catch (e) {
      setFeedback({ key: setupKey, error: true, text: t("Could not copy: {error}", { error: String(e) }) });
    }
  };
  return (
    <>
      <p className="muted small">{t("AI agents connect over the Model Context Protocol (MCP) to read sessions and, with full control, set rules and send requests. Only programs on this computer that know the token can connect.")}</p>
      <label className="f-check">
        <input
          type="checkbox"
          checked={m.enabled}
          onChange={async (e) => {
            const enabled = e.target.checked;
            const token = enabled && !m.token ? await api.mcpNewToken() : m.token;
            up((x) => (x.mcp = { ...m, enabled, token }));
          }}
        />{" "}
        {t("Enable MCP server")}
      </label>
      <div className="f-row">
        <span>{t("Port")}</span>
        <input type="number" min={1} max={65535} value={m.port} onChange={(e) => up((x) => (x.mcp = { ...m, port: Number(e.target.value) }))} />
      </div>
      <div className="f-row">
        <span>{t("Agents may")}</span>
        <select value={m.access} onChange={(e) => up((x) => (x.mcp = { ...m, access: e.target.value as "readOnly" | "full" }))}>
          <option value="readOnly">{t("…only read sessions, rules and statistics")}</option>
          <option value="full">{t("…also change rules and breakpoints, capture and send requests")}</option>
        </select>
      </div>
      <label className="f-check" title={t("Agents send what they read to their model provider. Off: Authorization, cookies, tokens and secret parameters and fields are replaced first.")}>
        <input type="checkbox" checked={m.includeSecrets} onChange={(e) => up((x) => (x.mcp = { ...m, includeSecrets: e.target.checked }))} /> {t("Show credentials and tokens to agents unredacted (unsafe)")}
      </label>
      <div className="f-row">
        <span>{t("Folder for agent files")}</span>
        <input
          className="mono"
          placeholder={t("empty = mcp-files in the data folder")}
          value={m.filesDir}
          onChange={(e) => up((x) => (x.mcp = { ...m, filesDir: e.target.value }))}
        />
        <button
          onClick={async () => {
            const p = await openDialog({ directory: true, multiple: false });
            if (typeof p === "string") up((x) => (x.mcp = { ...m, filesDir: p }));
          }}
        >
          {t("Choose folder…")}
        </button>
      </div>
      <p className="muted small">{t("Exports, .http collections and files served by mock rules: agents may only read and write files in this folder. Captured traffic is foreign content; an agent with full control could be misled by it, so grant full control only while you watch.")}</p>
      <div className="f-row">
        <span>{t("Token")}</span>
        <input readOnly className="mono" value={m.token} />
        <button onClick={() => navigator.clipboard.writeText(m.token)} disabled={!m.token}>
          {t("Copy")}
        </button>
        <button onClick={newToken}>{t("New token")}</button>
      </div>
      <fieldset className="f-section mcp-client-setup">
        <legend>{t("Set up for")}</legend>
        <label className="f-row">
          <span>{t("AI client")}</span>
          <select value={client} disabled={settingUp} onChange={(e) => {
            const mcpClient = e.target.value as McpClient;
            set((st) => ({ layout: { ...st.layout, mcpClient } }));
            actions.saveLayout();
          }}>
            {Object.entries(CLIENTS).map(([id, label]) => <option key={id} value={id}>{id === "claudeCode" ? t("Claude Code") : label}</option>)}
          </select>
        </label>
        <p className="muted small">
          {client === "claudeCode" ? t("Run this command in a terminal (PowerShell on Windows), or use automatic setup.")
            : client === "codex" ? t("Merge this section into ~/.codex/config.toml, or use automatic setup.")
            : client === "vsCode" ? t("Merge this into VS Code's user mcp.json, or use automatic setup.")
            : t("Merge this into ~/.cursor/mcp.json, or use automatic setup.")}
        </p>
        <label className="mcp-setup-preview">
          <span>{client === "claudeCode" ? t("Command") : t("Configuration snippet")}</span>
          <textarea readOnly className="mono" spellCheck={false} autoCorrect="off" autoCapitalize="off" rows={Math.max(3, Math.min(10, instructions.split("\n").length))} value={instructions} />
        </label>
        <div className="f-inline">
          <button className="primary" disabled={!m.enabled || !m.token || unsaved || settingUp}
            title={t("Writes the Quena server with its token into the user configuration of {name} (a copy of the file is kept as .bak). Click OK first so the server runs with these settings.", { name })}
            onClick={() => void setup()}>
            {settingUp ? t("Setting up…") : t("Set up {name}", { name })}
          </button>
          <button disabled={!instructions || settingUp} onClick={() => void copy()}>{client === "claudeCode" ? t("Copy command") : t("Copy configuration")}</button>
        </div>
        {(!m.enabled || !m.token) ? <p className="muted small">{t("Enable the MCP server and click OK before setting up a client.")}</p>
          : unsaved && <p className="muted small">{t("Click OK first: the server's settings are not saved yet")}</p>}
        {feedback?.key === setupKey && <p className={feedback.error ? "err" : "small"} role={feedback.error ? "alert" : "status"}>{feedback.text}</p>}
      </fieldset>
      <div className="f-row">
        <span>{t("Agent skill")}</span>
        <div className="f-inline">
          {(
            [
              ["claudeCode", "Claude Code"],
              ["codex", "Codex"],
            ] as const
          ).map(([c, name]) => (
            <button
              key={c}
              disabled={installingSkill}
              title={t("Installs a skill that tells the agent how to debug traffic with Quena's tools (failing and slow requests, diagnostics, LLM calls, mocks).")}
              onClick={async () => {
                setInstallingSkill(true);
                setSkillFeedback(null);
                try {
                  setSkillFeedback({ error: false, text: t("Skill installed: {path}", { path: await api.mcpInstallSkill(c) }) });
                } catch (e) {
                  setSkillFeedback({ error: true, text: String(e) });
                } finally {
                  setInstallingSkill(false);
                }
              }}
            >
              {name}
            </button>
          ))}
        </div>
      </div>
      {skillFeedback && <p className={skillFeedback.error ? "err" : "small"} role={skillFeedback.error ? "alert" : "status"}>{skillFeedback.text}</p>}
      {status && (
        <p className="muted small">
          {status.running ? t("Running at {url}", { url: status.url ?? "" }) : status.error ? t("Not running: {error}", { error: status.error }) : t("Not running")}
        </p>
      )}
    </>
  );
}
