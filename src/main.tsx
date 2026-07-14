import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

// Avoid dark flash: apply last saved theme before first paint.
try {
  const saved = localStorage.getItem("gs-theme");
  if (saved === "light" || saved === "dark") {
    document.documentElement.setAttribute("data-theme", saved);
    document.documentElement.style.colorScheme = saved;
  } else if (saved === "system" && window.matchMedia) {
    const light = window.matchMedia("(prefers-color-scheme: light)").matches;
    const resolved = light ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", resolved);
    document.documentElement.style.colorScheme = resolved;
  }
} catch {
  /* ignore */
}

createRoot(document.getElementById("root")!).render(<App />);
