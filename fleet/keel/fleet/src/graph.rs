use blake3::Hasher;
use rusqlite::{params, types::Value as SqlValue, Connection, OptionalExtension};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tree_sitter::{Node, Parser};
use uuid::Uuid;

const EXIT_ENV: i32 = 3;
const EXIT_INVARIANT: i32 = 6;
const EXIT_REFUSAL: i32 = 7;
const DEFAULT_DEPTH: u64 = 8;
const DEFAULT_FLOOR: u64 = 1;

#[derive(Clone, Debug)]
struct InputFile {
    path: String,
    language: &'static str,
    source: String,
}

#[derive(Debug)]
struct Scan {
    indexed: Vec<InputFile>,
    digest: String,
    skipped: u64,
}

#[derive(Clone, Debug)]
struct SymbolDef {
    name: String,
    arity: u64,
    kind: &'static str,
    line: u64,
    symbol_id: Option<String>,
}

#[derive(Clone, Debug)]
struct CallRef {
    caller: usize,
    callee_name: String,
    arity: u64,
}

#[derive(Debug)]
struct ParsedFile {
    path: String,
    language: &'static str,
    source_digest: String,
    symbols: Vec<SymbolDef>,
    calls: Vec<CallRef>,
}

#[derive(Clone, Debug)]
struct ExistingProject {
    project_id: String,
    tree_digest: String,
    indexed_commit: String,
    indexed_file_count: u64,
    floor: u64,
    files_skipped: u64,
    languages: Vec<String>,
}

#[derive(Clone, Debug)]
struct ExistingSymbol {
    symbol_id: String,
    path: String,
    name: String,
    arity: u64,
}

#[derive(Clone, Debug)]
struct Rename {
    from: String,
    to: String,
}

#[cfg(test)]
#[derive(Debug)]
struct ReachabilityFunction {
    module: String,
    name: String,
    public: bool,
    calls: Vec<String>,
}

/// Parse the fleet sources and report whether every shipped module has at least one public
/// entry point reachable from the CLI dispatcher.
#[cfg(test)]
pub fn reachability_report() -> Result<String, String> {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    reachability_report_for(&source_dir)
}

#[cfg(test)]
fn reachability_report_for(source_dir: &Path) -> Result<String, String> {
    let entries = fs::read_dir(source_dir).map_err(|error| {
        format!(
            "reachability scan failed for {}: {error}",
            source_dir.display()
        )
    })?;
    let mut modules = BTreeSet::new();
    let mut functions = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("reachability directory entry failed: {error}"))?
            .path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let module = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("non-UTF-8 Rust module path: {}", path.display()))?
            .to_string();
        if module != "main" && module != "lib" {
            modules.insert(module.clone());
        }
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("reachability read failed for {}: {error}", path.display()))?;
        functions.extend(parse_reachability_functions(&module, &source)?);
    }

    evaluate_reachability(modules, functions)
}

#[cfg(test)]
fn evaluate_reachability(
    modules: BTreeSet<String>,
    functions: Vec<ReachabilityFunction>,
) -> Result<String, String> {
    let total = modules.len();
    if total == 0 {
        return Err("0 of 0 modules reachable; no modules were measured".to_string());
    }

    let by_key = functions
        .iter()
        .enumerate()
        .map(|(index, function)| (format!("{}::{}", function.module, function.name), index))
        .collect::<BTreeMap<_, _>>();
    let dispatch = by_key
        .get("main::dispatch")
        .copied()
        .ok_or_else(|| format!("0 of {total} modules reachable; main::dispatch was not found"))?;
    let mut pending = vec![dispatch];
    let mut reachable_functions = HashSet::new();
    while let Some(index) = pending.pop() {
        if !reachable_functions.insert(index) {
            continue;
        }
        let caller = functions
            .get(index)
            .ok_or_else(|| "reachability call graph contained an invalid index".to_string())?;
        for call in &caller.calls {
            if let Some(target) = resolve_reachability_call(&caller.module, call, &by_key) {
                pending.push(target);
            }
        }
    }

    let reachable_modules = functions
        .iter()
        .enumerate()
        .filter(|(index, function)| {
            function.public
                && reachable_functions.contains(index)
                && modules.contains(&function.module)
        })
        .map(|(_, function)| function.module.clone())
        .collect::<BTreeSet<_>>();
    let checked = reachable_modules.len();
    let unreachable = modules
        .difference(&reachable_modules)
        .cloned()
        .collect::<Vec<_>>();
    let denominator = format!("{checked} of {total} modules reachable");
    if unreachable.is_empty() {
        Ok(denominator)
    } else {
        Err(format!(
            "{denominator}; modules with zero reachable public entry points: {}",
            unreachable.join(", ")
        ))
    }
}

