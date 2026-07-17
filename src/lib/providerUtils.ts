import type { Provider } from "./types";

/** Same whitelist as Rust `is_safe_model_token` — used before shell/IPC. */
export function isSafeModelToken(s: string): boolean {
  const t = s.trim();
  if (!t || t.length > 128) return false;
  return /^[A-Za-z0-9._/:+-]+$/.test(t);
}

/** Managed Grok CLI model section id: gs-<entryId> */
export function modelFlag(p: Provider): string {
  const entry =
    p.models.find((m) => m.id === p.defaultModelEntryId) ?? p.models[0];
  const id = entry?.id ?? p.defaultModelEntryId;
  return id.startsWith("gs-") ? id : `gs-${id}`;
}

export function backendLabel(b: Provider["apiBackend"]): string {
  if (b === "messages") return "Anthropic";
  if (b === "responses") return "Responses";
  return "OpenAI";
}

export function modeLabel(
  mode: string | undefined,
  providerName?: string,
  accountName?: string,
): string {
  if (mode === "provider" && providerName) return `中转 · ${providerName}`;
  if (mode === "official" && accountName) return `官方 · ${accountName}`;
  if (mode === "provider") return "中转 · 未选中";
  if (mode === "official") return "官方 · 未选中";
  return "未启用";
}
