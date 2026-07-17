import { useEffect, useMemo, useState } from "react";
import {
  ArrowLeft,
  ChevronDown,
  Eye,
  EyeOff,
  Lightbulb,
  LoaderCircle,
  Plus,
  Trash2,
  Zap,
} from "lucide-react";
import type { ApiBackend, ModelEntry, Provider } from "../lib/types";
import * as api from "../lib/api";

type ExtraModelRow = {
  key: string;
  displayName: string;
  model: string;
  largeContext: boolean;
};

export type ProviderFormValues = {
  name: string;
  baseUrl: string;
  apiKey: string;
  apiBackend: ApiBackend;
  defaultModel: string;
  defaultDisplayName: string;
  defaultLargeContext: boolean;
  appendV1: boolean;
  fullUrlMode: boolean;
  websiteUrl: string;
  notes: string;
  fallbackModel: string;
  extras: ExtraModelRow[];
  advancedOpen: boolean;
  priority: number;
  weight: number;
  poolEnabled: boolean;
};

function ensureTrailingSlashFree(url: string): string {
  return url.replace(/\/+$/, "");
}

function slugify(s: string): string {
  const slug = s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40);
  return slug || "model";
}

function applyAppendV1(baseUrl: string, appendV1: boolean): string {
  const clean = ensureTrailingSlashFree(baseUrl.trim());
  if (!appendV1) return clean;
  if (/\/v1$/i.test(clean)) return clean;
  return `${clean}/v1`;
}

function avatarLabel(name: string): string {
  const t = name.trim();
  if (!t) return "新";
  return t.slice(0, 1).toUpperCase();
}

function fromProvider(p?: Provider | null): ProviderFormValues {
  if (!p) {
    return {
      name: "",
      baseUrl: "",
      apiKey: "",
      apiBackend: "chat_completions",
      defaultModel: "grok-4.5",
      defaultDisplayName: "grok-4.5",
      defaultLargeContext: true,
      appendV1: true,
      fullUrlMode: false,
      websiteUrl: "",
      notes: "",
      fallbackModel: "",
      extras: [],
      advancedOpen: true,
      priority: 0,
      weight: 100,
      poolEnabled: true,
    };
  }

  const endsWithV1 = /\/v1$/i.test(p.baseUrl);
  const defaultEntry =
    p.models.find((m) => m.id === p.defaultModelEntryId) ?? p.models[0];
  const extras = p.models
    .filter((m) => m.id !== (defaultEntry?.id ?? p.defaultModelEntryId))
    .map((m, i) => ({
      key: `ex-${m.id}-${i}`,
      displayName: m.name || m.model,
      model: m.model,
      largeContext: (m.contextWindow ?? p.contextWindow) >= 1_000_000,
    }));

  return {
    name: p.name,
    baseUrl: endsWithV1 ? p.baseUrl.replace(/\/v1$/i, "") : p.baseUrl,
    apiKey: p.apiKey,
    apiBackend: p.apiBackend,
    defaultModel: defaultEntry?.model ?? "",
    defaultDisplayName: defaultEntry?.name ?? defaultEntry?.model ?? "",
    defaultLargeContext: (defaultEntry?.contextWindow ?? p.contextWindow) >= 1_000_000,
    appendV1: endsWithV1,
    fullUrlMode: !endsWithV1 && /\/v\d+/i.test(p.baseUrl),
    websiteUrl: p.websiteUrl ?? "",
    notes: p.notes ?? "",
    fallbackModel: "",
    extras,
    advancedOpen: true,
    priority: p.priority ?? 0,
    weight: p.weight ?? 100,
    poolEnabled: p.poolEnabled ?? true,
  };
}

