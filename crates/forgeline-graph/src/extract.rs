use tree_sitter::Node;

use crate::hashutil::content_hash;
use crate::model::{
    language_for_path, symbol_id, CallSite, ExtractedFile, Import, Language, Symbol, SymbolKind,
};
use crate::GraphError;
use crate::EXTRACTOR_VERSION;

pub(crate) struct Frame {
    pub id: String,
    pub kind: SymbolKind,
    pub qualified_name: String,
}

pub(crate) struct Ctx<'a> {
    pub source_id: &'a str,
    pub relative_path: &'a str,
    pub language: Language,
    pub source: &'a str,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub calls: Vec<CallSite>,
    pub stack: Vec<Frame>,
}

impl<'a> Ctx<'a> {
    pub(crate) fn text(&self, node: Node<'_>) -> String {
        node.utf8_text(self.source.as_bytes())
            .unwrap_or("")
            .to_string()
    }

    pub(crate) fn push_symbol(
        &mut self,
        name: String,
        qualified_name: String,
        kind: SymbolKind,
        node: Node<'_>,
        parent_id: Option<String>,
    ) -> String {
        let start = node.start_position();
        let end = node.end_position();
        let base = symbol_id(
            self.source_id,
            self.relative_path,
            kind,
            &qualified_name,
            line_number(start.row),
        );
        let id = unique_id(&self.symbols, base);
        self.symbols.push(Symbol {
            id: id.clone(),
            parent_id,
            name,
            qualified_name,
            kind,
            language: self.language,
            relative_path: self.relative_path.to_string(),
            start_line: line_number(start.row),
            end_line: line_number(end.row),
            start_column: column_u32(start.column),
            end_column: column_u32(end.column),
        });
        id
    }

    pub(crate) fn caller_id(&self) -> String {
        self.stack
            .iter()
            .rev()
            .find(|frame| matches!(frame.kind, SymbolKind::Function | SymbolKind::Method))
            .or_else(|| {
                self.stack
                    .iter()
                    .rev()
                    .find(|frame| frame.kind == SymbolKind::Class)
            })
            .or_else(|| self.stack.last())
            .map(|frame| frame.id.clone())
            .unwrap_or_default()
    }

    pub(crate) fn import_scope_id(&self) -> String {
        self.stack
            .iter()
            .rev()
            .find(|frame| {
                matches!(
                    frame.kind,
                    SymbolKind::Function | SymbolKind::Method | SymbolKind::Module
                )
            })
            .map(|frame| frame.id.clone())
            .unwrap_or_default()
    }

    pub(crate) fn add_import(
        &mut self,
        local_name: String,
        imported_name: Option<String>,
        specifier: String,
        line: u32,
    ) {
        if local_name.is_empty() || specifier.is_empty() {
            return;
        }
        self.imports.push(Import {
            scope_id: self.import_scope_id(),
            local_name,
            imported_name,
            specifier,
            relative_path: self.relative_path.to_string(),
            line,
        });
    }

    pub(crate) fn add_call(&mut self, receiver: Option<String>, callee_name: String, line: u32) {
        if !is_simple_name(&callee_name) {
            return;
        }
        let receiver = receiver.and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        self.calls.push(CallSite {
            caller_id: self.caller_id(),
            callee_name,
            receiver,
            relative_path: self.relative_path.to_string(),
            line,
        });
    }
}

pub(crate) fn extract_file(
    source_id: &str,
    relative_path: &str,
    contents: &str,
) -> Result<ExtractedFile, GraphError> {
    let language = language_for_path(relative_path)
        .ok_or_else(|| GraphError::Unsupported(relative_path.to_string()))?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&grammar(language)?)
        .map_err(|_| GraphError::Grammar(language.grammar_name()))?;
    let Some(tree) = parse_with_budget(&mut parser, contents) else {
        return Ok(empty_file(source_id, relative_path, language, contents));
    };

    let mut ctx = Ctx {
        source_id,
        relative_path,
        language,
        source: contents,
        symbols: Vec::new(),
        imports: Vec::new(),
        calls: Vec::new(),
        stack: Vec::new(),
    };
    let root = tree.root_node();
    let module_name = relative_path.replace('\\', "/");
    let module_id = ctx.push_symbol(
        module_name.clone(),
        module_name,
        SymbolKind::Module,
        root,
        None,
    );
    ctx.stack.push(Frame {
        id: module_id,
        kind: SymbolKind::Module,
        qualified_name: String::new(),
    });
    match language {
        Language::Python => languages_walk_python(root, &mut ctx),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            languages_walk_ecma(root, &mut ctx);
        }
    }

    Ok(ExtractedFile {
        extractor_version: EXTRACTOR_VERSION,
        relative_path: relative_path.replace('\\', "/"),
        language,
        content_hash: content_hash(contents.as_bytes()),
        symbols: ctx.symbols,
        imports: ctx.imports,
        calls: ctx.calls,
    })
}

