import { useCallback, useEffect, useRef, useState } from "react";
import { Copy, Plus, RefreshCw, Terminal } from "lucide-react";
import * as api from "./lib/api";
import type { Account, Activity, CliStatus, Provider, Settings, Theme } from "./lib/types";
import { modelFlag, modeLabel } from "./lib/providerUtils";
import { Sidebar, type PageId } from "./components/Sidebar";
import { Toast, type ToastTone } from "./components/Toast";
import { SwitchOverlay } from "./components/SwitchOverlay";
import { OverviewPage } from "./pages/OverviewPage";
import { ProvidersPage } from "./pages/ProvidersPage";
import { AccountsPage } from "./pages/AccountsPage";
import { ImportPage } from "./pages/ImportPage";
import { SkillsPage } from "./pages/SkillsPage";
import { McpPage } from "./pages/McpPage";
import { ActivityPage } from "./pages/ActivityPage";
import { SettingsPage } from "./pages/SettingsPage";

const PAGE_META: Record<PageId, { title: string; sub: string }> = {
  overview: { title: "总览", sub: "Grok CLI 中转与官方账号切换" },
  providers: { title: "供应商", sub: "管理 base_url / key / 模型，一键启用" },
  accounts: { title: "官方账号", sub: "捕获与切换 grok login 会话" },
  import: { title: "从 CC Switch 导入", sub: "读取本机 ~/.cc-switch 配置" },
  skills: { title: "Skills", sub: "管理 ~/.grok/skills 与提示词包" },
  mcp: { title: "MCP", sub: "管理 config.toml 中的 mcp_servers" },
  activity: { title: "日志与备份", sub: "操作记录 + 一键恢复备份" },
  settings: { title: "设置", sub: "路径、备份与自动化" },
};

function resolveTheme(theme: Theme | undefined): "light" | "dark" {
  if (theme === "light") return "light";
  if (theme === "dark") return "dark";
  // system
  if (typeof window !== "undefined" && window.matchMedia) {
    return window.matchMedia("(prefers-color-scheme: light)").matches
      ? "light"
      : "dark";
  }
  return "dark";
}

function applyDocumentTheme(theme: Theme | undefined) {
  const resolved = resolveTheme(theme);
  document.documentElement.setAttribute("data-theme", resolved);
  document.documentElement.style.colorScheme = resolved;
  try {
    if (theme) localStorage.setItem("gs-theme", theme);
  } catch {
    /* ignore */
  }
}

