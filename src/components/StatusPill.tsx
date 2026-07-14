import { Check } from "lucide-react";

export function StatusPill({
  label = "Current",
  tone = "acid",
}: {
  label?: string;
  tone?: "acid" | "warn" | "muted";
}) {
  return (
    <span className={`status-pill status-pill-${tone}`}>
      {tone === "acid" && <Check size={12} />}
      {label}
    </span>
  );
}

export function BackendBadge({ backend }: { backend: string }) {
  const map: Record<string, string> = {
    chat_completions: "OpenAI",
    responses: "Responses",
    messages: "Anthropic",
  };
  return (
    <span className="backend-badge">{map[backend] ?? backend}</span>
  );
}
