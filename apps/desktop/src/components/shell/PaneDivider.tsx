export interface PaneDividerProps {
  onMouseDown: (e: React.MouseEvent) => void;
  onDoubleClick?: () => void;
  isActive?: boolean;
  orientation?: "vertical" | "horizontal";
  title?: string;
}

export function PaneDivider({
  onMouseDown,
  onDoubleClick,
  isActive = false,
  orientation = "vertical",
  title = "Kéo để thay đổi kích thước, nhấp đúp để đặt lại",
}: PaneDividerProps) {
  return (
    <div
      role="separator"
      tabIndex={0}
      aria-orientation={orientation}
      title={title}
      className={`pane-divider ${isActive ? "active" : ""}`}
      onMouseDown={onMouseDown}
      onDoubleClick={onDoubleClick}
    />
  );
}
