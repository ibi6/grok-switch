import { AlertTriangle, Check } from "lucide-react";

export type ToastTone = "ok" | "error";

export function Toast({
  message,
  tone = "ok",
}: {
  message: string;
  tone?: ToastTone;
}) {
  if (!message) return null;
  return (
    <div className={`toast ${tone === "error" ? "toast-error" : ""}`}>
      {tone === "error" ? <AlertTriangle size={16} /> : <Check size={16} />}
      {message}
    </div>
  );
}
