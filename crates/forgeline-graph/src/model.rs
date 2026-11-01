use serde::{Deserialize, Serialize};

/// Languages the tree-sitter layer can extract in this scaffold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Tsx,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
        }
    }

    pub fn grammar_name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
        }
    }
}

/// Symbol kinds extracted from syntax. Modules are one per indexed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Module,
    Function,
    Class,
    Method,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Function => "function",
            Self::Class => "class",
            Self::Method => "method",
        }
    }
}

/// Edge kinds. The cross-source kinds are reserved so later phases do not
/// overload [`EdgeKind::Calls`] with a second meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
    /// API client call to a backend route. Not emitted by the tree-sitter layer.
    CrossSourceApi,
    /// Shared model or schema across sources. Not emitted yet.
    SharesSchema,
    /// Config or env reference across sources. Not emitted yet.
    ConfigRef,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Imports => "imports",
            Self::Calls => "calls",
            Self::CrossSourceApi => "cross_source_api",
            Self::SharesSchema => "shares_schema",
            Self::ConfigRef => "config_ref",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "contains" => Self::Contains,
            "imports" => Self::Imports,
            "calls" => Self::Calls,
            "cross_source_api" => Self::CrossSourceApi,
            "shares_schema" => Self::SharesSchema,
            "config_ref" => Self::ConfigRef,
            _ => return None,
        })
    }
}

/// How an edge was produced. Default path and caller queries keep only [`Exact`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Syntax, scope, or an explicit import binding resolved to one symbol.
    Exact,
    /// A single bare-name match outside the local scope. Easy to get wrong.
    Inferred,
}

impl Provenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Inferred => "inferred",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Self::Exact),
            "inferred" => Some(Self::Inferred),
            _ => None,
        }
    }
}

/// Read order for an agent that should spend as few tokens as possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryLevel {
    Project,
    Source,
    Module,
    Symbol,
}

impl SummaryLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Source => "source",
            Self::Module => "module",
            Self::Symbol => "symbol",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "project" => Self::Project,
            "source" => Self::Source,
            "module" => Self::Module,
            "symbol" => Self::Symbol,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub language: Language,
    pub relative_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Import {
    /// Function, method, or module that owns this binding.
    pub scope_id: String,
    pub local_name: String,
    /// `None` means the local name is the whole module (namespace import).
    pub imported_name: Option<String>,
    pub specifier: String,
    pub relative_path: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallSite {
    pub caller_id: String,
    pub callee_name: String,
    /// `None` for a bare call. Attribute calls that are not `self`/`this`/`cls`
    /// or a namespace import stay unresolved instead of becoming inferred edges.
    pub receiver: Option<String>,
    pub relative_path: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub kind: EdgeKind,
    pub from_id: String,
    pub to_id: Option<String>,
    pub to_name: String,
    pub provenance: Provenance,
    pub confidence: f64,
    pub evidence_path: String,
    pub evidence_line: u32,
    /// Short reason: `lexical`, `import`, `namespace`, `method`, `name`, `syntax`.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedFile {
    pub extractor_version: u32,
    pub relative_path: String,
    pub language: Language,
    pub content_hash: String,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub calls: Vec<CallSite>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub level: SummaryLevel,
    pub owner_id: String,
    pub body: String,
    pub token_estimate: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceGraph {
    pub source_id: String,
    pub source_label: String,
    pub files: Vec<ExtractedFile>,
    pub edges: Vec<Edge>,
    pub summaries: Vec<Summary>,
    pub unresolved_calls: u32,
    pub ambiguous_calls: u32,
    pub reindexed_files: u32,
    pub reused_files: u32,
    pub removed_files: u32,
    pub skipped_files: u32,
}

impl SourceGraph {
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.files.iter().flat_map(|file| file.symbols.iter())
    }

    pub fn symbol(&self, id: &str) -> Option<&Symbol> {
        self.symbols().find(|symbol| symbol.id == id)
    }

    pub fn call_edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Calls)
    }
}

/// Rough token count for budgeting agent reads. Four characters per token.
pub fn token_estimate(body: &str) -> u32 {
    let chars = body.chars().count();
    u32::try_from(chars.div_ceil(4)).unwrap_or(u32::MAX)
}

pub fn language_for_path(relative_path: &str) -> Option<Language> {
    let path = relative_path.replace('\\', "/");
    if path.ends_with(".d.ts") {
        return None;
    }
    if path.ends_with(".py") {
        return Some(Language::Python);
    }
    if path.ends_with(".tsx") {
        return Some(Language::Tsx);
    }
    if path.ends_with(".ts") {
        return Some(Language::TypeScript);
    }
    if path.ends_with(".jsx")
        || path.ends_with(".js")
        || path.ends_with(".mjs")
        || path.ends_with(".cjs")
    {
        return Some(Language::JavaScript);
    }
    None
}

pub fn symbol_id(
    source_id: &str,
    relative_path: &str,
    kind: SymbolKind,
    qualified_name: &str,
    start_line: u32,
) -> String {
    format!(
        "{source_id}::{relative_path}::{kind}::{qualified_name}@{start_line}",
        kind = kind.as_str()
    )
}
