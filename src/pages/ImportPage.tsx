import { useEffect, useState } from "react";
import { Download, LoaderCircle, RefreshCw, Zap } from "lucide-react";
import type {
  ApiBackend,
  CcMcpCandidate,
  CcPromptCandidate,
  ImportCandidate,
  Provider,
} from "../lib/types";
import { maskSecret } from "../lib/mask";
import * as api from "../lib/api";

function backendLabel(b: ApiBackend): string {
  if (b === "messages") return "Anthropic";
  if (b === "responses") return "Responses";
  return "OpenAI";
}

type ImportTab = "providers" | "mcp" | "prompts";

export function ImportPage({
  onImported,
  notify,
  withSwitching,
}: {
  onImported: () => Promise<void>;
  notify: (msg: string, tone?: "ok" | "error") => void;
  withSwitching?: <T>(
    work: () => Promise<T>,
    labels?: { title?: string; detail?: string },
  ) => Promise<T | undefined>;
}) {
  const [tab, setTab] = useState<ImportTab>("providers");
  const [candidates, setCandidates] = useState<ImportCandidate[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [backendOverride, setBackendOverride] = useState<
    Record<string, ApiBackend>
  >({});
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [globalBackend, setGlobalBackend] = useState<ApiBackend | "keep">(
    "keep",
  );
  const [mcpList, setMcpList] = useState<CcMcpCandidate[]>([]);
  const [mcpSelected, setMcpSelected] = useState<Set<string>>(new Set());
  const [promptList, setPromptList] = useState<CcPromptCandidate[]>([]);
  const [promptSelected, setPromptSelected] = useState<Set<string>>(new Set());
  const [extraLoading, setExtraLoading] = useState(false);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.importCcswitchPreview();
      if (!res.ok || !res.data) {
        setError(res.error ?? "预览失败");
        setCandidates([]);
        return;
      }
      setCandidates(res.data);
      setSelected(new Set(res.data.map((c) => c.id)));
      const map: Record<string, ApiBackend> = {};
      for (const c of res.data) map[c.id] = c.suggestedBackend;
      setBackendOverride(map);
    } finally {
      setLoading(false);
    }
  };

  const loadMcp = async () => {
    setExtraLoading(true);
    try {
      const res = await api.importCcswitchMcpPreview();
      if (!res.ok || !res.data) {
        notify(res.error ?? "MCP 预览失败", "error");
        setMcpList([]);
        return;
      }
      setMcpList(res.data);
      setMcpSelected(new Set(res.data.map((c) => c.id)));
    } finally {
      setExtraLoading(false);
    }
  };

  const loadPrompts = async () => {
    setExtraLoading(true);
    try {
      const res = await api.importCcswitchPromptsPreview();
      if (!res.ok || !res.data) {
        notify(res.error ?? "提示词预览失败", "error");
        setPromptList([]);
        return;
      }
      setPromptList(res.data);
      setPromptSelected(new Set(res.data.map((c) => c.id)));
    } finally {
      setExtraLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  useEffect(() => {
    if (tab === "mcp" && mcpList.length === 0) void loadMcp();
    if (tab === "prompts" && promptList.length === 0) void loadPrompts();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab]);

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleAll = (on: boolean) => {
    setSelected(on ? new Set(candidates.map((c) => c.id)) : new Set());
  };

  const applyGlobalBackend = (b: ApiBackend | "keep") => {
    setGlobalBackend(b);
    if (b === "keep") return;
    setBackendOverride((prev) => {
      const next = { ...prev };
      for (const c of candidates) next[c.id] = b;
      return next;
    });
  };

  const apply = async (andEnableFirst: boolean) => {
    const ids = [...selected];
    if (ids.length === 0) {
      notify("请至少选择一个供应商", "error");
      return;
    }
    setApplying(true);
    try {
      const res = await api.importCcswitchApply(ids);
      if (!res.ok || !res.data) {
        notify(res.error ?? "导入失败", "error");
        return;
      }

      // Apply protocol overrides if user changed them
      let updated = res.data;
      const patches: Provider[] = [];
      for (const p of res.data) {
        // Match by name+baseUrl to original candidate
        const cand = candidates.find(
          (c) =>
            c.name === p.name &&
            (p.baseUrl.includes(c.baseUrl.replace(/\/v1$/i, "")) ||
              c.baseUrl.includes(p.baseUrl.replace(/\/v1$/i, ""))),
        );
        const want = cand
          ? backendOverride[cand.id] ?? cand.suggestedBackend
          : p.apiBackend;
        if (want !== p.apiBackend) {
          const patched: Provider = {
            ...p,
            apiBackend: want,
            models: p.models.map((m) => ({ ...m, apiBackend: want })),
            updatedAt: Math.floor(Date.now() / 1000),
          };
          const u = await api.upsertProvider(patched);
          if (u.ok && u.data) patches.push(u.data);
        }
      }
      if (patches.length) {
        updated = res.data.map(
          (p) => patches.find((x) => x.id === p.id) ?? p,
        );
      }

      // Optional health probe after import
      let okCount = 0;
      let failCount = 0;
      for (const p of updated.slice(0, 8)) {
        const h = await api.testProvider(p.id);
        if (h.ok && h.data?.ok) okCount++;
        else failCount++;
      }

      notify(
        `已导入 ${updated.length} 个 · 测通成功 ${okCount}${failCount ? ` · 失败 ${failCount}` : ""}`,
      );

      await onImported();

      if (andEnableFirst && updated[0] && withSwitching) {
        const first = updated[0];
        const en = await withSwitching(
          () => api.enableProvider(first.id, false),
          { title: "启用导入项", detail: `正在启用 ${first.name}…` },
        );
        if (en?.ok) notify(`已启用 ${first.name}`);
        else if (en && !en.ok && en.error?.includes("NEEDS_FORCE")) {
          const ok = window.confirm(`${en.error}\n\n仍要强制启用？`);
          if (ok) {
            const f = await withSwitching(
              () => api.enableProvider(first.id, true),
              { title: "强制启用", detail: first.name },
            );
            if (f?.ok) notify(`已强制启用 ${first.name}`);
            else notify(f?.error ?? "启用失败", "error");
          }
        } else if (en && !en.ok) {
          notify(en.error ?? "启用失败", "error");
        }
        await onImported();
      }
    } finally {
      setApplying(false);
    }
  };

  const applyMcp = async () => {
    const ids = [...mcpSelected];
    if (ids.length === 0) {
      notify("请至少选择一个 MCP", "error");
      return;
    }
    setApplying(true);
    try {
      const res = await api.importCcswitchMcpApply(ids);
      if (!res.ok || !res.data) {
        notify(res.error ?? "MCP 导入失败", "error");
        return;
      }
      notify(
        res.data.length
          ? `已导入 ${res.data.length} 个 MCP：${res.data.join(", ")}`
          : "没有新的 MCP（可能都已存在）",
      );
      await onImported();
    } finally {
      setApplying(false);
    }
  };

  const applyPrompts = async () => {
    const ids = [...promptSelected];
    if (ids.length === 0) {
      notify("请至少选择一条提示词", "error");
      return;
    }
    setApplying(true);
    try {
      const res = await api.importCcswitchPromptsApply(ids);
      if (!res.ok || res.data == null) {
        notify(res.error ?? "提示词导入失败", "error");
        return;
      }
      notify(
        res.data > 0
          ? `已导入 ${res.data} 条提示词`
          : "没有新的提示词（名称重复已跳过）",
      );
      await onImported();
    } finally {
      setApplying(false);
    }
  };

  return (
    <div className="page-wrap">
      <div className="section-head no-margin">
        <div>
          <h2>从 CC Switch 导入</h2>
          <p>读取本机 ~/.cc-switch：供应商 / MCP / 提示词。</p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            className="ghost-btn"
            onClick={() => {
              if (tab === "providers") void load();
              else if (tab === "mcp") void loadMcp();
              else void loadPrompts();
            }}
            disabled={loading || extraLoading}
          >
            {loading || extraLoading ? (
              <LoaderCircle className="spin" size={15} />
            ) : (
              <RefreshCw size={15} />
            )}
            刷新
          </button>
          {tab === "providers" && (
            <>
              <button
                type="button"
                className="ghost-btn"
                onClick={() => void apply(false)}
                disabled={applying || loading || selected.size === 0}
              >
                {applying ? (
                  <LoaderCircle className="spin" size={15} />
                ) : (
                  <Download size={15} />
                )}
                导入 ({selected.size})
              </button>
              <button
                type="button"
                className="primary-btn"
                onClick={() => void apply(true)}
                disabled={applying || loading || selected.size === 0}
              >
                {applying ? (
                  <LoaderCircle className="spin" size={15} />
                ) : (
                  <Zap size={15} />
                )}
                导入并启用首个
              </button>
            </>
          )}
          {tab === "mcp" && (
            <button
              type="button"
              className="primary-btn"
              onClick={() => void applyMcp()}
              disabled={applying || extraLoading || mcpSelected.size === 0}
            >
              {applying ? (
                <LoaderCircle className="spin" size={15} />
              ) : (
                <Download size={15} />
              )}
              导入 MCP ({mcpSelected.size})
            </button>
          )}
          {tab === "prompts" && (
            <button
              type="button"
              className="primary-btn"
              onClick={() => void applyPrompts()}
              disabled={applying || extraLoading || promptSelected.size === 0}
            >
              {applying ? (
                <LoaderCircle className="spin" size={15} />
              ) : (
                <Download size={15} />
              )}
              导入提示词 ({promptSelected.size})
            </button>
          )}
        </div>
      </div>

      <div className="import-protocol-bar" style={{ gap: 8 }}>
        {(
          [
            ["providers", "供应商"],
            ["mcp", "MCP"],
            ["prompts", "提示词"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            className={tab === id ? "primary-btn" : "ghost-btn"}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === "providers" && (
        <>
          {!loading && candidates.length > 0 && (
            <div className="import-protocol-bar">
              <span>批量协议</span>
              <select
                value={globalBackend}
                onChange={(e) =>
                  applyGlobalBackend(e.target.value as ApiBackend | "keep")
                }
              >
                <option value="keep">保持各项建议</option>
                <option value="chat_completions">全部 → OpenAI Chat</option>
                <option value="messages">全部 → Anthropic Messages</option>
                <option value="responses">全部 → OpenAI Responses</option>
              </select>
              <small>
                Grok 中转多数用 OpenAI Chat；Claude Code 中转常用 Messages。
              </small>
            </div>
          )}

          {loading ? (
            <div className="empty-state">
              <LoaderCircle className="spin" size={22} />
              <b>正在扫描 CC Switch…</b>
            </div>
          ) : error ? (
            <div className="empty-state">
              <b>无法读取</b>
              <p>{error}</p>
              <button
                type="button"
                className="primary-btn"
                onClick={() => void load()}
              >
                重试
              </button>
            </div>
          ) : candidates.length === 0 ? (
            <div className="empty-state">
              <b>没有可导入项</b>
              <p>未在 ~/.cc-switch 发现带 Base URL 的 Claude 供应商。</p>
            </div>
          ) : (
            <div className="import-card">
              <div className="import-toolbar">
                <label className="check-field">
                  <input
                    type="checkbox"
                    checked={selected.size === candidates.length}
                    onChange={(e) => toggleAll(e.target.checked)}
                  />
                  <span>全选 · 共 {candidates.length} 项</span>
                </label>
              </div>
              <div className="import-list">
                {candidates.map((c) => {
                  const backend = backendOverride[c.id] ?? c.suggestedBackend;
                  return (
                    <div key={c.id} className="import-row import-row-grid">
                      <input
                        type="checkbox"
                        checked={selected.has(c.id)}
                        onChange={() => toggle(c.id)}
                      />
                      <div className="import-main">
                        <div className="import-title">
                          <b>{c.name}</b>
                          <span className="badge badge-backend">
                            {backendLabel(backend)}
                          </span>
                        </div>
                        <span>
                          <code>{c.baseUrl}</code>
                        </span>
                        <span>
                          模型 {c.defaultModel} · Key {maskSecret(c.apiKey)}
                        </span>
                      </div>
                      <select
                        className="import-backend-select"
                        value={backend}
                        onChange={(e) =>
                          setBackendOverride((prev) => ({
                            ...prev,
                            [c.id]: e.target.value as ApiBackend,
                          }))
                        }
                        title="导入后使用的 API 协议"
                      >
                        <option value="chat_completions">OpenAI Chat</option>
                        <option value="messages">Anthropic</option>
                        <option value="responses">Responses</option>
                      </select>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </>
      )}

      {tab === "mcp" && (
        extraLoading ? (
          <div className="empty-state">
            <LoaderCircle className="spin" size={22} />
            <b>扫描 MCP…</b>
          </div>
        ) : mcpList.length === 0 ? (
          <div className="empty-state">
            <b>没有可导入的 MCP</b>
            <p>CC Switch 的 mcp_servers 表为空或无法解析。</p>
          </div>
        ) : (
          <div className="import-card">
            <div className="import-toolbar">
              <label className="check-field">
                <input
                  type="checkbox"
                  checked={mcpSelected.size === mcpList.length}
                  onChange={(e) =>
                    setMcpSelected(
                      e.target.checked
                        ? new Set(mcpList.map((c) => c.id))
                        : new Set(),
                    )
                  }
                />
                <span>全选 · 共 {mcpList.length} 项</span>
              </label>
            </div>
            <div className="import-list">
              {mcpList.map((c) => (
                <div key={c.id} className="import-row">
                  <input
                    type="checkbox"
                    checked={mcpSelected.has(c.id)}
                    onChange={() => {
                      setMcpSelected((prev) => {
                        const next = new Set(prev);
                        if (next.has(c.id)) next.delete(c.id);
                        else next.add(c.id);
                        return next;
                      });
                    }}
                  />
                  <div className="import-main">
                    <div className="import-title">
                      <b className="mono">{c.name}</b>
                      <span className="badge badge-backend">
                        {c.url ? "HTTP" : "stdio"}
                      </span>
                      {!c.enabled && (
                        <span className="badge badge-muted">未启用</span>
                      )}
                    </div>
                    <span>
                      <code>
                        {c.url
                          ? c.url
                          : `${c.command ?? ""} ${(c.args ?? []).slice(0, 3).join(" ")}`}
                      </code>
                    </span>
                    {c.description && <span>{c.description}</span>}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )
      )}

      {tab === "prompts" && (
        extraLoading ? (
          <div className="empty-state">
            <LoaderCircle className="spin" size={22} />
            <b>扫描提示词…</b>
          </div>
        ) : promptList.length === 0 ? (
          <div className="empty-state">
            <b>没有可导入的提示词</b>
            <p>CC Switch 的 prompts 表为空。</p>
          </div>
        ) : (
          <div className="import-card">
            <div className="import-toolbar">
              <label className="check-field">
                <input
                  type="checkbox"
                  checked={promptSelected.size === promptList.length}
                  onChange={(e) =>
                    setPromptSelected(
                      e.target.checked
                        ? new Set(promptList.map((c) => c.id))
                        : new Set(),
                    )
                  }
                />
                <span>全选 · 共 {promptList.length} 项</span>
              </label>
            </div>
            <div className="import-list">
              {promptList.map((c) => (
                <div key={c.id} className="import-row">
                  <input
                    type="checkbox"
                    checked={promptSelected.has(c.id)}
                    onChange={() => {
                      setPromptSelected((prev) => {
                        const next = new Set(prev);
                        if (next.has(c.id)) next.delete(c.id);
                        else next.add(c.id);
                        return next;
                      });
                    }}
                  />
                  <div className="import-main">
                    <div className="import-title">
                      <b>{c.name}</b>
                      <span className="badge badge-muted">{c.appType}</span>
                    </div>
                    <span>
                      {c.content.length > 120
                        ? `${c.content.slice(0, 120)}…`
                        : c.content}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )
      )}
    </div>
  );
}
