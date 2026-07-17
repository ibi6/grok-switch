import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Download,
  Link2,
  LoaderCircle,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Sparkles,
  Trash2,
} from "lucide-react";
import type { SkillDetail, SkillInfo } from "../lib/types";
import * as api from "../lib/api";

const NAME_RE = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/;

function scopeLabel(s: SkillInfo["scope"]): string {
  if (s === "grok") return "Grok";
  if (s === "claude") return "Claude";
  return "CC Switch";
}

function emptyDraft(name = ""): { name: string; description: string; body: string } {
  return {
    name,
    description: "",
    body: "# New skill\n\nDescribe the workflow steps here.\n",
  };
}

export function SkillsPage({
  notify,
}: {
  notify: (msg: string, tone?: "ok" | "error") => void;
}) {
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<SkillDetail | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(emptyDraft());
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await api.listSkills();
      if (!res.ok || !res.data) {
        notify(res.error ?? "加载 skills 失败", "error");
        setSkills([]);
        return;
      }
      setSkills(res.data);
    } finally {
      setLoading(false);
    }
  }, [notify]);

  useEffect(() => {
    void load();
  }, [load]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return skills;
    return skills.filter(
      (s) =>
        s.name.includes(q) ||
        s.description.toLowerCase().includes(q) ||
        s.scope.includes(q),
    );
  }, [skills, query]);

  const openSkill = async (name: string) => {
    setSelected(name);
    setEditing(false);
    setCreating(false);
    setBusy(`get-${name}`);
    try {
      const res = await api.getSkill(name);
      if (!res.ok || !res.data) {
        notify(res.error ?? "读取失败", "error");
        setDetail(null);
        return;
      }
      setDetail(res.data);
      setDraft({
        name: res.data.info.name,
        description: res.data.info.description,
        body: res.data.content.replace(/^---[\s\S]*?---\s*/, ""),
      });
    } finally {
      setBusy(null);
    }
  };

  const startCreate = () => {
    setCreating(true);
    setEditing(true);
    setSelected(null);
    setDetail(null);
    setDraft(emptyDraft());
  };

  const save = async () => {
    const name = draft.name.trim();
    if (!NAME_RE.test(name) || name.length < 2 || name.length > 64) {
      notify("名称须为 2–64 位小写字母/数字/连字符", "error");
      return;
    }
    if (!draft.description.trim()) {
      notify("请填写 description（决定自动触发时机）", "error");
      return;
    }
    setBusy("save");
    try {
      const res = await api.upsertSkill({
        name,
        description: draft.description.trim(),
        content: draft.body,
      });
      if (!res.ok || !res.data) {
        notify(res.error ?? "保存失败", "error");
        return;
      }
      notify(`已保存 skill：${name}`);
      setCreating(false);
      setEditing(false);
      setDetail(res.data);
      setSelected(name);
      await load();
    } finally {
      setBusy(null);
    }
  };

  const onDelete = async (s: SkillInfo) => {
    if (!s.editable && !s.isSymlink) {
      notify("只能删除 ~/.grok/skills 下的 skill", "error");
      return;
    }
    const msg = s.isSymlink
      ? `确定移除符号链接「${s.name}」？\n不会删除链接目标。`
      : `确定删除 skill「${s.name}」？\n会先备份到 ~/.grok-switch/skill-backups。`;
    if (!window.confirm(msg)) return;
    setBusy(`del-${s.name}`);
    try {
      const res = await api.deleteSkill(s.name);
      if (!res.ok) {
        notify(res.error ?? "删除失败", "error");
        return;
      }
      notify(s.isSymlink ? "已移除链接" : "已删除（已备份）");
      if (selected === s.name) {
        setSelected(null);
        setDetail(null);
        setEditing(false);
      }
      await load();
    } finally {
      setBusy(null);
    }
  };

  const onImportCc = async () => {
    if (
      !window.confirm(
        "从 ~/.cc-switch/skills 导入到 ~/.grok/skills？\n已存在的同名 skill 会跳过，不会覆盖。",
      )
    ) {
      return;
    }
    setBusy("import");
    try {
      const res = await api.importSkills([], "cc-switch");
      if (!res.ok || !res.data) {
        notify(res.error ?? "导入失败", "error");
        return;
      }
      notify(
        res.data.length
          ? `已导入 ${res.data.length} 个 skill`
          : "没有可导入的新 skill（可能都已存在）",
      );
      await load();
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="page-wrap">
      <div className="section-head no-margin">
        <div>
          <h2>Skills</h2>
          <p>
            管理 Grok CLI 可复用技能包（SKILL.md）。用户目录：
            <code> ~/.grok/skills</code>
          </p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            className="ghost-btn"
            onClick={() => void load()}
            disabled={loading || !!busy}
          >
            {loading ? <LoaderCircle className="spin" size={15} /> : <RefreshCw size={15} />}
            刷新
          </button>
          <button
            type="button"
            className="ghost-btn"
            onClick={() => void onImportCc()}
            disabled={!!busy}
            title="从 CC Switch 导入"
          >
            {busy === "import" ? (
              <LoaderCircle className="spin" size={15} />
            ) : (
              <Download size={15} />
            )}
            从 CC Switch 导入
          </button>
          <button type="button" className="primary-btn" onClick={startCreate} disabled={!!busy}>
            <Plus size={15} /> 新建
          </button>
        </div>
      </div>

      <div className="toolbar">
        <div className="search">
          <Search size={15} />
          <input
            placeholder="搜索 name / description / 来源"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>

      {loading ? (
        <div className="empty-state">
          <LoaderCircle className="spin" size={22} />
          <b>扫描 skills…</b>
        </div>
      ) : (
        <div className="skills-layout">
          <div className="skills-list-pane">
            {filtered.length === 0 ? (
              <div className="empty-state">
                <Sparkles size={28} style={{ opacity: 0.5 }} />
                <b>{skills.length === 0 ? "还没有 skill" : "无匹配结果"}</b>
                <p>
                  {skills.length === 0
                    ? "新建一个，或从 CC Switch 导入。"
                    : "试试其他关键词。"}
                </p>
              </div>
            ) : (
              <div className="provider-list">
                {filtered.map((s) => {
                  const active = selected === s.name && !creating;
                  return (
                    <div
                      key={`${s.scope}-${s.name}`}
                      className={`provider-card is-clickable ${active ? "is-current" : ""}`}
                      role="button"
                      tabIndex={0}
                      onClick={() => void openSkill(s.name)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          void openSkill(s.name);
                        }
                      }}
                    >
                      <div
                        className="provider-avatar"
                        style={{
                          background: s.editable
                            ? "#7c6cff"
                            : s.scope === "cc-switch"
                              ? "#06b6d4"
                              : "#64748b",
                        }}
                      >
                        <Sparkles size={16} color="#fff" />
                      </div>
                      <div className="provider-body">
                        <div className="provider-title-row">
                          <b className="mono">{s.name}</b>
                          <span className="badge badge-muted">{scopeLabel(s.scope)}</span>
                          {s.isSymlink && (
                            <span className="badge badge-backend" title={s.linkTarget}>
                              <Link2 size={11} /> 链接
                            </span>
                          )}
                          {!s.editable && !s.isSymlink && (
                            <span className="badge badge-muted">只读</span>
                          )}
                        </div>
                        <div className="provider-meta">
                          <span>
                            {s.description || (s.hasSkillMd ? "（无 description）" : "缺少 SKILL.md")}
                          </span>
                        </div>
                      </div>
                      {(s.editable || s.isSymlink) && (
                        <div
                          className="provider-actions"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <button
                            type="button"
                            className="icon-btn"
                            title="删除"
                            disabled={!!busy}
                            onClick={() => void onDelete(s)}
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          <div className="skills-editor-pane">
            {creating || (detail && editing) ? (
              <div className="skills-editor card-like">
                <div className="skills-editor-head">
                  <b>{creating ? "新建 skill" : `编辑 ${draft.name}`}</b>
                  <div className="header-actions">
                    <button
                      type="button"
                      className="ghost-btn"
                      onClick={() => {
                        setCreating(false);
                        setEditing(false);
                        if (selected) void openSkill(selected);
                      }}
                    >
                      取消
                    </button>
                    <button
                      type="button"
                      className="primary-btn"
                      disabled={busy === "save"}
                      onClick={() => void save()}
                    >
                      {busy === "save" ? (
                        <LoaderCircle className="spin" size={15} />
                      ) : null}
                      保存到 ~/.grok/skills
                    </button>
                  </div>
                </div>
                <label className="sheet-field">
                  <span>名称（目录名 / 斜杠命令）</span>
                  <input
                    className="mono"
                    value={draft.name}
                    disabled={!creating}
                    onChange={(e) =>
                      setDraft((d) => ({
                        ...d,
                        name: e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ""),
                      }))
                    }
                    placeholder="my-skill"
                  />
                </label>
                <label className="sheet-field">
                  <span>Description（自动触发关键词，很重要）</span>
                  <textarea
                    rows={3}
                    value={draft.description}
                    onChange={(e) =>
                      setDraft((d) => ({ ...d, description: e.target.value }))
                    }
                    placeholder="When to use this skill…"
                  />
                </label>
                <label className="sheet-field">
                  <span>正文（提示词 / 步骤）</span>
                  <textarea
                    className="skills-body mono"
                    rows={16}
                    value={draft.body}
                    onChange={(e) => setDraft((d) => ({ ...d, body: e.target.value }))}
                  />
                </label>
              </div>
            ) : detail ? (
              <div className="skills-editor card-like">
                <div className="skills-editor-head">
                  <div>
                    <b className="mono">{detail.info.name}</b>
                    <div className="topbar-sub">
                      {scopeLabel(detail.info.scope)}
                      {detail.info.isSymlink ? " · 符号链接" : ""}
                      {detail.info.editable ? " · 可编辑" : " · 只读"}
                    </div>
                  </div>
                  <div className="header-actions">
                    {detail.info.editable && (
                      <button
                        type="button"
                        className="primary-btn"
                        onClick={() => setEditing(true)}
                      >
                        <Pencil size={14} /> 编辑
                      </button>
                    )}
                  </div>
                </div>
                <p className="skills-desc">{detail.info.description || "（无 description）"}</p>
                <pre className="skills-preview mono">{detail.content}</pre>
                <div className="provider-meta" style={{ marginTop: 12 }}>
                  <code title={detail.info.path}>{detail.info.path}</code>
                  {detail.info.linkTarget && (
                    <span title={detail.info.linkTarget}>
                      → {detail.info.linkTarget}
                    </span>
                  )}
                </div>
                <div className="notice" style={{ marginTop: 16 }}>
                  <Sparkles size={16} />
                  <div>
                    <b>在 Grok 中使用</b>
                    <p>
                      TUI 输入 <code>/{detail.info.name}</code> 或{" "}
                      <code>/skills {detail.info.name}</code>
                      ；description 匹配时也会自动注入。
                    </p>
                  </div>
                </div>
              </div>
            ) : (
              <div className="empty-state">
                <Sparkles size={28} style={{ opacity: 0.45 }} />
                <b>选择一个 skill</b>
                <p>查看 SKILL.md，或新建 / 从 CC Switch 导入。</p>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
