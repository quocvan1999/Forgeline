use std::path::{Path, PathBuf};

/// App data directory. `FORGELINE_DATA_DIR` wins so tests and the CLI can
/// point at a scratch folder without touching the real profile.
pub fn default_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("FORGELINE_DATA_DIR") {
        return PathBuf::from(dir);
    }
    directories::BaseDirs::new()
        .map(|dirs| dirs.data_dir().join("forgeline"))
        .unwrap_or_else(|| PathBuf::from(".forgeline"))
}

pub fn database_path(data_dir: &Path) -> PathBuf {
    data_dir.join("forgeline.db")
}

pub fn project_dir(data_dir: &Path, project_id: &str) -> PathBuf {
    data_dir.join("projects").join(project_id)
}

pub fn source_checkout_dir(data_dir: &Path, project_id: &str, source_id: &str) -> PathBuf {
    project_dir(data_dir, project_id)
        .join("sources")
        .join(source_id)
        .join("checkout")
}
