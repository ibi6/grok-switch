import { useCallback, useEffect, useState } from "react";
import {
  AlertTriangle,
  Archive,
  Check,
  Copy,
  Download,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  Terminal,
  UserRound,
} from "lucide-react";
import type { Activity, BackupInfo, RequestLog, TokenStats } from "../lib/types";
import * as api from "../lib/api";

function formatTs(ts: number): string {
  return new Date(ts * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function labelFor(type: Activity["type"]): string {
  switch (type) {
    case "switch_provider":
      return "切换供应商";
    case "switch_account":
      return "切换账号";
    case "health":
      return "连通性检测";
    case "backup":
      return "备份";
    case "restore":
      return "恢复备份";
    case "import":
      return "导入";
    case "capture_account":
      return "捕获账号";
    case "skill":
      return "Skill";
    case "mcp":
      return "MCP";
    case "proxy":
      return "代理";
    case "failover":
      return "故障切换";
    case "error":
      return "错误";
    default:
      return type;
  }
}

function iconFor(type: Activity["type"]) {
  switch (type) {
    case "switch_provider":
    case "switch_account":
      return <Check size={13} />;
    case "health":
      return <RefreshCw size={13} />;
    case "backup":
    case "restore":
      return <Copy size={13} />;
    case "import":
      return <Download size={13} />;
    case "capture_account":
      return <UserRound size={13} />;
    case "error":
      return <AlertTriangle size={13} />;
    default:
      return <Terminal size={13} />;
  }
}

export function ActivityPage({
  activity,
  onRestored,
  notify,
}: {
  activity: Activity[];
  onRestored: () => Promise<void>;
  notify: (msg: string, tone?: "ok" | "error") => void;
}) {
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [loadingBackups, setLoadingBackups] = useState(true);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [reqLogs, setReqLogs] = useState<RequestLog[]>([]);
  const [tokenStats, setTokenStats] = useState<TokenStats | null>(null);

  const loadBackups = useCallback(async () => {
    setLoadingBackups(true);
    try {
      const res = await api.listBackups();
      if (res.ok && res.data) setBackups(res.data);
      else setBackups([]);
    } finally {
      setLoadingBackups(false);
    }
  }, []);

  const loadProxyTelemetry = useCallback(async () => {
    const [logs, stats] = await Promise.all([
      api.listRequestLogs(50),
      api.getTokenStats(),
    ]);
    if (logs.ok && logs.data) setReqLogs(logs.data);
    if (stats.ok && stats.data) setTokenStats(stats.data);
  }, []);

  useEffect(() => {
    void loadBackups();
    void loadProxyTelemetry();
  }, [loadBackups, loadProxyTelemetry, activity.length]);

  const onRestore = async (id: string) => {
    if (
      !window.confirm(
        `确定恢复备份 ${id}？\n将覆盖当前 ~/.grok 的 config.toml / auth.json，并清空「当前启用」状态。`,
      )
    ) {
      return;
    }
    setRestoring(id);
    try {
      const res = await api.restoreBackup(id);
      if (!res.ok) {
        notify(res.error ?? "恢复失败", "error");
        return;
      }
      notify(`已恢复备份 ${id}`);
      await onRestored();
      await loadBackups();
    } finally {
      setRestoring(null);
    }
  };

  return (
    <div className="page-wrap">
      <div className="section-head no-margin">
        <div>
          <h2>备份恢复</h2>
          <p>切换前自动备份，可一键回滚 config / auth。</p>
        </div>
        <button
          type="button"
          className="ghost-btn"
          onClick={() => void loadBackups()}
          disabled={loadingBackups}
        >
          {loadingBackups ? (
            <LoaderCircle className="spin" size={15} />
          ) : (
            <RefreshCw size={15} />
          )}
          刷新
        </button>
      </div>

      {loadingBackups ? (
        <div className="empty-state" style={{ marginBottom: 20 }}>
          <LoaderCircle className="spin" size={22} />
          <b>加载备份列表…</b>
        </div>
      ) : backups.length === 0 ? (
        <div className="empty-state" style={{ marginBottom: 20 }}>
          <Archive size={28} style={{ opacity: 0.5 }} />
          <b>暂无备份</b>
          <p>启用供应商或账号且开启自动备份后，会出现在这里。</p>
        </div>
      ) : (
        <div className="provider-list" style={{ marginBottom: 24 }}>
          {backups.map((b) => (
            <div key={b.id} className="provider-card backup-card">
              <div
                className="provider-avatar"
                style={{
                  background: "linear-gradient(135deg,#7c6cff,#38bdf8)",
                }}
              >
                <Archive size={18} />
              </div>
              <div className="provider-body">
                <div className="provider-title-row">
                  <b className="mono">{b.id}</b>
                  {b.reason && (
                    <span className="badge badge-muted">{b.reason}</span>
                  )}
                </div>
                <div className="provider-meta">
                  <span>
                    {b.createdAt
                      ? formatTs(b.createdAt)
                      : b.meta?.createdAt
                        ? formatTs(b.meta.createdAt)
                        : "—"}
                  </span>
                  {b.meta?.extra?.mode && <span>模式 {b.meta.extra.mode}</span>}
                  {b.meta?.extra?.providerId && (
                    <span className="mono">
                      provider {b.meta.extra.providerId.slice(0, 8)}…
                    </span>
                  )}
                </div>
              </div>
              <div className="provider-actions">
                <button
                  type="button"
                  className="ghost-btn"
                  disabled={!!restoring}
                  onClick={() => void onRestore(b.id)}
                >
                  {restoring === b.id ? (
                    <LoaderCircle className="spin" size={14} />
                  ) : (
                    <RotateCcw size={14} />
                  )}
                  恢复
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="section-head">
        <div>
          <h2>请求与 Token</h2>
          <p>本地代理产生的上游请求日志（SQLite）。</p>
        </div>
        <button
          type="button"
          className="ghost-btn"
          onClick={() => void loadProxyTelemetry()}
        >
          <RefreshCw size={15} /> 刷新
        </button>
      </div>

      {tokenStats && (
        <div className="stats-row" style={{ marginBottom: 16 }}>
          <div className="stat-card">
            <small>请求数</small>
            <b>{tokenStats.requests}</b>
            <span>
              成功 {tokenStats.okCount} · 失败 {tokenStats.failCount}
            </span>
          </div>
          <div className="stat-card">
            <small>Prompt tokens</small>
            <b>{tokenStats.promptTokens}</b>
            <span>输入侧累计</span>
          </div>
          <div className="stat-card">
            <small>Completion tokens</small>
            <b>{tokenStats.completionTokens}</b>
            <span>输出侧累计</span>
          </div>
        </div>
      )}

      {reqLogs.length === 0 ? (
        <div className="empty-state" style={{ marginBottom: 20 }}>
          <b>暂无代理请求</b>
          <p>在设置中启动本地代理，并将 Grok base_url 指向它后会出现日志。</p>
        </div>
      ) : (
        <div className="activity-list full" style={{ marginBottom: 24 }}>
          {reqLogs.map((l) => (
            <div className="activity-row" key={l.id}>
              <div className={l.ok ? "activity-icon green" : "activity-icon"}>
                {l.ok ? <Check size={13} /> : <AlertTriangle size={13} />}
              </div>
              <div>
                <b>
                  {l.method} {l.path} · {l.status || "ERR"}
                </b>
                <span>
                  {(l.model ?? "—") +
                    ` · ${l.latencyMs}ms · in ${l.promptTokens} / out ${l.completionTokens}`}
                  {l.detail ? ` · ${l.detail}` : ""}
                </span>
              </div>
              <time>{formatTs(l.ts)}</time>
            </div>
          ))}
        </div>
      )}

      <div className="section-head">
        <div>
          <h2>操作日志</h2>
          <p>本机切换、测通、备份记录（不含完整密钥）。</p>
        </div>
      </div>

      {activity.length === 0 ? (
        <div className="empty-state">
          <b>暂无记录</b>
          <p>完成切换或测通后，会显示在这里。</p>
        </div>
      ) : (
        <div className="activity-list full">
          {activity.map((a, i) => {
            const green =
              a.type.startsWith("switch") ||
              a.type === "health" ||
              a.type === "import";
            return (
              <div className="activity-row" key={`${a.ts}-${a.type}-${i}`}>
                <div className={green ? "activity-icon green" : "activity-icon"}>
                  {iconFor(a.type)}
                </div>
                <div>
                  <b>{labelFor(a.type)}</b>
                  <span>{a.message}</span>
                </div>
                <time>{formatTs(a.ts)}</time>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
