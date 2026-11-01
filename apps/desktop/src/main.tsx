import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/tokens.css";
import "./styles/reset.css";
import "./styles/app.css";
import "./styles.css";

// In browser preview/screenshot environments outside Tauri runtime, provide a harmless IPC mock
if (typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window)) {
  (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string) => Promise<unknown> } }).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string) => {
      if (cmd === "list_projects") return [];
      if (cmd === "list_sources") return [];
      return null;
    },
  };
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("missing root");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
