import { useCallback, useEffect, useState } from "react";
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

const STATUS_LABEL: Record<string, string> = {
  pending: "Chờ",
  indexing: "Đang phân tích",
  ready: "Sẵn sàng",
  error: "Lỗi",
};

export function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sources, setSources] = useState<Source[]>([]);
  const [stats, setStats] = useState<ProjectStats | null>(null);
  const [projectName, setProjectName] = useState("");
  const [rename, setRename] = useState("");
  const [sourceName, setSourceName] = useState("");
  const [sourceKind, setSourceKind] = useState("local_path");
  const [sourceLocation, setSourceLocation] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <strong>Forgeline</strong>
          <span>Đồ thị mã cục bộ</span>
        </div>
        <form
          className="stack"
          onSubmit={(event) => {
            event.preventDefault();
            const name = projectName.trim();
            if (!name) {
              return;
            }
            void run(async () => {
              const created = await createProject(name);
              setProjectName("");
              await refreshProjects(created.id);
            });
          }}
        >
          <input
            aria-label="Tên dự án"
            placeholder="Tên dự án mới"
            value={projectName}
            onChange={(event) => setProjectName(event.target.value)}
          />
          <button className="primary" type="submit" disabled={busy || projectName.trim() === ""}>
            Tạo dự án
          </button>
        </form>
        <div className="project-list">
          {projects.length === 0 ? <p className="empty">Chưa có dự án.</p> : null}
          {projects.map((project) => (
            <button
              key={project.id}
              type="button"
              className={project.id === selectedId ? "active" : ""}
              onClick={() => setSelectedId(project.id)}
            >
              {project.name}
            </button>
          ))}
        </div>
        <p className="security">Dữ liệu chỉ nằm trên máy này. Không telemetry, không tự cài thêm công cụ.</p>
      </aside>
      <main className="main">
        {error ? <p className="error">{error}</p> : null}
        {!selected || !stats ? (
          <p className="empty">Chọn hoặc tạo một dự án. Mỗi dự án là một cuộc làm việc và có thể gắn nhiều nguồn mã.</p>
        ) : (
          <>
            <header className="header">
              <div>
                <h1>{selected.name}</h1>
                <p className="muted">Một cuộc hội thoại, nhiều repo.</p>
              </div>
              <form
                className="row"
                onSubmit={(event) => {
                  event.preventDefault();
                  const name = rename.trim();
                  if (!name || name === selected.name) {
                    return;
                  }
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
                    if (!window.confirm(`Xóa dự án “${selected.name}”?`)) {
                      return;
                    }
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
                <span className="muted">Lời gọi chính xác</span>
                <b>{stats.exactCallCount}</b>
              </article>
              <article className="card">
                <span className="muted">Lời gọi suy đoán</span>
                <b>{stats.inferredCallCount}</b>
              </article>
            </section>
            <pre className="summary">{stats.summary.trim()}</pre>
            <section>
              <h2>Nguồn mã</h2>
              {sources.length === 0 ? <p className="empty">Chưa có nguồn. Thêm đường dẫn cục bộ hoặc URL git.</p> : null}
              {sources.map((source) => (
                <article className="source" key={source.id}>
                  <div>
                    <h2>
                      {source.name}
                      <span className={`badge ${source.status}`}>
                        {STATUS_LABEL[source.status] ?? source.status}
                      </span>
                    </h2>
                    <p>
                      {source.kind === "git_url" ? "Git" : "Cục bộ"} · {source.location}
                    </p>
                    <p>
                      {source.fileCount} tệp · {source.symbolCount} ký hiệu · chính xác {source.exactCallCount} ·
                      suy đoán {source.inferredCallCount} · chưa phân giải {source.unresolvedCallCount}
                    </p>
                    {source.statusMessage ? <p className="error">{source.statusMessage}</p> : null}
                  </div>
                  <button
                    className="primary"
                    type="button"
                    disabled={busy}
                    onClick={() => {
                      void run(async () => {
                        await indexSource(source.id);
                        await refreshDetail(selected.id);
                        await refreshProjects(selected.id);
                      });
                    }}
                  >
                    Phân tích
                  </button>
                </article>
              ))}
              <form
                className="form inline"
                onSubmit={(event) => {
                  event.preventDefault();
                  const name = sourceName.trim();
                  const location = sourceLocation.trim();
                  if (!name || !location) {
                    return;
                  }
                  void run(async () => {
                    await addSource(selected.id, name, sourceKind, location);
                    setSourceName("");
                    setSourceLocation("");
                    await refreshDetail(selected.id);
                  });
                }}
              >
                <input
                  aria-label="Tên nguồn"
                  placeholder="Tên nguồn"
                  value={sourceName}
                  onChange={(event) => setSourceName(event.target.value)}
                />
                <select
                  aria-label="Loại nguồn"
                  value={sourceKind}
                  onChange={(event) => setSourceKind(event.target.value)}
                >
                  <option value="local_path">Đường dẫn cục bộ</option>
                  <option value="git_url">URL git</option>
                </select>
                <input
                  aria-label="Vị trí nguồn"
                  placeholder={sourceKind === "git_url" ? "https://github.com/org/repo.git" : "/đường/dẫn/repo"}
                  value={sourceLocation}
                  onChange={(event) => setSourceLocation(event.target.value)}
                />
                <button type="submit" disabled={busy}>
                  Thêm nguồn
                </button>
              </form>
            </section>
          </>
        )}
      </main>
    </div>
  );
}

function messageOf(reason: unknown): string {
  if (typeof reason === "string") {
    return reason;
  }
  if (reason instanceof Error) {
    return reason.message;
  }
  return "Không thực hiện được.";
}
