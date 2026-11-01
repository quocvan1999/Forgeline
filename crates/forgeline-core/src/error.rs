use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("graph error: {0}")]
    Graph(#[from] forgeline_graph::GraphError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("project not found")]
    ProjectNotFound,
    #[error("more than one project; pass --project <name>")]
    ProjectAmbiguous,
    #[error("source not found")]
    SourceNotFound,
    #[error("a project named {0:?} already exists")]
    DuplicateName(String),
    #[error("invalid name")]
    InvalidName,
    #[error("invalid git url")]
    InvalidGitUrl,
    #[error("unknown source kind {0:?}")]
    UnknownKind(String),
    #[error("path does not exist: {0}")]
    PathMissing(PathBuf),
    #[error("path is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("git clone failed: {0}")]
    GitClone(String),
}
