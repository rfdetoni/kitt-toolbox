use crate::language::{grammar, identify, kind_label, name_of, symbol_kinds};
use crate::model::{Symbol, SymbolRead, SymbolReference};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;
use tree_sitter::{Node, Parser};

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
}

impl SymbolIndex {
    pub fn invalidate(&mut self, relative: &str) {
        self.files.remove(&relative.replace('\\', "/"));
    }

    fn refresh_path(&mut self, root: &Path, relative: &str) -> Result<()> {
        let normalized = relative.replace('\\', "/");
        let path = root.join(&normalized);
        if identify(&path).is_none() {
            self.files.remove(&normalized);
            return Ok(());
        }
        let metadata = match fs::metadata(&path) {
            Ok(value) if value.is_file() => value,
            _ => {
                self.files.remove(&normalized);
                return Ok(());
            }
        };
        let stamp = mtime_ns(&metadata);
        let size = metadata.len();
        if self
            .files
            .get(&normalized)
            .is_some_and(|cached| cached.mtime_ns == stamp && cached.size == size)
        {
            return Ok(());
        }

        let source = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
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
        let max_files = max_files.max(1);
        let mut seen = HashSet::new();
        let mut count = 0usize;
        let mut complete = true;

        for entry in WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .build()
            .filter_map(Result::ok)
        {
            if !entry
                .file_type()
                .map(|value| value.is_file())
                .unwrap_or(false)
            {
                continue;
            }
            let path = entry.path();
            if identify(path).is_none() {
                continue;
            }
            if count >= max_files {
                complete = false;
                break;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            seen.insert(relative.clone());
            self.refresh_path(root, &relative)?;
            count += 1;
        }

        if complete {
            self.files.retain(|path, _| seen.contains(path));
        }
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
        let mut symbols = self.all_symbols();
        symbols.retain(|symbol| {
            symbol.name.to_ascii_lowercase().contains(&q)
                || symbol.qualified_name.to_ascii_lowercase().contains(&q)
                || symbol.id.to_ascii_lowercase().contains(&q)
        });
        symbols.sort_by_key(|symbol| {
            let exact = symbol.name.eq_ignore_ascii_case(query)
                || symbol.qualified_name.eq_ignore_ascii_case(query);
            (!exact, symbol.qualified_name.len(), symbol.path.clone())
        });
        symbols.truncate(limit.clamp(1, 500));
        Ok(symbols)
    }

    pub fn read_symbol(&mut self, root: &Path, symbol_id: &str) -> Result<Option<SymbolRead>> {
        let relative = symbol_id.split("::").next().unwrap_or("");
        if relative.is_empty() {
            return Ok(None);
        }
        self.refresh_path(root, relative)?;
        let Some(symbol) = self
            .files
            .get(relative)
            .and_then(|entry| entry.symbols.iter().find(|symbol| symbol.id == symbol_id))
            .cloned()
        else {
            return Ok(None);
        };
        let source = fs::read(root.join(relative))?;
        let text = String::from_utf8_lossy(
            source
                .get(symbol.start_byte..symbol.end_byte)
                .unwrap_or_default(),
        )
        .to_string();
        Ok(Some(SymbolRead {
            symbol,
            source: text,
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
        for relative in paths {
            if out.len() >= limit {
                break;
            }
            let bytes = match fs::read(root.join(&relative)) {
                Ok(value) => value,
                Err(_) => continue,
            };
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

        for symbol in symbols.iter().take(max_symbols) {
            if !sources.contains_key(&symbol.path) {
                sources.insert(symbol.path.clone(), fs::read(root.join(&symbol.path))?);
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
        let updated = index.find_symbols(dir.path(), "second_name", 10).unwrap();
        assert_eq!(updated.len(), 1);
    }
}
