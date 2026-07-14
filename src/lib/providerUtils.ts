import type { Provider } from "./types";

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
