import {
  Activity,
  Cable,
  Download,
  LayoutDashboard,
  Server,
  Settings2,
  Sparkles,
  UserRound,
} from "lucide-react";
import type { Account, CliStatus, Provider, Settings } from "../lib/types";
import grokIcon from "../assets/grok-icon.png";

export type PageId =
  | "overview"
  | "providers"
  | "accounts"
  | "import"
  | "skills"
  | "mcp"
  | "activity"
  | "settings";

const NAV: { id: PageId; label: string; Icon: typeof LayoutDashboard }[] = [
  { id: "overview", label: "总览", Icon: LayoutDashboard },
  { id: "providers", label: "供应商", Icon: Server },
  { id: "accounts", label: "官方账号", Icon: UserRound },
  { id: "import", label: "导入", Icon: Download },
  { id: "skills", label: "Skills", Icon: Sparkles },
  { id: "mcp", label: "MCP", Icon: Cable },
  { id: "activity", label: "日志备份", Icon: Activity },
  { id: "settings", label: "设置", Icon: Settings2 },
];

export function Sidebar({
  page,
  onNavigate,
  settings,
  providers,
  accounts,
  cli,
}: {
  page: PageId;
  onNavigate: (p: PageId) => void;
  settings: Settings | null;
  providers: Provider[];
  accounts: Account[];
  cli: CliStatus | null;
}) {
  const badgeFor = (id: PageId): string | null => {
    if (id === "providers" && providers.length)
      return String(providers.length);
    if (id === "accounts" && accounts.length) return String(accounts.length);
    if (id === "overview" && settings?.proxyEnabled) return "代理";
    return null;
  };

  const modeHint =
    settings?.currentMode === "provider"
      ? "中转"
      : settings?.currentMode === "official"
        ? "官方"
        : "未启用";

  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark">
          <img src={grokIcon} alt="Grok" width={40} height={40} draggable={false} />
        </div>
        <div className="brand-copy">
          <b>Grok Switch</b>
          <span>{modeHint}</span>
        </div>
      </div>

      <nav className="nav-rail">
        {NAV.map(({ id, label, Icon }) => {
          const badge = badgeFor(id);
          return (
            <button
              key={id}
              type="button"
              className={page === id ? "nav-item active" : "nav-item"}
              onClick={() => onNavigate(id)}
              title={label}
            >
              <Icon size={18} />
              <span>{label}</span>
              {badge && <em className="nav-badge">{badge}</em>}
            </button>
          );
        })}
      </nav>

      <div className="sidebar-foot">
        <div className="cli-mini" title={cli?.path ?? "Grok CLI"}>
          <span className={`status-dot ${cli?.found ? "" : "status-dot-warn"}`} />
          <span>Grok CLI</span>
          <b>{cli?.version ? `v${cli.version}` : cli?.found ? "OK" : "—"}</b>
        </div>
      </div>
    </aside>
  );
}
