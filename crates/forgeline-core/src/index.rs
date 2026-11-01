use std::path::Path;

use forgeline_graph::index_path;

use crate::git::ensure_managed_checkout;
use crate::model::{Source, KIND_GIT_URL, KIND_LOCAL_PATH, STATUS_ERROR, STATUS_INDEXING};
use crate::paths::source_checkout_dir;
use crate::store::Store;
use crate::CoreError;

/// Index one source in place. Local paths are read as given. Git URLs are
/// cloned once into the app data directory with the system `git` binary.
pub fn index_source(
    store: &mut Store,
    data_dir: &Path,
    source_id: &str,
) -> Result<Source, CoreError> {
    let source = store
        .get_source(source_id)?
        .ok_or(CoreError::SourceNotFound)?;
    store.set_status(source_id, STATUS_INDEXING, None)?;
    match index_ready(store, data_dir, &source) {
        Ok(()) => store
            .get_source(source_id)?
            .ok_or(CoreError::SourceNotFound),
        Err(error) => {
            let _ = store.set_status(source_id, STATUS_ERROR, Some(&error.to_string()));
            Err(error)
        }
    }
}

fn index_ready(store: &mut Store, data_dir: &Path, source: &Source) -> Result<(), CoreError> {
    let root = match source.kind.as_str() {
        KIND_LOCAL_PATH => {
            let path = Path::new(&source.location);
            if !path.is_dir() {
                return Err(CoreError::NotADirectory(path.to_path_buf()));
            }
            path.to_path_buf()
        }
        KIND_GIT_URL => {
            let checkout = source_checkout_dir(data_dir, &source.project_id, &source.id);
            ensure_managed_checkout(&source.location, &checkout)?;
            checkout
        }
        other => return Err(CoreError::UnknownKind(other.to_string())),
    };
    let previous = store.load_extractions(&source.id)?;
    let graph = index_path(&root, &source.id, &source.name, &previous)?;
    store.save_graph(&source.project_id, &source.id, &graph)?;
    Ok(())
}
