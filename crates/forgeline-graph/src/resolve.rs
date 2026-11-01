//! Call resolution policy.
//!
//! Exact edges require syntax: the callee is in lexical scope, bound by an
//! import that resolves to one symbol, a namespace attribute, or `self`/`this`.
//! A bare name that matches exactly one symbol elsewhere is inferred and tagged.
//! Attribute calls without a known receiver are left unresolved. They are not
//! guessed, because those guesses are what pollute path queries.

use std::collections::HashMap;

use crate::model::{
    Edge, EdgeKind, ExtractedFile, Import, Language, Provenance, SourceGraph, Symbol, SymbolKind,
};
use crate::summarize::summarize;
use crate::{CONFIDENCE_EXACT, CONFIDENCE_INFERRED};

enum CallDecision {
    Exact(String, &'static str),
    Inferred(String),
    Unresolved,
    Ambiguous,
}

pub(crate) fn resolve_graph(
    source_id: &str,
    source_label: &str,
    files: Vec<ExtractedFile>,
    reindexed_files: u32,
    reused_files: u32,
    removed_files: u32,
    skipped_files: u32,
) -> SourceGraph {
    let mut edges = Vec::new();
    let mut unresolved_calls = 0u32;
    let mut ambiguous_calls = 0u32;

    for file in &files {
        for symbol in &file.symbols {
            if let Some(parent_id) = &symbol.parent_id {
                edges.push(Edge {
                    kind: EdgeKind::Contains,
                    from_id: parent_id.clone(),
                    to_id: Some(symbol.id.clone()),
                    to_name: symbol.name.clone(),
                    provenance: Provenance::Exact,
                    confidence: CONFIDENCE_EXACT,
                    evidence_path: symbol.relative_path.clone(),
                    evidence_line: symbol.start_line,
                    detail: "syntax".to_string(),
                });
            }
        }
    }

    let symbols: Vec<&Symbol> = files.iter().flat_map(|file| file.symbols.iter()).collect();
    let mut by_id: HashMap<&str, &Symbol> = HashMap::new();
    for symbol in &symbols {
        by_id.insert(symbol.id.as_str(), *symbol);
    }

    for file in &files {
        for import in &file.imports {
            if let Some(target) = resolve_import_symbol(file, import, &files) {
                edges.push(Edge {
                    kind: EdgeKind::Imports,
                    from_id: import.scope_id.clone(),
                    to_id: Some(target.id.clone()),
                    to_name: target.name.clone(),
                    provenance: Provenance::Exact,
                    confidence: CONFIDENCE_EXACT,
                    evidence_path: import.relative_path.clone(),
                    evidence_line: import.line,
                    detail: "import".to_string(),
                });
            }
        }

        for call in &file.calls {
            match resolve_call(file, call, &files, &symbols) {
                CallDecision::Exact(target_id, detail) => {
                    if let Some(target) = by_id.get(target_id.as_str()) {
                        edges.push(call_edge(
                            call,
                            target,
                            Provenance::Exact,
                            CONFIDENCE_EXACT,
                            detail,
                        ));
                    }
                }
                CallDecision::Inferred(target_id) => {
                    if let Some(target) = by_id.get(target_id.as_str()) {
                        edges.push(call_edge(
                            call,
                            target,
                            Provenance::Inferred,
                            CONFIDENCE_INFERRED,
                            "name",
                        ));
                    }
                }
                CallDecision::Unresolved => unresolved_calls += 1,
                CallDecision::Ambiguous => ambiguous_calls += 1,
            }
        }
    }

    edges.sort_by(|left, right| {
        (
            left.kind.as_str(),
            left.evidence_path.as_str(),
            left.evidence_line,
            left.to_name.as_str(),
        )
            .cmp(&(
                right.kind.as_str(),
                right.evidence_path.as_str(),
                right.evidence_line,
                right.to_name.as_str(),
            ))
    });

    let summaries = summarize(
        source_id,
        source_label,
        &files,
        &edges,
        unresolved_calls,
        ambiguous_calls,
    );

    SourceGraph {
        source_id: source_id.to_string(),
        source_label: source_label.to_string(),
        files,
        edges,
        summaries,
        unresolved_calls,
        ambiguous_calls,
        reindexed_files,
        reused_files,
        removed_files,
        skipped_files,
    }
}

fn call_edge(
    call: &crate::model::CallSite,
    target: &Symbol,
    provenance: Provenance,
    confidence: f64,
    detail: &str,
) -> Edge {
    Edge {
        kind: EdgeKind::Calls,
        from_id: call.caller_id.clone(),
        to_id: Some(target.id.clone()),
        to_name: target.name.clone(),
        provenance,
        confidence,
        evidence_path: call.relative_path.clone(),
        evidence_line: call.line,
        detail: detail.to_string(),
    }
}

fn resolve_call(
    file: &ExtractedFile,
    call: &crate::model::CallSite,
    files: &[ExtractedFile],
    symbols: &[&Symbol],
) -> CallDecision {
    if let Some(receiver) = &call.receiver {
        if is_method_receiver(receiver) {
            return resolve_method(file, call);
        }
        if let Some(import) = visible_imports(file, &call.caller_id).find(|import| {
            import.imported_name.is_none()
                && receiver_matches_namespace(receiver, &import.local_name)
        }) {
            return match namespace_member(file, import, &call.callee_name, files) {
                Some(target) => CallDecision::Exact(target.id.clone(), "namespace"),
                None => CallDecision::Unresolved,
            };
        }
        return CallDecision::Unresolved;
    }

    let lexical = lexical_candidates(file, call);
    match lexical.len() {
        1 => return CallDecision::Exact(lexical[0].id.clone(), "lexical"),
        n if n > 1 => return CallDecision::Ambiguous,
        _ => {}
    }

    if let Some(import) =
        visible_imports(file, &call.caller_id).find(|import| import.local_name == call.callee_name)
    {
        return match resolve_import_symbol(file, import, files) {
            Some(target) => CallDecision::Exact(target.id.clone(), "import"),
            None => CallDecision::Unresolved,
        };
    }

    let mut global: Vec<&Symbol> = symbols
        .iter()
        .copied()
        .filter(|symbol| {
            symbol.name == call.callee_name
                && matches!(
                    symbol.kind,
                    SymbolKind::Function | SymbolKind::Method | SymbolKind::Class
                )
        })
        .collect();
    global.sort_by(|left, right| left.id.cmp(&right.id));
    global.dedup_by(|left, right| left.id == right.id);
    match global.len() {
        1 => CallDecision::Inferred(global[0].id.clone()),
        0 => CallDecision::Unresolved,
        _ => CallDecision::Ambiguous,
    }
}

fn is_method_receiver(receiver: &str) -> bool {
    matches!(receiver, "self" | "cls" | "this")
}

fn receiver_matches_namespace(receiver: &str, local_name: &str) -> bool {
    // `pkg.mod.fn()` stays unresolved in this layer. Guessing through dotted
    // receivers is how inferred edges leak into path queries.
    receiver == local_name
}

fn resolve_method(file: &ExtractedFile, call: &crate::model::CallSite) -> CallDecision {
    let Some(class_id) = enclosing_class_id(file, &call.caller_id) else {
        return CallDecision::Unresolved;
    };
    let methods: Vec<&Symbol> = file
        .symbols
        .iter()
        .filter(|symbol| {
            symbol.kind == SymbolKind::Method
                && symbol.name == call.callee_name
                && symbol.parent_id.as_deref() == Some(class_id.as_str())
        })
        .collect();
    match methods.len() {
        1 => CallDecision::Exact(methods[0].id.clone(), "method"),
        0 => CallDecision::Unresolved,
        _ => CallDecision::Ambiguous,
    }
}

fn enclosing_class_id(file: &ExtractedFile, start_id: &str) -> Option<String> {
    let mut current = start_id.to_string();
    for _ in 0..64 {
        let symbol = file.symbols.iter().find(|symbol| symbol.id == current)?;
        if symbol.kind == SymbolKind::Class {
            return Some(symbol.id.clone());
        }
        current = symbol.parent_id.clone()?;
    }
    None
}

fn lexical_candidates<'a>(
    file: &'a ExtractedFile,
    call: &crate::model::CallSite,
) -> Vec<&'a Symbol> {
    let module_id = module_id(file);
    let mut ranked: Vec<(u32, &Symbol)> = Vec::new();
    for symbol in &file.symbols {
        if symbol.name != call.callee_name {
            continue;
        }
        if !matches!(symbol.kind, SymbolKind::Function | SymbolKind::Class) {
            continue;
        }
        if let Some(distance) = distance_to_caller(file, symbol, &call.caller_id) {
            ranked.push((distance, symbol));
        } else if symbol.parent_id.as_deref() == module_id {
            ranked.push((10_000, symbol));
        }
    }
    let Some(best) = ranked.iter().map(|(distance, _)| *distance).min() else {
        return Vec::new();
    };
    ranked
        .into_iter()
        .filter(|(distance, _)| *distance == best)
        .map(|(_, symbol)| symbol)
        .collect()
}

