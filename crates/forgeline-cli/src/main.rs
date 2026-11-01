//! Compact developer CLI. Queries default to exact edges so inferred names
//! do not drown out real callers.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use forgeline_core::{
    database_path, default_data_dir, index_source, CallerGroup, CoreError, EdgeHit, ProjectStats,
    Source, Store, SymbolReport, KIND_LOCAL_PATH,
};

#[derive(Parser)]
#[command(name = "forgeline", version, about = "Local code graph for Forgeline")]
struct Cli {
    /// SQLite database path. Defaults to the app data directory.
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    /// Project name. Required when more than one project is stored.
    #[arg(long, global = true)]
    project: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Index a local directory as a source in a project.
    Index { path: PathBuf },
    /// Print compact graph statistics.
    Stats,
    /// Show one symbol, its calls, and caller counts.
    Explain { symbol: String },
    /// List callers of a symbol. Inferred edges are hidden unless requested.
    Callers {
        symbol: String,
        /// Include inferred name-only edges.
        #[arg(long)]
        include_inferred: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("forgeline: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode, CoreError> {
    let cli = Cli::parse();
    let (mut store, data_dir) = open_store(cli.db)?;
    match cli.command {
        Command::Index { path } => cmd_index(&mut store, &data_dir, cli.project, path),
        Command::Stats => {
            let project = store.require_project(cli.project.as_deref())?;
            let stats = store.project_stats(&project.id)?;
            print_stats(&stats, &store.list_sources(&project.id)?);
            Ok(ExitCode::SUCCESS)
        }
        Command::Explain { symbol } => {
            let project = store.require_project(cli.project.as_deref())?;
            let reports = store.explain(&project.id, &symbol)?;
            if reports.is_empty() {
                println!("no symbol matches {symbol}");
                return Ok(ExitCode::from(1));
            }
            for report in &reports {
                print_explain(report);
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Callers {
            symbol,
            include_inferred,
        } => {
            let project = store.require_project(cli.project.as_deref())?;
            let groups = store.callers(&project.id, &symbol, include_inferred)?;
            if groups.is_empty() {
                println!("no symbol matches {symbol}");
                return Ok(ExitCode::from(1));
            }
            for group in &groups {
                print_callers(group, include_inferred);
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn open_store(db: Option<PathBuf>) -> Result<(Store, PathBuf), CoreError> {
    let (db_path, data_dir) = match db {
        Some(db) => {
            let parent = db
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            (db, parent)
        }
        None => {
            let data_dir = default_data_dir();
            (database_path(&data_dir), data_dir)
        }
    };
    if !data_dir.as_os_str().is_empty() {
        std::fs::create_dir_all(&data_dir)?;
    }
    let store = Store::open(&db_path)?;
    Ok((store, data_dir))
}

fn cmd_index(
    store: &mut Store,
    data_dir: &Path,
    project_name: Option<String>,
    path: PathBuf,
) -> Result<ExitCode, CoreError> {
    if !path.exists() {
        return Err(CoreError::PathMissing(path));
    }
    if !path.is_dir() {
        return Err(CoreError::NotADirectory(path));
    }
    let canonical = dunce::canonicalize(&path)?;
    let folder = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source");
    let project_name = project_name.unwrap_or_else(|| folder.to_string());
    let project = match store.get_project_by_name(&project_name)? {
        Some(project) => project,
        None => store.create_project(&project_name)?,
    };
    let source = store.add_source(
        &project.id,
        folder,
        KIND_LOCAL_PATH,
        &canonical.to_string_lossy(),
    )?;
    let indexed = index_source(store, data_dir, &source.id)?;
    print_index(&project.name, &indexed);
    Ok(ExitCode::SUCCESS)
}

fn print_index(project: &str, source: &Source) {
    println!(
        "indexed {project}/{name} files {files} reused {reused} reindexed {reindexed} removed {removed} skipped {skipped}",
        name = source.name,
        files = source.file_count,
        reused = source.reused_files,
        reindexed = source.reindexed_files,
        removed = source.removed_files,
        skipped = source.skipped_files
    );
    println!(
        "symbols {symbols} | calls exact {exact} inferred {inferred} | unresolved {unresolved} | ambiguous {ambiguous}",
        symbols = source.symbol_count,
        exact = source.exact_call_count,
        inferred = source.inferred_call_count,
        unresolved = source.unresolved_call_count,
        ambiguous = source.ambiguous_call_count
    );
}

fn print_stats(stats: &ProjectStats, sources: &[Source]) {
    println!("{}", stats.summary.trim_end());
    for source in sources {
        println!(
            "{name} {status} files {files} symbols {symbols} calls exact {exact} inferred {inferred} unresolved {unresolved}",
            name = source.name,
            status = source.status,
            files = source.file_count,
            symbols = source.symbol_count,
            exact = source.exact_call_count,
            inferred = source.inferred_call_count,
            unresolved = source.unresolved_call_count
        );
    }
}

fn print_explain(report: &SymbolReport) {
    let symbol = &report.symbol;
    println!(
        "{kind} {qualified} @ {path}:{start}-{end} {language} [{source}]",
        kind = symbol.kind,
        qualified = symbol.qualified_name,
        path = symbol.relative_path,
        start = symbol.start_line,
        end = symbol.end_line,
        language = symbol.language,
        source = symbol.source_name
    );
    if report.outgoing_calls.is_empty() {
        println!("  calls none");
    }
    for edge in &report.outgoing_calls {
        println!("  {}", format_edge(edge));
    }
    println!(
        "  callers exact {} inferred {}",
        report.exact_caller_count, report.inferred_caller_count
    );
}

fn print_callers(group: &CallerGroup, include_inferred: bool) {
    let symbol = &group.symbol;
    println!(
        "{kind} {qualified} @ {path}:{line} [{source}]",
        kind = symbol.kind,
        qualified = symbol.qualified_name,
        path = symbol.relative_path,
        line = symbol.start_line,
        source = symbol.source_name
    );
    if group.callers.is_empty() {
        println!("  no callers");
    }
    for edge in &group.callers {
        println!("  {}", format_edge(edge));
    }
    if !include_inferred && group.hidden_inferred > 0 {
        println!(
            "  {count} inferred caller(s) hidden; pass --include-inferred",
            count = group.hidden_inferred
        );
    }
    if group.truncated {
        println!("  truncated");
    }
}

fn format_edge(edge: &EdgeHit) -> String {
    format!(
        "{name} @ {path}:{line} {provenance} {confidence:.2} {detail} [{source}]",
        name = edge.symbol_qualified_name,
        path = edge.evidence_path,
        line = edge.evidence_line,
        provenance = edge.provenance,
        confidence = edge.confidence,
        detail = edge.detail,
        source = edge.source_name
    )
}
