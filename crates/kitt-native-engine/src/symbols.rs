use crate::language::{grammar, identify, kind_label, name_of, symbol_kinds};
use crate::model::{Symbol, SymbolRead, SymbolReference};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tree_sitter::{Node, Parser};

const MAX_SYMBOL_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_INDEX_FILES: usize = 10_000;
const MAX_INDEX_ENTRIES: usize = 100_000;
const MAX_INDEX_SYMBOLS: usize = 100_000;
const MAX_FILE_SYMBOLS: usize = 10_000;
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const SCAN_TIMEOUT: Duration = Duration::from_secs(2);

fn read_source(root: &Path, relative: &str) -> Result<(std::path::PathBuf, String, Vec<u8>)> {
    let (path, display) = crate::workspace::contained_existing(root, relative)?;
    let metadata = fs::metadata(&path)?;
    anyhow::ensure!(metadata.is_file(), "symbol path is not a regular file");
    anyhow::ensure!(
        metadata.len() <= MAX_SYMBOL_FILE_BYTES,
        "symbol file exceeds read limit"
    );
    let file = fs::File::open(&path)?;
    let metadata = file.metadata()?;
    anyhow::ensure!(metadata.is_file(), "symbol path is not a regular file");
    anyhow::ensure!(
        metadata.len() <= MAX_SYMBOL_FILE_BYTES,
        "symbol file exceeds read limit"
    );
    let mut bytes = Vec::new();
    file.take(MAX_SYMBOL_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_SYMBOL_FILE_BYTES,
        "symbol file exceeds read limit"
    );
    Ok((path, display, bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn mtime_ns(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_nanos())
        .unwrap_or(0)
}

fn visit_symbols(
    node: Node<'_>,
    source: &[u8],
    path: &str,
    kinds: &[&str],
    parents: &mut Vec<String>,
    out: &mut Vec<Symbol>,
) {
    if out.len() >= MAX_FILE_SYMBOLS {
        return;
    }
    let is_symbol = kinds.iter().any(|kind| *kind == node.kind());
    let mut pushed = false;
    if is_symbol && let Some(name) = name_of(node, source) {
        let qualified = if parents.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", parents.join("::"), name)
        };
        let start = node.start_position().row + 1;
        let end = node.end_position().row + 1;
        let slice = source.get(node.byte_range()).unwrap_or_default();
        let id = format!("{}::{}", path, qualified);
        out.push(Symbol {
            id,
            path: path.to_string(),
            name: name.clone(),
            qualified_name: qualified,
            kind: kind_label(node.kind()),
            start_line: start,
            end_line: end,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            source_hash: hash_bytes(slice),
        });
        parents.push(name);
        pushed = true;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_symbols(child, source, path, kinds, parents, out);
    }
    if pushed {
        parents.pop();
    }
}

fn parse_symbols(path: &Path, relative: &str, source: &[u8]) -> Result<Vec<Symbol>> {
    let Some(lang_id) = identify(path) else {
        return Ok(Vec::new());
    };
    let mut parser = Parser::new();
    parser.set_language(&grammar(lang_id)?)?;
    let tree = parser
        .parse(source, None)
        .context("parser returned no tree")?;
    let mut out = Vec::new();
    visit_symbols(
        tree.root_node(),
        source,
        &relative.replace('\\', "/"),
        symbol_kinds(lang_id),
        &mut Vec::new(),
        &mut out,
    );
    Ok(out)
}

#[derive(Debug, Clone)]
struct CachedSymbols {
    mtime_ns: u128,
    size: u64,
    symbols: Vec<Symbol>,
}

#[derive(Default)]
pub struct SymbolIndex {
    files: HashMap<String, CachedSymbols>,
    last_refresh: Option<Instant>,
    status: SymbolIndexStatus,
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct SymbolIndexStatus {
    pub truncated: bool,
    pub skipped_files: usize,
    pub scanned_entries: usize,
    pub indexed_files: usize,
    pub indexed_bytes: u64,
}

impl SymbolIndex {
    pub fn invalidate(&mut self, relative: &str) {
        self.files.remove(&relative.replace('\\', "/"));
        self.last_refresh = None;
    }

    pub fn status(&self) -> SymbolIndexStatus {
        self.status.clone()
    }

    fn refresh_path(&mut self, root: &Path, relative: &str) -> Result<()> {
        let normalized = relative.replace('\\', "/");
        let lexical = root.join(&normalized);
        if identify(&lexical).is_none() {
            self.files.remove(&normalized);
            return Ok(());
        }
        let (path, normalized) = crate::workspace::contained_existing(root, &normalized)?;
        let metadata = match fs::metadata(&path) {
            Ok(value) if value.is_file() => value,
            _ => {
                self.files.remove(&normalized);
                return Ok(());
            }
        };
        anyhow::ensure!(
            metadata.len() <= MAX_SYMBOL_FILE_BYTES,
            "symbol file exceeds read limit"
        );
        let stamp = mtime_ns(&metadata);
        let size = metadata.len();
        if self
            .files
            .get(&normalized)
            .is_some_and(|cached| cached.mtime_ns == stamp && cached.size == size)
        {
            return Ok(());
        }

        let (_, _, source) = read_source(root, &normalized)?;
        let symbols = parse_symbols(&path, &normalized, &source)?;
        self.files.insert(
            normalized,
            CachedSymbols {
                mtime_ns: stamp,
                size,
                symbols,
            },
        );
        Ok(())
    }

    fn refresh(&mut self, root: &Path, max_files: usize) -> Result<()> {
        if self
            .last_refresh
            .is_some_and(|time| time.elapsed() < REFRESH_INTERVAL)
        {
            return Ok(());
        }
        let max_files = max_files.clamp(1, MAX_INDEX_FILES);
        let started = Instant::now();
        let mut seen = HashSet::new();
        let mut status = SymbolIndexStatus::default();
        let mut symbols = 0;

        for entry in WalkBuilder::new(root).hidden(true).git_ignore(true).build() {
            if status.scanned_entries >= MAX_INDEX_ENTRIES || started.elapsed() >= SCAN_TIMEOUT {
                status.truncated = true;
                break;
            }
            status.scanned_entries += 1;
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    status.truncated = true;
                    continue;
                }
            };
            if !entry.file_type().is_some_and(|kind| kind.is_file())
                || identify(entry.path()).is_none()
            {
                continue;
            }
            if status.indexed_files >= max_files || symbols >= MAX_INDEX_SYMBOLS {
                status.truncated = true;
                break;
            }
            let relative = entry
                .path()
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let size = match entry.metadata() {
                Ok(metadata) => metadata.len(),
                Err(_) => {
                    status.truncated = true;
                    status.skipped_files += 1;
                    continue;
                }
            };
            if size > MAX_SYMBOL_FILE_BYTES
                || status.indexed_bytes.saturating_add(size) > MAX_INDEX_BYTES
            {
                self.files.remove(&relative);
                status.truncated = true;
                status.skipped_files += 1;
                continue;
            }
            if self.refresh_path(root, &relative).is_err() {
                self.files.remove(&relative);
                status.truncated = true;
                status.skipped_files += 1;
                continue;
            }
            if let Some(file) = self.files.get(&relative) {
                symbols += file.symbols.len();
                status.truncated |= file.symbols.len() >= MAX_FILE_SYMBOLS;
                status.indexed_bytes += file.size;
                status.indexed_files += 1;
                seen.insert(relative);
            }
        }
        // A bounded partial scan must not retain unseen files from previous generations.
        self.files.retain(|path, _| seen.contains(path));
        self.status = status;
        self.last_refresh = Some(Instant::now());
        Ok(())
    }

    fn all_symbols(&self) -> Vec<Symbol> {
        let mut symbols = self
            .files
            .values()
            .flat_map(|entry| entry.symbols.iter().cloned())
            .collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.start_line.cmp(&right.start_line))
                .then(left.qualified_name.cmp(&right.qualified_name))
        });
        symbols
    }

    pub fn find_symbols(&mut self, root: &Path, query: &str, limit: usize) -> Result<Vec<Symbol>> {
        self.refresh(root, 100_000)?;
        let q = query.to_ascii_lowercase();
        let mut symbols = self
            .files
            .values()
            .flat_map(|entry| entry.symbols.iter())
            .filter(|symbol| {
                symbol.name.to_ascii_lowercase().contains(&q)
                    || symbol.qualified_name.to_ascii_lowercase().contains(&q)
                    || symbol.id.to_ascii_lowercase().contains(&q)
            })
            .collect::<Vec<_>>();
        let compare = |left: &&Symbol, right: &&Symbol| {
            let exact = |symbol: &Symbol| {
                symbol.name.eq_ignore_ascii_case(query)
                    || symbol.qualified_name.eq_ignore_ascii_case(query)
            };
            (!exact(left))
                .cmp(&!exact(right))
                .then(left.qualified_name.len().cmp(&right.qualified_name.len()))
                .then(left.path.cmp(&right.path))
                .then(left.start_line.cmp(&right.start_line))
                .then(left.qualified_name.cmp(&right.qualified_name))
                .then(left.start_byte.cmp(&right.start_byte))
                .then(left.kind.cmp(&right.kind))
        };
        let bounded = limit.clamp(1, 500);
        if symbols.len() > bounded {
            symbols.select_nth_unstable_by(bounded, compare);
            symbols.truncate(bounded);
        }
        symbols.sort_by(compare);
        Ok(symbols.into_iter().cloned().collect())
    }

    pub fn read_symbol(&mut self, root: &Path, symbol_id: &str) -> Result<Option<SymbolRead>> {
        let relative = symbol_id
            .split("::")
            .next()
            .unwrap_or("")
            .replace('\\', "/");
        if relative.is_empty() || identify(Path::new(&relative)).is_none() {
            return Ok(None);
        }
        if !root.join(&relative).exists() {
            return Ok(None);
        }
        let (path, display, source) = read_source(root, &relative)?;
        let normalized_id = symbol_id.replace('\\', "/");
        let symbol = parse_symbols(&path, &display, &source)?
            .into_iter()
            .find(|symbol| symbol.id == normalized_id);
        Ok(symbol.map(|symbol| {
            let text =
                String::from_utf8_lossy(&source[symbol.start_byte..symbol.end_byte]).to_string();
            SymbolRead {
                symbol,
                source: text,
            }
        }))
    }

    pub fn find_references(
        &mut self,
        root: &Path,
        target: &str,
        limit: usize,
    ) -> Result<Vec<SymbolReference>> {
        self.refresh(root, 100_000)?;
        let target_name = target.rsplit("::").next().unwrap_or(target);
        let word = regex::Regex::new(&format!(r"\b{}\b", regex::escape(target_name)))?;
        let mut paths = self.files.keys().cloned().collect::<Vec<_>>();
        paths.sort();

        let mut out = Vec::new();
        let limit = limit.clamp(1, 500);
        let started = Instant::now();
        let mut read_bytes = 0u64;
        for relative in paths {
            if out.len() >= limit {
                break;
            }
            if started.elapsed() >= SCAN_TIMEOUT || read_bytes >= MAX_INDEX_BYTES {
                self.status.truncated = true;
                break;
            }
            let bytes = match read_source(root, &relative) {
                Ok((_, _, bytes)) => bytes,
                Err(_) => {
                    self.status.truncated = true;
                    continue;
                }
            };
            read_bytes += bytes.len() as u64;
            let text = String::from_utf8_lossy(&bytes);
            let symbols = self
                .files
                .get(&relative)
                .map(|entry| entry.symbols.as_slice())
                .unwrap_or_default();
            for (idx, line) in text.lines().enumerate() {
                if out.len() >= limit {
                    break;
                }
                if !word.is_match(line) {
                    continue;
                }
                let line_no = idx + 1;
                if symbols
                    .iter()
                    .any(|symbol| symbol.name == target_name && symbol.start_line == line_no)
                {
                    continue;
                }
                out.push(SymbolReference {
                    path: relative.clone(),
                    line: line_no,
                    containing_symbol: containing_symbol(symbols, line_no)
                        .map(|symbol| symbol.id.clone()),
                    target_name: target_name.to_string(),
                    kind: "lexical_ast_reference".to_string(),
                });
            }
        }
        Ok(out)
    }

    pub fn dependency_edges(
        &mut self,
        root: &Path,
        max_symbols: usize,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.refresh(root, 100_000)?;
        let symbols = self.all_symbols();
        let mut by_name: HashMap<String, Vec<&Symbol>> = HashMap::new();
        for symbol in &symbols {
            by_name.entry(symbol.name.clone()).or_default().push(symbol);
        }

        let callish = regex::Regex::new(r"\b([A-Za-z_$][A-Za-z0-9_$]*)\s*(?:\(|\.)")?;
        let mut graph = HashMap::new();
        let mut sources: HashMap<String, Vec<u8>> = HashMap::new();
        let started = Instant::now();
        let mut read_bytes = 0u64;

        for symbol in symbols.iter().take(max_symbols.min(MAX_INDEX_SYMBOLS)) {
            if started.elapsed() >= SCAN_TIMEOUT || read_bytes >= MAX_INDEX_BYTES {
                self.status.truncated = true;
                break;
            }
            if !sources.contains_key(&symbol.path) {
                let bytes = match read_source(root, &symbol.path) {
                    Ok((_, _, bytes)) => bytes,
                    Err(_) => {
                        self.status.truncated = true;
                        continue;
                    }
                };
                read_bytes += bytes.len() as u64;
                sources.insert(symbol.path.clone(), bytes);
            }
            let source = sources
                .get(&symbol.path)
                .expect("source inserted immediately above");
            let symbol_source = String::from_utf8_lossy(
                source
                    .get(symbol.start_byte..symbol.end_byte)
                    .unwrap_or_default(),
            );
            let mut dependencies = Vec::new();
            for capture in callish.captures_iter(&symbol_source) {
                let Some(name) = capture.get(1).map(|value| value.as_str()) else {
                    continue;
                };
                if name == symbol.name {
                    continue;
                }
                if let Some(candidates) = by_name.get(name)
                    && candidates.len() == 1
                {
                    dependencies.push(candidates[0].id.clone());
                }
            }
            dependencies.sort();
            dependencies.dedup();
            if !dependencies.is_empty() {
                graph.insert(symbol.id.clone(), dependencies);
            }
        }
        Ok(graph)
    }
}