export default function App() {
  const [page, setPage] = useState<PageId>("providers");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [activity, setActivity] = useState<Activity[]>([]);
  const [cli, setCli] = useState<CliStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [toast, setToast] = useState({ message: "", tone: "ok" as ToastTone });
  const [switching, setSwitching] = useState(false);
  const [switchCopy, setSwitchCopy] = useState({
    title: "正在切换…",
    detail: "备份配置并写入 Grok CLI…",
  });
  const toastTimer = useRef<number | null>(null);

  const notify = useCallback((message: string, tone: ToastTone = "ok") => {
    setToast({ message, tone });
    if (toastTimer.current != null) {
      window.clearTimeout(toastTimer.current);
    }
    // Longer messages (model id hints) need more time to read.
    const ms = message.length > 36 ? 5200 : 3200;
    toastTimer.current = window.setTimeout(() => {
      setToast({ message: "", tone: "ok" });
      toastTimer.current = null;
    }, ms);
  }, []);

  const refresh = useCallback(async () => {
    const [s, p, a, act, c] = await Promise.all([
      api.getSettings(),
      api.listProviders(),
      api.listAccounts(),
      api.listActivity(50),
      api.getCliStatus(),
    ]);

    if (s.ok && s.data) {
      setSettings(s.data);
      applyDocumentTheme(s.data.theme);
    } else if (!s.ok) notify(s.error ?? "加载设置失败", "error");

    if (p.ok && p.data) setProviders(p.data);
    if (a.ok && a.data) setAccounts(a.data);
    if (act.ok && act.data) setActivity(act.data);
    if (c.ok && c.data) setCli(c.data);
  }, [notify]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setLoading(true);
      await refresh();
      if (!cancelled) setLoading(false);
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  // Apply theme whenever settings.theme changes (including system preference).
  useEffect(() => {
    applyDocumentTheme(settings?.theme);
    if (settings?.theme !== "system" || typeof window === "undefined") return;
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const onChange = () => applyDocumentTheme("system");
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [settings?.theme]);

  const withSwitching = useCallback(
    async <T,>(
      work: () => Promise<T>,
      labels?: { title?: string; detail?: string },
    ): Promise<T | undefined> => {
      setSwitchCopy({
        title: labels?.title ?? "正在切换…",
        detail: labels?.detail ?? "备份配置并写入 Grok CLI…",
      });
      setSwitching(true);
      try {
        return await work();
      } catch (e) {
        notify(e instanceof Error ? e.message : String(e), "error");
        return undefined;
      } finally {
        setSwitching(false);
      }
    },
    [notify],
  );

  const refreshCli = async () => {
    const res = await api.getCliStatus();
    if (res.ok && res.data) {
      setCli(res.data);
      notify(
        res.data.found
          ? `CLI ${res.data.version ? `v${res.data.version}` : "已检测"}`
          : "未找到 Grok CLI",
        res.data.found ? "ok" : "error",
      );
    } else {
      notify(res.error ?? "CLI 状态失败", "error");
    }
  };

  const meta = PAGE_META[page];
  const currentProvider =
    settings?.currentMode === "provider"
      ? providers.find((p) => p.id === settings.currentProviderId)
      : undefined;
  const currentAccount =
    settings?.currentMode === "official"
      ? accounts.find((a) => a.id === settings.currentAccountId)
      : undefined;
  const hasCurrent = Boolean(currentProvider || currentAccount);
  const currentLabel = modeLabel(
    settings?.currentMode,
    currentProvider?.name,
    currentAccount?.name,
  );
  const currentModelId = currentProvider
    ? modelFlag(currentProvider)
    : settings?.currentMode === "official"
      ? settings.officialDefaultModel
      : null;

  const copyCurrentModel = async () => {
    if (!currentModelId) return;
    try {
      await navigator.clipboard.writeText(currentModelId);
      notify(`已复制 ${currentModelId}`);
    } catch {
      notify(`模型：${currentModelId}`);
    }
  };

  const openGrokTerminal = async () => {
    // Model is whitelist-validated in Rust; no frontend shell execution.
    const res = await api.openGrokTerminal(currentModelId);
    if (res.ok && res.data) {
      notify(`已打开终端：${res.data}`);
      return;
    }
    const fallback = currentModelId
      ? `grok -m ${currentModelId}`
      : "grok";
    notify(
      `请手动运行：${fallback}${res.error ? `（${res.error}）` : ""}`,
      "error",
    );
  };

  return (
    <div className="app-shell">
      <Sidebar
        page={page}
        onNavigate={setPage}
        settings={settings}
        providers={providers}
        accounts={accounts}
        cli={cli}
      />

      <main className="main">
        <div className="topbar">
          <div className="topbar-left">
            <div>
              <div className="topbar-title">{meta.title}</div>
              <div className="topbar-sub">{meta.sub}</div>
            </div>
            <div
              className={`current-chip ${hasCurrent ? "" : "is-idle"}`}
              title={hasCurrent ? "当前启用" : "尚未启用任何供应商/账号"}
            >
              <span
                className={`status-dot ${hasCurrent ? "" : "status-dot-warn"}`}
              />
              <span>{currentLabel}</span>
            </div>
            {currentModelId && (
              <button
                type="button"
                className="model-chip"
                title="点击复制模型 id"
                onClick={() => void copyCurrentModel()}
              >
                <span className="mono">{currentModelId}</span>
                <Copy size={12} />
              </button>
            )}
          </div>
          <div className="header-actions">
            {hasCurrent && (
              <button
                type="button"
                className="ghost-btn"
                onClick={() => void openGrokTerminal()}
                title="在终端打开 grok"
              >
                <Terminal size={14} /> 打开 Grok
              </button>
            )}
            <button
              type="button"
              className="ghost-btn"
              onClick={() => void refreshCli()}
            >
              <RefreshCw size={14} /> 检测 CLI
            </button>
            {page === "providers" && (
              <button
                type="button"
                className="primary-btn"
                onClick={() => {
                  window.dispatchEvent(new CustomEvent("gs-open-provider-form"));
                }}
              >
                <Plus size={15} /> 添加
              </button>
            )}
            {page === "import" && (
              <button
                type="button"
                className="primary-btn"
                onClick={() => setPage("providers")}
              >
                返回供应商
              </button>
            )}
          </div>
        </div>

        <div className="content">
          {loading ? (
            <div className="empty-state">
              <b>加载中…</b>
              <p>正在读取本机设置、供应商与 CLI 状态。</p>
            </div>
          ) : (
            <>
              {page === "overview" && (
                <OverviewPage
                  settings={settings}
                  providers={providers}
                  accounts={accounts}
                  cli={cli}
                  activity={activity}
                  onNavigate={setPage}
                  onRefreshCli={() => void refreshCli()}
                />
              )}
              {page === "providers" && (
                <ProvidersPage
                  providers={providers}
                  settings={settings}
                  onRefresh={refresh}
                  notify={notify}
                  withSwitching={withSwitching}
                />
              )}
              {page === "accounts" && (
                <AccountsPage
                  accounts={accounts}
                  settings={settings}
                  onRefresh={refresh}
                  notify={notify}
                  withSwitching={withSwitching}
                />
              )}
              {page === "import" && (
                <ImportPage
                  onImported={refresh}
                  notify={notify}
                  withSwitching={withSwitching}
                />
              )}
              {page === "skills" && <SkillsPage notify={notify} />}
              {page === "mcp" && <McpPage notify={notify} />}
              {page === "activity" && (
                <ActivityPage
                  activity={activity}
                  onRestored={refresh}
                  notify={notify}
                />
              )}
              {page === "settings" && (
                <SettingsPage
                  settings={settings}
                  onSaved={refresh}
                  notify={notify}
                />
              )}
            </>
          )}
        </div>
      </main>

      <Toast message={toast.message} tone={toast.tone} />
      <SwitchOverlay
        open={switching}
        title={switchCopy.title}
        detail={switchCopy.detail}
      />
    </div>
  );
}