#[cfg(test)]
fn parse_reachability_functions(
    module: &str,
    source: &str,
) -> Result<Vec<ReachabilityFunction>, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|error| format!("failed to load Rust grammar: {error}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| format!("failed to parse module {module}"))?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(format!("Rust parse errors in module {module}"));
    }
    let mut functions = Vec::new();
    for index in 0..root.named_child_count() {
        let node = root
            .named_child(index)
            .ok_or_else(|| format!("invalid syntax tree child in module {module}"))?;
        if node.kind() != "function_item" {
            continue;
        }
        let name_node = node
            .child_by_field_name("name")
            .ok_or_else(|| format!("unnamed function in module {module}"))?;
        let name = name_node
            .utf8_text(source.as_bytes())
            .map_err(|_| format!("non-UTF-8 function name in module {module}"))?
            .to_string();
        let public = (0..node.named_child_count()).any(|child_index| {
            node.named_child(child_index)
                .is_some_and(|child| child.kind() == "visibility_modifier")
        });
        let mut calls = Vec::new();
        collect_reachability_calls(node, source, &mut calls)?;
        functions.push(ReachabilityFunction {
            module: module.to_string(),
            name,
            public,
            calls,
        });
    }
    Ok(functions)
}

#[cfg(test)]
fn collect_reachability_calls(
    node: Node<'_>,
    source: &str,
    calls: &mut Vec<String>,
) -> Result<(), String> {
    if node.kind() == "call_expression" {
        let callee = node
            .child_by_field_name("function")
            .ok_or_else(|| "call expression had no function".to_string())?;
        if matches!(callee.kind(), "identifier" | "scoped_identifier") {
            calls.push(
                callee
                    .utf8_text(source.as_bytes())
                    .map_err(|_| "non-UTF-8 call target".to_string())?
                    .to_string(),
            );
        }
    }
    for index in 0..node.named_child_count() {
        let child = node
            .named_child(index)
            .ok_or_else(|| "invalid call syntax tree child".to_string())?;
        collect_reachability_calls(child, source, calls)?;
    }
    Ok(())
}

#[cfg(test)]
fn resolve_reachability_call(
    caller_module: &str,
    call: &str,
    by_key: &BTreeMap<String, usize>,
) -> Option<usize> {
    let components = call.split("::").collect::<Vec<_>>();
    let key = if components.len() == 1 {
        format!("{caller_module}::{call}")
    } else {
        let name = components.last()?;
        let module = components.get(components.len().checked_sub(2)?)?;
        format!("{module}::{name}")
    };
    by_key.get(&key).copied()
}

pub fn graph_command(args: &[String]) -> Result<(), i32> {
    match args.first().map(String::as_str) {
        Some("index") => index_command(&args[1..]),
        _ => Err(EXIT_REFUSAL),
    }
}

pub fn impact_command(args: &[String]) -> Result<(), i32> {
    let (symbol, root_arg, db_arg, depth) = parse_impact_args(args)?;
    let root = resolve_root(root_arg.as_deref())?;
    let db = resolve_db(db_arg.as_deref(), &root)?;
    let connection = open_db(&db)?;
    let project = load_project(&connection, &root)?.ok_or_else(|| {
        eprintln!(
            "project is not registered; run: fleet graph index --root {} --db {}",
            root.display(),
            db.display()
        );
        EXIT_REFUSAL
    })?;
    check_freshness(&connection, &project, &root, &db)?;

    let target_ids = target_symbol_ids(&connection, &project.project_id, &symbol)?;
    let dependents = if target_ids.is_empty() {
        Vec::new()
    } else {
        query_dependents(&connection, &project.project_id, &target_ids, depth)?
    };
    let coverage = json!({
        "languages_indexed": project.languages,
        "files_indexed": project.indexed_file_count,
        "files_skipped": project.files_skipped,
    });
    println!(
        "{}",
        serde_json::to_string(&json!({
            "symbol": symbol,
            "depth_used": depth,
            "dependents": dependents,
            "coverage": coverage,
        }))
        .map_err(|_| EXIT_INVARIANT)?
    );
    Ok(())
}