export function ProviderForm({
  open,
  initial,
  onClose,
  onSaved,
  onSavedAndEnable,
  notify,
}: {
  open: boolean;
  initial?: Provider | null;
  onClose: () => void;
  onSaved: (p: Provider) => void | Promise<void>;
  onSavedAndEnable?: (p: Provider) => void | Promise<void>;
  notify: (msg: string, tone?: "ok" | "error") => void;
}) {
  const editing = Boolean(initial);
  const [values, setValues] = useState<ProviderFormValues>(() =>
    fromProvider(initial),
  );
  const [revealKey, setRevealKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [savingEnable, setSavingEnable] = useState(false);
  const [testing, setTesting] = useState(false);

  useEffect(() => {
    if (open) {
      setValues(fromProvider(initial));
      setRevealKey(false);
    }
  }, [open, initial]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  const set = <K extends keyof ProviderFormValues>(
    key: K,
    value: ProviderFormValues[K],
  ) => setValues((v) => ({ ...v, [key]: value }));

  const authHint = useMemo(() => {
    if (values.apiBackend === "messages") {
      return "x-api-key（Anthropic Messages）";
    }
    return "Authorization: Bearer（OpenAI 兼容）";
  }, [values.apiBackend]);

  const resolvedBase = useMemo(() => {
    if (values.fullUrlMode) return ensureTrailingSlashFree(values.baseUrl.trim());
    return applyAppendV1(values.baseUrl, values.appendV1);
  }, [values.baseUrl, values.appendV1, values.fullUrlMode]);

  if (!open) return null;

  const isSafeModel = (s: string) =>
    /^[A-Za-z0-9._/:+-]{1,128}$/.test(s.trim());

  const buildProvider = (): Provider | null => {
    const now = Math.floor(Date.now() / 1000);
    const name = values.name.trim();
    const defaultModel = values.defaultModel.trim();
    if (!isSafeModel(defaultModel)) {
      notify(
        "实际请求模型含非法字符（仅允许字母数字与 - _ . / : +）",
        "error",
      );
      return null;
    }
    const display = values.defaultDisplayName.trim() || defaultModel;
    const modelId =
      initial?.defaultModelEntryId ||
      `${slugify(name)}-${slugify(defaultModel)}`;
    if (!isSafeModel(modelId)) {
      notify("模型 id 无效，请调整供应商名称或模型名", "error");
      return null;
    }
    const defaultCtx = values.defaultLargeContext ? 1_000_000 : 200_000;

    const models: ModelEntry[] = [
      {
        id: modelId,
        model: defaultModel,
        name: display,
        contextWindow: defaultCtx,
      },
    ];

    for (const row of values.extras) {
      const m = row.model.trim();
      if (!m) continue;
      if (!isSafeModel(m)) {
        notify(`备用模型「${m}」含非法字符`, "error");
        return null;
      }
      const id = `${slugify(name)}-${slugify(m)}`;
      if (models.some((x) => x.id === id || x.model === m)) continue;
      models.push({
        id,
        model: m,
        name: row.displayName.trim() || m,
        contextWindow: row.largeContext ? 1_000_000 : 200_000,
      });
    }

    // Preserve unknown existing models not represented in form rows
    if (initial?.models?.length) {
      for (const m of initial.models) {
        if (!models.some((x) => x.id === m.id || x.model === m.model)) {
          models.push(m);
        }
      }
    }

    return {
      id: initial?.id ?? crypto.randomUUID(),
      name,
      baseUrl: resolvedBase,
      apiKey: values.apiKey.trim(),
      apiBackend: values.apiBackend,
      defaultModelEntryId: modelId,
      models,
      contextWindow: defaultCtx,
      websiteUrl: values.websiteUrl.trim() || undefined,
      notes: values.notes.trim() || undefined,
      source: initial?.source ?? "manual",
      createdAt: initial?.createdAt ?? now,
      updatedAt: now,
      priority: values.priority,
      weight: Math.max(1, values.weight || 100),
      poolEnabled: values.poolEnabled,
      cooldownUntil: initial?.cooldownUntil,
    };
  };

  const onTest = async () => {
    if (!values.baseUrl.trim() || !values.apiKey.trim()) {
      notify("请先填写请求地址和 API Key", "error");
      return;
    }
    setTesting(true);
    try {
      const res = await api.testProviderDraft({
        baseUrl: resolvedBase,
        apiKey: values.apiKey.trim(),
        apiBackend: values.apiBackend,
        model: values.defaultModel.trim() || "grok-4.5",
      });
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
    } finally {
      setTesting(false);
    }
  };

  const persist = async (andEnable: boolean) => {
    if (!values.name.trim() || !values.baseUrl.trim() || !values.apiKey.trim()) {
      notify("供应商名称、请求地址和 API Key 为必填", "error");
      return;
    }
    if (!values.defaultModel.trim()) {
      notify("请填写默认实际请求模型", "error");
      return;
    }
    if (andEnable) setSavingEnable(true);
    else setSaving(true);
    try {
      const provider = buildProvider();
      if (!provider) return;
      const res = await api.upsertProvider(provider);
      if (!res.ok || !res.data) {
        notify(res.error ?? "保存失败", "error");
        return;
      }
      if (andEnable && onSavedAndEnable) {
        notify(editing ? "已保存，正在启用…" : "已创建，正在启用…");
        await onSavedAndEnable(res.data);
        onClose();
        return;
      }
      notify(editing ? "供应商已更新" : "供应商已添加");
      await onSaved(res.data);
      onClose();
    } finally {
      setSaving(false);
      setSavingEnable(false);
    }
  };

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    await persist(false);
  };

  const addExtra = () => {
    setValues((v) => ({
      ...v,
      extras: [
        ...v.extras,
        {
          key: `ex-${Date.now()}`,
          displayName: "",
          model: "",
          largeContext: false,
        },
      ],
    }));
  };

  const updateExtra = (
    key: string,
    patch: Partial<Omit<ExtraModelRow, "key">>,
  ) => {
    setValues((v) => ({
      ...v,
      extras: v.extras.map((r) => (r.key === key ? { ...r, ...patch } : r)),
    }));
  };

  const removeExtra = (key: string) => {
    setValues((v) => ({
      ...v,
      extras: v.extras.filter((r) => r.key !== key),
    }));
  };

  const quickFillModels = () => {
    const base = values.defaultModel.trim() || "grok-4.5";
    setValues((v) => ({
      ...v,
      defaultDisplayName: base,
      defaultModel: base,
      defaultLargeContext: true,
      extras: [
        {
          key: "ex-fast",
          displayName: `${base}-fast`,
          model: `${base}-fast`,
          largeContext: false,
        },
        {
          key: "ex-lite",
          displayName: `${base}-lite`,
          model: `${base}-lite`,
          largeContext: false,
        },
      ],
      advancedOpen: true,
    }));
    notify("已填充示例模型映射，请按中转站实际模型名修改");
  };

  return (
    <div className="sheet-overlay" role="dialog" aria-modal="true">
      <form className="sheet-panel" onSubmit={onSubmit}>
        <header className="sheet-top">
          <button
            type="button"
            className="sheet-back"
            onClick={onClose}
            aria-label="返回"
          >
            <ArrowLeft size={18} />
          </button>
          <h1>{editing ? "编辑供应商" : "添加供应商"}</h1>
          <div className="sheet-top-actions">
            <button
              type="button"
              className="ghost-btn"
              onClick={() => void onTest()}
              disabled={testing || saving || savingEnable}
            >
              {testing ? (
                <LoaderCircle className="spin" size={15} />
              ) : (
                <Zap size={15} />
              )}
              测通
            </button>
            <button
              type="button"
              className="ghost-btn"
              disabled={saving || savingEnable}
              onClick={() => void persist(false)}
            >
              {saving ? <LoaderCircle className="spin" size={15} /> : null}
              仅保存
            </button>
            <button
              type="button"
              className="primary-btn"
              disabled={saving || savingEnable}
              onClick={() => void persist(true)}
            >
              {savingEnable ? (
                <LoaderCircle className="spin" size={15} />
              ) : null}
              保存并启用
            </button>
          </div>
        </header>

        <div className="sheet-body">
          <div className="sheet-avatar-wrap">
            <div className="sheet-avatar">{avatarLabel(values.name)}</div>
          </div>

          <div className="sheet-row-2">
            <label className="sheet-field">
              <span>供应商名称</span>
              <input
                value={values.name}
                onChange={(e) => set("name", e.target.value)}
                placeholder="服务器 cpa"
                required
              />
            </label>
            <label className="sheet-field">
              <span>备注</span>
              <input
                value={values.notes}
                onChange={(e) => set("notes", e.target.value)}
                placeholder="例如：公司专用账号"
              />
            </label>
          </div>

          <label className="sheet-field">
            <span>官网链接</span>
            <input
              value={values.websiteUrl}
              onChange={(e) => set("websiteUrl", e.target.value)}
              placeholder="https://example.com"
            />
          </label>

          <label className="sheet-field">
            <span>API Key</span>
            <div className="sheet-secret">
              <input
                type={revealKey ? "text" : "password"}
                value={values.apiKey}
                onChange={(e) => set("apiKey", e.target.value)}
                placeholder="sk-..."
                required
                autoComplete="off"
              />
              <button
                type="button"
                className="sheet-icon-inline"
                onClick={() => setRevealKey((v) => !v)}
                aria-label={revealKey ? "隐藏密钥" : "显示密钥"}
              >
                {revealKey ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </label>

          <div className="sheet-field">
            <div className="sheet-label-row">
              <span>请求地址</span>
              <div className="sheet-inline-toggles">
                <button
                  type="button"
                  className={`sheet-chip ${values.fullUrlMode ? "on" : ""}`}
                  onClick={() => {
                    set("fullUrlMode", !values.fullUrlMode);
                    if (!values.fullUrlMode) set("appendV1", false);
                    else set("appendV1", true);
                  }}
                >
                  完整 URL
                </button>
                {!values.fullUrlMode && (
                  <button
                    type="button"
                    className={`sheet-chip ${values.appendV1 ? "on" : ""}`}
                    onClick={() => set("appendV1", !values.appendV1)}
                  >
                    自动 /v1
                  </button>
                )}
                <button
                  type="button"
                  className="sheet-link-btn"
                  onClick={() => void onTest()}
                  disabled={testing}
                >
                  <Zap size={14} /> 管理与测通
                </button>
              </div>
            </div>
            <input
              value={values.baseUrl}
              onChange={(e) => set("baseUrl", e.target.value)}
              placeholder="https://api.example.com"
              required
            />
            <div className="sheet-tip">
              <Lightbulb size={15} />
              <span>
                {values.fullUrlMode
                  ? "完整 URL 模式：将按你填写的地址原样请求，请自行包含 /v1 等路径。"
                  : "填写兼容 OpenAI / Anthropic 的服务端点地址，不要以斜杠结尾。默认会自动补全 /v1。"}
              </span>
            </div>
            {values.baseUrl.trim() && (
              <div className="sheet-resolved mono">
                实际请求基址：{resolvedBase || "—"}
              </div>
            )}
          </div>

          <button
            type="button"
            className="sheet-advanced-toggle"
            onClick={() => set("advancedOpen", !values.advancedOpen)}
          >
            <ChevronDown
              size={16}
              className={values.advancedOpen ? "rot" : ""}
            />
            高级选项
          </button>

          {values.advancedOpen && (
            <div className="sheet-advanced">
              <label className="sheet-field">
                <span>API 格式</span>
                <select
                  value={values.apiBackend}
                  onChange={(e) =>
                    set("apiBackend", e.target.value as ApiBackend)
                  }
                >
                  <option value="chat_completions">
                    OpenAI Chat Completions（默认）
                  </option>
                  <option value="responses">OpenAI Responses</option>
                  <option value="messages">Anthropic Messages（原生）</option>
                </select>
                <small className="sheet-help">
                  选择供应商 API 的输入格式。当前认证：{authHint}
                </small>
              </label>

              <div className="sheet-field">
                <div className="sheet-label-row">
                  <span>模型映射</span>
                  <div className="sheet-inline-toggles">
                    <button
                      type="button"
                      className="sheet-chip"
                      onClick={quickFillModels}
                    >
                      一键示例
                    </button>
                    <button
                      type="button"
                      className="sheet-chip"
                      onClick={addExtra}
                    >
                      <Plus size={13} /> 添加模型
                    </button>
                  </div>
                </div>
                <small className="sheet-help">
                  显示名称用于界面；实际请求模型会写入 Grok CLI 的 model 字段。勾选
                  1M 表示大上下文（约 100 万 tokens）。
                </small>

                <div className="sheet-model-table">
                  <div className="sheet-model-head">
                    <span>角色</span>
                    <span>显示名称</span>
                    <span>实际请求模型</span>
                    <span>大上下文 1M</span>
                  </div>

                  <div className="sheet-model-row">
                    <span className="sheet-role">默认</span>
                    <input
                      value={values.defaultDisplayName}
                      onChange={(e) =>
                        set("defaultDisplayName", e.target.value)
                      }
                      placeholder="grok-4.5"
                    />
                    <input
                      value={values.defaultModel}
                      onChange={(e) => set("defaultModel", e.target.value)}
                      placeholder="grok-4.5"
                      required
                    />
                    <label className="sheet-check">
                      <input
                        type="checkbox"
                        checked={values.defaultLargeContext}
                        onChange={(e) =>
                          set("defaultLargeContext", e.target.checked)
                        }
                      />
                      <span>1M</span>
                    </label>
                  </div>

                  {values.extras.map((row, idx) => (
                    <div className="sheet-model-row" key={row.key}>
                      <span className="sheet-role">备用{idx + 1}</span>
                      <input
                        value={row.displayName}
                        onChange={(e) =>
                          updateExtra(row.key, {
                            displayName: e.target.value,
                          })
                        }
                        placeholder="显示名称"
                      />
                      <input
                        value={row.model}
                        onChange={(e) =>
                          updateExtra(row.key, { model: e.target.value })
                        }
                        placeholder="实际模型 id"
                      />
                      <div className="sheet-model-end">
                        <label className="sheet-check">
                          <input
                            type="checkbox"
                            checked={row.largeContext}
                            onChange={(e) =>
                              updateExtra(row.key, {
                                largeContext: e.target.checked,
                              })
                            }
                          />
                          <span>1M</span>
                        </label>
                        <button
                          type="button"
                          className="sheet-icon-inline danger"
                          onClick={() => removeExtra(row.key)}
                          aria-label="删除"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              <label className="sheet-field">
                <span>默认兜底模型（可选）</span>
                <input
                  value={values.fallbackModel}
                  onChange={(e) => set("fallbackModel", e.target.value)}
                  placeholder="未命中映射时的兜底模型 id"
                />
                <small className="sheet-help">
                  用于额外备注；Grok CLI 当前以「默认」行作为启用后的
                  [models].default。备用模型会一并写入 config 的 gs-* 列表，便于
                  /model 切换。
                </small>
              </label>

              <div className="sheet-row-2">
                <label className="sheet-field">
                  <span>池优先级（越大越先）</span>
                  <input
                    type="number"
                    value={values.priority}
                    onChange={(e) =>
                      set("priority", Number(e.target.value) || 0)
                    }
                  />
                </label>
                <label className="sheet-field">
                  <span>池权重</span>
                  <input
                    type="number"
                    min={1}
                    value={values.weight}
                    onChange={(e) =>
                      set("weight", Math.max(1, Number(e.target.value) || 100))
                    }
                  />
                </label>
              </div>
              <label className="sheet-check" style={{ gap: 8 }}>
                <input
                  type="checkbox"
                  checked={values.poolEnabled}
                  onChange={(e) => set("poolEnabled", e.target.checked)}
                />
                <span>参与本地代理池 / 故障切换</span>
              </label>
            </div>
          )}
        </div>

        <footer className="sheet-foot">
          <button type="button" className="outline-btn" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="ghost-btn"
            disabled={saving || savingEnable}
            onClick={() => void persist(false)}
          >
            {saving ? <LoaderCircle className="spin" size={15} /> : null}
            仅保存
          </button>
          <button
            type="button"
            className="primary-btn"
            disabled={saving || savingEnable}
            onClick={() => void persist(true)}
          >
            {savingEnable ? <LoaderCircle className="spin" size={15} /> : null}
            保存并启用
          </button>
        </footer>
      </form>
    </div>
  );
}
