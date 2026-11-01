use forgeline_graph::{
    index_contents, index_path, EdgeKind, FileInput, Provenance, SourceGraph, SymbolKind,
};

fn index(files: &[(&str, &str)]) -> SourceGraph {
    let inputs: Vec<FileInput<'_>> = files
        .iter()
        .map(|(relative_path, contents)| FileInput {
            relative_path,
            contents,
        })
        .collect();
    index_contents("src", "sample", &inputs, &[]).expect("index")
}

fn call<'a>(graph: &'a SourceGraph, from: &str, to: &str) -> Option<&'a forgeline_graph::Edge> {
    graph.call_edges().find(|edge| {
        let caller = graph.symbol(&edge.from_id);
        let callee = edge.to_id.as_deref().and_then(|id| graph.symbol(id));
        caller.is_some_and(|symbol| symbol.qualified_name == from)
            && callee.is_some_and(|symbol| symbol.qualified_name == to)
    })
}

#[test]
fn csharp_symbols_routes_and_exact_helper_call() {
    let graph = index(&[(
        "Controllers/ItemsController.cs",
        r#"
namespace Api.Controllers;

[Route("api/items")]
public class ItemsController {
  [HttpGet("{id}")]
  public string Get(int id) {
    return Helper.Run();
  }

  [HttpPost]
  public string Create() {
    return "x";
  }
}

public static class Helper {
  public static string Run() { return "ok"; }
}
"#,
    )]);

    assert!(graph
        .symbols()
        .any(|symbol| symbol.kind == SymbolKind::Namespace && symbol.name == "Api.Controllers"));
    assert!(graph
        .symbols()
        .any(|symbol| symbol.kind == SymbolKind::Class
            && symbol.qualified_name.ends_with("ItemsController")));
    assert!(graph
        .symbols()
        .any(|symbol| symbol.kind == SymbolKind::Method && symbol.name == "Get"));
    assert!(graph
        .symbols()
        .any(|symbol| symbol.kind == SymbolKind::Constructor || symbol.name == "ItemsController"));

    let edge = call(
        &graph,
        "Api.Controllers.ItemsController.Get",
        "Api.Controllers.Helper.Run",
    )
    .or_else(|| call(&graph, "ItemsController.Get", "Helper.Run"))
    .expect("Helper.Run exact");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert!(matches!(edge.detail.as_str(), "type" | "fqn" | "namespace"));

    let routes: Vec<_> = graph
        .files
        .iter()
        .flat_map(|file| file.routes.iter())
        .collect();
    assert!(routes.iter().any(|route| {
        route.http_methods.iter().any(|method| method == "GET")
            && route.template.contains("api/items")
    }));
    assert_eq!(
        graph
            .call_edges()
            .filter(|edge| edge.provenance != Provenance::Exact)
            .count(),
        0
    );
}

#[test]
fn csharp_service_method_is_unresolved_without_types() {
    let graph = index(&[(
        "ItemsController.cs",
        r#"
public class ItemsController {
  public string Get() {
    return _service.Find(1);
  }
}
public class ItemService {
  public string Find(int id) { return id.ToString(); }
}
"#,
    )]);
    assert!(call(&graph, "ItemsController.Get", "ItemService.Find").is_none());
    assert!(graph.unresolved_calls >= 1);
}

#[test]
fn react_component_import_and_fetch_literal() {
    let graph = index(&[
        (
            "api.js",
            "export async function getItem(id) {\n  return fetch(\"/api/items/\" + id);\n}\nexport async function createItem() {\n  return axios.post(\"/api/items\", {});\n}\n",
        ),
        (
            "App.jsx",
            "import { getItem } from \"./api\";\nexport function App() {\n  function onLoad() { getItem(1); }\n  return <main />;}\n",
        ),
    ]);
    assert!(graph
        .symbols()
        .any(|symbol| symbol.kind == SymbolKind::Component && symbol.name == "App"));
    let edge = call(&graph, "App.onLoad", "getItem").or_else(|| call(&graph, "onLoad", "getItem"));
    // onLoad nested under App → App.onLoad
    let edge = edge.expect("getItem import call");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert_eq!(edge.detail, "import");

    let http: Vec<_> = graph
        .files
        .iter()
        .flat_map(|file| file.http_calls.iter())
        .collect();
    assert!(http.iter().any(|item| item.client == "fetch"));
    assert!(http
        .iter()
        .any(|item| item.client == "axios" && item.url_literal == "/api/items"));
}