fn index_command(args: &[String]) -> Result<(), i32> {
    let (root_arg, db_arg, floor) = parse_index_args(args)?;
    let root = resolve_root(root_arg.as_deref())?;
    let db = resolve_db(db_arg.as_deref(), &root)?;
    let scan = scan_tree(&root, Some(&db))?;
    let indexed_count = u64::try_from(scan.indexed.len()).map_err(|_| EXIT_INVARIANT)?;
    if indexed_count == 0 || indexed_count < floor {
        eprintln!(
            "coverage error: files_indexed={} floor={} files_skipped={}",
            indexed_count, floor, scan.skipped
        );
        return Err(EXIT_INVARIANT);
    }

    let mut connection = open_db(&db)?;
    let previous = load_project(&connection, &root)?;
    let project_id = previous
        .as_ref()
        .map(|project| project.project_id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let old_symbols = load_symbols(&connection, &project_id)?;
    let current_commit = git_commit(&root);
    let renames = previous
        .as_ref()
        .map(|project| git_renames(&root, &project.indexed_commit))
        .unwrap_or_default();
    let Scan {
        indexed,
        digest: scan_digest,
        skipped: scan_skipped_count,
    } = scan;
    let mut parsed = indexed
        .iter()
        .map(parse_file)
        .collect::<Result<Vec<_>, _>>()?;
    assign_symbol_ids(
        &mut parsed,
        &old_symbols,
        &renames,
        previous
            .as_ref()
            .map(|project| project.indexed_commit.as_str()),
        &current_commit,
    );
    let (edges, symbols) = resolve_edges(&parsed);
    if symbols.is_empty() {
        eprintln!("invariant error: no symbols were indexed");
        return Err(EXIT_INVARIANT);
    }

    let languages = parsed
        .iter()
        .map(|file| file.language.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let indexed_file_count = u64::try_from(parsed.len()).map_err(|_| EXIT_INVARIANT)?;
    let files_skipped = scan_skipped_count;
    let tree_digest = scan_digest;
    let transaction = connection.transaction().map_err(|_| EXIT_ENV)?;
    transaction
        .execute(
            "INSERT INTO projects(project_id, root_path, tree_digest, indexed_commit, indexed_file_count, floor, files_skipped, languages)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(root_path) DO UPDATE SET project_id=excluded.project_id, tree_digest=excluded.tree_digest,
             indexed_commit=excluded.indexed_commit, indexed_file_count=excluded.indexed_file_count,
             floor=excluded.floor, files_skipped=excluded.files_skipped, languages=excluded.languages",
            params![
                project_id,
                root.to_string_lossy().as_ref(),
                tree_digest,
                current_commit,
                i64::try_from(indexed_file_count).map_err(|_| EXIT_INVARIANT)?,
                i64::try_from(floor).map_err(|_| EXIT_INVARIANT)?,
                i64::try_from(files_skipped).map_err(|_| EXIT_INVARIANT)?,
                languages.join(",")
            ],
        )
        .map_err(|_| EXIT_ENV)?;
    transaction
        .execute("DELETE FROM edges WHERE project_id=?1", params![project_id])
        .map_err(|_| EXIT_ENV)?;
    transaction
        .execute("DELETE FROM files WHERE project_id=?1", params![project_id])
        .map_err(|_| EXIT_ENV)?;
    transaction
        .execute(
            "DELETE FROM symbols WHERE project_id=?1",
            params![project_id],
        )
        .map_err(|_| EXIT_ENV)?;
    for file in &parsed {
        transaction
            .execute(
                "INSERT INTO files(project_id, path, language, digest) VALUES(?1, ?2, ?3, ?4)",
                params![project_id, file.path, file.language, file.source_digest],
            )
            .map_err(|_| EXIT_ENV)?;
        for symbol in &file.symbols {
            transaction
                .execute(
                    "INSERT INTO symbols(project_id, symbol_id, path, name, arity, kind, line) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        project_id,
                        symbol.symbol_id.as_deref().ok_or(EXIT_INVARIANT)?,
                        file.path,
                        symbol.name,
                        i64::try_from(symbol.arity).map_err(|_| EXIT_INVARIANT)?,
                        symbol.kind,
                        i64::try_from(symbol.line).map_err(|_| EXIT_INVARIANT)?
                    ],
                )
                .map_err(|_| EXIT_ENV)?;
        }
    }
    for (caller, callee) in edges {
        transaction
            .execute(
                "INSERT OR IGNORE INTO edges(project_id, caller_id, callee_id) VALUES(?1, ?2, ?3)",
                params![project_id, caller, callee],
            )
            .map_err(|_| EXIT_ENV)?;
    }
    for rename in &renames {
        let old = old_symbols
            .iter()
            .filter(|symbol| symbol.path == rename.from);
        for old_symbol in old {
            let preserved = parsed.iter().any(|file| {
                file.path == rename.to
                    && file.symbols.iter().any(|symbol| {
                        symbol.symbol_id.as_deref() == Some(old_symbol.symbol_id.as_str())
                    })
            });
            if preserved {
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO aliases(project_id, path, name, arity, from_commit, to_commit, symbol_id)
                         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            project_id,
                            rename.from,
                            old_symbol.name,
                            i64::try_from(old_symbol.arity).map_err(|_| EXIT_INVARIANT)?,
                            previous.as_ref().map(|project| project.indexed_commit.as_str()).unwrap_or("WORKTREE"),
                            current_commit,
                            old_symbol.symbol_id
                        ],
                    )
                    .map_err(|_| EXIT_ENV)?;
            }
        }
    }
    transaction.commit().map_err(|_| EXIT_ENV)?;

    println!(
        "{}",
        serde_json::to_string(&json!({
            "project": root,
            "tree_digest": tree_digest,
            "depth_used": DEFAULT_DEPTH,
            "coverage": {
                "languages_indexed": languages,
                "files_indexed": indexed_file_count,
                "files_skipped": files_skipped,
            }
        }))
        .map_err(|_| EXIT_INVARIANT)?
    );
    Ok(())
}

fn parse_index_args(args: &[String]) -> Result<(Option<String>, Option<String>, u64), i32> {
    let mut root = None;
    let mut db = None;
    let mut floor = DEFAULT_FLOOR;
    let mut i = 0usize;
    while i < args.len() {
        let value = args.get(i + 1).ok_or(EXIT_REFUSAL)?.clone();
        match args[i].as_str() {
            "--root" => root = Some(value),
            "--db" => db = Some(value),
            "--floor" => floor = parse_positive(&value)?,
            _ => return Err(EXIT_REFUSAL),
        }
        i += 2;
    }
    Ok((root, db, floor))
}

