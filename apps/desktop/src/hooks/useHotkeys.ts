import { useEffect } from "react";

export interface HotkeyHandlers {
  onNewProject?: () => void;
  onAddSource?: () => void;
  onReanalyze?: () => void;
  onSearch?: () => void;
  onTabOverview?: () => void;
  onTabGraph?: () => void;
  onTabSources?: () => void;
  onSettings?: () => void;
  onEscape?: () => void;
}

export function useHotkeys(handlers: HotkeyHandlers) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const isInput =
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);

      if (e.key === "Escape") {
        if (handlers.onEscape) {
          handlers.onEscape();
        }
        return;
      }

      // If user is actively typing in an input field, do not trigger navigation/action hotkeys
      if (isInput) {
        return;
      }

      const isMac = navigator.platform.toUpperCase().indexOf("MAC") >= 0;
      const modKey = isMac ? e.metaKey : e.ctrlKey;

      if (!modKey) return;

      // ⌘1 / ⌘2 / ⌘3 - Switch tabs
      if (e.key === "1") {
        e.preventDefault();
        handlers.onTabOverview?.();
      } else if (e.key === "2") {
        e.preventDefault();
        handlers.onTabGraph?.();
      } else if (e.key === "3") {
        e.preventDefault();
        handlers.onTabSources?.();
      }
      // ⌘N - New project
      else if (e.key.toLowerCase() === "n" && !e.shiftKey) {
        e.preventDefault();
        handlers.onNewProject?.();
      }
      // ⇧⌘O - Add source
      else if (e.key.toLowerCase() === "o" && e.shiftKey) {
        e.preventDefault();
        handlers.onAddSource?.();
      }
      // ⌘F - Focus search
      else if (e.key.toLowerCase() === "f") {
        e.preventDefault();
        handlers.onSearch?.();
      }
      // ⌘R - Re-analyze
      else if (e.key.toLowerCase() === "r" && !e.shiftKey) {
        e.preventDefault();
        handlers.onReanalyze?.();
      }
      // ⌘, - Settings
      else if (e.key === ",") {
        e.preventDefault();
        handlers.onSettings?.();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handlers]);
}
