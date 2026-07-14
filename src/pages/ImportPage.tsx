import { useEffect, useState } from "react";
import { Download, LoaderCircle, RefreshCw, Zap } from "lucide-react";
import type { ApiBackend, ImportCandidate, Provider } from "../lib/types";
import { maskSecret } from "../lib/mask";
import * as api from "../lib/api";

function backendLabel(b: ApiBackend): string {
  if (b === "messages") return "Anthropic";
  if (b === "responses") return "Responses";
  return "OpenAI";
}

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

  useEffect(() => {
    void load();
  }, []);

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

  return (
    <div className="page-wrap">
      <div className="section-head no-margin">
        <div>
          <h2>从 CC Switch 导入</h2>
          <p>读取本机 ~/.cc-switch；可改协议后导入并测通。</p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            className="ghost-btn"
            onClick={() => void load()}
            disabled={loading}
          >
            {loading ? (
              <LoaderCircle className="spin" size={15} />
            ) : (
              <RefreshCw size={15} />
            )}
            刷新
          </button>
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
        </div>
      </div>

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
          <small>Grok 中转多数用 OpenAI Chat；Claude Code 中转常用 Messages。</small>
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
    </div>
  );
}
