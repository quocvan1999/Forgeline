use std::path::PathBuf;

use forgeline_core::{index_source, CoreError, Store, KIND_GIT_URL, KIND_LOCAL_PATH, STATUS_READY};

fn open_store() -> (tempfile::TempDir, Store, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let store = Store::open(&data_dir.join("forgeline.db")).unwrap();
    (dir, store, data_dir)
}

#[test]
fn project_names_are_unique_case_insensitive_with_vietnamese_error() {
    let (_dir, mut store, data_dir) = open_store();
    let project = store.create_project("Shop").unwrap();
    let err = store.create_project("shop").unwrap_err();
    assert!(matches!(err, CoreError::DuplicateName(_)));
    assert!(err.to_string().contains("không phân biệt hoa thường"));
    store.rename_project(&project.id, "Cửa hàng").unwrap();
    assert!(matches!(
        store.create_project("cửa hàng"),
        Err(CoreError::DuplicateName(_))
    ));
    assert!(store
        .add_source(&project.id, "web", KIND_GIT_URL, "file:///tmp/repo")
        .is_err());
    let created = store
        .add_source(
            &project.id,
            "web",
            KIND_GIT_URL,
            "https://github.com/org/repo.git",
        )
        .unwrap();
    let again = store
        .add_source(
            &project.id,
            "web",
            KIND_GIT_URL,
            "https://github.com/org/repo.git",
        )
        .unwrap();
    assert_eq!(again.id, created.id);
    store.delete_project(&data_dir, &project.id).unwrap();
}

#[test]
fn indexes_csharp_react_sample_with_exact_callers_only() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample");
    let (_dir, mut store, data_dir) = open_store();
    let project = store.create_project("sample").unwrap();
    let source = store
        .add_source(
            &project.id,
            "sample",
            KIND_LOCAL_PATH,
            root.to_str().unwrap(),
        )
        .unwrap();
    let indexed = index_source(&mut store, &data_dir, &source.id).unwrap();
    assert_eq!(indexed.status, STATUS_READY);
    assert!(indexed.file_count >= 5);
    assert_eq!(indexed.inferred_call_count, 0);
    assert!(indexed.exact_call_count >= 2);
    assert!(indexed.unresolved_call_count >= 1);

    let again = index_source(&mut store, &data_dir, &source.id).unwrap();
    assert_eq!(again.reused_files, indexed.file_count);
    assert_eq!(again.reindexed_files, 0);

    let run = store.callers(&project.id, "Run").unwrap();
    assert!(!run.is_empty());
    assert!(run[0].callers.iter().all(|edge| edge.provenance == "exact"));

    let find = store.callers(&project.id, "Find").unwrap();
    assert!(
        find.iter().any(|group| {
            group
                .callers
                .iter()
                .any(|edge| edge.detail.contains("roslyn") || edge.detail == "import")
        }) || indexed.status_message.is_some(),
        "Roslyn Find edge or Vietnamese SDK notice"
    );

    let get_item = store.callers(&project.id, "getItem").unwrap();
    assert!(get_item[0]
        .callers
        .iter()
        .any(|edge| edge.detail == "import"));

    let reports = store.explain(&project.id, "App").unwrap();
    assert!(!reports.is_empty());
    assert!(reports[0].symbol.kind == "component" || reports[0].symbol.name == "App");

    let overview = store.source_overview(&source.id, None, None, None).unwrap();
    assert!(!overview.nodes.is_empty());
    let json = store.export_source_overview_json(&source.id, None).unwrap();
    assert!(json.contains("sourceId"));

    let stats = store.project_stats(&project.id).unwrap();
    assert!(stats.summary.contains("calls exact"));
    assert!(!stats.summary.contains("inferred"));
    assert!(stats.summary.contains("unresolved"));
}
