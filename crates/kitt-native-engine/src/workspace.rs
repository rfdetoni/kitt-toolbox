use crate::model::{FileListResponse, FileReadResponse};
use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

const DEFAULT_MAX_FILE_BYTES: usize = 4 * 1024 * 1024;

fn estimated_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

fn validate_relative(relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute() {
        return Err(anyhow!("absolute paths are not allowed"));
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => clean.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("path traversal is not allowed"));
            }
        }
    }
    Ok(clean)
}

pub(crate) fn contained_existing(root: &Path, relative: &str) -> Result<(PathBuf, String)> {
    let clean = validate_relative(relative)?;
    let target = root.join(&clean);
    let mut cursor = root.to_path_buf();
    for component in clean.components() {
        cursor.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&cursor)
            .map_err(|e| anyhow!("cannot inspect '{}': {e}", relative))?;
        if metadata.file_type().is_symlink() {
            return Err(anyhow!("symlink paths are not allowed"));
        }
    }
    let canonical = target
        .canonicalize()
        .map_err(|e| anyhow!("cannot resolve '{}': {e}", relative))?;
    if !canonical.starts_with(root) {
        return Err(anyhow!("path escapes repository root"));
    }
    let display = canonical
        .strip_prefix(root)
        .map_err(|_| anyhow!("path escapes repository root"))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok((
        canonical,
        if display.is_empty() {
            ".".into()
        } else {
            display
        },
    ))
}

