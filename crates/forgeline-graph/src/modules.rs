//! Exact JS/TS module resolution: relative imports, index files, extensions,
//! tsconfig/jsconfig `baseUrl`/`paths` (with `extends`), and local package.json
//! `exports`/`main`. External npm packages become [`EdgeKind::ExternalModule`]
//! or stay unresolved — never guessed into a fake symbol.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::model::{ExtractedFile, Language, Symbol, SymbolKind};

const EXTENSIONS: &[&str] = &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".d.ts"];
const INDEX_EXTENSIONS: &[&str] = &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"];

#[derive(Debug, Default, Clone)]
pub(crate) struct ModuleResolver {
    /// relative path -> absolute-ish relative path of resolved file
    path_set: HashMap<String, ()>,
    configs: Vec<TsConfig>,
    packages: Vec<PackageJson>,
}

#[derive(Debug, Clone)]
struct TsConfig {
    /// Directory containing the config, relative to source root (`""` for root).
    dir: String,
    base_url: Option<String>,
    paths: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone)]
struct PackageJson {
    /// Directory of the package relative to source root.
    dir: String,
    name: String,
    main: Option<String>,
    exports: Option<Value>,
}

impl ModuleResolver {
    pub(crate) fn from_root(root: &Path, files: &[ExtractedFile]) -> Self {
        let mut path_set = HashMap::new();
        for file in files {
            path_set.insert(file.relative_path.clone(), ());
        }
        // Also index non-extracted companions (.d.ts, json configs) if present on disk.
        let mut configs = Vec::new();
        let mut packages = Vec::new();
        if root.is_dir() {
            collect_configs(root, root, &mut configs);
            collect_packages(root, root, &mut packages);
            // Include on-disk JS/TS paths that may not yet be in `files` (shouldn't happen).
            for entry in walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                if let Some(rel) = relative_slash(root, entry.path()) {
                    if language_like(&rel) {
                        path_set.entry(rel).or_insert(());
                    }
                }
            }
        }
        Self {
            path_set,
            configs,
            packages,
        }
    }

    pub(crate) fn from_files_only(files: &[ExtractedFile]) -> Self {
        let mut path_set = HashMap::new();
        for file in files {
            path_set.insert(file.relative_path.clone(), ());
        }
        Self {
            path_set,
            configs: Vec::new(),
            packages: Vec::new(),
        }
    }

    pub(crate) fn resolve_specifier(&self, from_file: &str, specifier: &str) -> ResolveResult {
        let spec = specifier.trim();
        if spec.is_empty() {
            return ResolveResult::Unresolved;
        }
        if let Some(path) = self.resolve_relative(from_file, spec) {
            return ResolveResult::File(path);
        }
        // Path aliases (`@lib/…`) and baseUrl must win over the external-package bucket.
        if let Some(path) = self.resolve_tsconfig(from_file, spec) {
            return ResolveResult::File(path);
        }
        if is_external_package(spec) {
            if let Some(path) = self.resolve_workspace_package(spec) {
                return ResolveResult::File(path);
            }
            return ResolveResult::External(spec.to_string());
        }
        ResolveResult::Unresolved
    }

    fn resolve_relative(&self, from_file: &str, specifier: &str) -> Option<String> {
        if !(specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier == "."
            || specifier == "..")
        {
            return None;
        }
        let joined = normalize_path(&join_path(&parent_path(from_file), specifier));
        self.expand_candidates(&joined)
    }

    fn resolve_tsconfig(&self, from_file: &str, specifier: &str) -> Option<String> {
        let from_dir = parent_path(from_file);
        // Nearest config: longest matching dir prefix.
        let mut configs: Vec<&TsConfig> = self
            .configs
            .iter()
            .filter(|cfg| {
                from_dir == cfg.dir
                    || from_dir.starts_with(&format!("{}/", cfg.dir))
                    || cfg.dir.is_empty()
            })
            .collect();
        configs.sort_by_key(|cfg| std::cmp::Reverse(cfg.dir.len()));
        for cfg in configs {
            for (pattern, targets) in &cfg.paths {
                if let Some(mapped) = apply_path_mapping(pattern, targets, specifier) {
                    for candidate in mapped {
                        let base = if let Some(base_url) = &cfg.base_url {
                            join_path(&cfg.dir, &join_path(base_url, &candidate))
                        } else {
                            join_path(&cfg.dir, &candidate)
                        };
                        if let Some(path) = self.expand_candidates(&normalize_path(&base)) {
                            return Some(path);
                        }
                    }
                }
            }
            if let Some(base_url) = &cfg.base_url {
                let base = join_path(&cfg.dir, &join_path(base_url, specifier));
                if let Some(path) = self.expand_candidates(&normalize_path(&base)) {
                    return Some(path);
                }
            }
        }
        None
    }

    fn resolve_workspace_package(&self, specifier: &str) -> Option<String> {
        // Exact package name or name/subpath for local workspace packages only.
        for pkg in &self.packages {
            if specifier == pkg.name {
                if let Some(via_exports) = self.resolve_exports(pkg, ".") {
                    return Some(via_exports);
                }
                if let Some(main) = &pkg.main {
                    let joined = normalize_path(&join_path(&pkg.dir, main));
                    if let Some(path) = self.expand_candidates(&joined) {
                        return Some(path);
                    }
                }
                let index = normalize_path(&join_path(&pkg.dir, "index"));
                if let Some(path) = self.expand_candidates(&index) {
                    return Some(path);
                }
            } else if let Some(rest) = specifier.strip_prefix(&format!("{}/", pkg.name)) {
                if let Some(via_exports) = self.resolve_exports(pkg, &format!("./{rest}")) {
                    return Some(via_exports);
                }
                let joined = normalize_path(&join_path(&pkg.dir, rest));
                if let Some(path) = self.expand_candidates(&joined) {
                    return Some(path);
                }
            }
        }
        None
    }

    fn resolve_exports(&self, pkg: &PackageJson, subpath: &str) -> Option<String> {
        let exports = pkg.exports.as_ref()?;
        let target = match exports {
            Value::String(path) if subpath == "." => path.clone(),
            Value::Object(map) => {
                if let Some(value) = map.get(subpath) {
                    export_target(value)?
                } else if subpath == "." {
                    export_target(map.get(".")?)?
                } else {
                    // Simple pattern: "./*" -> "./*"
                    let mut found = None;
                    for (key, value) in map {
                        if let Some(mapped) = apply_export_pattern(key, value, subpath) {
                            found = Some(mapped);
                            break;
                        }
                    }
                    found?
                }
            }
            _ => return None,
        };
        let joined = normalize_path(&join_path(&pkg.dir, &target));
        self.expand_candidates(&joined)
    }

    fn expand_candidates(&self, stem: &str) -> Option<String> {
        let mut candidates = Vec::new();
        if has_known_extension(stem) {
            candidates.push(stem.to_string());
        } else {
            for ext in EXTENSIONS {
                candidates.push(format!("{stem}{ext}"));
            }
            for ext in INDEX_EXTENSIONS {
                candidates.push(format!("{stem}/index{ext}"));
            }
        }
        let mut matched: Vec<String> = candidates
            .into_iter()
            .filter(|path| self.path_set.contains_key(path))
            .collect();
        // Prefer implementation over .d.ts when both exist.
        matched.sort_by_key(|path| path.ends_with(".d.ts"));
        matched.into_iter().next()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveResult {
    File(String),
    External(String),
    Unresolved,
}

/// Resolve an exported name through local declarations and re-exports (exact only).
pub(crate) fn resolve_export<'a>(
    files: &'a [ExtractedFile],
    resolver: &ModuleResolver,
    file: &'a ExtractedFile,
    exported_name: &str,
    depth: usize,
) -> Option<&'a Symbol> {
    if depth > 16 {
        return None;
    }
    // Local declaration with matching export or matching local name.
    for export in &file.exports {
        if export.star {
            continue;
        }
        if export.exported_name != exported_name {
            continue;
        }
        if let Some(from) = &export.from_specifier {
            let ResolveResult::File(target_path) =
                resolver.resolve_specifier(&file.relative_path, from)
            else {
                return None;
            };
            let target = files
                .iter()
                .find(|item| item.relative_path == target_path)?;
            let name = export.local_name.as_deref().unwrap_or(exported_name);
            return resolve_export(files, resolver, target, name, depth + 1);
        }
        if let Some(local) = &export.local_name {
            return module_level_symbol(file, local);
        }
        return module_level_symbol(file, exported_name);
    }

    // `export * from` — unique match across star targets.
    let mut star_hits: Vec<&Symbol> = Vec::new();
    for export in &file.exports {
        if !export.star {
            continue;
        }
        let Some(from) = &export.from_specifier else {
            continue;
        };
        let ResolveResult::File(target_path) =
            resolver.resolve_specifier(&file.relative_path, from)
        else {
            continue;
        };
        let Some(target) = files.iter().find(|item| item.relative_path == target_path) else {
            continue;
        };
        if let Some(symbol) = resolve_export(files, resolver, target, exported_name, depth + 1) {
            star_hits.push(symbol);
        }
    }
    star_hits.sort_by(|a, b| a.id.cmp(&b.id));
    star_hits.dedup_by(|a, b| a.id == b.id);
    if star_hits.len() == 1 {
        return Some(star_hits[0]);
    }

    // Fallback: module-level symbol of that name (common for `export function Foo`).
    module_level_symbol(file, exported_name)
}

