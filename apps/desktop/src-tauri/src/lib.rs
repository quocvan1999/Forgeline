//! Desktop shell. Commands only touch the local database and, when the user
//! indexes a git source, a single `git clone` of that URL.

use std::path::PathBuf;
use std::sync::Mutex;

use forgeline_core::{
    index_source as run_index, OverviewGraph, Project, ProjectStats, Source, Store,
    SymbolNeighbors, KIND_GIT_URL, KIND_LOCAL_PATH,
};
use tauri::{Manager, State};

struct AppState {
    store: Mutex<Store>,
    data_dir: PathBuf,
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    let store = lock(&state)?;
    store.list_projects().map_err(|error| error.to_string())
}

#[tauri::command]
fn create_project(state: State<'_, AppState>, name: String) -> Result<Project, String> {
    let mut store = lock(&state)?;
    store
        .create_project(&name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_project(state: State<'_, AppState>, id: String, name: String) -> Result<Project, String> {
    let mut store = lock(&state)?;
    store
        .rename_project(&id, &name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let data_dir = state.data_dir.clone();
    let mut store = lock(&state)?;
    store
        .delete_project(&data_dir, &id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_sources(state: State<'_, AppState>, project_id: String) -> Result<Vec<Source>, String> {
    let store = lock(&state)?;
    store
        .list_sources(&project_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn add_source(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
    kind: String,
    location: String,
) -> Result<Source, String> {
    if kind != KIND_LOCAL_PATH && kind != KIND_GIT_URL {
        return Err(format!("unknown source kind {kind}"));
    }
    let mut store = lock(&state)?;
    store
        .add_source(&project_id, &name, &kind, &location)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn index_source(state: State<'_, AppState>, source_id: String) -> Result<Source, String> {
    let data_dir = state.data_dir.clone();
    let mut store = lock(&state)?;
    run_index(&mut store, &data_dir, &source_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn project_stats(state: State<'_, AppState>, project_id: String) -> Result<ProjectStats, String> {
    let store = lock(&state)?;
    store
        .project_stats(&project_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn source_overview(
    state: State<'_, AppState>,
    source_id: String,
    focus: Option<String>,
    kind_filter: Option<String>,
    search: Option<String>,
) -> Result<OverviewGraph, String> {
    let store = lock(&state)?;
    store
        .source_overview(
            &source_id,
            focus.as_deref(),
            kind_filter.as_deref(),
            search.as_deref(),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn export_source_overview(
    state: State<'_, AppState>,
    source_id: String,
    focus: Option<String>,
) -> Result<String, String> {
    let store = lock(&state)?;
    store
        .export_source_overview_json(&source_id, focus.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn symbol_neighbors(
    state: State<'_, AppState>,
    symbol_id: String,
) -> Result<SymbolNeighbors, String> {
    let store = lock(&state)?;
    store
        .symbol_neighbors(&symbol_id)
        .map_err(|error| error.to_string())
}

fn lock<'a>(state: &'a State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, Store>, String> {
    state
        .store
        .lock()
        .map_err(|_| "database lock poisoned".to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = forgeline_core::default_data_dir();
            std::fs::create_dir_all(&data_dir)?;
            let store = Store::open(&forgeline_core::database_path(&data_dir))?;
            app.manage(AppState {
                store: Mutex::new(store),
                data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            create_project,
            rename_project,
            delete_project,
            list_sources,
            add_source,
            index_source,
            project_stats,
            source_overview,
            export_source_overview,
            symbol_neighbors
        ])
        .run(tauri::generate_context!())
        .expect("Forgeline window failed to start");
}
