use std::collections::{HashMap, HashSet};
use std::path::Path;

use walkdir::WalkDir;

use crate::extract::extract_file;
use crate::model::{language_for_path, ExtractedFile, SourceGraph};
use crate::resolve::{resolve_graph, resolve_graph_at};
use crate::GraphError;
use crate::{EXTRACTOR_VERSION, MAX_FILE_BYTES, MAX_SOURCE_FILES};

const SKIP_DIRECTORIES: &[&str] = &[
    ".git",
    "node_modules",
    "dist",
    "build",
    "target",
    "venv",
    ".venv",
    "__pycache__",
    "vendor",
    ".next",
    "coverage",
    "out",
    ".idea",
    ".vscode",
    ".tox",
    "site-packages",
];

/// One in-memory file. Paths use `/` separators and are relative to the source root.
pub struct FileInput<'a> {
    pub relative_path: &'a str,
    pub contents: &'a str,
}

/// Index buffers. Unchanged files are copied from `previous` when the hash and
/// extractor version match, then edges are resolved again for the whole source.
pub fn index_contents(
    source_id: &str,
    source_label: &str,
    inputs: &[FileInput<'_>],
    previous: &[ExtractedFile],
) -> Result<SourceGraph, GraphError> {
    let (files, reindexed_files, reused_files, removed_files) =
        extract_inputs(source_id, inputs, previous)?;
    Ok(resolve_graph(
        source_id,
        source_label,
        files,
        reindexed_files,
        reused_files,
        removed_files,
        0,
    ))
}

/// Like [`index_contents`] but resolves JS/TS against `root` configs and runs Roslyn for C#.
pub fn index_contents_at(
    root: &Path,
    source_id: &str,
    source_label: &str,
    inputs: &[FileInput<'_>],
    previous: &[ExtractedFile],
) -> Result<SourceGraph, GraphError> {
    let (files, reindexed_files, reused_files, removed_files) =
        extract_inputs(source_id, inputs, previous)?;
    Ok(resolve_graph_at(
        Some(root),
        source_id,
        source_label,
        files,
        reindexed_files,
        reused_files,
        removed_files,
        0,
    ))
}

/// Walk `root` without following symlinks and index supported sources.
pub fn index_path(
    root: &Path,
    source_id: &str,
    source_label: &str,
    previous: &[ExtractedFile],
) -> Result<SourceGraph, GraphError> {
    if !root.exists() {
        return Err(GraphError::Missing(root.to_path_buf()));
    }
    if !root.is_dir() {
        return Err(GraphError::NotADirectory(root.to_path_buf()));
    }
    if root.parent().is_none() {
        return Err(GraphError::FilesystemRoot(root.to_path_buf()));
    }

    let mut collected: Vec<(String, String)> = Vec::new();
    let mut skipped_files = 0u32;
    let walker = WalkDir::new(root).follow_links(false).into_iter();
    let walker = walker.filter_entry(keep_directory);
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                skipped_files += 1;
                continue;
            }
        };
        if entry.depth() == 0 || entry.file_type().is_dir() {
            continue;
        }
        if entry.file_type().is_symlink() {
            skipped_files += 1;
            continue;
        }
        let relative = match relative_slash(root, entry.path()) {
            Some(relative) => relative,
            None => continue,
        };
        if language_for_path(&relative).is_none() {
            continue;
        }
        let bytes = match entry.metadata() {
            Ok(metadata) => metadata.len(),
            Err(_) => {
                skipped_files += 1;
                continue;
            }
        };
        if bytes > MAX_FILE_BYTES {
            skipped_files += 1;
            continue;
        }
        let bytes = match std::fs::read(entry.path()) {
            Ok(bytes) => bytes,
            Err(_) => {
                skipped_files += 1;
                continue;
            }
        };
        let Ok(contents) = String::from_utf8(bytes) else {
            skipped_files += 1;
            continue;
        };
        if collected.len() >= MAX_SOURCE_FILES {
            return Err(GraphError::TooManyFiles {
                limit: MAX_SOURCE_FILES,
            });
        }
        collected.push((relative, contents));
    }
    collected.sort_by(|left, right| left.0.cmp(&right.0));

    let inputs: Vec<FileInput<'_>> = collected
        .iter()
        .map(|(path, contents)| FileInput {
            relative_path: path,
            contents,
        })
        .collect();
    let mut graph = index_contents_at(root, source_id, source_label, &inputs, previous)?;
    graph.skipped_files = skipped_files;
    Ok(graph)
}

fn extract_inputs(
    source_id: &str,
    inputs: &[FileInput<'_>],
    previous: &[ExtractedFile],
) -> Result<(Vec<ExtractedFile>, u32, u32, u32), GraphError> {
    if inputs.len() > MAX_SOURCE_FILES {
        return Err(GraphError::TooManyFiles {
            limit: MAX_SOURCE_FILES,
        });
    }
    let mut previous_by_path: HashMap<&str, &ExtractedFile> = HashMap::new();
    for file in previous {
        previous_by_path.insert(file.relative_path.as_str(), file);
    }

    let mut files = Vec::with_capacity(inputs.len());
    let mut reindexed_files = 0u32;
    let mut reused_files = 0u32;
    for input in inputs {
        let relative_path = input.relative_path.replace('\\', "/");
        if let Some(prev) = previous_by_path.get(relative_path.as_str()) {
            let hash = crate::content_hash(input.contents.as_bytes());
            let same_source = prev
                .symbols
                .iter()
                .all(|symbol| symbol.id.starts_with(&format!("{source_id}::")));
            if same_source
                && prev.extractor_version == EXTRACTOR_VERSION
                && prev.content_hash == hash
            {
                files.push((*prev).clone());
                reused_files += 1;
                continue;
            }
        }
        files.push(extract_file(source_id, &relative_path, input.contents)?);
        reindexed_files += 1;
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    let current: HashSet<&str> = files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect();
    let removed_files = u32::try_from(
        previous
            .iter()
            .filter(|file| !current.contains(file.relative_path.as_str()))
            .count(),
    )
    .unwrap_or(u32::MAX);

    Ok((files, reindexed_files, reused_files, removed_files))
}

fn keep_directory(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    // Symlinks are yielded (and later skipped) but never descended into.
    if entry.file_type().is_symlink() || !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    if name.starts_with('.') {
        return false;
    }
    !SKIP_DIRECTORIES.contains(&name.as_ref())
}

fn relative_slash(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}