pub(crate) fn module_level_symbol<'a>(file: &'a ExtractedFile, name: &str) -> Option<&'a Symbol> {
    let module_id = file
        .symbols
        .iter()
        .find(|symbol| symbol.kind == SymbolKind::Module)
        .map(|symbol| symbol.id.as_str())?;
    let mut matches = file.symbols.iter().filter(|symbol| {
        symbol.name == name
            && symbol.parent_id.as_deref() == Some(module_id)
            && symbol.kind != SymbolKind::Module
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(first)
    }
}

fn collect_configs(root: &Path, dir: &Path, out: &mut Vec<TsConfig>) {
    for name in ["tsconfig.json", "jsconfig.json"] {
        let path = dir.join(name);
        if path.is_file() {
            if let Some(cfg) = load_tsconfig(root, &path, 0) {
                out.push(cfg);
            }
        }
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.')
            || matches!(
                name.as_str(),
                "node_modules" | "dist" | "build" | "target" | "coverage" | "out"
            )
        {
            continue;
        }
        collect_configs(root, &entry.path(), out);
    }
}

fn load_tsconfig(root: &Path, path: &Path, depth: usize) -> Option<TsConfig> {
    if depth > 8 {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = parse_jsonc(&text)?;
    let dir = relative_slash(root, path.parent()?)?;
    let mut base_url = value
        .pointer("/compilerOptions/baseUrl")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let mut paths = Vec::new();
    if let Some(Value::Object(map)) = value.pointer("/compilerOptions/paths") {
        for (pattern, targets) in map {
            let list = match targets {
                Value::Array(items) => items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
                Value::String(one) => vec![one.clone()],
                _ => Vec::new(),
            };
            if !list.is_empty() {
                paths.push((pattern.clone(), list));
            }
        }
    }
    if let Some(extends) = value.get("extends").and_then(Value::as_str) {
        let parent_path = resolve_extends(path, extends)?;
        if let Some(parent) = load_tsconfig(root, &parent_path, depth + 1) {
            if base_url.is_none() {
                base_url = parent.base_url;
            }
            if paths.is_empty() {
                paths = parent.paths;
            }
        }
    }
    Some(TsConfig {
        dir,
        base_url,
        paths,
    })
}

fn resolve_extends(from: &Path, extends: &str) -> Option<PathBuf> {
    let parent = from.parent()?;
    let mut candidate = parent.join(extends);
    if candidate.is_dir() {
        candidate = candidate.join("tsconfig.json");
    } else if candidate.extension().is_none() {
        let with_json = candidate.with_extension("json");
        if with_json.is_file() {
            candidate = with_json;
        }
    }
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

fn collect_packages(root: &Path, dir: &Path, out: &mut Vec<PackageJson>) {
    let path = dir.join("package.json");
    if path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if let Some(name) = value.get("name").and_then(Value::as_str) {
                    let dir_rel = relative_slash(root, dir).unwrap_or_default();
                    out.push(PackageJson {
                        dir: dir_rel,
                        name: name.to_string(),
                        main: value
                            .get("main")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        exports: value.get("exports").cloned(),
                    });
                }
            }
        }
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.')
            || matches!(
                name.as_str(),
                "node_modules" | "dist" | "build" | "target" | "coverage" | "out"
            )
        {
            continue;
        }
        collect_packages(root, &entry.path(), out);
    }
}

