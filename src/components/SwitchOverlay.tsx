import { useEffect, useRef } from "react";
import { LoaderCircle } from "lucide-react";

export function SwitchOverlay({
  open,
  title = "正在切换…",
  detail = "备份配置并验证 Grok CLI…",
}: {
  open: boolean;
  title?: string;
  detail?: string;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const previousFocus = document.activeElement as HTMLElement | null;
    dialogRef.current?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      event.preventDefault();
      dialogRef.current?.focus();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previousFocus?.focus({ preventScroll: true });
    };
  }, [open]);

  if (!open) return null;
  return (
    <div className="switch-overlay" aria-busy="true">
      <div
        ref={dialogRef}
        className="switch-modal"
        role="dialog"
        aria-modal="true"
        aria-busy="true"
        aria-labelledby="switch-overlay-title"
        aria-describedby="switch-overlay-detail"
        tabIndex={-1}
      >
        <LoaderCircle className="spin" size={28} aria-hidden="true" />
        <h3 id="switch-overlay-title">{title}</h3>
        <p id="switch-overlay-detail">{detail}</p>
      </div>
    </div>
  );
}
