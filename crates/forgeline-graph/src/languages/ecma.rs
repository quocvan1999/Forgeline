use tree_sitter::Node;

use crate::extract::{
    callee_parts, first_string_literal, is_function_node, line_number, looks_like_component,
    node_text, pop_scope, push_scope, unquote, walk_children, Ctx,
};
use crate::model::SymbolKind;

pub(crate) fn walk(node: Node<'_>, ctx: &mut Ctx<'_>) {
    match node.kind() {
        "function_declaration"
        | "function_signature"
        | "method_definition"
        | "abstract_method_signature" => define_function(node, ctx),
        "class_declaration" | "abstract_class_declaration" => define_class(node, ctx),
        "variable_declarator" => define_variable(node, ctx),
        "public_field_definition" | "property_definition" | "field_definition" => {
            define_field(node, ctx);
        }
        "import_statement" => record_import(node, ctx),
        "call_expression" => {
            record_call(node, ctx);
            walk_children(node, ctx, walk);
        }
        "new_expression" => {
            record_new(node, ctx);
            walk_children(node, ctx, walk);
        }
        "arrow_function" | "function_expression" => walk_children(node, ctx, walk),
        _ => walk_children(node, ctx, walk),
    }
}

fn define_function(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(name) = function_name(node, ctx) else {
        walk_children(node, ctx, walk);
        return;
    };
    let kind = component_or_nested(ctx, &name, returns_jsx(node));
    push_scope(ctx, &name, kind, node);
    if let Some(parameters) = node.child_by_field_name("parameters") {
        walk(parameters, ctx);
    }
    if let Some(body) = node.child_by_field_name("body") {
        walk(body, ctx);
    }
    pop_scope(ctx);
}

fn define_class(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(name_node) = node.child_by_field_name("name") else {
        walk_children(node, ctx, walk);
        return;
    };
    let name = ctx.text(name_node);
    if name.is_empty() {
        walk_children(node, ctx, walk);
        return;
    }
    let kind = if looks_like_component(&name) {
        SymbolKind::Component
    } else {
        SymbolKind::Class
    };
    push_scope(ctx, &name, kind, node);
    if let Some(body) = node.child_by_field_name("body") {
        walk(body, ctx);
    }
    pop_scope(ctx);
}

fn define_variable(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(value) = node.child_by_field_name("value") else {
        walk_children(node, ctx, walk);
        return;
    };
    let Some(name_node) = node.child_by_field_name("name") else {
        walk(value, ctx);
        return;
    };
    if name_node.kind() == "identifier" && is_function_node(value.kind()) {
        let name = ctx.text(name_node);
        let kind = component_or_nested(ctx, &name, returns_jsx(value));
        push_scope(ctx, &name, kind, node);
        walk(value, ctx);
        pop_scope(ctx);
        return;
    }
    walk_children(node, ctx, walk);
}

fn define_field(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(value) = node.child_by_field_name("value") else {
        walk_children(node, ctx, walk);
        return;
    };
    let name_node = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("property"));
    if let Some(name_node) = name_node {
        if is_function_node(value.kind()) {
            let name = node_text(name_node, ctx.source);
            if !name.is_empty() {
                let kind = component_or_nested(ctx, &name, returns_jsx(value));
                push_scope(ctx, &name, kind, node);
                walk(value, ctx);
                pop_scope(ctx);
                return;
            }
        }
    }
    walk_children(node, ctx, walk);
}

fn component_or_nested(ctx: &Ctx<'_>, name: &str, jsx: bool) -> SymbolKind {
    // React components are PascalCase. Prefer JSX evidence; still accept
    // PascalCase module-level functions in TS/TSX/JSX files.
    let at_module = ctx
        .stack
        .last()
        .is_some_and(|frame| matches!(frame.kind, SymbolKind::Module | SymbolKind::Namespace));
    if looks_like_component(name) && (jsx || at_module) {
        return SymbolKind::Component;
    }
    crate::extract::nested_kind(ctx, SymbolKind::Function)
}

fn returns_jsx(node: Node<'_>) -> bool {
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if matches!(
            current.kind(),
            "jsx_element" | "jsx_self_closing_element" | "jsx_fragment"
        ) {
            return true;
        }
        let mut cursor = current.walk();
        for child in current.named_children(&mut cursor) {
            stack.push(child);
        }
    }
    false
}

fn function_name(node: Node<'_>, ctx: &Ctx<'_>) -> Option<String> {
    let name_node = node.child_by_field_name("name")?;
    let name = ctx.text(name_node);
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn record_call(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    record_callee(func, node, ctx);
}

fn record_new(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(ctor) = node.child_by_field_name("constructor") else {
        return;
    };
    record_callee(ctor, node, ctx);
}

fn record_callee(func: Node<'_>, site: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some((receiver, callee)) = callee_parts(func, ctx.source) else {
        return;
    };
    let line = line_number(site.start_position().row);
    let literal = site
        .child_by_field_name("arguments")
        .and_then(|args| first_string_literal(args, ctx.source));
    maybe_http(&receiver, &callee, literal.as_deref(), line, ctx);
    ctx.add_call(receiver, callee, line, literal);
}

fn maybe_http(
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
    if callee == "fetch" && receiver.is_none() {
        ctx.add_http_call("fetch".to_string(), None, url.to_string(), line);
        return;
    }
    if recv == "axios"
        || recv.ends_with(".axios")
        || (recv.is_empty() && callee == "axios")
        || matches!(callee, "get" | "post" | "put" | "delete" | "patch")
            && (recv == "axios" || recv.ends_with("axios"))
    {
        let method = if matches!(callee, "get" | "post" | "put" | "delete" | "patch") {
            Some(callee.to_ascii_uppercase())
        } else {
            None
        };
        ctx.add_http_call("axios".to_string(), method, url.to_string(), line);
    }
}

fn record_import(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(source_node) = named_child(node, "string") else {
        return;
    };
    let specifier = unquote(&node_text(source_node, ctx.source));
    if specifier.is_empty() || specifier.starts_with('`') {
        return;
    }
    let Some(clause) = named_child(node, "import_clause") else {
        return;
    };
    let line = line_number(node.start_position().row);
    let mut cursor = clause.walk();
    for child in clause.named_children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                ctx.add_import(
                    node_text(child, ctx.source),
                    Some("default".to_string()),
                    specifier.clone(),
                    line,
                );
            }
            "named_imports" => record_named_imports(child, &specifier, line, ctx),
            "namespace_import" => {
                if let Some(ident) = named_child(child, "identifier") {
                    ctx.add_import(node_text(ident, ctx.source), None, specifier.clone(), line);
                }
            }
            _ => {}
        }
    }
}

fn record_named_imports(node: Node<'_>, specifier: &str, line: u32, ctx: &mut Ctx<'_>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "import_specifier" {
            continue;
        }
        let Some(name_node) = child.child_by_field_name("name") else {
            continue;
        };
        let imported = node_text(name_node, ctx.source);
        let local = child
            .child_by_field_name("alias")
            .map(|alias| node_text(alias, ctx.source))
            .filter(|alias| !alias.is_empty())
            .unwrap_or_else(|| imported.clone());
        ctx.add_import(local, Some(imported), specifier.to_string(), line);
    }
}

fn named_child<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    let mut found = None;
    for child in node.named_children(&mut cursor) {
        if child.kind() == kind {
            found = Some(child);
            break;
        }
    }
    found
}