fn mtime_ns(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn read_file(
    root: &Path,
    relative: &str,
    start_line: usize,
    end_line: Option<usize>,
    max_bytes: usize,
    token_budget: usize,
) -> Result<FileReadResponse> {
    read_file_from_byte(
        root,
        relative,
        start_line,
        end_line,
        max_bytes,
        token_budget,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn read_file_from_byte(
    root: &Path,
    relative: &str,
    start_line: usize,
    end_line: Option<usize>,
    max_bytes: usize,
    token_budget: usize,
    start_byte: Option<usize>,
) -> Result<FileReadResponse> {
    let (path, display) = contained_existing(root, relative)?;
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(anyhow!("path is not a regular file"));
    }
    if metadata.len() > DEFAULT_MAX_FILE_BYTES as u64 {
        return Err(anyhow!("file exceeds read limit"));
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(&path)?
        .take(DEFAULT_MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > DEFAULT_MAX_FILE_BYTES {
        return Err(anyhow!("file exceeds read limit"));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| anyhow!("read_file requires UTF-8 text"))?;
    let mut starts = vec![0];
    for (offset, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' && offset + 1 < bytes.len() {
            starts.push(offset + 1);
        }
    }
    let total_lines = if bytes.is_empty() { 0 } else { starts.len() };
    let start =
        start_byte.unwrap_or_else(|| *starts.get(start_line.max(1) - 1).unwrap_or(&bytes.len()));
    if start > bytes.len() || !text.is_char_boundary(start) {
        return Err(anyhow!(
            "start_byte must be a UTF-8 byte boundary within the file"
        ));
    }
    let first_line = starts.partition_point(|offset| *offset <= start).max(1);
    let requested_end_line = end_line
        .unwrap_or(first_line.saturating_add(199))
        .max(first_line)
        .min(first_line.saturating_add(4999));
    let requested_end = *starts.get(requested_end_line).unwrap_or(&bytes.len());
    let byte_budget = if max_bytes == 0 {
        DEFAULT_MAX_FILE_BYTES
    } else {
        max_bytes.min(DEFAULT_MAX_FILE_BYTES)
    };
    let char_budget = token_budget.clamp(64, 32_000).saturating_mul(4);
    let mut end = start
        .saturating_add(byte_budget)
        .min(requested_end)
        .min(bytes.len());
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    if let Some((offset, _)) = text[start..end].char_indices().nth(char_budget) {
        end = start + offset;
    }
    if end == start && start < bytes.len() {
        return Err(anyhow!("read budget cannot fit the next UTF-8 character"));
    }
    let content = text[start..end].to_string();
    let returned_end = if bytes.is_empty() {
        0
    } else {
        starts
            .partition_point(|offset| *offset < end)
            .max(first_line)
    };
    let truncated = end < bytes.len();
    let partial_line_truncated = truncated && end > 0 && bytes[end - 1] != b'\n';
    let next_start_line = truncated.then(|| starts.partition_point(|offset| *offset <= end).max(1));
    Ok(FileReadResponse {
        path: display,
        content_hash: sha256_bytes(content.as_bytes()),
        full_file_hash: sha256_bytes(&bytes),
        estimated_tokens: estimated_tokens(&content),
        content,
        start_line: first_line,
        end_line: returned_end,
        total_lines,
        omitted_lines: total_lines.saturating_sub(returned_end),
        next_start_line,
        start_byte: start,
        next_start_byte: truncated.then_some(end),
        truncated,
        partial_line_truncated,
        file_size: bytes.len() as u64,
        mtime_ns: mtime_ns(&metadata),
    })
}

pub fn list_files(
    root: &Path,
    relative: &str,
    limit: usize,
    token_budget: usize,
) -> Result<FileListResponse> {
    let (directory, _) = contained_existing(root, relative)?;
    if !directory.is_dir() {
        return Err(anyhow!("path is not a directory"));
    }

    let limit = limit.clamp(1, 500);
    let char_budget = token_budget.clamp(64, 8_000).saturating_mul(4);
    let mut candidates = Vec::new();

    for entry in fs::read_dir(&directory)? {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let canonical = match entry.path().canonicalize() {
            Ok(value) if value.starts_with(root) => value,
            _ => continue,
        };
        let relative = match canonical.strip_prefix(root) {
            Ok(value) => value.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        candidates.push(relative);
    }
    candidates.sort();

    let total = candidates.len();
    let mut files = Vec::new();
    let mut chars = 0usize;
    for value in candidates.into_iter().take(limit) {
        let extra = value.chars().count() + usize::from(!files.is_empty());
        if !files.is_empty() && chars.saturating_add(extra) > char_budget {
            break;
        }
        chars += extra;
        files.push(value);
    }

    let omitted = total.saturating_sub(files.len());
    let joined = files.join("\n");
    Ok(FileListResponse {
        files,
        omitted,
        estimated_tokens: estimated_tokens(&joined),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "kitt-native-workspace-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root.canonicalize().unwrap()
    }

    #[test]
    fn read_is_token_bounded_and_contained() {
        let root = temp_root();
        let mut f = fs::File::create(root.join("many.txt")).unwrap();
        for i in 0..500 {
            writeln!(f, "line-{i:04} {}", "x".repeat(40)).unwrap();
        }
        let response = read_file(&root, "many.txt", 1, Some(500), 4 * 1024 * 1024, 64).unwrap();
        assert!(response.estimated_tokens <= 80);
        assert!(response.next_start_line.is_some());
        assert!(read_file(&root, "../outside", 1, None, 1024, 64).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_is_sorted_and_bounded() {
        let root = temp_root();
        for name in ["c.py", "a.py", "b.py"] {
            fs::write(root.join(name), name).unwrap();
        }
        let response = list_files(&root, ".", 2, 64).unwrap();
        assert_eq!(response.files, vec!["a.py", "b.py"]);
        assert_eq!(response.omitted, 1);
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod cursor_regression {
    use super::*;
    #[test]
    fn byte_cursor_round_trips_long_unicode_lines_and_original_newlines() {
        let dir = tempfile::tempdir().unwrap();
        let original = format!("{}\r\nlast line\n", "á🦀".repeat(300));
        fs::write(dir.path().join("long.txt"), &original).unwrap();
        let mut cursor = None;
        let mut rebuilt = String::new();
        loop {
            let page =
                read_file_from_byte(dir.path(), "long.txt", 1, None, 129, 64, cursor).unwrap();
            assert!(page.content.len() <= 129);
            rebuilt.push_str(&page.content);
            match page.next_start_byte {
                Some(next) => {
                    assert!(next > cursor.unwrap_or(0));
                    cursor = Some(next);
                }
                None => break,
            }
        }
        assert_eq!(rebuilt, original);
    }
}