fn distance_to_caller(file: &ExtractedFile, symbol: &Symbol, caller_id: &str) -> Option<u32> {
    if symbol.id == caller_id {
        return Some(0);
    }
    let mut current = symbol.id.clone();
    let mut steps = 0u32;
    while steps < 64 {
        let current_symbol = file.symbols.iter().find(|item| item.id == current)?;
        let parent = current_symbol.parent_id.as_ref()?;
        steps += 1;
        if parent == caller_id {
            return Some(steps);
        }
        current = parent.clone();
    }
    None
}

fn visible_imports<'a>(
    file: &'a ExtractedFile,
    caller_id: &str,
) -> impl Iterator<Item = &'a Import> {
    let module = module_id(file).unwrap_or("").to_string();
    let caller = caller_id.to_string();
    file.imports.iter().filter(move |import| {
        import.scope_id == caller
            || import.scope_id == module
            || is_ancestor(file, &caller, &import.scope_id)
    })
}

fn is_ancestor(file: &ExtractedFile, start_id: &str, ancestor_id: &str) -> bool {
    let mut current = start_id.to_string();
    for _ in 0..64 {
        let Some(symbol) = file.symbols.iter().find(|symbol| symbol.id == current) else {
            return false;
        };
        let Some(parent) = &symbol.parent_id else {
            return false;
        };
        if parent == ancestor_id {
            return true;
        }
        current = parent.clone();
    }
    false
}

