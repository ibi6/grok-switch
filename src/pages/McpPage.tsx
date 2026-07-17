import { useCallback, useEffect, useMemo, useState } from "react";
import {
  LoaderCircle,
  Plus,
  Power,
  RefreshCw,
  Search,
  Trash2,
  Zap,
  Cable,
} from "lucide-react";
import type { McpDraft, McpServer } from "../lib/types";
import * as api from "../lib/api";

const NAME_RE = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/;

function emptyDraft(): McpDraft {
  return {
    name: "",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", ""],
    url: "",
    env: {},
    headers: {},
    enabled: true,
    startupTimeoutSec: 30,
  };
}

function transportLabel(t: McpServer["transport"]): string {
  if (t === "http") return "HTTP";
  if (t === "stdio") return "stdio";
  return "?";
}

function envToLines(env: Record<string, string>): string {
  return Object.entries(env)
    .map(([k, v]) => `${k}=${v}`)
    .join("\n");
}

function linesToMap(text: string): Record<string, string> {
  const map: Record<string, string> = {};
  for (const line of text.split(/\r?\n/)) {
    const t = line.trim();
    if (!t || t.startsWith("#")) continue;
    const i = t.indexOf("=");
    if (i <= 0) continue;
    map[t.slice(0, i).trim()] = t.slice(i + 1).trim();
  }
  return map;
}

