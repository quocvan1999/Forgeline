import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { FolderOpen, Layers } from "lucide-react";
import {
  addSource,
  createProject,
  deleteProject,
  indexSource,
  listProjects,
  listSources,
  projectStats,
  renameProject,
  type Project,
  type ProjectStats,
  type Source,
} from "./api";
import { AppShell } from "./components/shell/AppShell";
import { TabKey } from "./components/shell/Toolbar";
import { EmptyState } from "./components/controls/EmptyState";
import { StatusBadge } from "./components/controls/StatusBadge";
import { Button } from "./components/controls/Button";
import { SourceGraphView } from "./SourceGraphView";
import { usePaneWidths } from "./hooks/usePaneWidths";
import { useTheme } from "./hooks/useTheme";
import { useHotkeys } from "./hooks/useHotkeys";

export function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sources, setSources] = useState<Source[]>([]);
  const [stats, setStats] = useState<ProjectStats | null>(null);
  const [activeTab, setActiveTab] = useState<TabKey>("overview");

  // Project creation state
  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [newProjectName, setNewProjectName] = useState("");

  // Existing form states
  const [rename, setRename] = useState("");
  const [sourceName, setSourceName] = useState("");
  const [sourceKind, setSourceKind] = useState("local_path");
  const [sourceLocation, setSourceLocation] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [graphSource, setGraphSource] = useState<Source | null>(null);

  // Pane resizing and theme hooks
  const {
    sidebarWidth,
    inspectorWidth,
    isInspectorCollapsed,
    isResizingSidebar,
    isResizingInspector,
    startSidebarResize,
    startInspectorResize,
    resetSidebarWidth,
    resetInspectorWidth,
    toggleInspector,
  } = usePaneWidths();

  const { setTheme, resolvedTheme } = useTheme();

  const selected = projects.find((project) => project.id === selectedId) ?? null;

  const refreshProjects = useCallback(async (prefer?: string) => {
    const next = await listProjects();
    setProjects(next);
    setSelectedId((current) => {
      if (prefer && next.some((project) => project.id === prefer)) {
        return prefer;
      }
      if (current && next.some((project) => project.id === current)) {
        return current;
      }
      return next[0]?.id ?? null;
    });
  }, []);

  const refreshDetail = useCallback(async (projectId: string) => {
    const [nextSources, nextStats] = await Promise.all([
      listSources(projectId),
      projectStats(projectId),
    ]);
    setSources(nextSources);
    setStats(nextStats);
  }, []);

  useEffect(() => {
    refreshProjects().catch((reason: unknown) => setError(messageOf(reason)));
  }, [refreshProjects]);

  useEffect(() => {
    if (!selectedId) {
      setSources([]);
      setStats(null);
      return;
    }
    refreshDetail(selectedId).catch((reason: unknown) => setError(messageOf(reason)));
  }, [refreshDetail, selectedId]);

  useEffect(() => {
    setRename(selected?.name ?? "");
  }, [selected?.id, selected?.name]);

  // Global hotkeys
  useHotkeys({
    onTabOverview: () => selected && setActiveTab("overview"),
    onTabGraph: () => selected && setActiveTab("graph"),
    onTabSources: () => selected && setActiveTab("sources"),
    onNewProject: () => setIsCreatingProject(true),
    onEscape: () => {
      if (isCreatingProject) {
        setIsCreatingProject(false);
        setNewProjectName("");
      }
    },
  });

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (reason) {
      setError(messageOf(reason));
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateProjectSubmit(e: React.FormEvent) {
    e.preventDefault();
    const name = newProjectName.trim();
    if (!name) return;
    await run(async () => {
      const created = await createProject(name);
      setNewProjectName("");
      setIsCreatingProject(false);
      await refreshProjects(created.id);
    });
  }

  async function chooseLocalFolder() {
    setError(null);
    try {
      const selectedPath = await open({
        directory: true,
        multiple: false,
        title: "Chọn thư mục nguồn",
        defaultPath: sourceLocation || undefined,
      });
      if (typeof selectedPath !== "string" || selectedPath.length === 0) {
        return;
      }
      setSourceLocation(selectedPath);
      if (sourceName.trim() === "") {
        setSourceName(folderName(selectedPath));
      }
    } catch (reason) {
      setError(messageOf(reason));
    }
  }

  const isInspectorOpen = activeTab === "graph" && !isInspectorCollapsed;

  const toggleThemeMode = () => {
    setTheme(resolvedTheme === "dark" ? "light" : "dark");
  };

  return (
    <AppShell
      sidebarWidth={sidebarWidth}
      isResizingSidebar={isResizingSidebar}
      onStartSidebarResize={startSidebarResize}
      onResetSidebarWidth={resetSidebarWidth}
      sidebarProps={{
        projects,
        selectedProjectId: selectedId,
        onSelectProject: (id) => setSelectedId(id),
        onCreateProjectClick: () => setIsCreatingProject(true),
        children: (
          <>
            {isCreatingProject && (
              <form
                onSubmit={handleCreateProjectSubmit}
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: "var(--space-1)",
                  padding: "var(--space-1) 0",
                  marginBottom: "var(--space-2)",
                }}
              >
                <input
                  autoFocus
                  aria-label="Tên dự án mới"
                  placeholder="Tên dự án mới"
                  value={newProjectName}
                  onChange={(e) => setNewProjectName(e.target.value)}
                  style={{
                    height: "28px",
                    padding: "0 8px",
                    borderRadius: "var(--radius-sm)",
                    border: "1px solid var(--accent)",
                    backgroundColor: "var(--bg-control)",
                    color: "var(--label)",
                    fontSize: "var(--text-body)",
                    outline: "none",
                  }}
                />
                <div style={{ display: "flex", gap: "var(--space-1)", justifyContent: "flex-end" }}>
                  <Button
                    size="compact"
                    variant="secondary"
                    type="button"
                    onClick={() => {
                      setIsCreatingProject(false);
                      setNewProjectName("");
                    }}
                  >
                    Hủy
                  </Button>
                  <Button
                    size="compact"
                    variant="primary"
                    type="submit"
                    disabled={busy || newProjectName.trim() === ""}
                  >
                    Tạo
                  </Button>
                </div>
              </form>
            )}
          </>
        ),
      }}
      inspectorWidth={inspectorWidth}
      isResizingInspector={isResizingInspector}
      isInspectorOpen={isInspectorOpen}
      onStartInspectorResize={startInspectorResize}
      onResetInspectorWidth={resetInspectorWidth}
      onCloseInspector={toggleInspector}
      activeTab={activeTab}
      onTabChange={setActiveTab}
      projectTitle={selected?.name ?? null}
      hasProjectSelected={selected !== null}
      toolbarProps={{
        theme: resolvedTheme,
        onToggleTheme: toggleThemeMode,
      }}
    >
      {error && (
        <div
          role="alert"
          style={{
            padding: "var(--space-2) var(--space-3)",
            marginBottom: "var(--space-3)",
            backgroundColor: "color-mix(in srgb, var(--system-red) 12%, transparent)",
            border: "1px solid var(--system-red)",
            borderRadius: "var(--radius-sm)",
            color: "var(--system-red)",
            fontSize: "var(--text-body)",
          }}
        >
          {error}
        </div>
      )}

      {/* When no project selected or no projects at all */}
      {!selected ? (
        projects.length === 0 ? (
          <EmptyState
            icon={<FolderOpen size={44} />}
            title="Chưa có dự án"
            body="Tạo dự án để gắn nguồn mã và xem sơ đồ liên kết chính xác."
            actionLabel="Tạo dự án"
            onAction={() => setIsCreatingProject(true)}
          />
        ) : (
          <EmptyState
            icon={<Layers size={44} />}
            title="Chọn một dự án"
            body="Chọn dự án ở thanh bên, hoặc tạo dự án mới."
            actionLabel="Tạo dự án"
            onAction={() => setIsCreatingProject(true)}
          />
        )
      ) : (
        /* Selected Project Content by Tab */
        <>
          {activeTab === "overview" && (
            <div>
              <header className="header" style={{ marginBottom: "var(--space-4)" }}>
                <div>
                  <h2 style={{ fontSize: "var(--text-title1)", margin: 0 }}>{selected.name}</h2>
                  <p style={{ color: "var(--secondary-label)", fontSize: "var(--text-callout)", marginTop: "var(--space-1)" }}>
                    Một cuộc hội thoại, nhiều repo.
                  </p>
                </div>
                <form
                  className="row"
                  onSubmit={(event) => {
                    event.preventDefault();
                    const name = rename.trim();
                    if (!name || name === selected.name) return;
                    void run(async () => {
                      await renameProject(selected.id, name);
                      await refreshProjects(selected.id);
                    });
                  }}
                >
                  <input
                    aria-label="Đổi tên dự án"
                    value={rename}
                    onChange={(event) => setRename(event.target.value)}
                  />
                  <button type="submit" disabled={busy}>
                    Đổi tên
                  </button>
                  <button
                    type="button"
                    className="danger"
                    disabled={busy}
                    onClick={() => {
                      if (!window.confirm(`Xóa dự án “${selected.name}”?`)) return;
                      void run(async () => {
                        await deleteProject(selected.id);
                        await refreshProjects();
                      });
                    }}
                  >
                    Xóa
                  </button>
                </form>
              </header>

              {stats && (
                <>
                  <section className="cards">
                    <article className="card">
                      <span className="muted">Nguồn</span>
                      <b>{stats.sourceCount}</b>
                    </article>
                    <article className="card">
                      <span className="muted">Ký hiệu</span>
                      <b>{stats.symbolCount}</b>
                    </article>
                    <article className="card">
                      <span className="muted">Liên kết chính xác</span>
                      <b>{stats.exactCallCount}</b>
                    </article>
                    <article className="card">
                      <span className="muted">Chưa xác định</span>
                      <b>{stats.unresolvedCallCount}</b>
                    </article>
                  </section>
                  <pre className="summary">{stats.summary.trim()}</pre>
                </>
              )}
            </div>
          )}

          {activeTab === "graph" && (
            <div style={{ height: "100%", minHeight: "500px" }}>
              {graphSource || sources[0] ? (
                <SourceGraphView
                  source={graphSource || sources[0]}
                  onClose={() => setGraphSource(null)}
                />
              ) : (
                <EmptyState
                  icon={<Layers size={44} />}
                  title="Chưa có sơ đồ"
                  body="Phân tích nguồn ở trạng thái Sẵn sàng rồi mở tab Sơ đồ."
                  actionLabel="Đến Nguồn"
                  onAction={() => setActiveTab("sources")}
                />
              )}
            </div>
          )}

          {activeTab === "sources" && (
            <div>
              <section>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "var(--space-3)" }}>
                  <h2 style={{ fontSize: "var(--text-title2)", margin: 0 }}>Nguồn mã</h2>
                </div>

                <form
                  className="stack"
                  onSubmit={(event) => {
                    event.preventDefault();
                    const name = sourceName.trim();
                    const location = sourceLocation.trim();
                    if (!name || !location) return;
                    void run(async () => {
                      await addSource(selected.id, name, sourceKind, location);
                      setSourceName("");
                      setSourceLocation("");
                      await refreshDetail(selected.id);
                    });
                  }}
                  style={{ marginBottom: "var(--space-4)" }}
                >
                  <div className="row">
                    <select
                      aria-label="Loại nguồn"
                      value={sourceKind}
                      onChange={(event) => setSourceKind(event.target.value)}
                    >
                      <option value="local_path">Thư mục cục bộ</option>
                      <option value="git_url">URL git</option>
                    </select>
                    {sourceKind === "local_path" ? (
                      <button type="button" onClick={() => void chooseLocalFolder()} disabled={busy}>
                        Chọn thư mục…
                      </button>
                    ) : null}
                  </div>
                  <input
                    aria-label="Đường dẫn hoặc URL"
                    placeholder={sourceKind === "git_url" ? "https://github.com/org/repo.git" : "Đường dẫn thư mục"}
                    value={sourceLocation}
                    onChange={(event) => {
                      const nextLocation = event.target.value;
                      setSourceLocation(nextLocation);
                      if (sourceName.trim() === "" && nextLocation.trim() !== "") {
                        setSourceName(folderName(nextLocation));
                      }
                    }}
                  />
                  <input
                    aria-label="Tên nguồn"
                    placeholder="Tên nguồn (vd. backend, web)"
                    value={sourceName}
                    onChange={(event) => setSourceName(event.target.value)}
                  />
                  <button className="primary" type="submit" disabled={busy || sourceName.trim() === "" || sourceLocation.trim() === ""}>
                    Thêm nguồn
                  </button>
                </form>

                {sources.length === 0 ? (
                  <p className="empty">Chưa có nguồn. Thêm đường dẫn cục bộ hoặc URL git.</p>
                ) : null}

                {sources.map((source) => (
                  <article className="source" key={source.id} style={{ marginBottom: "var(--space-2)" }}>
                    <div>
                      <h2>
                        {source.name}
                        <StatusBadge status={source.status} />
                      </h2>
                      <p>
                        {source.kind === "git_url" ? "Git" : "Cục bộ"} · {source.location}
                      </p>
                      <p>
                        {source.fileCount} tệp · {source.symbolCount} ký hiệu · chính xác {source.exactCallCount} · chưa xác định {source.unresolvedCallCount}
                      </p>
                      {source.statusMessage ? <p className="error">{source.statusMessage}</p> : null}
                    </div>
                    <div className="row">
                      <button
                        type="button"
                        disabled={busy || source.status === "indexing"}
                        onClick={() => {
                          setGraphSource(source);
                          setActiveTab("graph");
                        }}
                      >
                        Sơ đồ
                      </button>
                      <button
                        className="primary"
                        type="button"
                        disabled={busy || source.status === "indexing"}
                        onClick={() => {
                          void run(async () => {
                            await indexSource(source.id);
                            await refreshDetail(selected.id);
                          });
                        }}
                      >
                        Phân tích
                      </button>
                    </div>
                  </article>
                ))}
              </section>
            </div>
          )}
        </>
      )}
    </AppShell>
  );
}

function messageOf(reason: unknown): string {
  if (reason instanceof Error) {
    return reason.message;
  }
  if (typeof reason === "string") {
    return reason;
  }
  return "Không thực hiện được.";
}

function folderName(path: string): string {
  const normalized = path.replace(/[\\/]+$/, "");
  const segments = normalized.split(/[\\/]/);
  return segments[segments.length - 1] ?? "";
}
