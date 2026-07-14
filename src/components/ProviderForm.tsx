import { useEffect, useState } from "react";
import { LoaderCircle, X } from "lucide-react";
import type { ApiBackend, Provider } from "../lib/types";
import { maskSecret } from "../lib/mask";
import * as api from "../lib/api";

export type ProviderFormValues = {
  name: string;
  baseUrl: string;
  apiKey: string;
  apiBackend: ApiBackend;
  defaultModel: string;
  appendV1: boolean;
  contextWindow: number;
  websiteUrl: string;
  notes: string;
};

function ensureTrailingSlashFree(url: string): string {
  return url.replace(/\/+$/, "");
}

/** Stable section id for [model.gs-<id>] — unique per provider+model. */
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

function fromProvider(p?: Provider | null): ProviderFormValues {
  if (!p) {
    return {
      name: "",
      baseUrl: "",
      apiKey: "",
      apiBackend: "chat_completions",
      defaultModel: "grok-4",
      appendV1: true,
      contextWindow: 200_000,
      websiteUrl: "",
      notes: "",
    };
  }
  const endsWithV1 = /\/v1$/i.test(p.baseUrl);
  return {
    name: p.name,
    baseUrl: endsWithV1 ? p.baseUrl.replace(/\/v1$/i, "") : p.baseUrl,
    apiKey: p.apiKey,
    apiBackend: p.apiBackend,
    defaultModel:
      p.models.find((m) => m.id === p.defaultModelEntryId)?.model ??
      p.models[0]?.model ??
      "",
    appendV1: endsWithV1,
    contextWindow: p.contextWindow || 200_000,
    websiteUrl: p.websiteUrl ?? "",
    notes: p.notes ?? "",
  };
}