export function McpPage({
  notify,
}: {
  notify: (msg: string, tone?: "ok" | "error") => void;
}) {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<McpDraft>(emptyDraft());
  const [argsText, setArgsText] = useState("");
  const [envText, setEnvText] = useState("");
  const [headersText, setHeadersText] = useState("");
  const [mode, setMode] = useState<"stdio" | "http">("stdio");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await api.listMcpServers();
      if (!res.ok || !res.data) {
        notify(res.error ?? "加载 MCP 失败", "error");
        setServers([]);
        return;
      }
      setServers(res.data);
    } finally {
      setLoading(false);
    }
  }, [notify]);

  useEffect(() => {
    void load();
  }, [load]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return servers;
    return servers.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        (s.command ?? "").toLowerCase().includes(q) ||
        (s.url ?? "").toLowerCase().includes(q),
    );
  }, [servers, query]);

  const openServer = (s: McpServer) => {
    setSelected(s.name);
    setCreating(false);
    setEditing(false);
    setDraft({
      name: s.name,
      command: s.command,
      args: s.args,
      url: s.url,
      env: s.env,
      headers: s.headers,
      enabled: s.enabled,
      startupTimeoutSec: s.startupTimeoutSec,
      toolTimeoutSec: s.toolTimeoutSec,
    });
    setArgsText(s.args.join("\n"));
    setEnvText(envToLines(s.env));
    setHeadersText(envToLines(s.headers));
    setMode(s.transport === "http" ? "http" : "stdio");
  };

  const startCreate = () => {
    setCreating(true);
    setEditing(true);
    setSelected(null);
    const d = emptyDraft();
    setDraft(d);
    setArgsText(d.args.join("\n"));
    setEnvText("");
    setHeadersText("");
    setMode("stdio");
  };

  const startEdit = () => {
    setEditing(true);
  };

  const buildDraft = (): McpDraft | null => {
    const name = draft.name.trim();
    if (!NAME_RE.test(name) || name.endsWith("-")) {
      notify("名称须为字母/数字/下划线/连字符，且不以 - 开头结尾", "error");
      return null;
    }
    const args = argsText
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter(Boolean);
    const env = linesToMap(envText);
    const headers = linesToMap(headersText);
    if (mode === "stdio") {
      if (!draft.command?.trim()) {
        notify("stdio 模式需要 command", "error");
        return null;
      }
      return {
        ...draft,
        name,
        command: draft.command.trim(),
        args,
        url: undefined,
        env,
        headers,
      };
    }
    if (!draft.url?.trim()) {
      notify("HTTP 模式需要 url", "error");
      return null;
    }
    return {
      ...draft,
      name,
      command: undefined,
      args: [],
      url: draft.url.trim(),
      env,
      headers,
    };
  };

  const save = async () => {
    const d = buildDraft();
    if (!d) return;
    setBusy("save");
    try {
      const res = await api.upsertMcpServer(d);
      if (!res.ok || !res.data) {
        notify(res.error ?? "保存失败", "error");
        return;
      }
      notify(`已保存 MCP：${res.data.name}`);
      setCreating(false);
      setEditing(false);
      setSelected(res.data.name);
      openServer(res.data);
      await load();
    } finally {
      setBusy(null);
    }
  };

  const onToggle = async (s: McpServer) => {
    setBusy(`tog-${s.name}`);
    try {
      const res = await api.setMcpEnabled(s.name, !s.enabled);
      if (!res.ok || !res.data) {
        notify(res.error ?? "切换失败", "error");
        return;
      }
      notify(res.data.enabled ? `已启用 ${s.name}` : `已禁用 ${s.name}`);
      if (selected === s.name) openServer(res.data);
      await load();
    } finally {
      setBusy(null);
    }
  };

  const onTest = async (name: string) => {
    setBusy(`test-${name}`);
    try {
      const res = await api.testMcpServer(name);
      if (!res.ok || !res.data) {
        notify(res.error ?? "探测失败", "error");
        return;
      }
      notify(
        res.data.ok
          ? `探测成功 · ${res.data.latencyMs}ms · ${res.data.detail}`
          : `探测失败 · ${res.data.detail}`,
        res.data.ok ? "ok" : "error",
      );
    } finally {
      setBusy(null);
    }
  };

  const onDelete = async (name: string) => {
    if (!window.confirm(`确定从 config.toml 删除 MCP「${name}」？`)) return;
    setBusy(`del-${name}`);
    try {
      const res = await api.deleteMcpServer(name);
      if (!res.ok) {
        notify(res.error ?? "删除失败", "error");
        return;
      }
      notify("已删除");
      if (selected === name) {
        setSelected(null);
        setEditing(false);
        setCreating(false);
      }
      await load();
    } finally {
      setBusy(null);
    }
  };

  const current = selected
    ? servers.find((s) => s.name === selected) ?? null
    : null;

  return (
    <div className="page-wrap">
      <div className="section-head no-margin">
        <div>
          <h2>MCP</h2>
          <p>
            管理 <code>~/.grok/config.toml</code> 中的{" "}
            <code>[mcp_servers.*]</code>。切换供应商不会覆盖这些表。
          </p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            className="ghost-btn"
            onClick={() => void load()}
            disabled={loading || !!busy}
          >
            {loading ? <LoaderCircle className="spin" size={15} /> : <RefreshCw size={15} />}
            刷新
          </button>
          <button type="button" className="primary-btn" onClick={startCreate} disabled={!!busy}>
            <Plus size={15} /> 添加
          </button>
        </div>
      </div>

      <div className="toolbar">
        <div className="search">
          <Search size={15} />
          <input
            placeholder="搜索名称 / command / url"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>

      {loading ? (
        <div className="empty-state">
          <LoaderCircle className="spin" size={22} />
          <b>读取 MCP 配置…</b>
        </div>
      ) : (
        <div className="skills-layout">
          <div className="skills-list-pane">
            {filtered.length === 0 ? (
              <div className="empty-state">
                <Cable size={28} style={{ opacity: 0.5 }} />
                <b>{servers.length === 0 ? "还没有 MCP 服务器" : "无匹配结果"}</b>
                <p>
                  {servers.length === 0
                    ? "添加 stdio（command）或 HTTP（url）服务器。"
                    : "试试其他关键词。"}
                </p>
              </div>
            ) : (
              <div className="provider-list">
                {filtered.map((s) => {
                  const active = selected === s.name && !creating;
                  return (
                    <div
                      key={s.name}
                      className={`provider-card is-clickable ${active ? "is-current" : ""} ${s.enabled ? "" : "is-idle"}`}
                      role="button"
                      tabIndex={0}
                      onClick={() => openServer(s)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          openServer(s);
                        }
                      }}
                    >
                      <div
                        className="provider-avatar"
                        style={{
                          background: s.enabled ? "#06b6d4" : "#64748b",
                        }}
                      >
                        <Cable size={16} color="#fff" />
                      </div>
                      <div className="provider-body">
                        <div className="provider-title-row">
                          <b className="mono">{s.name}</b>
                          <span className="badge badge-backend">
                            {transportLabel(s.transport)}
                          </span>
                          {!s.enabled && (
                            <span className="badge badge-muted">已禁用</span>
                          )}
                        </div>
                        <div className="provider-meta">
                          <code>
                            {s.transport === "http"
                              ? s.url
                              : `${s.command ?? ""} ${s.args.slice(0, 2).join(" ")}`.trim()}
                          </code>
                        </div>
                      </div>
                      <div
                        className="provider-actions"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <button
                          type="button"
                          className="icon-btn"
                          title={s.enabled ? "禁用" : "启用"}
                          disabled={!!busy}
                          onClick={() => void onToggle(s)}
                        >
                          <Power size={14} />
                        </button>
                        <button
                          type="button"
                          className="icon-btn"
                          title="探测"
                          disabled={!!busy}
                          onClick={() => void onTest(s.name)}
                        >
                          {busy === `test-${s.name}` ? (
                            <LoaderCircle className="spin" size={14} />
                          ) : (
                            <Zap size={14} />
                          )}
                        </button>
                        <button
                          type="button"
                          className="icon-btn"
                          title="删除"
                          disabled={!!busy}
                          onClick={() => void onDelete(s.name)}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          <div className="skills-editor-pane">
            {creating || editing ? (
              <div className="skills-editor card-like">
                <div className="skills-editor-head">
                  <b>{creating ? "添加 MCP 服务器" : `编辑 ${draft.name}`}</b>
                  <div className="header-actions">
                    <button
                      type="button"
                      className="ghost-btn"
                      onClick={() => {
                        setCreating(false);
                        setEditing(false);
                        if (selected) {
                          const s = servers.find((x) => x.name === selected);
                          if (s) openServer(s);
                        }
                      }}
                    >
                      取消
                    </button>
                    <button
                      type="button"
                      className="primary-btn"
                      disabled={busy === "save"}
                      onClick={() => void save()}
                    >
                      {busy === "save" ? (
                        <LoaderCircle className="spin" size={15} />
                      ) : null}
                      写入 config.toml
                    </button>
                  </div>
                </div>

                <label className="sheet-field">
                  <span>名称</span>
                  <input
                    className="mono"
                    value={draft.name}
                    disabled={!creating}
                    onChange={(e) =>
                      setDraft((d) => ({
                        ...d,
                        name: e.target.value.replace(/[^A-Za-z0-9_-]/g, ""),
                      }))
                    }
                    placeholder="filesystem"
                  />
                </label>

                <div className="sheet-field">
                  <span>传输方式</span>
                  <div className="sheet-inline-toggles">
                    <button
                      type="button"
                      className={`sheet-chip ${mode === "stdio" ? "on" : ""}`}
                      onClick={() => setMode("stdio")}
                    >
                      stdio (command)
                    </button>
                    <button
                      type="button"
                      className={`sheet-chip ${mode === "http" ? "on" : ""}`}
                      onClick={() => setMode("http")}
                    >
                      HTTP (url)
                    </button>
                  </div>
                </div>

                {mode === "stdio" ? (
                  <>
                    <label className="sheet-field">
                      <span>command</span>
                      <input
                        className="mono"
                        value={draft.command ?? ""}
                        onChange={(e) =>
                          setDraft((d) => ({ ...d, command: e.target.value }))
                        }
                        placeholder="npx"
                      />
                    </label>
                    <label className="sheet-field">
                      <span>args（每行一个）</span>
                      <textarea
                        className="skills-body mono"
                        rows={5}
                        value={argsText}
                        onChange={(e) => setArgsText(e.target.value)}
                        placeholder={"-y\n@modelcontextprotocol/server-filesystem\nC:\\\\path"}
                      />
                    </label>
                  </>
                ) : (
                  <label className="sheet-field">
                    <span>url</span>
                    <input
                      className="mono"
                      value={draft.url ?? ""}
                      onChange={(e) =>
                        setDraft((d) => ({ ...d, url: e.target.value }))
                      }
                      placeholder="http://localhost:5000/api/mcp"
                    />
                  </label>
                )}

                <label className="sheet-field">
                  <span>env（KEY=VALUE，每行一个）</span>
                  <textarea
                    className="mono"
                    rows={3}
                    value={envText}
                    onChange={(e) => setEnvText(e.target.value)}
                    placeholder="GITHUB_PERSONAL_ACCESS_TOKEN=ghp_…"
                  />
                </label>

                {mode === "http" && (
                  <label className="sheet-field">
                    <span>headers（KEY=VALUE）</span>
                    <textarea
                      className="mono"
                      rows={2}
                      value={headersText}
                      onChange={(e) => setHeadersText(e.target.value)}
                    />
                  </label>
                )}

                <label className="sheet-field">
                  <span>startup_timeout_sec（可选）</span>
                  <input
                    type="number"
                    min={1}
                    value={draft.startupTimeoutSec ?? ""}
                    onChange={(e) =>
                      setDraft((d) => ({
                        ...d,
                        startupTimeoutSec: e.target.value
                          ? Number(e.target.value)
                          : undefined,
                      }))
                    }
                  />
                </label>

                <label className="sheet-check" style={{ gap: 8 }}>
                  <input
                    type="checkbox"
                    checked={draft.enabled}
                    onChange={(e) =>
                      setDraft((d) => ({ ...d, enabled: e.target.checked }))
                    }
                  />
                  <span>启用（enabled = true）</span>
                </label>
              </div>
            ) : current ? (
              <div className="skills-editor card-like">
                <div className="skills-editor-head">
                  <div>
                    <b className="mono">{current.name}</b>
                    <div className="topbar-sub">
                      {transportLabel(current.transport)} ·{" "}
                      {current.enabled ? "已启用" : "已禁用"}
                    </div>
                  </div>
                  <div className="header-actions">
                    <button
                      type="button"
                      className="ghost-btn"
                      onClick={() => void onTest(current.name)}
                      disabled={!!busy}
                    >
                      <Zap size={14} /> 探测
                    </button>
                    <button
                      type="button"
                      className="primary-btn"
                      onClick={startEdit}
                    >
                      编辑
                    </button>
                  </div>
                </div>
                <div className="provider-meta">
                  {current.command && (
                    <code>
                      {current.command} {current.args.join(" ")}
                    </code>
                  )}
                  {current.url && <code>{current.url}</code>}
                </div>
                {Object.keys(current.env).length > 0 && (
                  <pre className="skills-preview mono">
                    {Object.entries(current.env)
                      .map(([k, v]) => `${k}=${v.length > 12 ? `${v.slice(0, 4)}…` : v}`)
                      .join("\n")}
                  </pre>
                )}
                <div className="notice" style={{ marginTop: 8 }}>
                  <Cable size={16} />
                  <div>
                    <b>Grok 如何加载</b>
                    <p>
                      写入 <code>~/.grok/config.toml</code> 的{" "}
                      <code>[mcp_servers.{current.name}]</code>
                      。重启 / 新开 Grok 会话后生效。
                    </p>
                  </div>
                </div>
              </div>
            ) : (
              <div className="empty-state">
                <Cable size={28} style={{ opacity: 0.45 }} />
                <b>选择一个 MCP 服务器</b>
                <p>查看配置，或添加 stdio / HTTP 服务器。</p>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
