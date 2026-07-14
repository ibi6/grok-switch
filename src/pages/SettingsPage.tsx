import { useEffect, useState } from "react";
import {
  Check,
  Copy,
  HeartPulse,
  LoaderCircle,
  Palette,
  ShieldCheck,
  Terminal,
} from "lucide-react";
import type { Settings, Theme } from "../lib/types";
import * as api from "../lib/api";

export function SettingsPage({
  settings,
  onSaved,
  notify,
}: {
  settings: Settings | null;
  onSaved: () => Promise<void>;
  notify: (msg: string, tone?: "ok" | "error") => void;
}) {
  const [draft, setDraft] = useState<Settings | null>(settings);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  if (!draft) {
    return (
      <div className="page-wrap">
        <div className="empty-state">
          <LoaderCircle className="spin" size={22} />
          <b>加载设置…</b>
        </div>
      </div>
    );
  }

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setDraft((d) => (d ? { ...d, [key]: value } : d));
    // Live-preview theme before save
    if (key === "theme" && typeof document !== "undefined") {
      const t = value as Theme;
      const resolved =
        t === "light"
          ? "light"
          : t === "dark"
            ? "dark"
            : window.matchMedia?.("(prefers-color-scheme: light)").matches
              ? "light"
              : "dark";
      document.documentElement.setAttribute("data-theme", resolved);
      document.documentElement.style.colorScheme = resolved;
    }
  };

  const save = async () => {
    setSaving(true);
    try {
      const res = await api.updateSettings(draft);
      if (!res.ok) {
        notify(res.error ?? "保存失败", "error");
        return;
      }
      notify("设置已保存");
      await onSaved();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="page-wrap settings">
      <div className="section-head no-margin">
        <div>
          <h2>设置</h2>
          <p>路径、备份与桌面行为。</p>
        </div>
        <button
          type="button"
          className="primary-btn"
          onClick={() => void save()}
          disabled={saving}
        >
          {saving ? <LoaderCircle className="spin" size={15} /> : <Check size={15} />}
          保存
        </button>
      </div>

      <div className="settings-card">
        <div className="setting-row">
          <div className="setting-icon">
            <Terminal size={17} />
          </div>
          <div className="setting-copy">
            <b>Grok 目录</b>
            <span>Grok CLI 的配置与会话目录。</span>
          </div>
          <input
            className="setting-input mono"
            value={draft.grokHome}
            onChange={(e) => set("grokHome", e.target.value)}
          />
        </div>

        <div className="setting-row">
          <div className="setting-icon">
            <Terminal size={17} />
          </div>
          <div className="setting-copy">
            <b>Grok CLI 可执行文件</b>
            <span>用于版本检测与启动。</span>
          </div>
          <input
            className="setting-input mono"
            value={draft.grokExecutable}
            onChange={(e) => set("grokExecutable", e.target.value)}
          />
        </div>

        <div className="setting-row">
          <div className="setting-icon">
            <HeartPulse size={17} />
          </div>
          <div className="setting-copy">
            <b>官方默认模型</b>
            <span>启用官方账号时写入的模型名。</span>
          </div>
          <input
            className="setting-input mono"
            value={draft.officialDefaultModel}
            onChange={(e) => set("officialDefaultModel", e.target.value)}
          />
        </div>

        <div className="setting-row">
          <div className="setting-icon">
            <Copy size={17} />
          </div>
          <div className="setting-copy">
            <b>自动备份</b>
            <span>每次切换前备份 config / auth。</span>
          </div>
          <button
            type="button"
            className={draft.autoBackup ? "toggle on" : "toggle"}
            onClick={() => set("autoBackup", !draft.autoBackup)}
            aria-pressed={draft.autoBackup}
          >
            <span />
          </button>
        </div>

        <div className="setting-row">
          <div className="setting-icon">
            <HeartPulse size={17} />
          </div>
          <div className="setting-copy">
            <b>启用前测通</b>
            <span>切换供应商前先探测接口连通性。</span>
          </div>
          <button
            type="button"
            className={draft.autoHealthCheck ? "toggle on" : "toggle"}
            onClick={() => set("autoHealthCheck", !draft.autoHealthCheck)}
            aria-pressed={draft.autoHealthCheck}
          >
            <span />
          </button>
        </div>

        <div className="setting-row">
          <div className="setting-icon">
            <Palette size={17} />
          </div>
          <div className="setting-copy">
            <b>主题</b>
            <span>界面外观偏好（当前深色已优化）。</span>
          </div>
          <select
            value={draft.theme}
            onChange={(e) => set("theme", e.target.value as Theme)}
          >
            <option value="dark">深色</option>
            <option value="light">浅色</option>
            <option value="system">跟随系统</option>
          </select>
        </div>

        <div className="setting-row">
          <div className="setting-icon">
            <ShieldCheck size={17} />
          </div>
          <div className="setting-copy">
            <b>系统托盘</b>
            <span>最小化后保留托盘图标。</span>
          </div>
          <button
            type="button"
            className={draft.trayEnabled ? "toggle on" : "toggle"}
            onClick={() => set("trayEnabled", !draft.trayEnabled)}
            aria-pressed={draft.trayEnabled}
          >
            <span />
          </button>
        </div>

        <div className="setting-row">
          <div className="setting-icon">
            <ShieldCheck size={17} />
          </div>
          <div className="setting-copy">
            <b>开机启动</b>
            <span>即将支持（当前开关仅保存偏好，不会写入系统启动项）。</span>
          </div>
          <button
            type="button"
            className={draft.launchOnStartup ? "toggle on" : "toggle"}
            onClick={() => set("launchOnStartup", !draft.launchOnStartup)}
            aria-pressed={draft.launchOnStartup}
            title="即将支持"
          >
            <span />
          </button>
        </div>

        <div className="setting-row">
          <div className="setting-icon">
            <ShieldCheck size={17} />
          </div>
          <div className="setting-copy">
            <b>凭证存储</b>
            <span>密钥仅保存在本机 ~/.grok-switch。</span>
          </div>
          <span className="protected">
            <Check size={13} /> 本地
          </span>
        </div>
      </div>
    </div>
  );
}
