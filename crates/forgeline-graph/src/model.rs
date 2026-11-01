use serde::{Deserialize, Serialize};

/// Languages the tree-sitter layer can extract.
/// Phase 1 focus is C# and JavaScript/React (including TypeScript/TSX).
/// Python remains supported but is not a priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    CSharp,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CSharp => "csharp",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
        }
    }

    pub fn grammar_name(self) -> &'static str {
        match self {
            Self::CSharp => "c_sharp",
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
    Namespace,
    Class,
    Interface,
    Record,
    Struct,
    Function,
    Method,
    Constructor,
    Property,
    /// React function or class component (PascalCase UI unit).
    Component,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Namespace => "namespace",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Record => "record",
            Self::Struct => "struct",
            Self::Function => "function",
            Self::Method => "method",
            Self::Constructor => "constructor",
            Self::Property => "property",
            Self::Component => "component",
        }
    }

    pub fn is_type_scope(self) -> bool {
        matches!(
            self,
            Self::Class | Self::Interface | Self::Record | Self::Struct | Self::Component
        )
    }

    pub fn is_callable(self) -> bool {
        matches!(
            self,
            Self::Function
                | Self::Method
                | Self::Constructor
                | Self::Component
                | Self::Class
                | Self::Record
                | Self::Struct
        )
    }
}

/// Edge kinds. Only exact edges are stored. Cross-source kinds are reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
    /// C# base type / interface implemented (Roslyn).
    Inherits,
    /// Exact type reference (field/param/property) resolved by Roslyn.
    TypeRef,
    /// Import of an external npm package (not guessed into a fake symbol).
    ExternalModule,
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
            Self::Inherits => "inherits",
            Self::TypeRef => "type_ref",
            Self::ExternalModule => "external_module",
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
            "inherits" => Self::Inherits,
            "type_ref" => Self::TypeRef,
            "external_module" => Self::ExternalModule,
            "cross_source_api" => Self::CrossSourceApi,
            "shares_schema" => Self::SharesSchema,
            "config_ref" => Self::ConfigRef,
            _ => return None,
        })
    }
}

/// Every stored edge is exact. Inferred edges are never written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Exact,
}

impl Provenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Self::Exact),
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
    /// `None` means the local name is the whole module/namespace import.
    pub imported_name: Option<String>,
    pub specifier: String,
    pub relative_path: String,
    pub line: u32,
}

/// One export binding declared or re-exported by a JS/TS module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Export {
    /// Exported name (`default` for default export).
    pub exported_name: String,
    /// Local symbol name when the export points at a declaration in this file.
    pub local_name: Option<String>,
    /// `export … from "mod"` / `export * from "mod"` source.
    pub from_specifier: Option<String>,
    /// True for `export * from`.
    #[serde(default)]
    pub star: bool,
    pub relative_path: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallSite {
    pub caller_id: String,
    pub callee_name: String,
    /// `None` for a bare call. Unknown receivers stay unresolved — never guessed.
    pub receiver: Option<String>,
    pub relative_path: String,
    pub line: u32,
    /// First string literal argument when present (fetch/axios URL, etc.).
    #[serde(default)]
    pub literal_arg: Option<String>,
}

/// ASP.NET-style HTTP endpoint extracted from controller attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteEndpoint {
    pub symbol_id: String,
    pub http_methods: Vec<String>,
    pub template: String,
    pub relative_path: String,
    pub line: u32,
}

/// Client HTTP call with a string URL literal (fetch, axios, HttpClient).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpCall {
    pub caller_id: String,
    pub client: String,
    pub method: Option<String>,
    pub url_literal: String,
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
    /// Short reason: `lexical`, `import`, `namespace`, `method`, `fqn`, `syntax`.
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
    #[serde(default)]
    pub exports: Vec<Export>,
    #[serde(default)]
    pub routes: Vec<RouteEndpoint>,
    #[serde(default)]
    pub http_calls: Vec<HttpCall>,
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
    /// Call sites that did not resolve to an exact edge. Not stored as edges.
    pub unresolved_calls: u32,
    pub ambiguous_calls: u32,
    pub reindexed_files: u32,
    pub reused_files: u32,
    pub removed_files: u32,
    pub skipped_files: u32,
    /// Vietnamese notice when Roslyn could not run (missing SDK, timeout, …).
    pub resolver_notice: Option<String>,
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
    if path.ends_with(".cs") {
        return Some(Language::CSharp);
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
