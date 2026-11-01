//! Local project store for Forgeline.
//!
//! One SQLite file in the app data directory is the system of record.
//! Nothing in this crate opens a network connection except [`index_source`]
//! when the source itself is a git URL the user asked to clone.

#![forbid(unsafe_code)]

mod error;
mod git;
mod index;
mod migrate;
mod model;
mod overview;
mod paths;
mod store;
mod validate;

pub use error::CoreError;
pub use forgeline_graph::{EdgeKind, Provenance, SummaryLevel};
pub use index::index_source;
pub use model::*;
pub use overview::{NeighborHit, OverviewEdge, OverviewGraph, OverviewNode, SymbolNeighbors};
pub use paths::{database_path, default_data_dir, project_dir, source_checkout_dir};
pub use store::Store;
pub use validate::{validate_git_url, validate_name};
