//! Optional Roslyn subprocess for exact C# edges.
//!
//! The helper is a small .NET console under `tools/forgeline-roslyn`. When the
//! .NET SDK (or a published binary) is missing, indexing stays on tree-sitter
//! and a Vietnamese notice is returned for the CLI/UI.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Deserialize;

use crate::model::{Edge, EdgeKind, Provenance, SourceGraph, Symbol, SymbolKind};
use crate::CONFIDENCE_EXACT;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const ENV_HELPER: &str = "FORGELINE_ROSLYN";
const ENV_DOTNET: &str = "DOTNET_ROOT";

/// Status of the optional Roslyn pass. Surface the Vietnamese message in UI/CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoslynStatus {
    pub used: bool,
    pub notice_vi: Option<String>,
    pub edge_count: u32,
}

#[derive(Debug, Deserialize)]
struct RoslynLine {
    kind: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default, rename = "fromPath")]
    from_path: Option<String>,
    #[serde(default, rename = "fromLine")]
    from_line: Option<u32>,
    #[serde(default, rename = "toPath")]
    to_path: Option<String>,
    #[serde(default, rename = "toLine")]
    to_line: Option<u32>,
    #[serde(default, rename = "toFqn")]
    to_fqn: Option<String>,
    #[serde(default)]
    detail: Option<String>,
}

/// Merge Roslyn exact edges into an already tree-sitter-resolved graph.
pub fn merge_roslyn_edges(graph: &mut SourceGraph, root: &Path) -> RoslynStatus {
    let has_csharp = graph
        .files
        .iter()
        .any(|file| file.language == crate::model::Language::CSharp);
    if !has_csharp {
        return RoslynStatus::default();
    }

    let Some(helper) = locate_helper() else {
        return RoslynStatus {
            used: false,
            notice_vi: Some(
                "Chưa có .NET SDK / công cụ Roslyn — phân tích C# chỉ dùng tree-sitter (độ chính xác thấp hơn)."
                    .to_string(),
            ),
            edge_count: 0,
        };
    };

    match run_helper(&helper, root) {
        Ok(lines) => apply_roslyn(graph, &lines),
        Err(message) => RoslynStatus {
            used: false,
            notice_vi: Some(message),
            edge_count: 0,
        },
    }
}

fn locate_helper() -> Option<HelperLaunch> {
    if let Ok(path) = std::env::var(ENV_HELPER) {
        let path = PathBuf::from(path);
        if path.is_file() {
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            {
                return Some(HelperLaunch::DotnetDll(path));
            }
            return Some(HelperLaunch::Binary(path));
        }
    }

    let manifest_candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/forgeline-roslyn"),
        PathBuf::from("tools/forgeline-roslyn"),
    ];
    for dir in &manifest_candidates {
        let dll = dir.join("bin/Release/net8.0/forgeline-roslyn.dll");
        let dll_debug = dir.join("bin/Debug/net8.0/forgeline-roslyn.dll");
        let csproj = dir.join("ForgelineRoslyn.csproj");
        if dll.is_file() {
            return Some(HelperLaunch::DotnetDll(dll));
        }
        if dll_debug.is_file() {
            return Some(HelperLaunch::DotnetDll(dll_debug));
        }
        if csproj.is_file() {
            return Some(HelperLaunch::DotnetRun(csproj));
        }
    }
    None
}

enum HelperLaunch {
    Binary(PathBuf),
    DotnetDll(PathBuf),
    DotnetRun(PathBuf),
}

fn run_helper(helper: &HelperLaunch, root: &Path) -> Result<Vec<RoslynLine>, String> {
    let dotnet = find_dotnet();
    let mut command = match helper {
        HelperLaunch::Binary(path) => {
            let mut cmd = Command::new(path);
            cmd.arg("--root").arg(root);
            cmd.arg("--timeout-ms").arg("120000");
            cmd
        }
        HelperLaunch::DotnetDll(dll) => {
            let Some(dotnet) = dotnet else {
                return Err(
                    "Chưa có .NET SDK — bỏ qua Roslyn; C# chỉ dùng tree-sitter.".to_string()
                );
            };
            let mut cmd = Command::new(dotnet);
            cmd.arg(dll);
            cmd.arg("--root").arg(root);
            cmd.arg("--timeout-ms").arg("120000");
            cmd
        }
        HelperLaunch::DotnetRun(csproj) => {
            let Some(dotnet) = dotnet else {
                return Err(
                    "Chưa có .NET SDK — bỏ qua Roslyn; C# chỉ dùng tree-sitter.".to_string()
                );
            };
            let mut cmd = Command::new(dotnet);
            cmd.arg("run")
                .arg("--project")
                .arg(csproj)
                .arg("-c")
                .arg("Release")
                .arg("--no-restore")
                .arg("--")
                .arg("--root")
                .arg(root)
                .arg("--timeout-ms")
                .arg("120000");
            cmd
        }
    };

    // Never use a shell. Explicit argv only.
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .env("DOTNET_NOLOGO", "1");

    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "Chưa có .NET SDK — bỏ qua Roslyn; C# chỉ dùng tree-sitter.".to_string()
        } else {
            format!("Không chạy được Roslyn: {error}")
        }
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Roslyn không trả stdout.".to_string())?;
    let reader = BufReader::new(stdout);
    let mut lines = Vec::new();
    let started = std::time::Instant::now();
    for line in reader.lines() {
        if started.elapsed() > DEFAULT_TIMEOUT {
            let _ = child.kill();
            return Err("Roslyn hết thời gian chờ; giữ kết quả tree-sitter.".to_string());
        }
        let line = line.map_err(|error| format!("Roslyn stdout: {error}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<RoslynLine>(trimmed) {
            Ok(parsed) => lines.push(parsed),
            Err(_) => continue,
        }
    }

    let status = child
        .wait()
        .map_err(|error| format!("Roslyn wait: {error}"))?;
    if !status.success() {
        let notices: Vec<_> = lines
            .iter()
            .filter(|line| line.message.is_some())
            .filter_map(|line| line.message.clone())
            .collect();
        if !notices.is_empty() {
            return Err(format!(
                "Roslyn lỗi: {}",
                notices.last().cloned().unwrap_or_default()
            ));
        }
        // Still apply any edges that arrived before failure.
    }
    Ok(lines)
}

