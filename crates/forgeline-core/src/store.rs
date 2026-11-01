use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use crate::migrate::migrate;
use crate::model::{
    now_rfc3339, CallerGroup, EdgeHit, Project, ProjectStats, Source, SymbolHit, SymbolReport,
    KIND_GIT_URL, KIND_LOCAL_PATH, STATUS_PENDING,
};
use crate::validate::{validate_git_url, validate_name};
use crate::CoreError;
use forgeline_graph::{EdgeKind, ExtractedFile, Provenance, SourceGraph, SummaryLevel};

const LIST_LIMIT: i64 = 100;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn open(path: &Path) -> Result<Self, CoreError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut store = Self { conn };
        migrate(&mut store.conn)?;
        Ok(store)
    }

    pub fn create_project(&mut self, name: &str) -> Result<Project, CoreError> {
        let name = validate_name(name)?;
        if self.get_project_by_name(&name)?.is_some() {
            return Err(CoreError::DuplicateName(name));
        }
        let now = now_rfc3339();
        let project = Project {
            id: Uuid::new_v4().to_string(),
            name,
            created_at: now.clone(),
            updated_at: now,
        };
        self.conn.execute(
            "INSERT INTO projects (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                project.id,
                project.name,
                project.created_at,
                project.updated_at
            ],
        )?;
        Ok(project)
    }

    pub fn rename_project(&mut self, id: &str, name: &str) -> Result<Project, CoreError> {
        let name = validate_name(name)?;
        if let Some(existing) = self.get_project_by_name(&name)? {
            if existing.id != id {
                return Err(CoreError::DuplicateName(name));
            }
        }
        let now = now_rfc3339();
        let changed = self.conn.execute(
            "UPDATE projects SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, now, id],
        )?;
        if changed == 0 {
            return Err(CoreError::ProjectNotFound);
        }
        self.rebuild_project_summary(id)?;
        self.get_project(id)?.ok_or(CoreError::ProjectNotFound)
    }

    pub fn delete_project(&mut self, data_dir: &Path, id: &str) -> Result<(), CoreError> {
        let changed = self
            .conn
            .execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(CoreError::ProjectNotFound);
        }
        let dir = crate::paths::project_dir(data_dir, id);
        if dir.is_dir() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, created_at, updated_at FROM projects ORDER BY updated_at DESC, name",
        )?;
        let rows = stmt.query_map([], map_project)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>, CoreError> {
        self.conn
            .query_row(
                "SELECT id, name, created_at, updated_at FROM projects WHERE id = ?1",
                params![id],
                map_project,
            )
            .optional()
            .map_err(CoreError::from)
    }

    pub fn get_project_by_name(&self, name: &str) -> Result<Option<Project>, CoreError> {
        self.conn
            .query_row(
                "SELECT id, name, created_at, updated_at FROM projects
                 WHERE name = ?1 COLLATE NOCASE",
                params![name],
                map_project,
            )
            .optional()
            .map_err(CoreError::from)
    }

    pub fn require_project(&self, name: Option<&str>) -> Result<Project, CoreError> {
        match name {
            Some(name) => self
                .get_project_by_name(name)?
                .ok_or(CoreError::ProjectNotFound),
            None => {
                let mut projects = self.list_projects()?;
                match projects.len() {
                    0 => Err(CoreError::ProjectNotFound),
                    1 => Ok(projects.remove(0)),
                    _ => Err(CoreError::ProjectAmbiguous),
                }
            }
        }
    }

    pub fn add_source(
        &mut self,
        project_id: &str,
        name: &str,
        kind: &str,
        location: &str,
    ) -> Result<Source, CoreError> {
        if self.get_project(project_id)?.is_none() {
            return Err(CoreError::ProjectNotFound);
        }
        let name = validate_name(name)?;
        let location = normalize_location(kind, location)?;
        if let Some(existing) = self.find_source_by_location(project_id, &location)? {
            return Ok(existing);
        }
        let now = now_rfc3339();
        let source_id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO sources (
                id, project_id, name, kind, location, status, status_message,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
            params![
                source_id,
                project_id,
                name,
                kind,
                location,
                STATUS_PENDING,
                now,
                now
            ],
        )?;
        self.touch_project(project_id)?;
        self.get_source(&source_id)?
            .ok_or(CoreError::SourceNotFound)
    }

    pub fn list_sources(&self, project_id: &str) -> Result<Vec<Source>, CoreError> {
        if self.get_project(project_id)?.is_none() {
            return Err(CoreError::ProjectNotFound);
        }
        let mut stmt = self.conn.prepare(&format!(
            "{SOURCE_SELECT} WHERE project_id = ?1 ORDER BY name, created_at"
        ))?;
        let rows = stmt.query_map(params![project_id], map_source)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    pub fn get_source(&self, id: &str) -> Result<Option<Source>, CoreError> {
        self.conn
            .query_row(
                &format!("{SOURCE_SELECT} WHERE id = ?1"),
                params![id],
                map_source,
            )
            .optional()
            .map_err(CoreError::from)
    }

    pub fn find_source_by_location(
        &self,
        project_id: &str,
        location: &str,
    ) -> Result<Option<Source>, CoreError> {
        self.conn
            .query_row(
                &format!("{SOURCE_SELECT} WHERE project_id = ?1 AND location = ?2"),
                params![project_id, location],
                map_source,
            )
            .optional()
            .map_err(CoreError::from)
    }

    pub fn delete_source(&mut self, data_dir: &Path, source_id: &str) -> Result<(), CoreError> {
        let source = self
            .get_source(source_id)?
            .ok_or(CoreError::SourceNotFound)?;
        let checkout = crate::paths::source_checkout_dir(data_dir, &source.project_id, source_id);
        self.conn
            .execute("DELETE FROM sources WHERE id = ?1", params![source_id])?;
        if checkout.is_dir() {
            std::fs::remove_dir_all(checkout)?;
        }
        self.rebuild_project_summary(&source.project_id)?;
        self.touch_project(&source.project_id)?;
        Ok(())
    }

    pub fn set_status(
        &mut self,
        source_id: &str,
        status: &str,
        message: Option<&str>,
    ) -> Result<(), CoreError> {
        let now = now_rfc3339();
        let changed = self.conn.execute(
            "UPDATE sources SET status = ?1, status_message = ?2, updated_at = ?3 WHERE id = ?4",
            params![status, message, now, source_id],
        )?;
        if changed == 0 {
            return Err(CoreError::SourceNotFound);
        }
        Ok(())
    }

    pub(crate) fn load_extractions(
        &self,
        source_id: &str,
    ) -> Result<Vec<ExtractedFile>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT payload FROM file_extractions WHERE source_id = ?1")?;
        let rows = stmt.query_map(params![source_id], |row| row.get::<_, String>(0))?;
        let mut files = Vec::new();
        for payload in rows {
            let payload = payload?;
            match serde_json::from_str::<ExtractedFile>(&payload) {
                Ok(file) => files.push(file),
                Err(_) => continue,
            }
        }
        Ok(files)
    }

    pub(crate) fn save_graph(
        &mut self,
        project_id: &str,
        source_id: &str,
        graph: &SourceGraph,
    ) -> Result<(), CoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM edges WHERE source_id = ?1", params![source_id])?;
        tx.execute(
            "DELETE FROM symbols WHERE source_id = ?1",
            params![source_id],
        )?;
        tx.execute(
            "DELETE FROM file_extractions WHERE source_id = ?1",
            params![source_id],
        )?;
        tx.execute(
            "DELETE FROM summaries WHERE source_id = ?1",
            params![source_id],
        )?;

        for file in &graph.files {
            let payload = serde_json::to_string(file)?;
            tx.execute(
                "INSERT INTO file_extractions (source_id, relative_path, language, content_hash, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    source_id,
                    file.relative_path,
                    file.language.as_str(),
                    file.content_hash,
                    payload
                ],
            )?;
            for symbol in &file.symbols {
                tx.execute(
                    "INSERT INTO symbols (
                        id, source_id, parent_id, name, qualified_name, kind, language,
                        relative_path, start_line, end_line, start_column, end_column
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        symbol.id,
                        source_id,
                        symbol.parent_id,
                        symbol.name,
                        symbol.qualified_name,
                        symbol.kind.as_str(),
                        symbol.language.as_str(),
                        symbol.relative_path,
                        symbol.start_line,
                        symbol.end_line,
                        symbol.start_column,
                        symbol.end_column
                    ],
                )?;
            }
        }

        for edge in &graph.edges {
            tx.execute(
                "INSERT INTO edges (
                    id, project_id, source_id, kind, from_symbol_id, to_symbol_id, to_name,
                    provenance, confidence, evidence_path, evidence_line, detail
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    Uuid::new_v4().to_string(),
                    project_id,
                    source_id,
                    edge.kind.as_str(),
                    edge.from_id,
                    edge.to_id,
                    edge.to_name,
                    edge.provenance.as_str(),
                    edge.confidence,
                    edge.evidence_path,
                    edge.evidence_line,
                    edge.detail
                ],
            )?;
        }

        let now = now_rfc3339();
        for summary in &graph.summaries {
            tx.execute(
                "INSERT INTO summaries (
                    id, project_id, source_id, level, owner_id, body, token_estimate, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("{}:{}", summary.level.as_str(), summary.owner_id),
                    project_id,
                    source_id,
                    summary.level.as_str(),
                    summary.owner_id,
                    summary.body,
                    summary.token_estimate,
                    now
                ],
            )?;
        }

        let symbol_count = graph.symbols().count() as i64;
        let exact_call_count = graph.call_edges().count() as i64;
        tx.execute(
            "UPDATE sources SET
                status = 'ready',
                status_message = ?12,
                file_count = ?1,
                symbol_count = ?2,
                exact_call_count = ?3,
                inferred_call_count = 0,
                unresolved_call_count = ?4,
                ambiguous_call_count = ?5,
                reindexed_files = ?6,
                reused_files = ?7,
                removed_files = ?8,
                skipped_files = ?9,
                last_indexed_at = ?10,
                updated_at = ?10
             WHERE id = ?11",
            params![
                graph.files.len() as i64,
                symbol_count,
                exact_call_count,
                graph.unresolved_calls,
                graph.ambiguous_calls,
                graph.reindexed_files,
                graph.reused_files,
                graph.removed_files,
                graph.skipped_files,
                now,
                source_id,
                graph.resolver_notice.as_deref()
            ],
        )?;
        tx.execute(
            "UPDATE projects SET updated_at = ?1 WHERE id = ?2",
            params![now, project_id],
        )?;
        tx.commit()?;
        self.rebuild_project_summary(project_id)?;
        Ok(())
    }

    pub fn project_stats(&self, project_id: &str) -> Result<ProjectStats, CoreError> {
        let project = self
            .get_project(project_id)?
            .ok_or(CoreError::ProjectNotFound)?;
        let (source_count, symbol_count, exact_call_count, inferred_call_count, unresolved): (
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = self.conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(symbol_count), 0),
                    COALESCE(SUM(exact_call_count), 0),
                    COALESCE(SUM(inferred_call_count), 0),
                    COALESCE(SUM(unresolved_call_count), 0)
             FROM sources WHERE project_id = ?1",
            params![project_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        let summary = self.summary_body(project_id, SummaryLevel::Project, project_id)?;
        Ok(ProjectStats {
            project_id: project.id,
            project_name: project.name,
            summary,
            source_count,
            symbol_count,
            exact_call_count,
            inferred_call_count,
            unresolved_call_count: unresolved,
        })
    }

    pub fn explain(&self, project_id: &str, query: &str) -> Result<Vec<SymbolReport>, CoreError> {
        let symbols = self.find_symbols(project_id, query)?;
        let mut reports = Vec::new();
        for symbol in symbols {
            let outgoing_calls = self.edges_from(&symbol.id, EdgeKind::Calls, None)?;
            let exact_caller_count = self.count_callers(&symbol.id)?;
            reports.push(SymbolReport {
                symbol,
                outgoing_calls,
                exact_caller_count,
            });
        }
        Ok(reports)
    }

    pub fn callers(&self, project_id: &str, query: &str) -> Result<Vec<CallerGroup>, CoreError> {
        let symbols = self.find_symbols(project_id, query)?;
        let mut groups = Vec::new();
        for symbol in symbols {
            let mut callers = self.edges_to(&symbol.id, Some(Provenance::Exact))?;
            let truncated = callers.len() as i64 > LIST_LIMIT;
            callers.truncate(LIST_LIMIT as usize);
            groups.push(CallerGroup {
                symbol,
                callers,
                truncated,
            });
        }
        Ok(groups)
    }

    fn find_symbols(&self, project_id: &str, query: &str) -> Result<Vec<SymbolHit>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.source_id, src.name, s.name, s.qualified_name, s.kind, s.language,
                    s.relative_path, s.start_line, s.end_line, COALESCE(sum.body, '')
             FROM symbols s
             JOIN sources src ON src.id = s.source_id
             LEFT JOIN summaries sum ON sum.owner_id = s.id AND sum.level = 'symbol'
             WHERE src.project_id = ?1
               AND (s.id = ?2 OR s.qualified_name = ?2 OR s.name = ?2)
             ORDER BY s.relative_path, s.start_line, s.name",
        )?;
        let rows = stmt.query_map(params![project_id, query], map_symbol)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    fn edges_from(
        &self,
        symbol_id: &str,
        kind: EdgeKind,
        provenance: Option<Provenance>,
    ) -> Result<Vec<EdgeHit>, CoreError> {
        self.edge_hits("e.from_symbol_id = ?1", symbol_id, kind, provenance, "t")
    }

    fn edges_to(
        &self,
        symbol_id: &str,
        provenance: Option<Provenance>,
    ) -> Result<Vec<EdgeHit>, CoreError> {
        self.edge_hits(
            "e.to_symbol_id = ?1",
            symbol_id,
            EdgeKind::Calls,
            provenance,
            "c",
        )
    }

    fn edge_hits(
        &self,
        predicate: &str,
        symbol_id: &str,
        kind: EdgeKind,
        provenance: Option<Provenance>,
        side: &str,
    ) -> Result<Vec<EdgeHit>, CoreError> {
        let provenance_sql = if provenance.is_some() {
            "AND e.provenance = ?3"
        } else {
            ""
        };
        let other = if side == "t" { "t" } else { "c" };
        let sql = format!(
            "SELECT e.provenance, e.confidence, e.detail, e.evidence_path, e.evidence_line,
                    COALESCE({other}.name, e.to_name),
                    COALESCE({other}.qualified_name, e.to_name),
                    COALESCE({other}.kind, ''),
                    COALESCE({other}.relative_path, ''),
                    COALESCE({other}.start_line, 0),
                    COALESCE(src.name, '')
             FROM edges e
             LEFT JOIN symbols t ON t.id = e.to_symbol_id
             LEFT JOIN symbols c ON c.id = e.from_symbol_id
             LEFT JOIN sources src ON src.id = {other}.source_id
             WHERE {predicate} AND e.kind = ?2 {provenance_sql}
             ORDER BY e.provenance, e.evidence_path, e.evidence_line
             LIMIT {limit}",
            limit = LIST_LIMIT + 1
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let map = |row: &Row<'_>| map_edge(row);
        let rows = if let Some(provenance) = provenance {
            stmt.query_map(params![symbol_id, kind.as_str(), provenance.as_str()], map)?
        } else {
            stmt.query_map(params![symbol_id, kind.as_str()], map)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    fn count_callers(&self, symbol_id: &str) -> Result<i64, CoreError> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM edges
                 WHERE to_symbol_id = ?1 AND kind = 'calls' AND provenance = 'exact'",
                params![symbol_id],
                |row| row.get(0),
            )
            .map_err(CoreError::from)
    }

    fn summary_body(
        &self,
        project_id: &str,
        level: SummaryLevel,
        owner_id: &str,
    ) -> Result<String, CoreError> {
        self.conn
            .query_row(
                "SELECT body FROM summaries
                 WHERE project_id = ?1 AND level = ?2 AND owner_id = ?3",
                params![project_id, level.as_str(), owner_id],
                |row| row.get(0),
            )
            .optional()
            .map(|body| body.unwrap_or_default())
            .map_err(CoreError::from)
    }

    fn touch_project(&mut self, project_id: &str) -> Result<(), CoreError> {
        self.conn.execute(
            "UPDATE projects SET updated_at = ?1 WHERE id = ?2",
            params![now_rfc3339(), project_id],
        )?;
        Ok(())
    }

    fn rebuild_project_summary(&mut self, project_id: &str) -> Result<(), CoreError> {
        let Some(project) = self.get_project(project_id)? else {
            return Ok(());
        };
        let sources = self.list_sources(project_id)?;
        let symbols: i64 = sources.iter().map(|source| source.symbol_count).sum();
        let exact: i64 = sources.iter().map(|source| source.exact_call_count).sum();
        let unresolved: i64 = sources
            .iter()
            .map(|source| source.unresolved_call_count)
            .sum();
        let mut body = format!(
            "project {}\nsources {} | symbols {symbols} | calls exact {exact} | unresolved {unresolved}\nread order: project, source, module, symbol\n",
            project.name,
            sources.len()
        );
        for source in &sources {
            body.push_str(&format!(
                "source {} {} symbols {} calls exact {} unresolved {}\n",
                source.name,
                source.status,
                source.symbol_count,
                source.exact_call_count,
                source.unresolved_call_count
            ));
        }
        let tokens = forgeline_graph::token_estimate(&body);
        let now = now_rfc3339();
        self.conn.execute(
            "DELETE FROM summaries WHERE project_id = ?1 AND level = 'project'",
            params![project_id],
        )?;
        self.conn.execute(
            "INSERT INTO summaries (
                id, project_id, source_id, level, owner_id, body, token_estimate, updated_at
            ) VALUES (?1, ?2, NULL, 'project', ?2, ?3, ?4, ?5)",
            params![
                format!("project:{project_id}"),
                project_id,
                body,
                tokens,
                now
            ],
        )?;
        Ok(())
    }
}

