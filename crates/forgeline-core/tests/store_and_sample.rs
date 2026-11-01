use std::path::PathBuf;

use forgeline_core::{index_source, CoreError, Store, KIND_GIT_URL, KIND_LOCAL_PATH, STATUS_READY};

fn open_store() -> (tempfile::TempDir, Store, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let store = Store::open(&data_dir.join("forgeline.db")).unwrap();
    (dir, store, data_dir)
}

#[test]
fn project_lifecycle_and_rejects_bad_git_urls() {
    let (_dir, mut store, data_dir) = open_store();
    let project = store.create_project("Shop").unwrap();
    assert_eq!(store.list_projects().unwrap().len(), 1);
    store.rename_project(&project.id, "Cửa hàng").unwrap();
    assert!(matches!(
        store.create_project("Cửa hàng"),
        Err(CoreError::DuplicateName(_))
    ));
    assert!(store.create_project(" ").is_err());
    assert!(store
        .add_source(&project.id, "web", KIND_GIT_URL, "file:///tmp/repo")
        .is_err());
    assert!(store
        .add_source(
            &project.id,
            "web",
            KIND_GIT_URL,
            "https://user:token@github.com/org/repo.git"
        )
        .is_err());
    let created = store
        .add_source(
            &project.id,
            "web",
            KIND_GIT_URL,
            "https://github.com/org/repo.git",
        )
        .unwrap();
    assert_eq!(created.status, "pending");
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
    assert!(store.list_projects().unwrap().is_empty());
}

#[test]
fn indexes_sample_and_keeps_inferred_callers_out_of_the_default_query() {
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
    assert_eq!(indexed.file_count, 5);
    assert_eq!(indexed.exact_call_count, 3);
    assert_eq!(indexed.inferred_call_count, 1);
    assert_eq!(indexed.unresolved_call_count, 1);
    assert_eq!(indexed.reindexed_files, 5);

    let again = index_source(&mut store, &data_dir, &source.id).unwrap();
    assert_eq!(again.reused_files, 5);
    assert_eq!(again.reindexed_files, 0);
    assert_eq!(again.exact_call_count, 3);

    let health = store.callers(&project.id, "health", false).unwrap();
    assert_eq!(health.len(), 1);
    assert_eq!(health[0].callers.len(), 1);
    assert_eq!(health[0].callers[0].provenance, "exact");
    assert_eq!(health[0].callers[0].symbol_qualified_name, "main");
    assert_eq!(health[0].callers[0].detail, "import");

    let orphan_exact = store.callers(&project.id, "orphan", false).unwrap();
    assert!(orphan_exact[0].callers.is_empty());
    assert_eq!(orphan_exact[0].hidden_inferred, 1);
    let orphan = store.callers(&project.id, "orphan", true).unwrap();
    assert_eq!(orphan[0].callers.len(), 1);
    assert_eq!(orphan[0].callers[0].provenance, "inferred");

    let web = store.callers(&project.id, "getHealth", false).unwrap();
    assert_eq!(web[0].callers[0].detail, "import");
    assert!(web[0].callers[0].evidence_path.ends_with("web/main.ts"));

    let reports = store.explain(&project.id, "main").unwrap();
    assert_eq!(reports.len(), 2);
    let python_main = reports
        .iter()
        .find(|report| report.symbol.relative_path.ends_with("backend/app.py"))
        .unwrap();
    assert!(python_main
        .outgoing_calls
        .iter()
        .any(|edge| edge.symbol_name == "health" && edge.provenance == "exact"));
    assert!(python_main
        .outgoing_calls
        .iter()
        .any(|edge| edge.symbol_name == "orphan" && edge.provenance == "inferred"));

    assert!(store.explain(&project.id, "log").unwrap().is_empty());
    let stats = store.project_stats(&project.id).unwrap();
    assert!(stats.summary.contains("read order"));
    assert!(stats.summary.contains("calls exact 3 inferred 1"));
    assert!(stats.symbol_count > 0);

    store.delete_source(&data_dir, &source.id).unwrap();
    assert!(store.list_sources(&project.id).unwrap().is_empty());
    assert_eq!(store.project_stats(&project.id).unwrap().symbol_count, 0);
}
