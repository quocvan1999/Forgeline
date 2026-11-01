import React from "react";
import { Folder, Plus } from "lucide-react";
import { Button } from "../controls/Button";

export interface ProjectItem {
  id: string;
  name: string;
  updated_at?: string;
}

export interface SidebarProps {
  width: number;
  isResizing: boolean;
  projects?: ProjectItem[];
  selectedProjectId?: string | null;
  onSelectProject?: (id: string) => void;
  onCreateProjectClick?: () => void;
  children?: React.ReactNode;
}

export function Sidebar({
  width,
  isResizing,
  projects = [],
  selectedProjectId = null,
  onSelectProject,
  onCreateProjectClick,
  children,
}: SidebarProps) {
  return (
    <aside
      className={`sidebar-pane ${isResizing ? "resizing" : ""}`}
      style={{ width: `${width}px` }}
      aria-label="Thanh bên dự án"
    >
      {/* Top Header with macOS traffic light safe area */}
      <header className="sidebar-header" data-tauri-drag-region>
        <span className="sidebar-brand">Forgeline</span>
        <div className="sidebar-actions">
          <Button
            variant="toolbar-icon"
            size="compact"
            iconOnly
            title="Tạo dự án mới (⌘N)"
            aria-label="Tạo dự án mới"
            onClick={onCreateProjectClick}
          >
            <Plus size={16} />
          </Button>
        </div>
      </header>

      {/* Main projects list or custom injected children */}
      <div className="sidebar-content" role="navigation">
        {children ? (
          children
        ) : projects.length === 0 ? (
          <div
            style={{
              padding: "var(--space-4) var(--space-2)",
              textAlign: "center",
              color: "var(--secondary-label)",
              fontSize: "var(--text-callout)",
            }}
          >
            <span>Chưa có dự án</span>
          </div>
        ) : (
          projects.map((project) => {
            const isSelected = project.id === selectedProjectId;
            return (
              <div
                key={project.id}
                role="button"
                tabIndex={0}
                aria-selected={isSelected}
                onClick={() => onSelectProject?.(project.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onSelectProject?.(project.id);
                  }
                }}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "var(--space-2)",
                  height: "30px",
                  padding: "0 var(--space-2)",
                  borderRadius: "var(--radius-sm)",
                  cursor: "default",
                  backgroundColor: isSelected
                    ? "var(--bg-selected)"
                    : "transparent",
                  color: "var(--label)",
                  fontSize: "var(--text-body)",
                  fontWeight: isSelected ? 500 : 400,
                  transition:
                    "background-color var(--motion-duration) var(--motion-ease)",
                }}
              >
                <Folder
                  size={15}
                  style={{
                    color: isSelected
                      ? "var(--accent)"
                      : "var(--secondary-label)",
                    flexShrink: 0,
                  }}
                />
                <span
                  style={{
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {project.name}
                </span>
              </div>
            );
          })
        )}
      </div>

      {/* Footer with Security Footnote */}
      <footer className="sidebar-footer">
        <p>Dữ liệu chỉ nằm trên máy này. Không telemetry.</p>
      </footer>
    </aside>
  );
}
