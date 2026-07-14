import { LoaderCircle } from "lucide-react";

export function SwitchOverlay({
  open,
  title = "Switching…",
  detail = "Backing up config and verifying Grok CLI…",
}: {
  open: boolean;
  title?: string;
  detail?: string;
}) {
  if (!open) return null;
  return (
    <div className="switch-overlay">
      <div className="switch-modal">
        <LoaderCircle className="spin" size={28} />
        <h3>{title}</h3>
        <p>{detail}</p>
      </div>
    </div>
  );
}
