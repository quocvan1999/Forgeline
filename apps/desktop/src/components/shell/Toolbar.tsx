import { FolderPlus, Moon, Search, Settings, Sun } from "lucide-react";
import { Button } from "../controls/Button";
import { SegmentedControl, SegmentOption } from "../controls/SegmentedControl";

export type TabKey = "overview" | "graph" | "sources";

const TAB_OPTIONS: SegmentOption<TabKey>[] = [
  { id: "overview", label: "Tổng quan" },
  { id: "graph", label: "Sơ đồ" },
  { id: "sources", label: "Nguồn" },
];

export interface ToolbarProps {
  projectTitle?: string | null;
  activeTab: TabKey;
  onTabChange: (tab: TabKey) => void;
  hasProjectSelected: boolean;
  onSearchClick?: () => void;
  onAddSourceClick?: () => void;
  onSettingsClick?: () => void;
  theme?: "light" | "dark" | "system";
  onToggleTheme?: () => void;
}

export function Toolbar({
  projectTitle,
  activeTab,
  onTabChange,
  hasProjectSelected,
  onSearchClick,
  onAddSourceClick,
  onSettingsClick,
  theme,
  onToggleTheme,
}: ToolbarProps) {
  const displayTitle = projectTitle && projectTitle.trim().length > 0
    ? projectTitle
    : "Forgeline";

  return (
    <header className="toolbar" data-tauri-drag-region>
      {/* Leading: Project Title */}
      <div className="toolbar-leading">
        <h1 className="toolbar-title" title={displayTitle}>
          {displayTitle}
        </h1>
      </div>

      {/* Center: Segmented Navigation Control */}
      <div className="toolbar-center">
        <SegmentedControl
          options={TAB_OPTIONS}
          value={activeTab}
          onChange={onTabChange}
          disabled={!hasProjectSelected}
          size="compact"
        />
      </div>

      {/* Trailing: Action Icons */}
      <div className="toolbar-trailing">
        <Button
          variant="toolbar-icon"
          size="compact"
          iconOnly
          title="Tìm kiếm (⌘F)"
          aria-label="Tìm kiếm"
          onClick={onSearchClick}
        >
          <Search size={15} />
        </Button>

        <Button
          variant="toolbar-icon"
          size="compact"
          iconOnly
          disabled={!hasProjectSelected}
          title="Thêm nguồn (⇧⌘O)"
          aria-label="Thêm nguồn"
          onClick={onAddSourceClick}
        >
          <FolderPlus size={15} />
        </Button>

        {onToggleTheme && (
          <Button
            variant="toolbar-icon"
            size="compact"
            iconOnly
            title={`Chuyển giao diện (${theme === "dark" ? "Sáng" : "Tối"})`}
            aria-label="Chuyển giao diện"
            onClick={onToggleTheme}
          >
            {theme === "dark" ? <Sun size={15} /> : <Moon size={15} />}
          </Button>
        )}

        <Button
          variant="toolbar-icon"
          size="compact"
          iconOnly
          title="Cài đặt (⌘,)"
          aria-label="Cài đặt"
          onClick={onSettingsClick}
        >
          <Settings size={15} />
        </Button>
      </div>
    </header>
  );
}