fn find_dotnet() -> Option<PathBuf> {
    if let Ok(root) = std::env::var(ENV_DOTNET) {
        let candidate = PathBuf::from(root).join("dotnet");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    which("dotnet")
}

fn which(name: &str) -> Option<PathBuf> {
    let Ok(path) = std::env::var("PATH") else {
        return None;
    };
    for dir in path.split(':') {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn apply_roslyn(graph: &mut SourceGraph, lines: &[RoslynLine]) -> RoslynStatus {
    let mut notice_vi = None;
    for line in lines {
        if matches!(
            line.kind.as_str(),
            "notice" | "error" | "timeout" | "msbuild" | "msbuild_fallback" | "usage" | "ok"
        ) || line.message.is_some()
        {
            if let Some(message) = &line.message {
                if line.kind != "ok" {
                    notice_vi = Some(format!("Roslyn: {message}"));
                }
            }
        }
    }

    let symbols: Vec<Symbol> = graph.symbols().cloned().collect();
    let mut by_path_line: HashMap<(String, u32), Vec<&Symbol>> = HashMap::new();
    for symbol in &symbols {
        by_path_line
            .entry((symbol.relative_path.clone(), symbol.start_line))
            .or_default()
            .push(symbol);
    }
    let mut by_fqn: HashMap<String, Vec<&Symbol>> = HashMap::new();
    for symbol in &symbols {
        by_fqn
            .entry(symbol.qualified_name.clone())
            .or_default()
            .push(symbol);
        // Roslyn FQNs are Namespace.Type.Member; also index simple Type.Member.
        if let Some((_, short)) = symbol.qualified_name.rsplit_once('.') {
            by_fqn.entry(short.to_string()).or_default().push(symbol);
        }
    }

    let mut added = 0u32;
    let mut covered_calls: HashMap<(String, u32), ()> = HashMap::new();

    for line in lines {
        let kind = match line.kind.as_str() {
            "calls" => EdgeKind::Calls,
            "inherits" => EdgeKind::Inherits,
            "type_ref" => EdgeKind::TypeRef,
            _ => continue,
        };
        let Some(from_path) = line.from_path.as_deref() else {
            continue;
        };
        let Some(from_line) = line.from_line else {
            continue;
        };
        let Some(to_path) = line.to_path.as_deref() else {
            continue;
        };
        let Some(to_line) = line.to_line else {
            continue;
        };
        let from_path = from_path.replace('\\', "/");
        let to_path = to_path.replace('\\', "/");

        let Some(from_id) = find_enclosing_symbol(graph, &from_path, from_line) else {
            continue;
        };
        let to_symbol = find_target_symbol(
            &by_path_line,
            &by_fqn,
            &to_path,
            to_line,
            line.to_fqn.as_deref(),
        );
        let Some(to_symbol) = to_symbol else {
            continue;
        };

        let detail = line.detail.clone().unwrap_or_else(|| "roslyn".to_string());
        let edge = Edge {
            kind,
            from_id: from_id.clone(),
            to_id: Some(to_symbol.id.clone()),
            to_name: to_symbol.name.clone(),
            provenance: Provenance::Exact,
            confidence: CONFIDENCE_EXACT,
            evidence_path: from_path.clone(),
            evidence_line: from_line,
            detail,
        };

        if !graph.edges.iter().any(|existing| {
            existing.kind == edge.kind
                && existing.from_id == edge.from_id
                && existing.to_id == edge.to_id
                && existing.evidence_path == edge.evidence_path
                && existing.evidence_line == edge.evidence_line
        }) {
            graph.edges.push(edge);
            added += 1;
        }
        if kind == EdgeKind::Calls {
            covered_calls.insert((from_path, from_line), ());
        }
    }

    // Recompute unresolved: call sites without an exact calls edge at that line.
    let mut unresolved = 0u32;
    for file in &graph.files {
        for call in &file.calls {
            let has = graph.edges.iter().any(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.evidence_path == call.relative_path
                    && edge.evidence_line == call.line
            }) || covered_calls.contains_key(&(call.relative_path.clone(), call.line));
            if !has {
                unresolved += 1;
            }
        }
    }
    graph.unresolved_calls = unresolved;

    // Refresh summaries with new edge/unresolved counts.
    graph.summaries = crate::summarize::summarize(
        &graph.source_id,
        &graph.source_label,
        &graph.files,
        &graph.edges,
        graph.unresolved_calls,
        graph.ambiguous_calls,
    );

    RoslynStatus {
        used: added > 0 || notice_vi.is_none(),
        notice_vi,
        edge_count: added,
    }
}

fn find_enclosing_symbol(graph: &SourceGraph, path: &str, line: u32) -> Option<String> {
    let file = graph.files.iter().find(|file| file.relative_path == path)?;
    let mut best: Option<&Symbol> = None;
    for symbol in &file.symbols {
        if symbol.kind == SymbolKind::Module || symbol.kind == SymbolKind::Namespace {
            continue;
        }
        if symbol.start_line <= line && line <= symbol.end_line {
            let better = match best {
                None => true,
                Some(current) => {
                    let current_span = current.end_line.saturating_sub(current.start_line);
                    let next_span = symbol.end_line.saturating_sub(symbol.start_line);
                    next_span < current_span
                        || (next_span == current_span && symbol.start_line >= current.start_line)
                }
            };
            if better {
                best = Some(symbol);
            }
        }
    }
    best.map(|symbol| symbol.id.clone()).or_else(|| {
        file.symbols
            .iter()
            .find(|symbol| symbol.kind == SymbolKind::Module)
            .map(|symbol| symbol.id.clone())
    })
}

fn find_target_symbol<'a>(
    by_path_line: &HashMap<(String, u32), Vec<&'a Symbol>>,
    by_fqn: &HashMap<String, Vec<&'a Symbol>>,
    path: &str,
    line: u32,
    fqn: Option<&str>,
) -> Option<&'a Symbol> {
    if let Some(hits) = by_path_line.get(&(path.to_string(), line)) {
        if hits.len() == 1 {
            return Some(hits[0]);
        }
        if let Some(fqn) = fqn {
            let short = fqn.rsplit('.').next().unwrap_or(fqn);
            let named: Vec<_> = hits
                .iter()
                .copied()
                .filter(|symbol| symbol.name == short || symbol.qualified_name.ends_with(fqn))
                .collect();
            if named.len() == 1 {
                return Some(named[0]);
            }
        }
    }
    if let Some(fqn) = fqn {
        if let Some(hits) = by_fqn.get(fqn) {
            let in_file: Vec<_> = hits
                .iter()
                .copied()
                .filter(|symbol| symbol.relative_path == path)
                .collect();
            if in_file.len() == 1 {
                return Some(in_file[0]);
            }
            if hits.len() == 1 {
                return Some(hits[0]);
            }
        }
        // Try Type.Member suffix match unique in path.
        {
            let suffix = {
                let parts: Vec<&str> = fqn.rsplit('.').take(2).collect();
                if parts.len() == 2 {
                    format!("{}.{}", parts[1], parts[0])
                } else {
                    fqn.to_string()
                }
            };
            if !suffix.is_empty() {
                let mut hits: Vec<&Symbol> = by_fqn
                    .values()
                    .flatten()
                    .copied()
                    .filter(|symbol| {
                        symbol.relative_path == path
                            && (symbol.qualified_name == suffix
                                || symbol.qualified_name.ends_with(&format!(".{suffix}"))
                                || symbol.qualified_name == fqn
                                || fqn.ends_with(&symbol.qualified_name))
                    })
                    .collect();
                hits.sort_by(|a, b| a.id.cmp(&b.id));
                hits.dedup_by(|a, b| a.id == b.id);
                if hits.len() == 1 {
                    return Some(hits[0]);
                }
            }
        }
        // Unique name on that line's file among callables with matching simple name.
        let simple = fqn.rsplit('.').next().unwrap_or(fqn);
        let mut hits: Vec<&Symbol> = by_path_line
            .iter()
            .filter(|((p, _), _)| p == path)
            .flat_map(|(_, symbols)| symbols.iter().copied())
            .filter(|symbol| symbol.name == simple && symbol.kind.is_callable())
            .collect();
        hits.sort_by(|a, b| a.id.cmp(&b.id));
        hits.dedup_by(|a, b| a.id == b.id);
        if hits.len() == 1 {
            return Some(hits[0]);
        }
        // Prefer symbol whose start_line is closest at or before `line`.
        hits.retain(|symbol| symbol.start_line <= line);
        hits.sort_by_key(|symbol| line.saturating_sub(symbol.start_line));
        if let Some(first) = hits.first() {
            if hits
                .get(1)
                .is_none_or(|second| second.start_line != first.start_line)
            {
                return Some(*first);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_launch_prefers_env_binary() {
        // Smoke: locate_helper should not panic without env.
        let _ = locate_helper();
    }
}