fn apply_path_mapping(pattern: &str, targets: &[String], specifier: &str) -> Option<Vec<String>> {
    if let Some(prefix) = pattern.strip_suffix("/*") {
        let rest = specifier.strip_prefix(prefix)?.strip_prefix('/')?;
        return Some(
            targets
                .iter()
                .map(|target| {
                    if let Some(tprefix) = target.strip_suffix("/*") {
                        format!("{tprefix}/{rest}")
                    } else {
                        target.replace("*", rest)
                    }
                })
                .collect(),
        );
    }
    if pattern == specifier {
        return Some(targets.to_vec());
    }
    None
}

fn apply_export_pattern(key: &str, value: &Value, subpath: &str) -> Option<String> {
    let prefix = key.strip_suffix("/*")?;
    let rest = subpath.strip_prefix(prefix)?.strip_prefix('/')?;
    let target = export_target(value)?;
    if let Some(tprefix) = target.strip_suffix("/*") {
        Some(format!("{tprefix}/{rest}"))
    } else {
        Some(target.replace('*', rest))
    }
}

fn export_target(value: &Value) -> Option<String> {
    match value {
        Value::String(path) => Some(path.clone()),
        Value::Object(map) => map
            .get("import")
            .or_else(|| map.get("require"))
            .or_else(|| map.get("default"))
            .and_then(export_target),
        Value::Array(items) => items.iter().find_map(export_target),
        _ => None,
    }
}

