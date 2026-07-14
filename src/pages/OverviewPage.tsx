import {
  ArrowRight,
  Check,
  Copy,
  RefreshCw,
  Server,
  Terminal,
  UserRound,
} from "lucide-react";
import type { Account, Activity, CliStatus, Provider, Settings } from "../lib/types";
import type { PageId } from "../components/Sidebar";

function formatTs(ts: number): string {
  return new Date(ts * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function activityIcon(type: Activity["type"]) {
  switch (type) {
    case "switch_provider":
    case "switch_account":
      return <Check size={13} />;
    case "health":
      return <RefreshCw size={13} />;
    case "backup":
    case "restore":
      return <Copy size={13} />;
    default:
      return <Terminal size={13} />;
  }
}

export function OverviewPage({
  settings,
  providers,
  accounts,
  cli,
  activity,
  onNavigate,
  onRefreshCli,
}: {
  settings: Settings | null;
  providers: Provider[];
  accounts: Account[];
  cli: CliStatus | null;
  activity: Activity[];
  onNavigate: (p: PageId) => void;
  onRefreshCli: () => void;
}) {
  const mode = settings?.currentMode ?? "none";
  const provider = providers.find((p) => p.id === settings?.currentProviderId);
  const account = accounts.find((a) => a.id === settings?.currentAccountId);

  let modeTitle = "未启用";
  let modeSub = "请启用供应商或官方账号";
  let modeMeta = "—";

  if (mode === "provider" && provider) {
    modeTitle = provider.name;
    modeSub = provider.baseUrl;
    modeMeta =
      provider.models.find((m) => m.id === provider.defaultModelEntryId)
        ?.model ?? "—";
  } else if (mode === "official" && account) {
    modeTitle = account.name;
    modeSub = account.email ?? "官方 Grok 账号";
    modeMeta = settings?.officialDefaultModel ?? "—";
  } else if (mode === "provider") {
    modeTitle = "供应商模式";
    modeSub = "未选中供应商";
  } else if (mode === "official") {
    modeTitle = "官方模式";
    modeSub = "未选中账号";
  }

  const recent = activity.slice(0, 6);

  return (
    <div className="page-wrap">
      <div className="stats-row">
        <div className="stat-card">
          <small>当前模式</small>
          <b>{mode === "provider" ? "中转供应商" : mode === "official" ? "官方账号" : "未启用"}</b>
          <span>{modeTitle}</span>
        </div>
        <div className="stat-card">
          <small>默认模型</small>
          <b>{modeMeta}</b>
          <span>{modeSub}</span>
        </div>
        <div className="stat-card">
          <small>Grok CLI</small>
          <b>{cli?.found ? cli.version ?? "已安装" : "未找到"}</b>
          <span>
            {cli?.configOk ? "配置可读" : "配置异常"} ·{" "}
            {cli?.authPresent ? "有 auth" : "无 auth"}
          </span>
        </div>
      </div>

      <div className="provider-list" style={{ marginBottom: 16 }}>
        <button
          type="button"
          className="provider-card"
          onClick={() => onNavigate("providers")}
        >
          <div className="provider-avatar" style={{ background: "#4c8dff" }}>
            <Server size={18} />
          </div>
          <div className="provider-body">
            <div className="provider-title-row">
              <b>供应商管理</b>
              <span className="badge badge-muted">{providers.length} 个</span>
            </div>
            <div className="provider-meta">
              <span>添加 / 启用中转站，写入 config.toml</span>
            </div>
          </div>
          <div className="provider-actions">
            <ArrowRight size={16} color="#6b7a8c" />
          </div>
        </button>

        <button
          type="button"
          className="provider-card"
          onClick={() => onNavigate("accounts")}
        >
          <div className="provider-avatar" style={{ background: "#22c55e" }}>
            <UserRound size={18} />
          </div>
          <div className="provider-body">
            <div className="provider-title-row">
              <b>官方账号</b>
              <span className="badge badge-muted">{accounts.length} 个</span>
            </div>
            <div className="provider-meta">
              <span>捕获 grok login 会话并切换</span>
            </div>
          </div>
          <div className="provider-actions">
            <ArrowRight size={16} color="#6b7a8c" />
          </div>
        </button>

        <button
          type="button"
          className="provider-card"
          onClick={() => onNavigate("import")}
        >
          <div className="provider-avatar" style={{ background: "#a855f7" }}>
            <Copy size={18} />
          </div>
          <div className="provider-body">
            <div className="provider-title-row">
              <b>从 CC Switch 导入</b>
            </div>
            <div className="provider-meta">
              <span>读取 ~/.cc-switch 中的 Claude 供应商</span>
            </div>
          </div>
          <div className="provider-actions">
            <ArrowRight size={16} color="#6b7a8c" />
          </div>
        </button>
      </div>

      <div className="section-head">
        <div>
          <h2>最近操作</h2>
          <p>本机活动日志</p>
        </div>
        <button type="button" className="text-btn" onClick={onRefreshCli}>
          <RefreshCw size={13} /> 刷新 CLI
        </button>
      </div>

      {recent.length === 0 ? (
        <div className="empty-state">
          <b>暂无日志</b>
          <p>切换供应商或测通后会显示在这里。</p>
        </div>
      ) : (
        <div className="activity-list">
          {recent.map((a, i) => (
            <div className="activity-row" key={`${a.ts}-${i}`}>
              <div
                className={`activity-icon ${
                  a.type.startsWith("switch") ? "green" : ""
                }`}
              >
                {activityIcon(a.type)}
              </div>
              <div>
                <b>{a.message}</b>
                <span>{a.type}</span>
              </div>
              <time>{formatTs(a.ts)}</time>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
