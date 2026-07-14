import { useEffect, useState } from "react";
import { Download, LoaderCircle, RefreshCw } from "lucide-react";
import type { ImportCandidate } from "../lib/types";
import { maskSecret } from "../lib/mask";
import * as api from "../lib/api";

function backendLabel(b: ImportCandidate["suggestedBackend"]): string {
  if (b === "messages") return "Anthropic";
  if (b === "responses") return "Responses";
  return "OpenAI";
}

export function ImportPage({
  onImported,
  notify,
}: {
  onImported: () => Promise<void>;
  notify: (msg: string, tone?: "ok" | "error") => void;
}) {
  const [candidates, setCandidates] = useState<ImportCandidate[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  const apply = async () => {
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
      notify(`已导入 ${res.data.length} 个供应商`);
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
          <p>读取本机 ~/.cc-switch，勾选后导入到 Grok Switch。</p>
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
            className="primary-btn"
            onClick={() => void apply()}
            disabled={applying || loading || selected.size === 0}
          >
            {applying ? (
              <LoaderCircle className="spin" size={15} />
            ) : (
              <Download size={15} />
            )}
            导入选中 ({selected.size})
          </button>
        </div>
      </div>

      {loading ? (
        <div className="empty-state">
          <LoaderCircle className="spin" size={22} />
          <b>正在扫描 CC Switch…</b>
        </div>
      ) : error ? (
        <div className="empty-state">
          <b>无法读取</b>
          <p>{error}</p>
          <button type="button" className="primary-btn" onClick={() => void load()}>
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
            {candidates.map((c) => (
              <label key={c.id} className="import-row">
                <input
                  type="checkbox"
                  checked={selected.has(c.id)}
                  onChange={() => toggle(c.id)}
                />
                <div className="import-main">
                  <div className="import-title">
                    <b>{c.name}</b>
                    <span className="badge badge-backend">
                      {backendLabel(c.suggestedBackend)}
                    </span>
                  </div>
                  <span>
                    <code>{c.baseUrl}</code>
                  </span>
                  <span>
                    模型 {c.defaultModel} · Key {maskSecret(c.apiKey)}
                  </span>
                </div>
              </label>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
