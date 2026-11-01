use tree_sitter::Node;

use crate::extract::{
    callee_parts, first_string_literal, line_number, node_text, pop_scope, push_scope,
    walk_children, Ctx,
};
use crate::model::SymbolKind;

pub(crate) fn walk(node: Node<'_>, ctx: &mut Ctx<'_>) {
    match node.kind() {
        "compilation_unit" => walk_compilation_unit(node, ctx),
        "namespace_declaration" => define_block_namespace(node, ctx),
        "file_scoped_namespace_declaration" => {}
        "class_declaration" => define_type(node, ctx, SymbolKind::Class),
        "interface_declaration" => define_type(node, ctx, SymbolKind::Interface),
        "record_declaration" => define_type(node, ctx, SymbolKind::Record),
        "struct_declaration" => define_type(node, ctx, SymbolKind::Struct),
        "method_declaration" => define_method(node, ctx),
        "constructor_declaration" => define_constructor(node, ctx),
        "property_declaration" => define_property(node, ctx),
        "using_directive" => record_using(node, ctx),
        "invocation_expression" => {
            record_invocation(node, ctx);
            walk_children(node, ctx, walk);
        }
        "object_creation_expression" => {
            record_creation(node, ctx);
            walk_children(node, ctx, walk);
        }
        _ => walk_children(node, ctx, walk),
    }
}

fn walk_compilation_unit(node: Node<'_>, ctx: &mut Ctx<'_>) {
    // File-scoped namespaces are siblings of the types they wrap in the AST.
    let mut file_ns = None;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "file_scoped_namespace_declaration" {
            file_ns = Some(child);
            break;
        }
    }
    if let Some(ns) = file_ns {
        if let Some(name_node) = ns.child_by_field_name("name") {
            let name = node_text(name_node, ctx.source);
            if !name.is_empty() {
                push_scope(ctx, &name, SymbolKind::Namespace, ns);
            } else {
                file_ns = None;
            }
        } else {
            file_ns = None;
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "file_scoped_namespace_declaration" {
            continue;
        }
        walk(child, ctx);
    }
    if file_ns.is_some() {
        pop_scope(ctx);
    }
}

fn define_block_namespace(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(name_node) = node.child_by_field_name("name") else {
        walk_children(node, ctx, walk);
        return;
    };
    let name = node_text(name_node, ctx.source);
    if name.is_empty() {
        walk_children(node, ctx, walk);
        return;
    }
    push_scope(ctx, &name, SymbolKind::Namespace, node);
    if let Some(body) = node.child_by_field_name("body") {
        walk(body, ctx);
    }
    pop_scope(ctx);
}

fn define_type(node: Node<'_>, ctx: &mut Ctx<'_>, kind: SymbolKind) {
    let Some(name_node) = node.child_by_field_name("name") else {
        walk_children(node, ctx, walk);
        return;
    };
    let name = ctx.text(name_node);
    if name.is_empty() {
        walk_children(node, ctx, walk);
        return;
    }
    let previous_prefix = ctx.route_prefix.clone();
    if let Some(prefix) = class_route_prefix(node, ctx.source) {
        ctx.route_prefix = Some(prefix);
    }
    push_scope(ctx, &name, kind, node);
    if let Some(body) = node.child_by_field_name("body") {
        walk(body, ctx);
    }
    pop_scope(ctx);
    ctx.route_prefix = previous_prefix;
}

fn class_route_prefix(node: Node<'_>, source: &str) -> Option<String> {
    let mut methods = Vec::new();
    let mut templates = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "attribute_list" {
            collect_route_attrs(child, &mut methods, &mut templates, source);
        }
    }
    templates.into_iter().next()
}

fn define_method(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(name_node) = node.child_by_field_name("name") else {
        walk_children(node, ctx, walk);
        return;
    };
    let name = ctx.text(name_node);
    if name.is_empty() {
        walk_children(node, ctx, walk);
        return;
    }
    let kind = crate::extract::nested_kind(ctx, SymbolKind::Method);
    push_scope(ctx, &name, kind, node);
    let symbol_id = ctx
        .stack
        .last()
        .map(|frame| frame.id.clone())
        .unwrap_or_default();
    extract_routes(node, &symbol_id, ctx);
    if let Some(parameters) = node.child_by_field_name("parameters") {
        walk(parameters, ctx);
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk(body, ctx);
    }
    pop_scope(ctx);
}

fn define_constructor(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(name_node) = node.child_by_field_name("name") else {
        walk_children(node, ctx, walk);
        return;
    };
    let name = ctx.text(name_node);
    push_scope(ctx, &name, SymbolKind::Constructor, node);
    if let Some(parameters) = node.child_by_field_name("parameters") {
        walk(parameters, ctx);
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk(body, ctx);
    }
    pop_scope(ctx);
}

