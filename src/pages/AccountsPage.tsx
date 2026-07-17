import { useMemo, useState } from "react";
import {
  Check,
  LoaderCircle,
  Plus,
  Search,
  Trash2,
  UserRound,
} from "lucide-react";
import type { Account, Settings } from "../lib/types";
import * as api from "../lib/api";

function formatTs(ts?: number): string {
  if (!ts) return "从未";
  return new Date(ts * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "AC";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[1][0]).toUpperCase();
}

export function AccountsPage({
  accounts,
  settings,
  onRefresh,
  notify,
  withSwitching,
}: {
  accounts: Account[];
  settings: Settings | null;
  onRefresh: () => Promise<void>;
  notify: (msg: string, tone?: "ok" | "error") => void;
  withSwitching: <T>(
    work: () => Promise<T>,
    labels?: { title?: string; detail?: string },
  ) => Promise<T | undefined>;
}) {
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(
    settings?.currentAccountId ?? accounts[0]?.id ?? null,
  );
  const [captureName, setCaptureName] = useState("");
  const [capturing, setCapturing] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const filtered = useMemo(
    () =>
      accounts.filter((a) =>
        (a.name + (a.email ?? "")).toLowerCase().includes(query.toLowerCase()),
      ),
    [accounts, query],
  );

  const selected =
    filtered.find((a) => a.id === selectedId) ||
    filtered[0] ||
    accounts.find((a) => a.id === selectedId) ||
    null;

  const onCapture = async () => {
    const name = captureName.trim() || `账号 ${accounts.length + 1}`;
    setCapturing(true);
    try {
      const res = await api.captureCurrentAccount(name);
      if (!res.ok || !res.data) {
        notify(res.error ?? "捕获失败（请先 grok login）", "error");
        return;
      }
      notify(`已捕获：${res.data.name}`);
      setCaptureName("");
      setSelectedId(res.data.id);
      await onRefresh();
    } finally {
      setCapturing(false);
    }
  };

  const onEnable = async (id: string) => {
    setBusy(`enable-${id}`);
    try {
      await withSwitching(
        async () => {
          const res = await api.enableAccount(id);
          if (!res.ok) {
            notify(res.error ?? "启用失败", "error");
            return;
          }
          notify("已切换到官方账号");
          await onRefresh();
        },
        {
          title: "切换官方账号",
          detail: "备份会话并写入 auth.json …",
        },
      );
    } finally {
      setBusy(null);
    }
  };

  const onDelete = async (id: string) => {
    if (settings?.currentAccountId === id && settings.currentMode === "official") {
      notify("请先切换到其他模式，再删除当前账号", "error");
      return;
    }
    if (!window.confirm("确定删除该账号快照？")) return;
    setBusy(`del-${id}`);
    try {
      const res = await api.deleteAccount(id);
      if (!res.ok) {
        notify(res.error ?? "删除失败", "error");
        return;
      }
      notify("已删除");
      if (selectedId === id) setSelectedId(null);
      await onRefresh();
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="page-wrap">
      <div className="capture-bar">
        <div className="capture-copy">
          <UserRound size={18} />
          <div>
            <b>捕获当前 Grok 登录态</b>
            <span>先在终端执行 grok login，再保存为本机账号快照。</span>
          </div>
        </div>
        <div className="capture-actions">
          <input
            value={captureName}
            onChange={(e) => setCaptureName(e.target.value)}
            placeholder="备注名（可选）"
          />
          <button
            type="button"
            className="primary-btn"
            onClick={() => void onCapture()}
            disabled={capturing}
          >
            {capturing ? (
              <LoaderCircle className="spin" size={15} />
            ) : (
              <Plus size={15} />
            )}
            捕获
          </button>
        </div>
      </div>

      <div className="toolbar">
        <div className="search">
          <Search size={15} />
          <input
            placeholder="搜索账号"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>

      {accounts.length === 0 ? (
        <div className="empty-state">
          <b>还没有官方账号</b>
          <p>捕获当前登录会话后，可以在多个官方账号间切换。</p>
        </div>
      ) : (
        <div className="provider-list">
          {filtered.map((a) => {
            const active =
              settings?.currentMode === "official" &&
              a.id === settings.currentAccountId;
            const enabling = busy === `enable-${a.id}`;
            return (
              <div
                key={a.id}
                className={`provider-card ${active ? "is-current" : ""}`}
              >
                <div
                  className="provider-avatar"
                  style={{ background: a.labelColor || "#7c6cff" }}
                >
                  {initials(a.name)}
                </div>
                <div className="provider-body">
                  <div className="provider-title-row">
                    <b>{a.name}</b>
                    {active && (
                      <span className="badge badge-current">
                        <Check size={12} /> 当前
                      </span>
                    )}
                    <span className="badge badge-muted">{a.status}</span>
                  </div>
                  <div className="provider-meta">
                    <span>{a.email ?? "官方会话"}</span>
                    <span>最近使用 {formatTs(a.lastUsedAt)}</span>
                    <span title="池优先级">P{a.priority ?? 0}</span>
                    <span title="池权重">w{a.weight ?? 100}</span>
                    {a.poolEnabled === false && (
                      <span className="badge badge-muted">池外</span>
                    )}
                  </div>
                </div>
                <div className="provider-actions">
                  {!active && (
                    <button
                      type="button"
                      className="primary-btn"
                      disabled={!!busy}
                      onClick={() => void onEnable(a.id)}
                    >
                      {enabling ? (
                        <LoaderCircle size={14} className="spin" />
                      ) : (
                        <Check size={14} />
                      )}
                      启用
                    </button>
                  )}
                  <button
                    type="button"
                    className="ghost-btn"
                    disabled={!!busy}
                    title="切换是否参与账号池"
                    onClick={() => {
                      void (async () => {
                        setBusy(`pool-${a.id}`);
                        try {
                          const res = await api.upsertAccount({
                            ...a,
                            poolEnabled: a.poolEnabled === false,
                          });
                          if (!res.ok || !res.data) {
                            notify(res.error ?? "更新失败", "error");
                            return;
                          }
                          notify(
                            res.data.poolEnabled === false
                              ? `${a.name} 已移出账号池`
                              : `${a.name} 已加入账号池`,
                          );
                          await onRefresh();
                        } finally {
                          setBusy(null);
                        }
                      })();
                    }}
                  >
                    {a.poolEnabled === false ? "入池" : "出池"}
                  </button>
                  <button
                    type="button"
                    className="icon-btn"
                    disabled={!!busy}
                    onClick={() => void onDelete(a.id)}
                    title="删除"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            );
          })}
          {selected && (
            <div className="notice">
              <UserRound size={16} />
              <div>
                <b>选中：{selected.name}</b>
                <p>
                  凭证只保存在本机 ~/.grok-switch/accounts，不会上传。
                </p>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
