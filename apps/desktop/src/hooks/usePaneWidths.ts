import { useCallback, useRef, useState } from "react";

const SIDEBAR_STORAGE_KEY = "forgeline.ui.sidebarWidth";
const INSPECTOR_STORAGE_KEY = "forgeline.ui.inspectorWidth";
const INSPECTOR_COLLAPSED_KEY = "forgeline.ui.inspectorCollapsed";

export const SIDEBAR_MIN_WIDTH = 180;
export const SIDEBAR_DEFAULT_WIDTH = 220;
export const SIDEBAR_MAX_WIDTH = 320;

export const INSPECTOR_MIN_WIDTH = 220;
export const INSPECTOR_DEFAULT_WIDTH = 280;
export const INSPECTOR_MAX_WIDTH = 400;

function getStoredNumber(key: string, fallback: number, min: number, max: number): number {
  try {
    const raw = localStorage.getItem(key);
    if (raw !== null) {
      const val = parseInt(raw, 10);
      if (!isNaN(val)) {
        return Math.min(Math.max(val, min), max);
      }
    }
  } catch {
    // ignore localStorage read failure
  }
  return fallback;
}

function getStoredBoolean(key: string, fallback: boolean): boolean {
  try {
    const raw = localStorage.getItem(key);
    if (raw !== null) {
      return raw === "true";
    }
  } catch {
    // ignore
  }
  return fallback;
}

export function usePaneWidths() {
  const [sidebarWidth, setSidebarWidth] = useState(() =>
    getStoredNumber(SIDEBAR_STORAGE_KEY, SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
  );

  const [inspectorWidth, setInspectorWidth] = useState(() =>
    getStoredNumber(INSPECTOR_STORAGE_KEY, INSPECTOR_DEFAULT_WIDTH, INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH),
  );

  const [isInspectorCollapsed, setIsInspectorCollapsed] = useState(() =>
    getStoredBoolean(INSPECTOR_COLLAPSED_KEY, false),
  );

  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [isResizingInspector, setIsResizingInspector] = useState(false);

  // Store references for drag handling
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);

  // Persist sidebar width
  const updateSidebarWidth = useCallback((width: number) => {
    const clamped = Math.min(Math.max(width, SIDEBAR_MIN_WIDTH), SIDEBAR_MAX_WIDTH);
    setSidebarWidth(clamped);
    try {
      localStorage.setItem(SIDEBAR_STORAGE_KEY, clamped.toString());
    } catch {
      // ignore
    }
  }, []);

  // Persist inspector width
  const updateInspectorWidth = useCallback((width: number) => {
    const clamped = Math.min(Math.max(width, INSPECTOR_MIN_WIDTH), INSPECTOR_MAX_WIDTH);
    setInspectorWidth(clamped);
    try {
      localStorage.setItem(INSPECTOR_STORAGE_KEY, clamped.toString());
    } catch {
      // ignore
    }
  }, []);

  // Toggle inspector collapsed state
  const toggleInspector = useCallback(() => {
    setIsInspectorCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem(INSPECTOR_COLLAPSED_KEY, next.toString());
      } catch {
        // ignore
      }
      return next;
    });
  }, []);

  // Start sidebar resize
  const startSidebarResize = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      startXRef.current = e.clientX;
      startWidthRef.current = sidebarWidth;
      setIsResizingSidebar(true);

      const handleMouseMove = (moveEvent: MouseEvent) => {
        const delta = moveEvent.clientX - startXRef.current;
        const newWidth = Math.min(Math.max(startWidthRef.current + delta, SIDEBAR_MIN_WIDTH), SIDEBAR_MAX_WIDTH);
        setSidebarWidth(newWidth);
      };

      const handleMouseUp = (upEvent: MouseEvent) => {
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        setIsResizingSidebar(false);

        const delta = upEvent.clientX - startXRef.current;
        const finalWidth = Math.min(Math.max(startWidthRef.current + delta, SIDEBAR_MIN_WIDTH), SIDEBAR_MAX_WIDTH);
        updateSidebarWidth(finalWidth);
      };

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [sidebarWidth, updateSidebarWidth],
  );

  // Start inspector resize
  const startInspectorResize = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      startXRef.current = e.clientX;
      startWidthRef.current = inspectorWidth;
      setIsResizingInspector(true);

      const handleMouseMove = (moveEvent: MouseEvent) => {
        const delta = startXRef.current - moveEvent.clientX;
        const newWidth = Math.min(Math.max(startWidthRef.current + delta, INSPECTOR_MIN_WIDTH), INSPECTOR_MAX_WIDTH);
        setInspectorWidth(newWidth);
      };

      const handleMouseUp = (upEvent: MouseEvent) => {
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        setIsResizingInspector(false);

        const delta = startXRef.current - upEvent.clientX;
        const finalWidth = Math.min(Math.max(startWidthRef.current + delta, INSPECTOR_MIN_WIDTH), INSPECTOR_MAX_WIDTH);
        updateInspectorWidth(finalWidth);
      };

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [inspectorWidth, updateInspectorWidth],
  );

  return {
    sidebarWidth,
    inspectorWidth,
    isInspectorCollapsed,
    isResizingSidebar,
    isResizingInspector,
    startSidebarResize,
    startInspectorResize,
    toggleInspector,
    resetSidebarWidth: () => updateSidebarWidth(SIDEBAR_DEFAULT_WIDTH),
    resetInspectorWidth: () => updateInspectorWidth(INSPECTOR_DEFAULT_WIDTH),
  };
}