fn define_property(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(name_node) = node.child_by_field_name("name") else {
        walk_children(node, ctx, walk);
        return;
    };
    let name = ctx.text(name_node);
    if name.is_empty() {
        walk_children(node, ctx, walk);
        return;
    }
    push_scope(ctx, &name, SymbolKind::Property, node);
    if let Some(accessors) = node.child_by_field_name("accessors") {
        walk(accessors, ctx);
    }
    if let Some(value) = node.child_by_field_name("value") {
        walk(value, ctx);
    }
    pop_scope(ctx);
}

fn record_using(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let line = line_number(node.start_position().row);
    let specifier = if let Some(name) = node.child_by_field_name("name") {
        node_text(name, ctx.source)
    } else {
        // `using X = Y;` or namespace usings may put the type in children.
        let mut cursor = node.walk();
        let mut found = String::new();
        for child in node.named_children(&mut cursor) {
            if matches!(
                child.kind(),
                "identifier" | "qualified_name" | "alias_qualified_name" | "generic_name"
            ) {
                found = node_text(child, ctx.source);
                break;
            }
        }
        found
    };
    if specifier.is_empty() {
        return;
    }
    let local = specifier
        .rsplit('.')
        .next()
        .unwrap_or(specifier.as_str())
        .to_string();
    ctx.add_import(local, None, specifier, line);
}

fn record_invocation(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let line = line_number(node.start_position().row);
    let literal = node
        .child_by_field_name("arguments")
        .and_then(|args| first_string_literal(args, ctx.source));
    let Some((receiver, callee)) = callee_parts(func, ctx.source) else {
        return;
    };
    maybe_http_client(&receiver, &callee, literal.as_deref(), line, ctx);
    ctx.add_call(receiver, callee, line, literal);
}

fn record_creation(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(type_node) = node.child_by_field_name("type") else {
        return;
    };
    let type_name = node_text(type_node, ctx.source);
    let simple = type_name
        .rsplit('.')
        .next()
        .unwrap_or(type_name.as_str())
        .to_string();
    if !is_ident(&simple) {
        return;
    }
    let receiver = if type_name.contains('.') {
        Some(
            type_name
                .rsplit_once('.')
                .map(|(left, _)| left.to_string())
                .unwrap_or_default(),
        )
        .filter(|value| !value.is_empty())
    } else {
        None
    };
    let line = line_number(node.start_position().row);
    ctx.add_call(receiver, simple, line, None);
}

fn extract_routes(node: Node<'_>, symbol_id: &str, ctx: &mut Ctx<'_>) {
    let line = line_number(node.start_position().row);
    let mut methods = Vec::new();
    let mut templates = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "attribute_list" {
            continue;
        }
        collect_route_attrs(child, &mut methods, &mut templates, ctx.source);
    }
    if methods.is_empty() && templates.is_empty() {
        return;
    }
    if methods.is_empty() {
        methods.push("ANY".to_string());
    }
    let method_template = templates.last().cloned().unwrap_or_default();
    let template = match (&ctx.route_prefix, method_template.as_str()) {
        (Some(prefix), "") => prefix.clone(),
        (Some(prefix), rest) => format!(
            "{}/{}",
            prefix.trim_end_matches('/'),
            rest.trim_start_matches('/')
        ),
        (None, rest) => rest.to_string(),
    };
    ctx.add_route(symbol_id.to_string(), methods, template, line);
}

fn collect_route_attrs(
    node: Node<'_>,
    methods: &mut Vec<String>,
    templates: &mut Vec<String>,
    source: &str,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "attribute" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                let name = node_text(name_node, source);
                let bare = name.rsplit('.').next().unwrap_or(name.as_str());
                let template = first_string_literal(child, source);
                match bare {
                    "HttpGet" | "HttpPost" | "HttpPut" | "HttpDelete" | "HttpPatch"
                    | "HttpHead" | "HttpOptions" => {
                        let method = bare.trim_start_matches("Http").to_ascii_uppercase();
                        methods.push(method);
                        if let Some(template) = template {
                            templates.push(template);
                        }
                    }
                    "Route" => {
                        if let Some(template) = template {
                            templates.push(template);
                        }
                    }
                    _ => {}
                }
            }
            "attribute_list" => collect_route_attrs(child, methods, templates, source),
            _ => {}
        }
    }
}

fn maybe_http_client(
    receiver: &Option<String>,
    callee: &str,
    literal: Option<&str>,
    line: u32,
    ctx: &mut Ctx<'_>,
) {
    let Some(url) = literal else {
        return;
    };
    let recv = receiver.as_deref().unwrap_or("");
    if callee == "GetAsync"
        || callee == "PostAsync"
        || callee == "PutAsync"
        || callee == "DeleteAsync"
        || callee == "SendAsync"
        || (recv.ends_with("HttpClient") || recv == "_http" || recv == "http" || recv == "client")
            && matches!(
                callee,
                "GetStringAsync" | "GetFromJsonAsync" | "PostAsJsonAsync"
            )
    {
        ctx.add_http_call(
            "HttpClient".to_string(),
            Some(callee.to_string()),
            url.to_string(),
            line,
        );
    }
}

fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
