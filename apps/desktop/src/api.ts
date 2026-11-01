import { invoke } from "@tauri-apps/api/core";

export type Project = {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
};

export type Source = {
  id: string;
  projectId: string;
  name: string;
  kind: string;
  location: string;
  status: string;
  statusMessage: string | null;
  fileCount: number;
  symbolCount: number;
  exactCallCount: number;
  inferredCallCount: number;
  unresolvedCallCount: number;
  ambiguousCallCount: number;
  reindexedFiles: number;
  reusedFiles: number;
  removedFiles: number;
  skippedFiles: number;
  lastIndexedAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ProjectStats = {
  projectId: string;
  projectName: string;
  summary: string;
  sourceCount: number;
  symbolCount: number;
  exactCallCount: number;
  inferredCallCount: number;
  unresolvedCallCount: number;
};

export type OverviewNode = {
  id: string;
  label: string;
  kind: string;
  path: string | null;
  symbolCount: number;
  exactOut: number;
  exactIn: number;
  x: number;
  y: number;
};

export type OverviewEdge = {
  id: string;
  from: string;
  to: string;
  weight: number;
  kind: string;
};

export type OverviewGraph = {
  sourceId: string;
  sourceName: string;
  level: string;
  parentId: string | null;
  nodes: OverviewNode[];
  edges: OverviewEdge[];
  exactCallCount: number;
  unresolvedCallCount: number;
};

export type NeighborHit = {
  symbolId: string;
  name: string;
  qualifiedName: string;
  kind: string;
  relativePath: string;
  line: number;
  evidencePath: string;
  evidenceLine: number;
  detail: string;
};

export type SymbolNeighbors = {
  symbolId: string;
  callers: NeighborHit[];
  callees: NeighborHit[];
};

export function listProjects(): Promise<Project[]> {
  return invoke("list_projects");
}

export function createProject(name: string): Promise<Project> {
  return invoke("create_project", { name });
}

export function renameProject(id: string, name: string): Promise<Project> {
  return invoke("rename_project", { id, name });
}

export function deleteProject(id: string): Promise<void> {
  return invoke("delete_project", { id });
}

export function listSources(projectId: string): Promise<Source[]> {
  return invoke("list_sources", { projectId });
}

export function addSource(
  projectId: string,
  name: string,
  kind: string,
  location: string,
): Promise<Source> {
  return invoke("add_source", { projectId, name, kind, location });
}

export function indexSource(sourceId: string): Promise<Source> {
  return invoke("index_source", { sourceId });
}

export function projectStats(projectId: string): Promise<ProjectStats> {
  return invoke("project_stats", { projectId });
}

export function sourceOverview(
  sourceId: string,
  focus?: string | null,
  kindFilter?: string | null,
  search?: string | null,
): Promise<OverviewGraph> {
  return invoke("source_overview", {
    sourceId,
    focus: focus ?? null,
    kindFilter: kindFilter ?? null,
    search: search ?? null,
  });
}

export function exportSourceOverview(sourceId: string, focus?: string | null): Promise<string> {
  return invoke("export_source_overview", { sourceId, focus: focus ?? null });
}

export function symbolNeighbors(symbolId: string): Promise<SymbolNeighbors> {
  return invoke("symbol_neighbors", { symbolId });
}
