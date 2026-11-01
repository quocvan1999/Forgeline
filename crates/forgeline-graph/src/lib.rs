//! Tree-sitter indexing for one source tree.
//!
//! Parsers never execute the code they read. Only exact edges are stored:
//! imports/usings, same-scope or same-class resolution, and fully qualified
//! names. Unresolvable calls stay as unresolved references, not edges.

#![forbid(unsafe_code)]

mod error;
mod extract;
mod hashutil;
mod languages;
mod model;
mod resolve;
mod scan;
mod summarize;

pub use error::GraphError;
pub use hashutil::content_hash;
pub use model::*;
pub use scan::{index_contents, index_path, FileInput};

pub const EXTRACTOR_VERSION: u32 = 2;

/// Every stored edge is exact.
pub const CONFIDENCE_EXACT: f64 = 1.0;

/// Files larger than this are skipped. Tree-sitter is not asked to parse them.
pub const MAX_FILE_BYTES: u64 = 1_048_576;
/// Hard stop so pointing the indexer at a filesystem root fails closed.
pub const MAX_SOURCE_FILES: usize = 20_000;
