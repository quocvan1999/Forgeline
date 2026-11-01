use rusqlite::Connection;

use crate::model::now_rfc3339;
use crate::CoreError;

const MIGRATION_1: &str = r#"
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_projects_name_nocase ON projects(name COLLATE NOCASE);

CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    location TEXT NOT NULL,
    status TEXT NOT NULL,
    status_message TEXT,
    file_count INTEGER NOT NULL DEFAULT 0,
    symbol_count INTEGER NOT NULL DEFAULT 0,
    exact_call_count INTEGER NOT NULL DEFAULT 0,
    inferred_call_count INTEGER NOT NULL DEFAULT 0,
    unresolved_call_count INTEGER NOT NULL DEFAULT 0,
    ambiguous_call_count INTEGER NOT NULL DEFAULT 0,
    reindexed_files INTEGER NOT NULL DEFAULT 0,
    reused_files INTEGER NOT NULL DEFAULT 0,
    removed_files INTEGER NOT NULL DEFAULT 0,
    skipped_files INTEGER NOT NULL DEFAULT 0,
    last_indexed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_sources_project ON sources(project_id, location);

CREATE TABLE file_extractions (
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    language TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (source_id, relative_path)
);

CREATE TABLE symbols (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    parent_id TEXT,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL,
    kind TEXT NOT NULL,
    language TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    start_column INTEGER NOT NULL,
    end_column INTEGER NOT NULL
);

CREATE INDEX idx_symbols_source_name ON symbols(source_id, name);
CREATE INDEX idx_symbols_source_qualified ON symbols(source_id, qualified_name);

CREATE TABLE edges (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    from_symbol_id TEXT NOT NULL,
    to_symbol_id TEXT,
    to_name TEXT NOT NULL,
    provenance TEXT NOT NULL,
    confidence REAL NOT NULL,
    evidence_path TEXT NOT NULL,
    evidence_line INTEGER NOT NULL,
    detail TEXT NOT NULL
);

CREATE INDEX idx_edges_to ON edges(project_id, kind, to_symbol_id, provenance);
CREATE INDEX idx_edges_from ON edges(project_id, from_symbol_id);

CREATE TABLE summaries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_id TEXT REFERENCES sources(id) ON DELETE CASCADE,
    level TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    body TEXT NOT NULL,
    token_estimate INTEGER NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_summaries_owner ON summaries(project_id, owner_id);
CREATE INDEX idx_summaries_level ON summaries(project_id, level);
"#;

const MIGRATION_2: &str = r#"
CREATE TABLE projects_nocase (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
INSERT INTO projects_nocase (id, name, created_at, updated_at)
SELECT id, name, created_at, updated_at FROM projects;
DROP TABLE projects;
ALTER TABLE projects_nocase RENAME TO projects;
CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_name_nocase ON projects(name COLLATE NOCASE);
"#;

pub(crate) fn migrate(conn: &mut Connection) -> Result<(), CoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if current < 1 {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATION_1)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
            [now_rfc3339()],
        )?;
        tx.commit()?;
    }
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if current < 2 {
        let tx = conn.transaction()?;
        // Fresh installs already have the NOCASE index from migration 1.
        let has_index: bool = tx
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_projects_name_nocase'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if !has_index {
            tx.execute_batch(MIGRATION_2)?;
        }
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (2, ?1)",
            [now_rfc3339()],
        )?;
        tx.commit()?;
    }
    Ok(())
}