fn containing_symbol(symbols: &[Symbol], line: usize) -> Option<&Symbol> {
    symbols
        .iter()
        .filter(|symbol| symbol.start_line <= line && line <= symbol.end_line)
        .min_by_key(|symbol| symbol.end_line.saturating_sub(symbol.start_line))
}

pub(crate) fn read_symbol_snapshot(root: &Path, symbol_id: &str) -> Result<(SymbolRead, Vec<u8>)> {
    let relative = symbol_id.split("::").next().unwrap_or("");
    let (path, display, bytes) = read_source(root, relative)?;
    let symbol = parse_symbols(&path, &display, &bytes)?
        .into_iter()
        .find(|symbol| symbol.id == symbol_id)
        .context("symbol not found")?;
    let source = std::str::from_utf8(&bytes[symbol.start_byte..symbol.end_byte])?.to_string();
    Ok((SymbolRead { symbol, source }, bytes))
}

#[cfg(test)]
pub fn read_symbol(root: &Path, symbol_id: &str) -> Result<Option<SymbolRead>> {
    SymbolIndex::default().read_symbol(root, symbol_id)
}

#[cfg(test)]
fn dependency_edges(root: &Path, max_symbols: usize) -> Result<HashMap<String, Vec<String>>> {
    SymbolIndex::default().dependency_edges(root, max_symbols)
}