const SOURCE_SELECT: &str = "SELECT id, project_id, name, kind, location, status, status_message,
    file_count, symbol_count, exact_call_count, inferred_call_count, unresolved_call_count,
    ambiguous_call_count, reindexed_files, reused_files, removed_files, skipped_files,
    last_indexed_at, created_at, updated_at FROM sources";

fn map_project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

fn map_source(row: &Row<'_>) -> rusqlite::Result<Source> {
    Ok(Source {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        kind: row.get(3)?,
        location: row.get(4)?,
        status: row.get(5)?,
        status_message: row.get(6)?,
        file_count: row.get(7)?,
        symbol_count: row.get(8)?,
        exact_call_count: row.get(9)?,
        inferred_call_count: row.get(10)?,
        unresolved_call_count: row.get(11)?,
        ambiguous_call_count: row.get(12)?,
        reindexed_files: row.get(13)?,
        reused_files: row.get(14)?,
        removed_files: row.get(15)?,
        skipped_files: row.get(16)?,
        last_indexed_at: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}

fn map_symbol(row: &Row<'_>) -> rusqlite::Result<SymbolHit> {
    let start_line: i64 = row.get(8)?;
    let end_line: i64 = row.get(9)?;
    Ok(SymbolHit {
        id: row.get(0)?,
        source_id: row.get(1)?,
        source_name: row.get(2)?,
        name: row.get(3)?,
        qualified_name: row.get(4)?,
        kind: row.get(5)?,
        language: row.get(6)?,
        relative_path: row.get(7)?,
        start_line: u32::try_from(start_line).unwrap_or(0),
        end_line: u32::try_from(end_line).unwrap_or(0),
        summary: row.get(10)?,
    })
}

fn map_edge(row: &Row<'_>) -> rusqlite::Result<EdgeHit> {
    let line: i64 = row.get(4)?;
    let symbol_line: i64 = row.get(9)?;
    Ok(EdgeHit {
        provenance: row.get(0)?,
        confidence: row.get(1)?,
        detail: row.get(2)?,
        evidence_path: row.get(3)?,
        evidence_line: u32::try_from(line).unwrap_or(0),
        symbol_name: row.get(5)?,
        symbol_qualified_name: row.get(6)?,
        symbol_kind: row.get(7)?,
        symbol_path: row.get(8)?,
        symbol_line: u32::try_from(symbol_line).unwrap_or(0),
        source_name: row.get(10)?,
    })
}

fn normalize_location(kind: &str, location: &str) -> Result<String, CoreError> {
    match kind {
        KIND_LOCAL_PATH => {
            let path = Path::new(location.trim());
            if !path.exists() {
                return Err(CoreError::PathMissing(path.to_path_buf()));
            }
            if !path.is_dir() {
                return Err(CoreError::NotADirectory(path.to_path_buf()));
            }
            let canonical = dunce::canonicalize(path)?;
            Ok(canonical.to_string_lossy().to_string())
        }
        KIND_GIT_URL => {
            let url = location.trim();
            validate_git_url(url)?;
            Ok(url.to_string())
        }
        other => Err(CoreError::UnknownKind(other.to_string())),
    }
}