fn module_id(file: &ExtractedFile) -> Option<&str> {
    file.symbols
        .iter()
        .find(|symbol| symbol.kind == SymbolKind::Module)
        .map(|symbol| symbol.id.as_str())
}

fn resolve_import_symbol<'a>(
    file: &ExtractedFile,
    import: &Import,
    files: &'a [ExtractedFile],
) -> Option<&'a Symbol> {
    let target = resolve_specifier(&file.relative_path, &import.specifier, file.language, files)?;
    match &import.imported_name {
        Some(name) if name == "default" => None,
        Some(name) => module_level_symbol(target, name),
        None => target
            .symbols
            .iter()
            .find(|symbol| symbol.kind == SymbolKind::Module),
    }
}

fn namespace_member<'a>(
    file: &ExtractedFile,
    import: &Import,
    member: &str,
    files: &'a [ExtractedFile],
) -> Option<&'a Symbol> {
    let target = resolve_specifier(&file.relative_path, &import.specifier, file.language, files)?;
    module_level_symbol(target, member)
}

fn module_level_symbol<'a>(file: &'a ExtractedFile, name: &str) -> Option<&'a Symbol> {
    let module_id = module_id(file)?;
    let mut matches = file.symbols.iter().filter(|symbol| {
        symbol.name == name
            && symbol.parent_id.as_deref() == Some(module_id)
            && symbol.kind != SymbolKind::Module
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(first)
    }
}

