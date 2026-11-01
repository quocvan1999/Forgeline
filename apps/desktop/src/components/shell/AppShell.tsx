import { PaneDivider } from "./PaneDivider";
import { Sidebar, SidebarProps } from "./Sidebar";
import { TabKey, Toolbar, ToolbarProps } from "./Toolbar";
import { Inspector, InspectorProps } from "./Inspector";

export interface AppShellProps {
  // Sidebar config
  sidebarWidth: number;
  isResizingSidebar: boolean;
  onStartSidebarResize: (e: React.MouseEvent) => void;
  onResetSidebarWidth: () => void;
  sidebarProps?: Partial<SidebarProps>;

  // Inspector config
  inspectorWidth: number;
  isResizingInspector: boolean;
  isInspectorOpen: boolean;
  onStartInspectorResize: (e: React.MouseEvent) => void;
  onResetInspectorWidth: () => void;
  onCloseInspector?: () => void;
  inspectorProps?: Partial<InspectorProps>;

  // Toolbar config
  activeTab: TabKey;
  onTabChange: (tab: TabKey) => void;
  projectTitle?: string | null;
  hasProjectSelected: boolean;
  toolbarProps?: Partial<ToolbarProps>;

  // Main content
  children: React.ReactNode;
}

export function AppShell({
  sidebarWidth,
  isResizingSidebar,
  onStartSidebarResize,
  onResetSidebarWidth,
  sidebarProps,

  inspectorWidth,
  isResizingInspector,
  isInspectorOpen,
  onStartInspectorResize,
  onResetInspectorWidth,
  onCloseInspector,
  inspectorProps,

  activeTab,
  onTabChange,
  projectTitle,
  hasProjectSelected,
  toolbarProps,

  children,
}: AppShellProps) {
  return (
    <div className="app-shell">
      {/* 1. Sidebar Pane */}
      <Sidebar
        width={sidebarWidth}
        isResizing={isResizingSidebar}
        {...sidebarProps}
      />

      {/* 2. Sidebar Splitter Divider */}
      <PaneDivider
        onMouseDown={onStartSidebarResize}
        onDoubleClick={onResetSidebarWidth}
        isActive={isResizingSidebar}
        title="Kéo để chỉnh độ rộng thanh bên, nhấp đúp để đặt lại mặc định"
      />

      {/* 3. Center Content Area (Toolbar + Body) */}
      <main className="content-pane">
        <Toolbar
          projectTitle={projectTitle}
          activeTab={activeTab}
          onTabChange={onTabChange}
          hasProjectSelected={hasProjectSelected}
          {...toolbarProps}
        />
        <div className="content-body" role="region" aria-label="Nội dung chính">
          {children}
        </div>
      </main>

      {/* 4. Inspector Splitter Divider (only when inspector is visible) */}
      {isInspectorOpen && (
        <PaneDivider
          onMouseDown={onStartInspectorResize}
          onDoubleClick={onResetInspectorWidth}
          isActive={isResizingInspector}
          title="Kéo để chỉnh độ rộng thanh chi tiết, nhấp đúp để đặt lại mặc định"
        />
      )}

      {/* 5. Right Inspector Pane */}
      <Inspector
        width={inspectorWidth}
        isResizing={isResizingInspector}
        isOpen={isInspectorOpen}
        onClose={onCloseInspector}
        {...inspectorProps}
      />
    </div>
  );
}