fn parse_impact_args(
    args: &[String],
) -> Result<(String, Option<String>, Option<String>, u64), i32> {
    let mut symbol = None;
    let mut root = None;
    let mut db = None;
    let mut depth = DEFAULT_DEPTH;
    let mut i = 0usize;
    while i < args.len() {
        let value = args.get(i + 1).ok_or(EXIT_REFUSAL)?.clone();
        match args[i].as_str() {
            "--symbol" => symbol = Some(value),
            "--root" => root = Some(value),
            "--db" => db = Some(value),
            "--depth" => depth = parse_positive(&value)?,
            _ => return Err(EXIT_REFUSAL),
        }
        i += 2;
    }
    let symbol = symbol.ok_or(EXIT_REFUSAL)?;
    if symbol.trim().is_empty() {
        return Err(EXIT_REFUSAL);
    }
    Ok((symbol, root, db, depth))
}

fn parse_positive(value: &str) -> Result<u64, i32> {
    let parsed = value.parse::<u64>().map_err(|_| EXIT_REFUSAL)?;
    if parsed == 0 || parsed > 1024 {
        return Err(EXIT_REFUSAL);
    }
    Ok(parsed)
}

fn resolve_root(argument: Option<&str>) -> Result<PathBuf, i32> {
    let requested = match argument {
        Some(value) => PathBuf::from(value),
        None => {
            let current = env::current_dir().map_err(|_| EXIT_ENV)?;
            if current.join("keel").is_dir() {
                current.join("keel")
            } else {
                current
            }
        }
    };
    fs::canonicalize(requested).map_err(|_| EXIT_ENV)
}

fn resolve_db(argument: Option<&str>, root: &Path) -> Result<PathBuf, i32> {
    if let Some(value) = argument {
        return absolute_path(Path::new(value));
    }
    if let Some(value) = env::var_os("FLEET_GRAPH_DB") {
        if value.is_empty() {
            return Err(EXIT_ENV);
        }
        return absolute_path(Path::new(&value));
    }
    if let Some(value) = env::var_os("FLEET_STATE") {
        if value.is_empty() {
            return Err(EXIT_ENV);
        }
        return absolute_path(&PathBuf::from(value).join("keel-graph.sqlite3"));
    }
    absolute_path(&root.join(".fleet").join("keel-graph.sqlite3"))
}

fn absolute_path(path: &Path) -> Result<PathBuf, i32> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().map_err(|_| EXIT_ENV)?.join(path)
    };
    if candidate.exists() {
        return fs::canonicalize(candidate).map_err(|_| EXIT_ENV);
    }
    let mut missing = Vec::new();
    let mut existing = candidate;
    while !existing.exists() {
        let name = existing.file_name().ok_or(EXIT_ENV)?.to_os_string();
        missing.push(name);
        existing = existing.parent().ok_or(EXIT_ENV)?.to_path_buf();
    }
    let mut result = fs::canonicalize(existing).map_err(|_| EXIT_ENV)?;
    for name in missing.iter().rev() {
        result.push(name);
    }
    Ok(result)
}

fn open_db(path: &Path) -> Result<Connection, i32> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| EXIT_ENV)?;
    }
    let connection = Connection::open(path).map_err(|_| EXIT_ENV)?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| EXIT_ENV)?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS projects(
               project_id TEXT PRIMARY KEY, root_path TEXT NOT NULL UNIQUE, tree_digest TEXT NOT NULL,
               indexed_commit TEXT NOT NULL, indexed_file_count INTEGER NOT NULL,
               floor INTEGER NOT NULL, files_skipped INTEGER NOT NULL, languages TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS files(
               project_id TEXT NOT NULL, path TEXT NOT NULL, language TEXT NOT NULL, digest TEXT NOT NULL,
               PRIMARY KEY(project_id, path)
             );
             CREATE TABLE IF NOT EXISTS symbols(
               project_id TEXT NOT NULL, symbol_id TEXT NOT NULL, path TEXT NOT NULL, name TEXT NOT NULL,
               arity INTEGER NOT NULL, kind TEXT NOT NULL, line INTEGER NOT NULL,
               PRIMARY KEY(project_id, symbol_id)
             );
             CREATE INDEX IF NOT EXISTS symbols_name ON symbols(project_id, name);
             CREATE TABLE IF NOT EXISTS edges(
               project_id TEXT NOT NULL, caller_id TEXT NOT NULL, callee_id TEXT NOT NULL,
               PRIMARY KEY(project_id, caller_id, callee_id)
             );
             CREATE TABLE IF NOT EXISTS aliases(
               project_id TEXT NOT NULL, path TEXT NOT NULL, name TEXT NOT NULL, arity INTEGER NOT NULL,
               from_commit TEXT NOT NULL, to_commit TEXT NOT NULL, symbol_id TEXT NOT NULL,
               PRIMARY KEY(project_id, path, name, arity, from_commit, to_commit)
             );
             CREATE INDEX IF NOT EXISTS aliases_name ON aliases(project_id, name);",
        )
        .map_err(|_| EXIT_ENV)?;
    Ok(connection)
}

fn load_project(connection: &Connection, root: &Path) -> Result<Option<ExistingProject>, i32> {
    connection
        .query_row(
            "SELECT project_id, root_path, tree_digest, indexed_commit, indexed_file_count, floor, files_skipped, languages
             FROM projects WHERE root_path=?1",
            params![root.to_string_lossy().as_ref()],
            |row| {
                let languages: String = row.get(7)?;
                Ok(ExistingProject {
                    project_id: row.get(0)?,
                    tree_digest: row.get(2)?,
                    indexed_commit: row.get(3)?,
                    indexed_file_count: row.get::<_, i64>(4)?.try_into().map_err(|_| rusqlite::Error::IntegralValueOutOfRange(4, 0))?,
                    floor: row.get::<_, i64>(5)?.try_into().map_err(|_| rusqlite::Error::IntegralValueOutOfRange(5, 0))?,
                    files_skipped: row.get::<_, i64>(6)?.try_into().map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, 0))?,
                    languages: if languages.is_empty() { Vec::new() } else { languages.split(',').map(str::to_string).collect() },
                })
            },
        )
        .optional()
        .map_err(|_| EXIT_ENV)
}

