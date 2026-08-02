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
    <div
      className={`toast ${tone === "error" ? "toast-error" : ""}`}
      role={tone === "error" ? "alert" : "status"}
      aria-live={tone === "error" ? "assertive" : "polite"}
      aria-atomic="true"
    >
      {tone === "error" ? (
        <AlertTriangle size={16} aria-hidden="true" />
      ) : (
        <Check size={16} aria-hidden="true" />
      )}
      <span>{message}</span>
    </div>
  );
}