#[test]
fn typescript_import_call_is_exact_and_bare_name_is_unresolved() {
    let graph = index(&[
        (
            "math.ts",
            "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
        ),
        (
            "main.ts",
            "import { add } from \"./math\";\nexport function main(): void {\n  add(1, 2);\n  orphan();\n}\n",
        ),
        ("other.ts", "export function orphan(): void {}\n"),
    ]);
    let edge = call(&graph, "main", "add").expect("import");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert!(call(&graph, "main", "orphan").is_none());
    assert!(graph.unresolved_calls >= 1);
}

#[test]
fn python_same_file_still_exact_without_inferred() {
    let graph = index(&[(
        "app.py",
        "def helper():\n    return 1\n\ndef main():\n    helper()\n",
    )]);
    let edge = call(&graph, "main", "helper").expect("lexical");
    assert_eq!(edge.provenance, Provenance::Exact);
    let cross = index(&[
        ("app.py", "def run():\n    other()\n"),
        ("other.py", "def other():\n    return 1\n"),
    ]);
    assert!(call(&cross, "run", "other").is_none());
    assert_eq!(cross.unresolved_calls, 1);
}

#[test]
fn incremental_reuses_unchanged_file() {
    let inputs = [
        ("app.py", "def main():\n    helper()\n"),
        ("util.py", "def helper():\n    return 1\n"),
    ];
    // Same-file import style so helper is unresolved across files under exact-only.
    let first_inputs: Vec<FileInput<'_>> = [
        FileInput {
            relative_path: "app.py",
            contents: "from util import helper\n\ndef main():\n    helper()\n",
        },
        FileInput {
            relative_path: "util.py",
            contents: inputs[1].1,
        },
    ]
    .into_iter()
    .collect();
    let first = index_contents("src", "sample", &first_inputs, &[]).unwrap();
    assert_eq!(first.reindexed_files, 2);
    let second = index_contents("src", "sample", &first_inputs, &first.files).unwrap();
    assert_eq!(second.reused_files, 2);
    assert!(call(&second, "main", "helper").is_some());
}

#[test]
fn summaries_are_exact_only() {
    let graph = index(&[(
        "app.py",
        "def main():\n    helper()\n\ndef helper():\n    return 1\n",
    )]);
    let source = graph
        .summaries
        .iter()
        .find(|summary| summary.level == forgeline_graph::SummaryLevel::Source)
        .expect("source summary");
    assert!(source.body.contains("calls exact"));
    assert!(!source.body.contains("inferred"));
}