fn empty_file(
    source_id: &str,
    relative_path: &str,
    language: Language,
    contents: &str,
) -> ExtractedFile {
    let relative_path = relative_path.replace('\\', "/");
    let end_line = u32::try_from(contents.lines().count().max(1)).unwrap_or(u32::MAX);
    let id = symbol_id(
        source_id,
        &relative_path,
        SymbolKind::Module,
        &relative_path,
        1,
    );
    ExtractedFile {
        extractor_version: EXTRACTOR_VERSION,
        relative_path: relative_path.clone(),
        language,
        content_hash: content_hash(contents.as_bytes()),
        symbols: vec![Symbol {
            id,
            parent_id: None,
            name: relative_path.clone(),
            qualified_name: relative_path.clone(),
            kind: SymbolKind::Module,
            language,
            relative_path,
            start_line: 1,
            end_line,
            start_column: 0,
            end_column: 0,
        }],
        imports: Vec::new(),
        calls: Vec::new(),
    }
}

fn parse_with_budget(
    parser: &mut tree_sitter::Parser,
    contents: &str,
) -> Option<tree_sitter::Tree> {
    let bytes = contents.as_bytes();
    let started = std::time::Instant::now();
    let mut cancel = move |_state: &tree_sitter::ParseState| -> bool {
        started.elapsed() >= std::time::Duration::from_secs(1)
    };
    let mut read = |offset: usize, _point: tree_sitter::Point| -> &[u8] {
        if offset >= bytes.len() {
            &[]
        } else {
            &bytes[offset..]
        }
    };
    let options = tree_sitter::ParseOptions::new().progress_callback(&mut cancel);
    parser.parse_with_options(&mut read, None, Some(options))
}

pub(crate) fn line_number(row: usize) -> u32 {
    u32::try_from(row.saturating_add(1)).unwrap_or(u32::MAX)
}

fn grammar(language: Language) -> Result<tree_sitter::Language, GraphError> {
    Ok(match language {
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
    })
}

fn languages_walk_python(node: tree_sitter::Node<'_>, ctx: &mut Ctx<'_>) {
    crate::languages::python::walk(node, ctx);
}

fn languages_walk_ecma(node: tree_sitter::Node<'_>, ctx: &mut Ctx<'_>) {
    crate::languages::ecma::walk(node, ctx);
}

pub(crate) fn walk_children(node: Node<'_>, ctx: &mut Ctx<'_>, walk: fn(Node<'_>, &mut Ctx<'_>)) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, ctx);
    }
}

pub(crate) fn push_scope(ctx: &mut Ctx<'_>, name: &str, kind: SymbolKind, node: Node<'_>) {
    let (parent_kind, parent_qualified, parent_id) = {
        let parent = ctx
            .stack
            .last()
            .expect("module frame is pushed before walking");
        (
            parent.kind,
            parent.qualified_name.clone(),
            parent.id.clone(),
        )
    };
    let qualified_name = if parent_kind == SymbolKind::Module {
        name.to_string()
    } else {
        format!("{parent_qualified}.{name}")
    };
    let id = ctx.push_symbol(
        name.to_string(),
        qualified_name.clone(),
        kind,
        node,
        Some(parent_id),
    );
    ctx.stack.push(Frame {
        id,
        kind,
        qualified_name,
    });
}

pub(crate) fn pop_scope(ctx: &mut Ctx<'_>) {
    ctx.stack.pop();
}

pub(crate) fn nested_kind(ctx: &Ctx<'_>, declared: SymbolKind) -> SymbolKind {
    if declared == SymbolKind::Class {
        return SymbolKind::Class;
    }
    if ctx
        .stack
        .last()
        .is_some_and(|frame| frame.kind == SymbolKind::Class)
    {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    }
}

pub(crate) fn is_function_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition"
            | "function_declaration"
            | "function_expression"
            | "method_definition"
            | "arrow_function"
            | "function_signature"
            | "abstract_method_signature"
    )
}

pub(crate) fn field_nodes<'t>(node: Node<'t>, field: &str) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if cursor.field_name() == Some(field) {
                out.push(cursor.node());
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    out
}

pub(crate) fn callee_parts(func: Node<'_>, source: &str) -> Option<(Option<String>, String)> {
    match func.kind() {
        "identifier" | "property_identifier" | "type_identifier" => {
            Some((None, node_text(func, source)))
        }
        "attribute" | "member_expression" => {
            let prop = func
                .child_by_field_name("attribute")
                .or_else(|| func.child_by_field_name("property"))?;
            let name = node_text(prop, source);
            let receiver = func
                .child_by_field_name("object")
                .map(|object| node_text(object, source));
            Some((receiver, name))
        }
        _ => None,
    }
}

pub(crate) fn node_text(node: Node<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(crate) fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn is_simple_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn unique_id(existing: &[Symbol], base: String) -> String {
    if existing.iter().all(|symbol| symbol.id != base) {
        return base;
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{base}#{n}");
        if existing.iter().all(|symbol| symbol.id != candidate) {
            return candidate;
        }
        n += 1;
    }
}

fn column_u32(column: usize) -> u32 {
    u32::try_from(column).unwrap_or(u32::MAX)
}
