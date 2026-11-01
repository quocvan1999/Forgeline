import { X } from "lucide-react";
import { Button } from "../controls/Button";

export interface InspectorProps {
  width: number;
  isResizing: boolean;
  isOpen: boolean;
  onClose?: () => void;
  title?: string;
  children?: React.ReactNode;
}

export function Inspector({
  width,
  isResizing,
  isOpen,
  onClose,
  title = "Chi tiết",
  children,
}: InspectorProps) {
  if (!isOpen) {
    return null;
  }

  return (
    <aside
      className={`inspector-pane ${isResizing ? "resizing" : ""}`}
      style={{ width: `${width}px` }}
      aria-label="Khung chi tiết"
    >
      <header className="inspector-header">
        <span>{title}</span>
        {onClose && (
          <Button
            variant="toolbar-icon"
            size="compact"
            iconOnly
            title="Đóng thanh chi tiết"
            aria-label="Đóng thanh chi tiết"
            onClick={onClose}
          >
            <X size={14} />
          </Button>
        )}
      </header>

      <div className="inspector-content">
        {children || (
          <div
            style={{
              padding: "var(--space-6) var(--space-2)",
              textAlign: "center",
              color: "var(--secondary-label)",
              fontSize: "var(--text-callout)",
            }}
          >
            <p>Chọn một nút để xem chi tiết.</p>
          </div>
        )}
      </div>
    </aside>
  );
}
