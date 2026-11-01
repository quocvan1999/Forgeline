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
