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
fn python_same_file_call_is_exact() {
    let graph = index(&[(
        "app.py",
        "def helper():\n    return 1\n\ndef main():\n    helper()\n    main()\n",
    )]);
    let edge = call(&graph, "main", "helper").expect("helper edge");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert_eq!(edge.detail, "lexical");
    assert_eq!(edge.evidence_line, 5);
    let recursive = call(&graph, "main", "main").expect("recursive edge");
    assert_eq!(recursive.provenance, Provenance::Exact);
}

#[test]
fn python_import_call_is_exact_and_missing_import_is_not_inferred() {
    let graph = index(&[
        (
            "app.py",
            "from utils import add\n\ndef run():\n    add(1, 2)\n",
        ),
        ("utils.py", "def something():\n    return 1\n"),
        ("other.py", "def add():\n    return 0\n"),
    ]);
    assert!(call(&graph, "run", "add").is_none());
    assert_eq!(graph.unresolved_calls, 1);
    assert_eq!(
        graph
            .call_edges()
            .filter(|edge| edge.provenance == Provenance::Inferred)
            .count(),
        0
    );
}

#[test]
fn python_resolved_import_and_namespace_are_exact() {
    let named = index(&[
        (
            "app.py",
            "from utils import add as plus\n\ndef run():\n    plus(1, 2)\n",
        ),
        ("utils.py", "def add(a, b):\n    return a + b\n"),
    ]);
    let edge = call(&named, "run", "add").expect("aliased import");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert_eq!(edge.detail, "import");
    assert!((edge.confidence - 1.0).abs() < f64::EPSILON);

    let namespace = index(&[
        ("app.py", "import utils\n\ndef run():\n    utils.add(1)\n"),
        ("utils.py", "def add(a):\n    return a\n"),
    ]);
    let edge = call(&namespace, "run", "add").expect("namespace");
    assert_eq!(edge.detail, "namespace");
    assert_eq!(edge.provenance, Provenance::Exact);
}

#[test]
fn python_relative_import_is_exact() {
    let graph = index(&[
        (
            "pkg/app.py",
            "from .utils import add\n\ndef run():\n    add()\n",
        ),
        ("pkg/utils.py", "def add():\n    return 1\n"),
    ]);
    let edge = call(&graph, "run", "add").expect("relative import");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert_eq!(edge.detail, "import");
}

#[test]
fn python_unique_bare_name_is_inferred_and_ambiguous_name_is_dropped() {
    let inferred = index(&[
        ("app.py", "def run():\n    other()\n"),
        ("other.py", "def other():\n    return 1\n"),
    ]);
    let edge = call(&inferred, "run", "other").expect("inferred");
    assert_eq!(edge.provenance, Provenance::Inferred);
    assert!((edge.confidence - 0.4).abs() < f64::EPSILON);

    let ambiguous = index(&[
        ("app.py", "def run():\n    other()\n"),
        ("b.py", "def other():\n    return 1\n"),
        ("c.py", "def other():\n    return 2\n"),
    ]);
    assert!(call(&ambiguous, "run", "other").is_none());
    assert_eq!(ambiguous.ambiguous_calls, 1);
    assert_eq!(ambiguous.call_edges().count(), 0);
}

#[test]
fn python_method_call_is_exact_and_untyped_attribute_is_not_inferred() {
    let graph = index(&[(
        "box.py",
        "class Box:\n    def ping(self):\n        return 1\n    def run(self):\n        self.ping()\n",
    )]);
    let edge = call(&graph, "Box.run", "Box.ping").expect("method");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert_eq!(edge.detail, "method");
    assert!(graph
        .symbols()
        .any(|symbol| symbol.kind == SymbolKind::Method && symbol.qualified_name == "Box.ping"));

    let untyped = index(&[
        ("app.py", "def run(client):\n    client.get('/x')\n"),
        (
            "client.py",
            "class Client:\n    def get(self, path):\n        return path\n",
        ),
    ]);
    assert_eq!(untyped.call_edges().count(), 0);
    assert_eq!(untyped.unresolved_calls, 1);
}

