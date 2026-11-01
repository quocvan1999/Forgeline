use serde::Serialize;

pub const KIND_LOCAL_PATH: &str = "local_path";
pub const KIND_GIT_URL: &str = "git_url";

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_INDEXING: &str = "indexing";
pub const STATUS_READY: &str = "ready";
pub const STATUS_ERROR: &str = "error";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub kind: String,
    pub location: String,
    pub status: String,
    pub status_message: Option<String>,
    pub file_count: i64,
    pub symbol_count: i64,
    pub exact_call_count: i64,
    pub inferred_call_count: i64,
    pub unresolved_call_count: i64,
    pub ambiguous_call_count: i64,
    pub reindexed_files: i64,
    pub reused_files: i64,
    pub removed_files: i64,
    pub skipped_files: i64,
    pub last_indexed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStats {
    pub project_id: String,
    pub project_name: String,
    pub summary: String,
    pub source_count: i64,
    pub symbol_count: i64,
    pub exact_call_count: i64,
    pub inferred_call_count: i64,
    pub unresolved_call_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolHit {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub language: String,
    pub relative_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeHit {
    pub provenance: String,
    pub confidence: f64,
    pub detail: String,
    pub evidence_path: String,
    pub evidence_line: u32,
    pub symbol_name: String,
    pub symbol_qualified_name: String,
    pub symbol_kind: String,
    pub symbol_path: String,
    pub symbol_line: u32,
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolReport {
    pub symbol: SymbolHit,
    pub outgoing_calls: Vec<EdgeHit>,
    pub exact_caller_count: i64,
    pub inferred_caller_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CallerGroup {
    pub symbol: SymbolHit,
    pub callers: Vec<EdgeHit>,
    pub hidden_inferred: i64,
    pub truncated: bool,
}

pub(crate) fn now_rfc3339() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