export function ProviderForm({
  open,
  initial,
  onClose,
  onSaved,
  notify,
}: {
  open: boolean;
  initial?: Provider | null;
  onClose: () => void;
  onSaved: (p: Provider) => void;
  notify: (msg: string, tone?: "ok" | "error") => void;
}) {
  const editing = Boolean(initial);
  const [values, setValues] = useState<ProviderFormValues>(() =>
    fromProvider(initial),
  );
  const [revealKey, setRevealKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);

  useEffect(() => {
    if (open) {
      setValues(fromProvider(initial));
      setRevealKey(false);
    }
  }, [open, initial]);

  if (!open) return null;

  const set = <K extends keyof ProviderFormValues>(
    key: K,
    value: ProviderFormValues[K],
  ) => setValues((v) => ({ ...v, [key]: value }));

  const buildProvider = (): Provider => {
    const now = Math.floor(Date.now() / 1000);
    const baseUrl = applyAppendV1(values.baseUrl, values.appendV1);
    const name = values.name.trim();
    const defaultModel = values.defaultModel.trim();
    // Keep existing entry id on edit; generate unique slug for new providers
    // so two providers don't both write [model.gs-m1].
    const modelId =
      initial?.defaultModelEntryId ||
      `${slugify(name)}-${slugify(defaultModel)}`;
    const existingModels = initial?.models ?? [];
    const models =
      existingModels.length > 0
        ? existingModels.map((m) =>
            m.id === modelId
              ? {
                  ...m,
                  model: defaultModel,
                  name: defaultModel,
                  contextWindow: values.contextWindow,
                }
              : m,
          )
        : [
            {
              id: modelId,
              model: defaultModel,
              name: defaultModel,
              contextWindow: values.contextWindow,
            },
          ];

    // Ensure default entry always exists (edit path if id mismatched).
    if (!models.some((m) => m.id === modelId)) {
      models.unshift({
        id: modelId,
        model: defaultModel,
        name: defaultModel,
        contextWindow: values.contextWindow,
      });
    }

    return {
      id: initial?.id ?? crypto.randomUUID(),
      name,
      baseUrl,
      apiKey: values.apiKey.trim(),
      apiBackend: values.apiBackend,
      defaultModelEntryId: modelId,
      models,
      contextWindow: values.contextWindow,
      websiteUrl: values.websiteUrl.trim() || undefined,
      notes: values.notes.trim() || undefined,
      source: initial?.source ?? "manual",
      createdAt: initial?.createdAt ?? now,
      updatedAt: now,
    };
  };

  const onTest = async () => {
    setTesting(true);
    try {
      const draft = {
        baseUrl: applyAppendV1(values.baseUrl, values.appendV1),
        apiKey: values.apiKey.trim(),
        apiBackend: values.apiBackend,
        model: values.defaultModel.trim(),
      };
      const res = await api.testProviderDraft(draft);
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

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!values.name.trim() || !values.baseUrl.trim() || !values.apiKey.trim()) {
      notify("名称、Base URL 和 API Key 为必填", "error");
      return;
    }
    setSaving(true);
    try {
      const provider = buildProvider();
      const res = await api.upsertProvider(provider);
      if (!res.ok || !res.data) {
        notify(res.error ?? "保存失败", "error");
        return;
      }
      notify(editing ? "供应商已更新" : "供应商已添加");
      onSaved(res.data);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-overlay" role="dialog" aria-modal="true">
      <form className="modal-card" onSubmit={onSubmit}>
        <div className="modal-head">
          <div>
            <span className="label">{editing ? "编辑供应商" : "新建供应商"}</span>
            <h2>{editing ? "更新供应商" : "添加供应商"}</h2>
          </div>
          <button type="button" className="icon-btn" onClick={onClose} aria-label="关闭">
            <X size={18} />
          </button>
        </div>

        <div className="form-grid">
          <label className="field">
            <span>名称</span>
            <input
              value={values.name}
              onChange={(e) => set("name", e.target.value)}
              placeholder="myallapi"
              required
            />
          </label>

          <label className="field">
            <span>协议</span>
            <select
              value={values.apiBackend}
              onChange={(e) => set("apiBackend", e.target.value as ApiBackend)}
            >
              <option value="chat_completions">OpenAI · Chat Completions</option>
              <option value="responses">OpenAI · Responses</option>
              <option value="messages">Anthropic · Messages</option>
            </select>
          </label>

          <label className="field field-span">
            <span>Base URL</span>
            <input
              value={values.baseUrl}
              onChange={(e) => set("baseUrl", e.target.value)}
              placeholder="https://api.example.com"
              required
            />
          </label>

          <label className="check-field field-span">
            <input
              type="checkbox"
              checked={values.appendV1}
              onChange={(e) => set("appendV1", e.target.checked)}
            />
            <span>
              自动追加 <code>/v1</code>
            </span>
          </label>

          <label className="field field-span">
            <span>API Key</span>
            <div className="secret-row">
              <input
                type={revealKey ? "text" : "password"}
                value={values.apiKey}
                onChange={(e) => set("apiKey", e.target.value)}
                placeholder="sk-..."
                required
              />
              <button
                type="button"
                className="ghost-btn"
                onClick={() => setRevealKey((v) => !v)}
              >
                {revealKey ? "隐藏" : "显示"}
              </button>
            </div>
            {!revealKey && values.apiKey ? (
              <small className="field-hint mono">{maskSecret(values.apiKey)}</small>
            ) : null}
          </label>

          <label className="field">
            <span>默认模型</span>
            <input
              value={values.defaultModel}
              onChange={(e) => set("defaultModel", e.target.value)}
              placeholder="grok-4.5"
              required
            />
          </label>

          <label className="field">
            <span>上下文窗口</span>
            <input
              type="number"
              min={1024}
              value={values.contextWindow}
              onChange={(e) => set("contextWindow", Number(e.target.value) || 0)}
            />
          </label>

          <label className="field field-span">
            <span>网站（可选）</span>
            <input
              value={values.websiteUrl}
              onChange={(e) => set("websiteUrl", e.target.value)}
              placeholder="https://..."
            />
          </label>

          <label className="field field-span">
            <span>备注（可选）</span>
            <textarea
              rows={2}
              value={values.notes}
              onChange={(e) => set("notes", e.target.value)}
              placeholder="内部备注"
            />
          </label>
        </div>

        <div className="modal-actions">
          <button
            type="button"
            className="ghost-btn"
            onClick={onTest}
            disabled={testing || saving}
          >
            {testing ? <LoaderCircle className="spin" size={15} /> : null}
            测通
          </button>
          <div className="modal-actions-right">
            <button type="button" className="outline-btn" onClick={onClose}>
              取消
            </button>
            <button type="submit" className="primary-btn" disabled={saving}>
              {saving ? <LoaderCircle className="spin" size={15} /> : null}
              {editing ? "保存" : "创建"}
            </button>
          </div>
        </div>
      </form>
    </div>
  );
}