fn load_symbols(connection: &Connection, project_id: &str) -> Result<Vec<ExistingSymbol>, i32> {
    let mut statement = connection
        .prepare("SELECT symbol_id, path, name, arity FROM symbols WHERE project_id=?1")
        .map_err(|_| EXIT_ENV)?;
    let rows = statement
        .query_map(params![project_id], |row| {
            Ok(ExistingSymbol {
                symbol_id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                arity: row
                    .get::<_, i64>(3)?
                    .try_into()
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, 0))?,
            })
        })
        .map_err(|_| EXIT_ENV)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| EXIT_ENV)
}

fn scan_tree(root: &Path, ignored: Option<&Path>) -> Result<Scan, i32> {
    let mut all = Vec::new();
    let mut indexed = Vec::new();
    let mut skipped = 0u64;
    visit_tree(root, root, ignored, &mut all, &mut indexed, &mut skipped)?;
    if indexed.is_empty() {
        return Ok(Scan {
            indexed,
            digest: digest_items(&all),
            skipped,
        });
    }
    Ok(Scan {
        indexed,
        digest: digest_items(&all),
        skipped,
    })
}

fn visit_tree(
    directory: &Path,
    root: &Path,
    ignored: Option<&Path>,
    all: &mut Vec<(String, Vec<u8>)>,
    indexed: &mut Vec<InputFile>,
    skipped: &mut u64,
) -> Result<(), i32> {
    let mut entries = fs::read_dir(directory)
        .map_err(|_| EXIT_ENV)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| EXIT_ENV)?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if ignored.is_some_and(|ignored_path| path == ignored_path) {
            continue;
        }
        let file_type = entry.file_type().map_err(|_| EXIT_ENV)?;
        if file_type.is_dir() {
            if matches!(
                entry.file_name().to_str(),
                Some(".git" | "target" | ".fleet")
            ) {
                continue;
            }
            visit_tree(&path, root, ignored, all, indexed, skipped)?;
            continue;
        }
        if !file_type.is_file() {
            *skipped = skipped.checked_add(1).ok_or(EXIT_INVARIANT)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| EXIT_INVARIANT)?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let bytes = fs::read(&path).map_err(|_| EXIT_ENV)?;
        all.push((relative.clone(), bytes.clone()));
        match language_for(&path) {
            Some(language) => indexed.push(InputFile {
                path: relative,
                language,
                source: String::from_utf8(bytes).map_err(|_| EXIT_INVARIANT)?,
            }),
            None => *skipped = skipped.checked_add(1).ok_or(EXIT_INVARIANT)?,
        }
    }
    Ok(())
}

fn language_for(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => Some("rust"),
        Some("sh") | Some("bash") => Some("bash"),
        Some("py") => Some("python"),
        _ => None,
    }
}

fn digest_items(items: &[(String, Vec<u8>)]) -> String {
    let mut sorted = items.to_vec();
    sorted.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Hasher::new();
    for (path, bytes) in sorted {
        hasher.update(path.as_bytes());
        hasher.update(&[0]);
        hasher.update(&bytes);
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().to_string()
}

fn tree_digest(root: &Path, ignored: Option<&Path>) -> Result<String, i32> {
    let mut all = Vec::new();
    let mut indexed = Vec::new();
    let mut skipped = 0u64;
    visit_tree(root, root, ignored, &mut all, &mut indexed, &mut skipped)?;
    Ok(digest_items(&all))
}

fn parse_file(input: &InputFile) -> Result<ParsedFile, i32> {
    let mut parser = Parser::new();
    let language = match input.language {
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "bash" => tree_sitter_bash::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        _ => return Err(EXIT_INVARIANT),
    };
    parser.set_language(&language).map_err(|_| EXIT_ENV)?;
    let tree = parser.parse(&input.source, None).ok_or(EXIT_INVARIANT)?;
    let mut symbols = Vec::new();
    let mut calls = Vec::new();
    collect_nodes(
        tree.root_node(),
        &input.source,
        input.language,
        &mut symbols,
        &mut calls,
        None,
    )?;
    Ok(ParsedFile {
        path: input.path.clone(),
        language: input.language,
        source_digest: blake3::hash(input.source.as_bytes()).to_hex().to_string(),
        symbols,
        calls,
    })
}

fn collect_nodes(
    node: Node<'_>,
    source: &str,
    language: &str,
    symbols: &mut Vec<SymbolDef>,
    calls: &mut Vec<CallRef>,
    current: Option<usize>,
) -> Result<(), i32> {
    let mut next = current;
    if let Some((name, arity, kind)) = definition(node, source, language)? {
        let line = u64::try_from(node.start_position().row)
            .map_err(|_| EXIT_INVARIANT)?
            .checked_add(1)
            .ok_or(EXIT_INVARIANT)?;
        symbols.push(SymbolDef {
            name,
            arity,
            kind,
            line,
            symbol_id: None,
        });
        next = Some(symbols.len().checked_sub(1).ok_or(EXIT_INVARIANT)?);
    }
    if let (Some(caller), Some((callee_name, arity))) = (next, call(node, source, language)?) {
        calls.push(CallRef {
            caller,
            callee_name,
            arity,
        });
    }
    for index in 0..node.named_child_count() {
        let child = node.named_child(index).ok_or(EXIT_INVARIANT)?;
        collect_nodes(child, source, language, symbols, calls, next)?;
    }
    Ok(())
}

fn definition(
    node: Node<'_>,
    source: &str,
    language: &str,
) -> Result<Option<(String, u64, &'static str)>, i32> {
    let kind = node.kind();
    let is_definition = match language {
        "rust" => kind == "function_item",
        "python" => kind == "function_definition",
        "bash" => kind == "function_definition",
        _ => false,
    };
    if !is_definition {
        return Ok(None);
    }
    let name_node = node.child_by_field_name("name").ok_or(EXIT_INVARIANT)?;
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Err(EXIT_INVARIANT);
    }
    let arity = match node.child_by_field_name("parameters") {
        Some(parameters) => {
            u64::try_from(parameters.named_child_count()).map_err(|_| EXIT_INVARIANT)?
        }
        None => 0,
    };
    Ok(Some((name, arity, "function")))
}