#[cfg(test)]
mod dependency_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn symbol_reads_refuse_traversal_absolute_paths_and_symlinks() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("repo");
        fs::create_dir(&root).unwrap();
        let outside = dir.path().join("outside.py");
        fs::write(&outside, "def target():\n    return 1\n").unwrap();
        let mut index = SymbolIndex::default();
        assert!(index.read_symbol(&root, "../outside.py::target").is_err());
        assert!(
            index
                .read_symbol(&root, &format!("{}::target", outside.display()))
                .is_err()
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("linked.py")).unwrap();
            assert!(index.read_symbol(&root, "linked.py::target").is_err());
        }
        fs::write(root.join("safe.py"), "def target():\n    return 2\n").unwrap();
        assert!(
            index
                .read_symbol(&root, "safe.py::target")
                .unwrap()
                .unwrap()
                .source
                .contains("return 2")
        );
    }

    #[test]
    fn oversized_sources_are_skipped_and_cached_scan_reports_partial_results() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("large.py"),
            vec![b' '; MAX_SYMBOL_FILE_BYTES as usize + 1],
        )
        .unwrap();
        fs::write(dir.path().join("safe.py"), "def target():\n    return 1\n").unwrap();
        let mut index = SymbolIndex::default();
        assert_eq!(
            index.find_symbols(dir.path(), "target", 5).unwrap().len(),
            1
        );
        assert!(index.status().truncated);
        assert_eq!(index.status().skipped_files, 1);
        assert!(index.read_symbol(dir.path(), "large.py::target").is_err());
        let refreshed = index.last_refresh;
        index.find_symbols(dir.path(), "target", 5).unwrap();
        index.find_references(dir.path(), "target", 5).unwrap();
        index.dependency_edges(dir.path(), 10).unwrap();
        assert_eq!(index.last_refresh, refreshed);
        fs::write(
            dir.path().join("new.py"),
            "def new_target():\n    return 1\n",
        )
        .unwrap();
        index.invalidate("new.py");
        assert_eq!(
            index
                .find_symbols(dir.path(), "new_target", 5)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn direct_symbol_read_uses_current_source_offsets_even_when_scan_is_cached() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("safe.py"), "def target():\n    return 1\n").unwrap();
        let mut index = SymbolIndex::default();
        index.find_symbols(dir.path(), "target", 5).unwrap();
        fs::write(
            dir.path().join("safe.py"),
            "# shifted offsets\ndef target():\n    return 42\n",
        )
        .unwrap();
        let read = index
            .read_symbol(dir.path(), "safe.py::target")
            .unwrap()
            .unwrap();
        assert_eq!(read.symbol.start_line, 2);
        assert!(read.source.contains("return 42"));
    }

    #[test]
    fn dependency_edges_use_scanned_symbol_offsets() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("sample.py"),
            "def helper():\n    return 1\n\ndef caller():\n    return helper()\n",
        )
        .unwrap();

        let graph = dependency_edges(dir.path(), 100).unwrap();
        assert_eq!(
            graph.get("sample.py::caller"),
            Some(&vec!["sample.py::helper".to_string()])
        );
    }

    #[test]
    fn limited_symbol_search_keeps_exact_matches_and_stable_ties() {
        let dir = tempdir().unwrap();
        for index in 0..32 {
            fs::write(
                dir.path().join(format!("{index:02}.py")),
                "def target_extra():\n    pass\ndef target():\n    pass\n",
            )
            .unwrap();
        }
        let mut index = SymbolIndex::default();
        let first = index.find_symbols(dir.path(), "target", 3).unwrap();
        assert_eq!(first.len(), 3);
        assert!(first.iter().all(|symbol| symbol.name == "target"));
        assert_eq!(first, index.find_symbols(dir.path(), "target", 3).unwrap());
        assert!(first[0].path < first[1].path);
    }

    #[test]
    fn same_line_duplicate_names_preserve_source_order_at_cutoff() {
        let dir = tempdir().unwrap();
        for file in 0..32 {
            fs::write(dir.path().join(format!("{file:02}.js")), "function target() { return 1; } function target() { return 2; } function target() { return 3; }").unwrap();
        }
        for _ in 0..8 {
            let mut index = SymbolIndex::default();
            let selected = index.find_symbols(dir.path(), "target", 1).unwrap();
            assert_eq!(selected[0].path, "00.js");
            assert_eq!(selected[0].start_byte, 0);
        }
    }

    #[test]
    fn symbol_index_reuses_unchanged_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("sample.py"), "def first():\n    return 1\n").unwrap();
        let mut index = SymbolIndex::default();
        let first = index.find_symbols(dir.path(), "first", 10).unwrap();
        assert_eq!(first.len(), 1);

        let second = index.find_symbols(dir.path(), "first", 10).unwrap();
        assert_eq!(second, first);

        fs::write(
            dir.path().join("sample.py"),
            "def second_name():\n    return 2\n",
        )
        .unwrap();
        index.invalidate("sample.py");
        let updated = index.find_symbols(dir.path(), "second_name", 10).unwrap();
        assert_eq!(updated.len(), 1);
    }
}