fn is_external_package(specifier: &str) -> bool {
    !specifier.starts_with('.') && !specifier.starts_with('/')
}

fn has_known_extension(path: &str) -> bool {
    EXTENSIONS.iter().any(|ext| path.ends_with(ext))
}

fn language_like(path: &str) -> bool {
    crate::model::language_for_path(path).is_some() || path.ends_with(".d.ts")
}

fn parse_jsonc(text: &str) -> Option<Value> {
    // Strip // and /* */ comments and trailing commas enough for tsconfig.
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let ch = chars[i];
        if in_string {
            out.push(ch);
            if ch == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = i.saturating_add(2);
            continue;
        }
        out.push(ch);
        i += 1;
    }
    let stripped = strip_trailing_commas(&out);
    serde_json::from_str(&stripped).ok()
}

fn strip_trailing_commas(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let ch = chars[i];
        if in_string {
            out.push(ch);
            if ch == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }
        if ch == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                i += 1;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn relative_slash(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

pub(crate) fn parent_path(path: &str) -> String {
    match path.rfind('/') {
        Some(index) => path[..index].to_string(),
        None => String::new(),
    }
}

pub(crate) fn join_path(base: &str, rel: &str) -> String {
    if base.is_empty() {
        rel.to_string()
    } else if rel.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{rel}")
    }
}

pub(crate) fn normalize_path(path: &str) -> String {
    let mut stack = Vec::new();
    for part in path.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            stack.pop();
        } else {
            stack.push(part);
        }
    }
    stack.join("/")
}

pub(crate) fn is_ecma(language: Language) -> bool {
    matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    )
}

#[cfg(test)]
mod path_tests {
    use super::*;
    use crate::model::{ExtractedFile, Language};

    #[test]
    fn resolves_tsconfig_paths() {
        let root = std::env::temp_dir().join(format!("fg-mod-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("tsconfig.json"),
            r#"{ "compilerOptions": { "baseUrl": ".", "paths": { "@lib/*": ["src/*"] } } }"#,
        )
        .unwrap();
        std::fs::write(root.join("src/math.ts"), "export function add() {}\n").unwrap();
        let files = vec![ExtractedFile {
            extractor_version: 3,
            relative_path: "src/math.ts".into(),
            language: Language::TypeScript,
            content_hash: "x".into(),
            symbols: vec![],
            imports: vec![],
            calls: vec![],
            exports: vec![],
            routes: vec![],
            http_calls: vec![],
        }];
        let resolver = ModuleResolver::from_root(&root, &files);
        assert_eq!(
            resolver.resolve_specifier("src/main.ts", "@lib/math"),
            ResolveResult::File("src/math.ts".into())
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
