import { useEffect, useMemo, useState } from "react";
import {
  Check,
  LoaderCircle,
  MoreHorizontal,
  Pencil,
  Plus,
  Search,
  Trash2,
  Zap,
} from "lucide-react";
import type { Provider, Settings } from "../lib/types";
import { maskSecret } from "../lib/mask";
import { backendLabel, modelFlag } from "../lib/providerUtils";
import * as api from "../lib/api";
import { ProviderForm } from "../components/ProviderForm";

const AVATAR_COLORS = [
  "#4c8dff",
  "#22c55e",
  "#a855f7",
  "#f59e0b",
  "#06b6d4",
  "#ef4444",
  "#8b5cf6",
  "#14b8a6",
];

function colorFor(id: string): string {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h + id.charCodeAt(i) * 17) % AVATAR_COLORS.length;
  return AVATAR_COLORS[h];
}

export function ProvidersPage({
  providers,
  settings,
  onRefresh,
  notify,
  withSwitching,
}: {
  providers: Provider[];
  settings: Settings | null;
  onRefresh: () => Promise<void>;
  notify: (msg: string, tone?: "ok" | "error") => void;
  withSwitching: <T>(
    work: () => Promise<T>,
    labels?: { title?: string; detail?: string },
  ) => Promise<T | undefined>;
}) {
  const [query, setQuery] = useState("");
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<Provider | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);

  useEffect(() => {
    const open = () => {
      setEditing(null);
      setFormOpen(true);
    };
    window.addEventListener("gs-open-provider-form", open);
    return () => window.removeEventListener("gs-open-provider-form", open);
  }, []);

  useEffect(() => {
    const close = () => setMenuId(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, []);

  const filtered = useMemo(() => {
    const q = query.toLowerCase();
    const list = providers.filter((p) =>
      (p.name + p.baseUrl + p.apiBackend).toLowerCase().includes(q),
    );
    // Current first, then by name
    const cur = settings?.currentProviderId;
    return [...list].sort((a, b) => {
      if (a.id === cur) return -1;
      if (b.id === cur) return 1;
      return a.name.localeCompare(b.name, "zh");
    });
  }, [providers, query, settings?.currentProviderId]);

  const openCreate = () => {
    setEditing(null);
    setFormOpen(true);
  };

  const openEdit = (p: Provider) => {
    setEditing(p);
    setFormOpen(true);
    setMenuId(null);
  };

  const finishEnableOk = async (
    provider: Provider | undefined,
    forced: boolean,
  ) => {
    const flag = provider ? modelFlag(provider) : "gs-…";
    notify(
      forced
        ? `已强制启用 · ${flag}`
        : `已切换 · ${flag}`,
    );
    try {
      await navigator.clipboard?.writeText(flag);
    } catch {
      /* optional */
    }
    await onRefresh();
  };

  const onEnable = async (id: string) => {
    const provider = providers.find((x) => x.id === id);
    if (
      settings?.currentMode === "provider" &&
      settings.currentProviderId === id
    ) {
      notify("已是当前供应商");
      return;
    }
    setBusy(`enable-${id}`);
    try {
      const res = await withSwitching(() => api.enableProvider(id, false), {
        title: "切换中",
        detail: "写入 Grok CLI 配置…",
      });
      if (!res) return;

      if (!res.ok) {
        if (res.error?.includes("NEEDS_FORCE")) {
          const ok = window.confirm(
            `${res.error}\n\n测通失败，仍要强制启用吗？`,
          );
          if (!ok) {
            notify(res.error, "error");
            return;
          }
          const forced = await withSwitching(
            () => api.enableProvider(id, true),
            { title: "强制启用", detail: "跳过测通，写入配置…" },
          );
          if (!forced) return;
          if (!forced.ok) {
            notify(forced.error ?? "启用失败", "error");
            return;
          }
          await finishEnableOk(provider, true);
          return;
        }
        notify(res.error ?? "启用失败", "error");
        return;
      }

      await finishEnableOk(provider, false);
    } finally {
      setBusy(null);
    }
  };

  const copyModelId = async (p: Provider) => {
    const flag = modelFlag(p);
    try {
      await navigator.clipboard.writeText(flag);
      notify(`已复制 ${flag}`);
    } catch {
      notify(`模型 id：${flag}`);
    }
  };

  const onTest = async (id: string) => {
    setBusy(`test-${id}`);
    setMenuId(null);
    try {
      const res = await api.testProvider(id);
      if (!res.ok || !res.data) {
        notify(res.error ?? "测通失败", "error");
        return;
      }
      notify(
        res.data.ok
          ? `测通成功 · ${res.data.latencyMs}ms`
          : `测通失败 · ${res.data.detail}`,
        res.data.ok ? "ok" : "error",
      );
      await onRefresh();
    } finally {
      setBusy(null);
    }
  };

  const onDelete = async (id: string) => {
    setMenuId(null);
    if (
      settings?.currentProviderId === id &&
      settings.currentMode === "provider"
    ) {
      notify("请先切换到其他供应商，再删除当前项", "error");
      return;
    }
    if (!window.confirm("确定删除该供应商？")) return;
    setBusy(`del-${id}`);
    try {
      const res = await api.deleteProvider(id);
      if (!res.ok) {
        notify(res.error ?? "删除失败", "error");
        return;
      }
      notify("已删除");
      await onRefresh();
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="page-wrap">
      <div className="toolbar">
        <div className="search">
          <Search size={15} />
          <input
            placeholder="搜索名称 / URL / 协议 · 点击卡片即可切换"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <button type="button" className="primary-btn" onClick={openCreate}>
          <Plus size={15} /> 添加
        </button>
      </div>

      {providers.length === 0 ? (
        <div className="empty-state">
          <b>还没有供应商</b>
          <p>添加中转站，或从 CC Switch 一键导入。</p>
          <button type="button" className="primary-btn" onClick={openCreate}>
            <Plus size={15} /> 添加供应商
          </button>
        </div>
      ) : filtered.length === 0 ? (
        <div className="empty-state">
          <b>无匹配结果</b>
          <p>试试其他关键词。</p>
        </div>
      ) : (
        <div className="provider-list">
          {filtered.map((p) => {
            const active =
              settings?.currentMode === "provider" &&
              p.id === settings.currentProviderId;
            const model =
              p.models.find((m) => m.id === p.defaultModelEntryId)?.model ??
              p.models[0]?.model ??
              "—";
            const enabling = busy === `enable-${p.id}`;
            const testing = busy === `test-${p.id}`;

            return (
              <div
                key={p.id}
                className={`provider-card is-clickable ${active ? "is-current" : ""} ${enabling ? "is-busy" : ""}`}
                role="button"
                tabIndex={0}
                onClick={() => {
                  if (!busy) void onEnable(p.id);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    if (!busy) void onEnable(p.id);
                  }
                }}
              >
                <div
                  className="provider-avatar"
                  style={{ background: colorFor(p.id) }}
                >
                  {enabling ? (
                    <LoaderCircle size={18} className="spin" color="#fff" />
                  ) : (
                    p.name.slice(0, 2).toUpperCase()
                  )}
                </div>

                <div className="provider-body">
                  <div className="provider-title-row">
                    <b>{p.name}</b>
                    {active && (
                      <span className="badge badge-current">
                        <Check size={12} /> 当前
                      </span>
                    )}
                    <span className="badge badge-backend">
                      {backendLabel(p.apiBackend)}
                    </span>
                    {p.source === "cc-switch" && (
                      <span className="badge badge-muted">CC Switch</span>
                    )}
                  </div>
                  <div className="provider-meta">
                    <code title={p.baseUrl}>{p.baseUrl}</code>
                    <span>{model}</span>
                    <code
                      className="copyable"
                      title="点击复制 grok -m 模型 id"
                      onClick={(e) => {
                        e.stopPropagation();
                        void copyModelId(p);
                      }}
                    >
                      {modelFlag(p)}
                    </code>
                    <span title="API Key">{maskSecret(p.apiKey)}</span>
                  </div>
                </div>

                <div
                  className="provider-actions"
                  onClick={(e) => e.stopPropagation()}
                >
                  {!active && (
                    <button
                      type="button"
                      className="primary-btn"
                      disabled={!!busy}
                      onClick={() => void onEnable(p.id)}
                    >
                      {enabling ? (
                        <LoaderCircle size={14} className="spin" />
                      ) : (
                        <Check size={14} />
                      )}
                      切换
                    </button>
                  )}
                  <div className="card-menu-wrap">
                    <button
                      type="button"
                      className="icon-btn"
                      disabled={!!busy}
                      title="更多"
                      onClick={(e) => {
                        e.stopPropagation();
                        setMenuId((id) => (id === p.id ? null : p.id));
                      }}
                    >
                      <MoreHorizontal size={16} />
                    </button>
                    {menuId === p.id && (
                      <div className="card-menu" role="menu">
                        <button
                          type="button"
                          onClick={() => void onTest(p.id)}
                        >
                          {testing ? (
                            <LoaderCircle size={14} className="spin" />
                          ) : (
                            <Zap size={14} />
                          )}
                          测通
                        </button>
                        <button type="button" onClick={() => openEdit(p)}>
                          <Pencil size={14} />
                          编辑
                        </button>
                        <button
                          type="button"
                          className="danger"
                          onClick={() => void onDelete(p.id)}
                        >
                          <Trash2 size={14} />
                          删除
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      <ProviderForm
        open={formOpen}
        initial={editing}
        onClose={() => setFormOpen(false)}
        onSaved={async (p) => {
          setFormOpen(false);
          await onRefresh();
        }}
        onSavedAndEnable={async (p) => {
          setFormOpen(false);
          await onRefresh();
          await onEnable(p.id);
        }}
        notify={notify}
      />
    </div>
  );
}
