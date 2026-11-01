//! Per-source overview graph: folder / namespace / file nodes with aggregated
//! exact call counts. Readable for 10k+ symbol repos; symbol drill-down is
//! available under a selected file.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Row};
use serde::Serialize;

use crate::error::CoreError;
use crate::store::Store;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OverviewGraph {
    pub source_id: String,
    pub source_name: String,
    pub level: String,
    pub parent_id: Option<String>,
    pub nodes: Vec<OverviewNode>,
    pub edges: Vec<OverviewEdge>,
    pub exact_call_count: i64,
    pub unresolved_call_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OverviewNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: Option<String>,
    pub symbol_count: i64,
    pub exact_out: i64,
    pub exact_in: i64,
    /// Layout hint in \[0,1\] for the bundled renderer.
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OverviewEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub weight: i64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolNeighbors {
    pub symbol_id: String,
    pub callers: Vec<NeighborHit>,
    pub callees: Vec<NeighborHit>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NeighborHit {
    pub symbol_id: String,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub relative_path: String,
    pub line: u32,
    pub evidence_path: String,
    pub evidence_line: u32,
    pub detail: String,
}

impl Store {
    /// Overview for one source. `focus` is a folder path, file path, or `None` for root folders.
    pub fn source_overview(
        &self,
        source_id: &str,
        focus: Option<&str>,
        kind_filter: Option<&str>,
        search: Option<&str>,
    ) -> Result<OverviewGraph, CoreError> {
        let source = self
            .get_source(source_id)?
            .ok_or(CoreError::SourceNotFound)?;

        let symbols = self.load_overview_symbols(source_id, kind_filter, search)?;
        let calls = self.load_exact_calls(source_id)?;

        let focus = focus.map(str::trim).filter(|value| !value.is_empty());
        match focus {
            None => Ok(folder_overview(&source.id, &source.name, &symbols, &calls)),
            Some(path) if looks_like_file(path) => Ok(file_symbol_overview(
                &source.id,
                &source.name,
                path,
                &symbols,
                &calls,
            )),
            Some(folder) => Ok(folder_children_overview(
                &source.id,
                &source.name,
                folder,
                &symbols,
                &calls,
            )),
        }
        .map(|mut graph| {
            graph.exact_call_count = source.exact_call_count;
            graph.unresolved_call_count = source.unresolved_call_count;
            graph
        })
    }

    pub fn symbol_neighbors(&self, symbol_id: &str) -> Result<SymbolNeighbors, CoreError> {
        let callers = self.neighbor_edges(
            "e.to_symbol_id = ?1",
            symbol_id,
            "c.id, c.name, c.qualified_name, c.kind, c.relative_path, c.start_line",
            "c",
        )?;
        let callees = self.neighbor_edges(
            "e.from_symbol_id = ?1",
            symbol_id,
            "t.id, t.name, t.qualified_name, t.kind, t.relative_path, t.start_line",
            "t",
        )?;
        Ok(SymbolNeighbors {
            symbol_id: symbol_id.to_string(),
            callers,
            callees,
        })
    }

    fn load_overview_symbols(
        &self,
        source_id: &str,
        kind_filter: Option<&str>,
        search: Option<&str>,
    ) -> Result<Vec<OverviewSymbol>, CoreError> {
        let mut sql = String::from(
            "SELECT id, name, qualified_name, kind, relative_path, start_line
             FROM symbols
             WHERE source_id = ?1 AND kind != 'module'",
        );
        let mut bind_kind = None;
        let mut bind_search = None;
        if let Some(kind) = kind_filter.map(str::trim).filter(|value| !value.is_empty()) {
            sql.push_str(" AND kind = ?2");
            bind_kind = Some(kind.to_string());
        }
        if let Some(query) = search.map(str::trim).filter(|value| !value.is_empty()) {
            let placeholder = if bind_kind.is_some() { "?3" } else { "?2" };
            sql.push_str(&format!(
                " AND (name LIKE {placeholder} OR qualified_name LIKE {placeholder} OR relative_path LIKE {placeholder})"
            ));
            bind_search = Some(format!("%{query}%"));
        }
        sql.push_str(" ORDER BY relative_path, start_line, name");

        let mut stmt = self.conn().prepare(&sql)?;
        let map = |row: &Row<'_>| -> rusqlite::Result<OverviewSymbol> {
            Ok(OverviewSymbol {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(3)?,
                relative_path: row.get(4)?,
            })
        };
        let rows = match (bind_kind.as_deref(), bind_search.as_deref()) {
            (Some(kind), Some(search)) => stmt.query_map(params![source_id, kind, search], map)?,
            (Some(kind), None) => stmt.query_map(params![source_id, kind], map)?,
            (None, Some(search)) => stmt.query_map(params![source_id, search], map)?,
            (None, None) => stmt.query_map(params![source_id], map)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    fn load_exact_calls(&self, source_id: &str) -> Result<Vec<ExactCall>, CoreError> {
        let mut stmt = self.conn().prepare(
            "SELECT e.from_symbol_id, e.to_symbol_id, fs.relative_path, ts.relative_path
             FROM edges e
             JOIN symbols fs ON fs.id = e.from_symbol_id
             JOIN symbols ts ON ts.id = e.to_symbol_id
             WHERE e.source_id = ?1 AND e.kind = 'calls' AND e.provenance = 'exact'
               AND e.to_symbol_id IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![source_id], |row| {
            Ok(ExactCall {
                from_id: row.get(0)?,
                to_id: row.get(1)?,
                from_path: row.get(2)?,
                to_path: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    fn neighbor_edges(
        &self,
        predicate: &str,
        symbol_id: &str,
        select_side: &str,
        side_alias: &str,
    ) -> Result<Vec<NeighborHit>, CoreError> {
        let sql = format!(
            "SELECT {select_side}, e.evidence_path, e.evidence_line, e.detail
             FROM edges e
             LEFT JOIN symbols t ON t.id = e.to_symbol_id
             LEFT JOIN symbols c ON c.id = e.from_symbol_id
             WHERE {predicate} AND e.kind = 'calls' AND e.provenance = 'exact'
             ORDER BY e.evidence_path, e.evidence_line
             LIMIT 200"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(params![symbol_id], |row| {
            let line: i64 = row.get(5)?;
            let evidence_line: i64 = row.get(7)?;
            Ok(NeighborHit {
                symbol_id: row.get(0)?,
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                relative_path: row.get(4)?,
                line: u32::try_from(line).unwrap_or(0),
                evidence_path: row.get(6)?,
                evidence_line: u32::try_from(evidence_line).unwrap_or(0),
                detail: row.get(8)?,
            })
        })?;
        let _ = side_alias;
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    /// JSON export of the overview (folder level by default).
    pub fn export_source_overview_json(
        &self,
        source_id: &str,
        focus: Option<&str>,
    ) -> Result<String, CoreError> {
        let graph = self.source_overview(source_id, focus, None, None)?;
        Ok(serde_json::to_string_pretty(&graph)?)
    }
}

#[derive(Debug, Clone)]
struct OverviewSymbol {
    id: String,
    name: String,
    kind: String,
    relative_path: String,
}

#[derive(Debug, Clone)]
struct ExactCall {
    from_id: String,
    to_id: String,
    from_path: String,
    to_path: String,
}

fn looks_like_file(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

fn top_folder(path: &str) -> String {
    match path.split_once('/') {
        Some((head, _)) => head.to_string(),
        None => path.to_string(),
    }
}

fn folder_overview(
    source_id: &str,
    source_name: &str,
    symbols: &[OverviewSymbol],
    calls: &[ExactCall],
) -> OverviewGraph {
    let mut buckets: BTreeMap<String, Vec<&OverviewSymbol>> = BTreeMap::new();
    for symbol in symbols {
        let key = top_folder(&symbol.relative_path);
        buckets.entry(key).or_default().push(symbol);
    }
    let node_ids: HashSet<String> = buckets.keys().cloned().collect();
    let mut edge_weights: HashMap<(String, String), i64> = HashMap::new();
    for call in calls {
        let from = top_folder(&call.from_path);
        let to = top_folder(&call.to_path);
        if from == to || !node_ids.contains(&from) || !node_ids.contains(&to) {
            continue;
        }
        *edge_weights.entry((from, to)).or_default() += 1;
    }
    build_graph(
        source_id,
        source_name,
        "folder",
        None,
        buckets
            .into_iter()
            .map(|(id, members)| {
                let label = if id.is_empty() {
                    "(gốc)".to_string()
                } else {
                    id.clone()
                };
                (id, label, "folder", members)
            })
            .collect(),
        edge_weights,
    )
}

fn folder_children_overview(
    source_id: &str,
    source_name: &str,
    folder: &str,
    symbols: &[OverviewSymbol],
    calls: &[ExactCall],
) -> OverviewGraph {
    let prefix = if folder.is_empty() {
        String::new()
    } else {
        format!("{folder}/")
    };
    let mut buckets: BTreeMap<String, Vec<&OverviewSymbol>> = BTreeMap::new();
    for symbol in symbols {
        let rest = if folder.is_empty() {
            symbol.relative_path.as_str()
        } else if let Some(stripped) = symbol.relative_path.strip_prefix(&prefix) {
            stripped
        } else {
            continue;
        };
        if let Some((child, _)) = rest.split_once('/') {
            let id = if folder.is_empty() {
                child.to_string()
            } else {
                format!("{folder}/{child}")
            };
            buckets.entry(id).or_default().push(symbol);
        } else {
            // File directly under this folder.
            buckets
                .entry(symbol.relative_path.clone())
                .or_default()
                .push(symbol);
        }
    }

    let node_ids: HashSet<String> = buckets.keys().cloned().collect();
    let mut edge_weights: HashMap<(String, String), i64> = HashMap::new();
    for call in calls {
        let from = bucket_for_path(folder, &call.from_path);
        let to = bucket_for_path(folder, &call.to_path);
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };
        if from == to || !node_ids.contains(&from) || !node_ids.contains(&to) {
            continue;
        }
        *edge_weights.entry((from, to)).or_default() += 1;
    }

    build_graph(
        source_id,
        source_name,
        "folder",
        Some(folder.to_string()),
        buckets
            .into_iter()
            .map(|(id, members)| {
                let kind = if looks_like_file(&id) {
                    "file"
                } else {
                    "folder"
                };
                let label = id.rsplit('/').next().unwrap_or(&id).to_string();
                (id, label, kind, members)
            })
            .collect(),
        edge_weights,
    )
}

fn bucket_for_path(folder: &str, path: &str) -> Option<String> {
    let prefix = if folder.is_empty() {
        String::new()
    } else {
        format!("{folder}/")
    };
    if folder.is_empty() {
        return Some(top_folder(path));
    }
    let rest = path.strip_prefix(&prefix)?;
    if let Some((child, _)) = rest.split_once('/') {
        Some(format!("{folder}/{child}"))
    } else {
        Some(path.to_string())
    }
}

fn file_symbol_overview(
    source_id: &str,
    source_name: &str,
    path: &str,
    symbols: &[OverviewSymbol],
    calls: &[ExactCall],
) -> OverviewGraph {
    let members: Vec<&OverviewSymbol> = symbols
        .iter()
        .filter(|symbol| symbol.relative_path == path)
        .collect();
    let ids: HashSet<&str> = members.iter().map(|symbol| symbol.id.as_str()).collect();
    let mut edge_weights: HashMap<(String, String), i64> = HashMap::new();
    for call in calls {
        if ids.contains(call.from_id.as_str()) && ids.contains(call.to_id.as_str()) {
            *edge_weights
                .entry((call.from_id.clone(), call.to_id.clone()))
                .or_default() += 1;
        }
    }
    let buckets: Vec<(String, String, &str, Vec<&OverviewSymbol>)> = members
        .iter()
        .map(|symbol| {
            (
                symbol.id.clone(),
                symbol.name.clone(),
                symbol.kind.as_str(),
                vec![*symbol],
            )
        })
        .collect();
    build_graph(
        source_id,
        source_name,
        "symbol",
        Some(path.to_string()),
        buckets,
        edge_weights,
    )
}

fn build_graph(
    source_id: &str,
    source_name: &str,
    level: &str,
    parent_id: Option<String>,
    buckets: Vec<(String, String, &str, Vec<&OverviewSymbol>)>,
    edge_weights: HashMap<(String, String), i64>,
) -> OverviewGraph {
    let n = buckets.len().max(1) as f64;
    let mut in_count: HashMap<String, i64> = HashMap::new();
    let mut out_count: HashMap<String, i64> = HashMap::new();
    for ((from, to), weight) in &edge_weights {
        *out_count.entry(from.clone()).or_default() += *weight;
        *in_count.entry(to.clone()).or_default() += *weight;
    }

    let nodes: Vec<OverviewNode> = buckets
        .iter()
        .enumerate()
        .map(|(index, (id, label, kind, members))| {
            let angle = std::f64::consts::TAU * (index as f64) / n;
            OverviewNode {
                id: id.clone(),
                label: label.clone(),
                kind: (*kind).to_string(),
                path: Some(id.clone()),
                symbol_count: members.len() as i64,
                exact_out: *out_count.get(id).unwrap_or(&0),
                exact_in: *in_count.get(id).unwrap_or(&0),
                x: 0.5 + 0.38 * angle.cos(),
                y: 0.5 + 0.38 * angle.sin(),
            }
        })
        .collect();

    let edges: Vec<OverviewEdge> = edge_weights
        .into_iter()
        .enumerate()
        .map(|(index, ((from, to), weight))| OverviewEdge {
            id: format!("e{index}"),
            from,
            to,
            weight,
            kind: "calls".to_string(),
        })
        .collect();

    OverviewGraph {
        source_id: source_id.to_string(),
        source_name: source_name.to_string(),
        level: level.to_string(),
        parent_id,
        nodes,
        edges,
        exact_call_count: 0,
        unresolved_call_count: 0,
    }
}