fn call(node: Node<'_>, source: &str, language: &str) -> Result<Option<(String, u64)>, i32> {
    let kind = node.kind();
    let is_call = match language {
        "rust" => kind == "call_expression",
        "python" => kind == "call",
        "bash" => kind == "command",
        _ => false,
    };
    if !is_call {
        return Ok(None);
    }
    let callee = if language == "bash" {
        node.child_by_field_name("name")
            .or_else(|| node.named_child(0))
    } else {
        node.child_by_field_name("function")
    }
    .ok_or(EXIT_INVARIANT)?;
    let raw_name = node_text(callee, source)?;
    let name = raw_name
        .trim()
        .trim_end_matches("()")
        .rsplit([':', '.'])
        .next()
        .unwrap_or(raw_name.trim())
        .trim()
        .to_string();
    if name.is_empty() || matches!(name.as_str(), "if" | "for" | "while" | "case") {
        return Ok(None);
    }
    let arity = if language == "bash" {
        u64::try_from(node.named_child_count().saturating_sub(1)).map_err(|_| EXIT_INVARIANT)?
    } else {
        let arguments = node
            .child_by_field_name("arguments")
            .ok_or(EXIT_INVARIANT)?;
        u64::try_from(arguments.named_child_count()).map_err(|_| EXIT_INVARIANT)?
    };
    Ok(Some((name, arity)))
}

fn node_text<'a>(node: Node<'a>, source: &'a str) -> Result<&'a str, i32> {
    node.utf8_text(source.as_bytes())
        .map_err(|_| EXIT_INVARIANT)
}

fn assign_symbol_ids(
    parsed: &mut [ParsedFile],
    old_symbols: &[ExistingSymbol],
    renames: &[Rename],
    previous_commit: Option<&str>,
    current_commit: &str,
) {
    let mut exact = HashMap::new();
    for symbol in old_symbols {
        exact.insert(
            (symbol.path.clone(), symbol.name.clone(), symbol.arity),
            symbol.symbol_id.clone(),
        );
    }
    let mut renamed = HashMap::new();
    for rename in renames {
        for symbol in old_symbols
            .iter()
            .filter(|symbol| symbol.path == rename.from)
        {
            renamed.insert(
                (rename.to.clone(), symbol.name.clone(), symbol.arity),
                symbol.symbol_id.clone(),
            );
        }
    }
    let _ = (previous_commit, current_commit);
    for file in parsed {
        for symbol in &mut file.symbols {
            let key = (file.path.clone(), symbol.name.clone(), symbol.arity);
            symbol.symbol_id = exact
                .get(&key)
                .cloned()
                .or_else(|| renamed.get(&key).cloned())
                .or_else(|| Some(Uuid::new_v4().to_string()));
        }
    }
}

#[allow(clippy::type_complexity)] // a 5-tuple harvest; factoring it is a refactor, not a fix
fn resolve_edges(
    parsed: &[ParsedFile],
) -> (
    BTreeSet<(String, String)>,
    Vec<(String, String, String, u64, String)>,
) {
    let mut by_key: BTreeMap<(String, u64), Vec<String>> = BTreeMap::new();
    let mut symbols = Vec::new();
    for file in parsed {
        for symbol in &file.symbols {
            if let Some(id) = symbol.symbol_id.as_ref() {
                by_key
                    .entry((symbol.name.clone(), symbol.arity))
                    .or_default()
                    .push(id.clone());
                symbols.push((
                    id.clone(),
                    file.path.clone(),
                    symbol.name.clone(),
                    symbol.arity,
                    symbol.kind.to_string(),
                ));
            }
        }
    }
    let mut edges = BTreeSet::new();
    for file in parsed {
        for call in &file.calls {
            let Some(caller) = file
                .symbols
                .get(call.caller)
                .and_then(|symbol| symbol.symbol_id.as_ref())
            else {
                continue;
            };
            let candidates = by_key.get(&(call.callee_name.clone(), call.arity));
            let target = candidates
                .filter(|ids| ids.len() == 1)
                .and_then(|ids| ids.first())
                .or_else(|| {
                    let mut same_name = by_key
                        .iter()
                        .filter(|((name, _), _)| name == &call.callee_name)
                        .flat_map(|(_, ids)| ids.iter());
                    let first = same_name.next();
                    if first.is_some() && same_name.next().is_none() {
                        first
                    } else {
                        None
                    }
                });
            if let Some(target) = target {
                edges.insert((caller.clone(), target.clone()));
            }
        }
    }
    (edges, symbols)
}

