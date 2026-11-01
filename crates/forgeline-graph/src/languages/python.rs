use tree_sitter::Node;

use crate::extract::{
    callee_parts, field_nodes, line_number, node_text, pop_scope, push_scope, walk_children, Ctx,
};
use crate::model::SymbolKind;

pub(crate) fn walk(node: Node<'_>, ctx: &mut Ctx<'_>) {
    match node.kind() {
        "function_definition" => define_function(node, ctx),
        "class_definition" => define_class(node, ctx),
        "decorated_definition" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() != "decorator" {
                    walk(child, ctx);
                }
            }
        }
        "import_statement" => record_import(node, ctx),
        "import_from_statement" => record_import_from(node, ctx),
        "call" => {
            record_call(node, ctx);
            walk_children(node, ctx, walk);
        }
        _ => walk_children(node, ctx, walk),
    }
}

fn define_function(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(name_node) = node.child_by_field_name("name") else {
        walk_children(node, ctx, walk);
        return;
    };
    let name = ctx.text(name_node);
    if name.is_empty() {
        walk_children(node, ctx, walk);
        return;
    }
    let kind = crate::extract::nested_kind(ctx, SymbolKind::Function);
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
    push_scope(ctx, &name, SymbolKind::Class, node);
    if let Some(body) = node.child_by_field_name("body") {
        walk(body, ctx);
    }
    pop_scope(ctx);
}

fn record_call(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    if let Some((receiver, callee)) = callee_parts(func, ctx.source) {
        let line = line_number(node.start_position().row);
        ctx.add_call(receiver, callee, line);
    }
}

fn record_import(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let line = line_number(node.start_position().row);
    for name_node in field_nodes(node, "name") {
        if let Some((local, specifier)) = import_binding(name_node, ctx.source) {
            ctx.add_import(local, None, specifier, line);
        }
    }
}

fn record_import_from(node: Node<'_>, ctx: &mut Ctx<'_>) {
    let Some(module_node) = node.child_by_field_name("module_name") else {
        return;
    };
    let module = node_text(module_node, ctx.source);
    let line = line_number(node.start_position().row);
    let relative_only = module.chars().all(|ch| ch == '.');
    for name_node in field_nodes(node, "name") {
        if name_node.kind() == "wildcard_import" {
            continue;
        }
        let Some((local, imported)) = imported_name(name_node, ctx.source) else {
            continue;
        };
        if relative_only {
            let specifier = format!("{module}{imported}");
            ctx.add_import(local, None, specifier, line);
        } else {
            ctx.add_import(local, Some(imported), module.clone(), line);
        }
    }
}

fn import_binding(node: Node<'_>, source: &str) -> Option<(String, String)> {
    match node.kind() {
        "dotted_name" => {
            let specifier = node_text(node, source);
            let local = specifier.split('.').next()?.to_string();
            Some((local, specifier))
        }
        "aliased_import" => {
            let specifier = node
                .child_by_field_name("name")
                .map(|name| node_text(name, source))?;
            let local = node
                .child_by_field_name("alias")
                .map(|alias| node_text(alias, source))
                .unwrap_or_else(|| specifier.split('.').next().unwrap_or("").to_string());
            Some((local, specifier))
        }
        _ => None,
    }
}

fn imported_name(node: Node<'_>, source: &str) -> Option<(String, String)> {
    match node.kind() {
        "dotted_name" | "identifier" => {
            let name = node_text(node, source);
            Some((name.clone(), name))
        }
        "aliased_import" => {
            let imported = node
                .child_by_field_name("name")
                .map(|name| node_text(name, source))?;
            let local = node
                .child_by_field_name("alias")
                .map(|alias| node_text(alias, source))
                .unwrap_or_else(|| imported.clone());
            Some((local, imported))
        }
        _ => None,
    }
}
