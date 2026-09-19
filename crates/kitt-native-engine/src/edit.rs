use crate::language::{grammar, identify};
use crate::model::{BlockReplaceRequest, BlockReplaceResponse, EditRequest, EditResponse};
use crate::symbols::read_symbol;
use crate::workspace::contained_existing;
use anyhow::{Context, Result, anyhow};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use tree_sitter::Parser;

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn validate(path: &Path, source: &[u8]) -> Result<()> {
    let Some(id) = identify(path) else {
        return Ok(());
    };
    let mut parser = Parser::new();
    parser.set_language(&grammar(id)?)?;
    let tree = parser
        .parse(source, None)
        .context("syntax parser returned no tree")?;
    if tree.root_node().has_error() {
        return Err(anyhow!("replacement introduces syntax errors"));
    }
    Ok(())
}

fn persist_atomic(path: &Path, metadata: &fs::Metadata, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| anyhow!("file has no parent"))?;
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.as_file().set_permissions(metadata.permissions())?;
    temp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("persist {}", path.display()))?;
    Ok(())
}

pub fn replace_block(root: &Path, request: BlockReplaceRequest) -> Result<BlockReplaceResponse> {
    if request.search.is_empty() {
        return Err(anyhow!("search block must not be empty"));
    }
    let (path, display) = contained_existing(root, &request.path)?;
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(anyhow!("path is not a regular file"));
    }

    let original = fs::read(&path)?;
    let old_file_hash = hash(&original);
    if let Some(expected) = &request.expected_file_hash
        && expected != &old_file_hash
    {
        return Err(anyhow!("optimistic edit conflict: file hash changed"));
    }
    let text = std::str::from_utf8(&original)
        .map_err(|_| anyhow!("block replacement requires a UTF-8 text file"))?;
    let matches = text.match_indices(request.search.as_str()).count();
    if matches == 0 {
        return Err(anyhow!("search block was not found"));
    }
    if matches > 1 {
        return Err(anyhow!(
            "search block is ambiguous: matched {matches} locations; provide more context"
        ));
    }
    if request.search == request.replacement {
        return Ok(BlockReplaceResponse {
            path: display,
            old_file_hash: old_file_hash.clone(),
            new_file_hash: old_file_hash,
            replacements: 0,
            changed: false,
        });
    }

    let updated = text.replacen(request.search.as_str(), request.replacement.as_str(), 1);
    let updated_bytes = updated.as_bytes();
    if request.validate_syntax {
        validate(&path, updated_bytes)?;
    }
    persist_atomic(&path, &metadata, updated_bytes)?;
    Ok(BlockReplaceResponse {
        path: display,
        old_file_hash,
        new_file_hash: hash(updated_bytes),
        replacements: 1,
        changed: true,
    })
}

pub fn replace_symbol(root: &Path, request: EditRequest) -> Result<EditResponse> {
    let current =
        read_symbol(root, &request.symbol_id)?.ok_or_else(|| anyhow!("symbol not found"))?;
    if let Some(expected) = &request.expected_hash
        && expected != &current.symbol.source_hash
    {
        return Err(anyhow!("optimistic edit conflict: symbol hash changed"));
    }
    let path = root.join(&current.symbol.path);
    let metadata = fs::metadata(&path)?;
    let mut bytes = fs::read(&path)?;
    let old_hash = current.symbol.source_hash.clone();
    if current.source == request.replacement {
        return Ok(EditResponse {
            path: current.symbol.path,
            old_hash: old_hash.clone(),
            new_hash: old_hash,
            changed: false,
        });
    }
    bytes.splice(
        current.symbol.start_byte..current.symbol.end_byte,
        request.replacement.as_bytes().iter().copied(),
    );
    if request.validate_syntax {
        validate(&path, &bytes)?;
    }
    persist_atomic(&path, &metadata, &bytes)?;

    // Do not re-resolve the old symbol id after persistence: a valid structural
    // edit may intentionally rename or remove the symbol.  The edit is already
    // durable at this point, so reporting an error would be a false failure.
    let new_hash = hash(request.replacement.as_bytes());
    Ok(EditResponse {
        path: current.symbol.path,
        old_hash,
        new_hash,
        changed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn block_replace_requires_unique_exact_context() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sample.py");
        fs::write(&file, "x = 1\ny = 2\nx = 1\n").unwrap();

        let ambiguous = replace_block(
            dir.path(),
            BlockReplaceRequest {
                path: "sample.py".to_string(),
                search: "x = 1".to_string(),
                replacement: "x = 3".to_string(),
                expected_file_hash: None,
                validate_syntax: true,
            },
        );
        assert!(ambiguous.unwrap_err().to_string().contains("ambiguous"));

        let result = replace_block(
            dir.path(),
            BlockReplaceRequest {
                path: "sample.py".to_string(),
                search: "x = 1\ny = 2".to_string(),
                replacement: "x = 3\ny = 4".to_string(),
                expected_file_hash: None,
                validate_syntax: true,
            },
        )
        .unwrap();
        assert!(result.changed);
        assert_eq!(result.replacements, 1);
        assert_eq!(fs::read_to_string(file).unwrap(), "x = 3\ny = 4\nx = 1\n");
    }

    #[test]
    fn block_replace_rejects_stale_hash_and_path_escape() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("sample.txt"), "before\n").unwrap();

        let stale = replace_block(
            dir.path(),
            BlockReplaceRequest {
                path: "sample.txt".to_string(),
                search: "before".to_string(),
                replacement: "after".to_string(),
                expected_file_hash: Some("stale".to_string()),
                validate_syntax: false,
            },
        );
        assert!(stale.unwrap_err().to_string().contains("file hash changed"));

        let escaped = replace_block(
            dir.path(),
            BlockReplaceRequest {
                path: "../outside.txt".to_string(),
                search: "before".to_string(),
                replacement: "after".to_string(),
                expected_file_hash: None,
                validate_syntax: false,
            },
        );
        assert!(escaped.is_err());
    }

    #[test]
    fn rename_does_not_report_failure_after_persist() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sample.py");
        fs::write(&file, "def before():\n    return 1\n").unwrap();
        let current = read_symbol(dir.path(), "sample.py::before")
            .unwrap()
            .unwrap();
        let result = replace_symbol(
            dir.path(),
            EditRequest {
                symbol_id: "sample.py::before".to_string(),
                replacement: "def after():\n    return 2".to_string(),
                expected_hash: Some(current.symbol.source_hash),
                validate_syntax: true,
            },
        )
        .unwrap();
        assert!(result.changed);
        assert!(fs::read_to_string(file).unwrap().contains("def after"));
    }
}