#[test]
fn typescript_and_javascript_calls_resolve() {
    let typescript = index(&[
        (
            "math.ts",
            "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
        ),
        (
            "main.ts",
            "import { add } from \"./math\";\n\nexport function main(): void {\n  add(1, 2);\n  console.log(\"start\");\n}\n",
        ),
    ]);
    let edge = call(&typescript, "main", "add").expect("ts import");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert_eq!(edge.detail, "import");
    assert_eq!(typescript.unresolved_calls, 1);
    assert!(typescript.symbols().all(|symbol| symbol.name != "log"));

    let javascript = index(&[(
        "greet.js",
        "function greet(name) {\n  return helper(name);\n}\nfunction helper(name) {\n  return name;\n}\n",
    )]);
    let edge = call(&javascript, "greet", "helper").expect("js lexical");
    assert_eq!(edge.provenance, Provenance::Exact);

    let tsx = index(&[
        (
            "Button.tsx",
            "export function Button() {\n  return \"ok\";\n}\n",
        ),
        (
            "App.tsx",
            "import { Button } from \"./Button\";\nexport function App() {\n  return Button();\n}\n",
        ),
    ]);
    let edge = call(&tsx, "App", "Button").expect("tsx import");
    assert_eq!(edge.provenance, Provenance::Exact);

    let method = index(&[(
        "box.ts",
        "export class Box {\n  ping() { return 1; }\n  run() { this.ping(); }\n}\n",
    )]);
    let edge = call(&method, "Box.run", "Box.ping").expect("ts method");
    assert_eq!(edge.detail, "method");
}

#[test]
fn arrow_function_and_class_constructor_are_symbols() {
    let graph = index(&[(
        "main.ts",
        "export class Service {}\nexport const main = () => {\n  return Service();\n};\n",
    )]);
    let edge = call(&graph, "main", "Service").expect("constructor");
    assert_eq!(edge.provenance, Provenance::Exact);
    assert_eq!(edge.kind, EdgeKind::Calls);
}

#[test]
fn incremental_reuses_unchanged_file_and_drops_deleted_file() {
    let inputs = [
        ("app.py", "def main():\n    helper()\n"),
        ("util.py", "def helper():\n    return 1\n"),
    ];
    let first_inputs: Vec<FileInput<'_>> = inputs
        .iter()
        .map(|(relative_path, contents)| FileInput {
            relative_path,
            contents,
        })
        .collect();
    let first = index_contents("src", "sample", &first_inputs, &[]).unwrap();
    assert_eq!(first.reindexed_files, 2);
    assert_eq!(first.reused_files, 0);

    let second = index_contents("src", "sample", &first_inputs, &first.files).unwrap();
    assert_eq!(second.reused_files, 2);
    assert_eq!(second.reindexed_files, 0);
    assert!(call(&second, "main", "helper").is_some());

    let changed = "def main():\n    helper()\n    helper()\n";
    let next = [
        FileInput {
            relative_path: "app.py",
            contents: changed,
        },
        FileInput {
            relative_path: "util.py",
            contents: inputs[1].1,
        },
    ];
    let third = index_contents("src", "sample", &next, &second.files).unwrap();
    assert_eq!(third.reindexed_files, 1);
    assert_eq!(third.reused_files, 1);

    let only_app = [FileInput {
        relative_path: "app.py",
        contents: changed,
    }];
    let fourth = index_contents("src", "sample", &only_app, &third.files).unwrap();
    assert_eq!(fourth.removed_files, 1);
    assert!(fourth
        .symbols()
        .all(|symbol| symbol.relative_path != "util.py"));
}

#[test]
fn summaries_are_layered_and_small() {
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
    assert!(source.token_estimate > 0);
    assert!(source.token_estimate < 200);
    let symbol = graph
        .summaries
        .iter()
        .find(|summary| summary.body.starts_with("function main "))
        .expect("symbol summary");
    assert!(symbol.body.contains("exact"));
    assert!(symbol.body.contains("lexical"));
}

#[test]
fn index_path_skips_vendor_trees_and_hashes_incrementally() {
    let root = std::env::temp_dir().join(format!("forgeline-graph-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    std::fs::write(root.join("app.py"), "def main():\n    return helper()\n").unwrap();
    std::fs::write(root.join("lib.py"), "def helper():\n    return 1\n").unwrap();
    std::fs::write(
        root.join("node_modules/pkg/hidden.py"),
        "def hidden():\n    return 1\n",
    )
    .unwrap();
    let first = index_path(&root, "src", "sample", &[]).unwrap();
    assert!(first.symbols().any(|symbol| symbol.name == "main"));
    assert!(first.symbols().all(|symbol| symbol.name != "hidden"));
    let second = index_path(&root, "src", "sample", &first.files).unwrap();
    assert_eq!(second.reused_files, 2);
    assert_eq!(second.reindexed_files, 0);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn refuses_to_index_filesystem_root() {
    let root = std::path::Path::new("/");
    if root.parent().is_some() {
        return;
    }
    let error = index_path(root, "src", "sample", &[]).unwrap_err();
    assert!(matches!(
        error,
        forgeline_graph::GraphError::FilesystemRoot(_)
    ));
}

#[test]
fn edge_kind_names_round_trip() {
    for kind in [
        EdgeKind::Contains,
        EdgeKind::Imports,
        EdgeKind::Calls,
        EdgeKind::CrossSourceApi,
        EdgeKind::SharesSchema,
        EdgeKind::ConfigRef,
    ] {
        assert_eq!(EdgeKind::parse(kind.as_str()), Some(kind));
    }
}