fn git_commit(root: &Path) -> String {
    Command::new("git")
        .args(["-C", root.to_string_lossy().as_ref(), "rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|commit| commit.trim().to_string())
        .filter(|commit| !commit.is_empty())
        .unwrap_or_else(|| "WORKTREE".to_string())
}

fn git_renames(root: &Path, from_commit: &str) -> Vec<Rename> {
    if from_commit == "WORKTREE" {
        return Vec::new();
    }
    let Some(git_root) = Command::new("git")
        .args([
            "-C",
            root.to_string_lossy().as_ref(),
            "rev-parse",
            "--show-toplevel",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|path| PathBuf::from(path.trim()))
    else {
        return Vec::new();
    };
    let root_relative = root.strip_prefix(&git_root).ok().map(|path| {
        path.to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    });
    let mut renames = Vec::new();
    for staged in [false, true] {
        let mut command = Command::new("git");
        command.args([
            "-C",
            git_root.to_string_lossy().as_ref(),
            "diff",
            "--name-status",
            "--find-renames",
        ]);
        if staged {
            command.arg("--cached");
        }
        command.arg(from_commit);
        if let Some(relative) = root_relative.as_deref() {
            if !relative.is_empty() {
                command.args(["--", relative]);
            }
        }
        let Ok(output) = command.output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let Ok(text) = String::from_utf8(output.stdout) else {
            continue;
        };
        for line in text.lines() {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() < 3 || !fields[0].starts_with('R') {
                continue;
            }
            let Some(from) = scan_relative(root, &git_root, fields[1]) else {
                continue;
            };
            let Some(to) = scan_relative(root, &git_root, fields[2]) else {
                continue;
            };
            renames.push(Rename { from, to });
        }
    }
    renames.sort_by(|left, right| left.from.cmp(&right.from).then(left.to.cmp(&right.to)));
    renames.dedup_by(|left, right| left.from == right.from && left.to == right.to);
    renames
}

fn scan_relative(root: &Path, git_root: &Path, path: &str) -> Option<String> {
    let absolute = git_root.join(path);
    absolute.strip_prefix(root).ok().map(|relative| {
        relative
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    })
}

fn check_freshness(
    connection: &Connection,
    project: &ExistingProject,
    root: &Path,
    db: &Path,
) -> Result<(), i32> {
    if project.indexed_file_count == 0 || project.indexed_file_count < project.floor {
        eprintln!(
            "coverage error: registered project files_indexed={} floor={}",
            project.indexed_file_count, project.floor
        );
        return Err(EXIT_INVARIANT);
    }
    let current = tree_digest(root, Some(db))?;
    if current != project.tree_digest {
        eprintln!(
            "stale index: indexed_tree_digest={} current_tree_digest={}",
            project.tree_digest, current
        );
        eprintln!(
            "reindex: fleet graph index --root {} --db {}",
            root.display(),
            db.display()
        );
        return Err(EXIT_INVARIANT);
    }
    let stored_count: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM files WHERE project_id=?1",
            params![project.project_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| EXIT_ENV)?
        .try_into()
        .map_err(|_| EXIT_INVARIANT)?;
    if stored_count != project.indexed_file_count || stored_count == 0 {
        eprintln!(
            "coverage error: files table checked={} expected={}",
            stored_count, project.indexed_file_count
        );
        return Err(EXIT_INVARIANT);
    }
    Ok(())
}

fn target_symbol_ids(
    connection: &Connection,
    project_id: &str,
    symbol: &str,
) -> Result<Vec<String>, i32> {
    let mut ids = HashSet::new();
    let mut statement = connection
        .prepare("SELECT symbol_id FROM symbols WHERE project_id=?1 AND (name=?2 OR symbol_id=?2)")
        .map_err(|_| EXIT_ENV)?;
    for row in statement
        .query_map(params![project_id, symbol], |row| row.get::<_, String>(0))
        .map_err(|_| EXIT_ENV)?
    {
        ids.insert(row.map_err(|_| EXIT_ENV)?);
    }
    let mut aliases = connection
        .prepare("SELECT symbol_id FROM aliases WHERE project_id=?1 AND name=?2")
        .map_err(|_| EXIT_ENV)?;
    for row in aliases
        .query_map(params![project_id, symbol], |row| row.get::<_, String>(0))
        .map_err(|_| EXIT_ENV)?
    {
        ids.insert(row.map_err(|_| EXIT_ENV)?);
    }
    let mut result = ids.into_iter().collect::<Vec<_>>();
    result.sort();
    Ok(result)
}

fn query_dependents(
    connection: &Connection,
    project_id: &str,
    target_ids: &[String],
    depth: u64,
) -> Result<Vec<serde_json::Value>, i32> {
    let placeholders = std::iter::repeat_n("?", target_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "WITH RECURSIVE closure(dependent_id, depth) AS (
           SELECT caller_id, 1 FROM edges WHERE project_id=? AND callee_id IN ({placeholders})
           UNION
           SELECT edges.caller_id, closure.depth + 1
           FROM edges JOIN closure ON edges.callee_id = closure.dependent_id
           WHERE edges.project_id=? AND closure.depth < ?
         )
         SELECT symbols.symbol_id, symbols.path, symbols.name, symbols.arity, symbols.kind, symbols.line, MIN(closure.depth)
         FROM closure JOIN symbols ON symbols.project_id=? AND symbols.symbol_id=closure.dependent_id
         GROUP BY symbols.symbol_id, symbols.path, symbols.name, symbols.arity, symbols.kind, symbols.line
         ORDER BY MIN(closure.depth), symbols.path, symbols.name, symbols.arity"
    );
    let mut values = Vec::new();
    values.push(SqlValue::Text(project_id.to_string()));
    values.extend(target_ids.iter().cloned().map(SqlValue::Text));
    values.push(SqlValue::Text(project_id.to_string()));
    values.push(SqlValue::Integer(
        i64::try_from(depth).map_err(|_| EXIT_INVARIANT)?,
    ));
    values.push(SqlValue::Text(project_id.to_string()));
    let mut statement = connection.prepare(&sql).map_err(|_| EXIT_ENV)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(values), |row| {
            Ok(json!({
                "symbol_id": row.get::<_, String>(0)?,
                "path": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "arity": row.get::<_, i64>(3)?,
                "kind": row.get::<_, String>(4)?,
                "line": row.get::<_, i64>(5)?,
                "depth": row.get::<_, i64>(6)?,
            }))
        })
        .map_err(|_| EXIT_ENV)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| EXIT_ENV)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_module_is_reachable_from_dispatch() {
        let report = reachability_report().unwrap_or_else(|report| panic!("{report}"));
        println!("{report}");
        assert_ne!(report, "0 of 0 modules reachable");
    }

    #[test]
    fn reachability_rejects_an_empty_denominator() {
        let report = evaluate_reachability(BTreeSet::new(), Vec::new())
            .expect_err("measuring zero modules must fail");
        assert_eq!(report, "0 of 0 modules reachable; no modules were measured");
    }

    #[test]
    fn known_callers_fixture_returns_non_zero_edges() {
        let input = InputFile {
            path: "known-callers.rs".to_string(),
            language: "rust",
            source: include_str!("../../tests/graph-fixtures/known-callers.rs").to_string(),
        };
        let mut parsed = vec![parse_file(&input).expect("known callers fixture must parse")];
        assign_symbol_ids(&mut parsed, &[], &[], None, "fixture");
        let (edges, _) = resolve_edges(&parsed);
        assert!(
            !edges.is_empty(),
            "known caller fixture silently returned zero edges"
        );
    }

    #[test]
    fn known_rename_fixture_preserves_symbol_identity_and_appends_alias_candidate() {
        let input = InputFile {
            path: "rename-after.rs".to_string(),
            language: "rust",
            source: include_str!("../../tests/graph-fixtures/rename-after.rs").to_string(),
        };
        let mut parsed = vec![parse_file(&input).expect("known rename fixture must parse")];
        let old_symbols = vec![ExistingSymbol {
            symbol_id: "stable-symbol-id".to_string(),
            path: "rename-before.rs".to_string(),
            name: "renamed_target".to_string(),
            arity: 0,
        }];
        let renames = vec![Rename {
            from: "rename-before.rs".to_string(),
            to: "rename-after.rs".to_string(),
        }];
        assign_symbol_ids(&mut parsed, &old_symbols, &renames, Some("old"), "new");
        assert_eq!(
            parsed[0].symbols[0].symbol_id.as_deref(),
            Some("stable-symbol-id")
        );
    }

    #[test]
    fn false_positive_control_has_zero_edges() {
        let input = InputFile {
            path: "false-positive.rs".to_string(),
            language: "rust",
            source: include_str!("../../tests/graph-fixtures/false-positive.rs").to_string(),
        };
        let mut parsed = vec![parse_file(&input).expect("false-positive fixture must parse")];
        assign_symbol_ids(&mut parsed, &[], &[], None, "fixture");
        let (edges, _) = resolve_edges(&parsed);
        assert!(
            edges.is_empty(),
            "unindexed external call became a graph edge"
        );
    }

    #[test]
    fn all_required_tree_sitter_languages_produce_symbols() {
        let fixtures = [
            (
                "known-python.py",
                "python",
                include_str!("../../tests/graph-fixtures/known-python.py"),
            ),
            (
                "known-bash.sh",
                "bash",
                include_str!("../../tests/graph-fixtures/known-bash.sh"),
            ),
        ];
        for (path, language, source) in fixtures {
            let parsed = parse_file(&InputFile {
                path: path.to_string(),
                language,
                source: source.to_string(),
            })
            .expect("required language fixture must parse");
            assert!(
                !parsed.symbols.is_empty(),
                "{language} fixture produced zero symbols"
            );
        }
    }
}
