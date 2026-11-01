use crate::model::{
    token_estimate, Edge, EdgeKind, ExtractedFile, Provenance, Summary, SummaryLevel, SymbolKind,
};

const MAX_MODULE_LINES: usize = 40;

pub(crate) fn summarize(
    source_id: &str,
    source_label: &str,
    files: &[ExtractedFile],
    edges: &[Edge],
    unresolved_calls: u32,
    ambiguous_calls: u32,
) -> Vec<Summary> {
    let mut summaries = Vec::new();
    let symbol_count = files.iter().map(|file| file.symbols.len()).sum::<usize>();
    let exact_calls = count_calls(edges, Provenance::Exact);
    let route_count = files.iter().map(|file| file.routes.len()).sum::<usize>();
    let http_count = files
        .iter()
        .map(|file| file.http_calls.len())
        .sum::<usize>();

    let mut source_body = format!(
        "source {source_label}\nfiles {} | symbols {symbol_count} | calls exact {exact_calls} | unresolved {unresolved_calls} | ambiguous {ambiguous_calls} | routes {route_count} | http {http_count}\n",
        files.len()
    );
    for file in files {
        let defined = file
            .symbols
            .iter()
            .filter(|symbol| symbol.kind != SymbolKind::Module)
            .count();
        source_body.push_str(&format!(
            "module {} symbols {defined}\n",
            file.relative_path
        ));
    }
    summaries.push(make_summary(SummaryLevel::Source, source_id, source_body));

    for file in files {
        let Some(module) = file
            .symbols
            .iter()
            .find(|symbol| symbol.kind == SymbolKind::Module)
        else {
            continue;
        };
        let mut body = format!("module {} {}\n", file.relative_path, file.language.as_str());
        for (listed, symbol) in file
            .symbols
            .iter()
            .filter(|symbol| symbol.kind != SymbolKind::Module)
            .enumerate()
        {
            if listed == MAX_MODULE_LINES {
                body.push_str("... truncated\n");
                break;
            }
            body.push_str(&format!(
                "{} {} @ {}\n",
                symbol.kind.as_str(),
                symbol.qualified_name,
                symbol.start_line
            ));
        }
        for route in &file.routes {
            body.push_str(&format!(
                "route {} {} @ {}\n",
                route.http_methods.join("|"),
                route.template,
                route.line
            ));
        }
        for http in &file.http_calls {
            body.push_str(&format!(
                "http {} {} {} @ {}\n",
                http.client,
                http.method.as_deref().unwrap_or("-"),
                http.url_literal,
                http.line
            ));
        }
        summaries.push(make_summary(SummaryLevel::Module, &module.id, body));

        for symbol in file
            .symbols
            .iter()
            .filter(|symbol| symbol.kind != SymbolKind::Module)
        {
            summaries.push(make_summary(
                SummaryLevel::Symbol,
                &symbol.id,
                symbol_body(symbol, edges, file),
            ));
        }
    }

    summaries
}

fn symbol_body(symbol: &crate::model::Symbol, edges: &[Edge], file: &ExtractedFile) -> String {
    let mut body = format!(
        "{} {} @ {}:{}-{}\n",
        symbol.kind.as_str(),
        symbol.qualified_name,
        symbol.relative_path,
        symbol.start_line,
        symbol.end_line
    );
    for route in file
        .routes
        .iter()
        .filter(|route| route.symbol_id == symbol.id)
    {
        body.push_str(&format!(
            "route {} {}\n",
            route.http_methods.join("|"),
            route.template
        ));
    }
    let mut calls: Vec<&Edge> = edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.from_id == symbol.id)
        .collect();
    calls.sort_by_key(|edge| edge.evidence_line);
    if calls.is_empty() {
        body.push_str("calls none\n");
    } else {
        for edge in calls {
            body.push_str(&format!(
                "calls {} {} {:.2} {}\n",
                edge.to_name,
                edge.provenance.as_str(),
                edge.confidence,
                edge.detail
            ));
        }
    }
    for http in file
        .http_calls
        .iter()
        .filter(|http| http.caller_id == symbol.id)
    {
        body.push_str(&format!(
            "http {} {} {}\n",
            http.client,
            http.method.as_deref().unwrap_or("-"),
            http.url_literal
        ));
    }
    body
}

fn count_calls(edges: &[Edge], provenance: Provenance) -> usize {
    edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.provenance == provenance)
        .count()
}

fn make_summary(level: SummaryLevel, owner_id: &str, body: String) -> Summary {
    let token_estimate = token_estimate(&body);
    Summary {
        level,
        owner_id: owner_id.to_string(),
        body,
        token_estimate,
    }
}