fn resolve_specifier<'a>(
    from_file: &str,
    specifier: &str,
    language: Language,
    files: &'a [ExtractedFile],
) -> Option<&'a ExtractedFile> {
    let candidates = candidate_paths(from_file, specifier, language);
    let mut matched = files.iter().filter(|file| {
        candidates
            .iter()
            .any(|candidate| candidate == &file.relative_path)
    });
    let first = matched.next()?;
    if matched.next().is_some() {
        None
    } else {
        Some(first)
    }
}

fn candidate_paths(from_file: &str, specifier: &str, language: Language) -> Vec<String> {
    match language {
        Language::Python => python_candidates(from_file, specifier),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            javascript_candidates(from_file, specifier)
        }
    }
}

fn python_candidates(from_file: &str, specifier: &str) -> Vec<String> {
    let spec = specifier.trim();
    if spec.is_empty() {
        return Vec::new();
    }
    let dir = parent_path(from_file);
    let mut seeds = Vec::new();
    if spec.starts_with('.') {
        let dots = spec.chars().take_while(|ch| *ch == '.').count();
        let rest = spec.trim_start_matches('.');
        let mut base = dir;
        for _ in 1..dots {
            base = parent_path(&base);
        }
        let joined = if rest.is_empty() {
            base
        } else {
            join_path(&base, &rest.replace('.', "/"))
        };
        seeds.push(normalize_path(&joined));
    } else {
        let dotted = spec.replace('.', "/");
        seeds.push(normalize_path(&join_path(&dir, &dotted)));
        seeds.push(normalize_path(&dotted));
        let mut cursor = dir;
        while !cursor.is_empty() {
            seeds.push(normalize_path(&join_path(&cursor, &dotted)));
            let parent = parent_path(&cursor);
            if parent == cursor {
                break;
            }
            cursor = parent;
        }
    }
    seeds.sort();
    seeds.dedup();
    let mut out = Vec::new();
    for seed in seeds {
        if seed.is_empty() {
            continue;
        }
        out.push(format!("{seed}.py"));
        out.push(format!("{seed}/__init__.py"));
    }
    out
}

fn javascript_candidates(from_file: &str, specifier: &str) -> Vec<String> {
    if !specifier.starts_with('.') {
        return Vec::new();
    }
    let joined = normalize_path(&join_path(&parent_path(from_file), specifier));
    let stem = strip_known_extension(&joined);
    let mut out = Vec::new();
    for ext in [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"] {
        out.push(format!("{stem}{ext}"));
    }
    for ext in [".ts", ".tsx", ".js", ".jsx"] {
        out.push(format!("{stem}/index{ext}"));
    }
    out
}

fn strip_known_extension(path: &str) -> String {
    for ext in [".tsx", ".ts", ".jsx", ".mjs", ".cjs", ".js"] {
        if let Some(stripped) = path.strip_suffix(ext) {
            return stripped.to_string();
        }
    }
    path.to_string()
}

fn parent_path(path: &str) -> String {
    match path.rfind('/') {
        Some(index) => path[..index].to_string(),
        None => String::new(),
    }
}

fn join_path(base: &str, rel: &str) -> String {
    if base.is_empty() {
        rel.to_string()
    } else if rel.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{rel}")
    }
}

fn normalize_path(path: &str) -> String {
    let mut stack = Vec::new();
    for part in path.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            stack.pop();
        } else {
            stack.push(part);
        }
    }
    stack.join("/")
}