#[test]
fn index_path_skips_vendor_and_indexes_csharp() {
    let root = std::env::temp_dir().join(format!("forgeline-graph-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    std::fs::write(
        root.join("Hello.cs"),
        "public class Hello { public static void Run() {} public void Main() { Hello.Run(); } }",
    )
    .unwrap();
    std::fs::write(
        root.join("node_modules/pkg/hidden.cs"),
        "public class Hidden {}",
    )
    .unwrap();
    let first = index_path(&root, "src", "sample", &[]).unwrap();
    assert!(first.symbols().any(|symbol| symbol.name == "Hello"));
    assert!(first.symbols().all(|symbol| symbol.name != "Hidden"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn edge_kind_names_round_trip() {
    for kind in [
        EdgeKind::Contains,
        EdgeKind::Imports,
        EdgeKind::Calls,
        EdgeKind::Inherits,
        EdgeKind::TypeRef,
        EdgeKind::ExternalModule,
        EdgeKind::CrossSourceApi,
        EdgeKind::SharesSchema,
        EdgeKind::ConfigRef,
    ] {
        assert_eq!(EdgeKind::parse(kind.as_str()), Some(kind));
    }
}

#[test]
fn jsx_component_usage_and_reexport_are_exact() {
    let graph = index(&[
        (
            "Button.tsx",
            "export function Button() {\n  return <button />;\n}\n",
        ),
        (
            "widgets.ts",
            "export { Button } from \"./Button\";\n",
        ),
        (
            "App.tsx",
            "import { Button } from \"./widgets\";\nexport function App() {\n  return <Button />;\n}\n",
        ),
    ]);
    let edge = call(&graph, "App", "Button").expect("jsx via re-export");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert_eq!(edge.detail, "import");
}

#[test]
fn tsconfig_paths_resolve_exact_import() {
    let root = std::env::temp_dir().join(format!("forgeline-paths-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("tsconfig.json"),
        r#"{ "compilerOptions": { "baseUrl": ".", "paths": { "@lib/*": ["src/*"] } } }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/math.ts"),
        "export function add(a: number, b: number) { return a + b; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main.ts"),
        "import { add } from \"@lib/math\";\nexport function main() { add(1, 2); }\n",
    )
    .unwrap();
    let graph = index_path(&root, "src", "paths", &[]).unwrap();
    let edge = call(&graph, "main", "add").expect("path mapping");
    assert_eq!(edge.detail, "import");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn external_npm_import_is_external_module_edge_not_guessed_call() {
    let graph = index(&[(
        "app.tsx",
        "import React from \"react\";\nexport function App() { return React.createElement(\"div\"); }\n",
    )]);
    assert!(graph
        .edges
        .iter()
        .any(|edge| edge.kind == EdgeKind::ExternalModule && edge.to_name == "react"));
    // createElement on external React stays unresolved — never guessed.
    assert!(graph.unresolved_calls >= 1);
}

#[test]
fn roslyn_resolves_field_call_when_sdk_available() {
    let root = std::env::temp_dir().join(format!("forgeline-roslyn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Services")).unwrap();
    std::fs::create_dir_all(root.join("Controllers")).unwrap();
    std::fs::write(
        root.join("Services/ItemService.cs"),
        "namespace Api.Services;\npublic class ItemService {\n  public string Find(int id) { return id.ToString(); }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Controllers/ItemsController.cs"),
        "using Api.Services;\nnamespace Api.Controllers;\npublic class ItemsController {\n  private readonly ItemService _service;\n  public ItemsController(ItemService service) { _service = service; }\n  public string Get(int id) { return _service.Find(id); }\n}\n",
    )
    .unwrap();
    let helper = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/forgeline-roslyn/bin/Release/net8.0/forgeline-roslyn.dll");
    if helper.is_file() {
        std::env::set_var("FORGELINE_ROSLYN", &helper);
    }
    let graph = index_path(&root, "api", "api", &[]).unwrap();
    let edge = call(
        &graph,
        "Api.Controllers.ItemsController.Get",
        "Api.Services.ItemService.Find",
    )
    .or_else(|| call(&graph, "ItemsController.Get", "ItemService.Find"));
    if which_dotnet() && helper.is_file() {
        assert!(
            edge.is_some(),
            "expected Roslyn exact edge for Find, notice={:?}, calls={:?}",
            graph.resolver_notice,
            graph
                .call_edges()
                .map(|edge| (
                    graph
                        .symbol(&edge.from_id)
                        .map(|s| s.qualified_name.clone()),
                    edge.to_name.clone(),
                    edge.detail.clone()
                ))
                .collect::<Vec<_>>()
        );
        assert!(edge.unwrap().detail.contains("roslyn"));
    } else {
        assert!(
            graph.resolver_notice.is_some() || edge.is_none(),
            "without SDK keep tree-sitter-only and prefer a notice"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

fn which_dotnet() -> bool {
    std::env::var_os("DOTNET_ROOT").is_some()
        || std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).any(|dir| dir.join("dotnet").is_file()))
            .unwrap_or(false)
}
