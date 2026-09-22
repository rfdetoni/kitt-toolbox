mod edit;
mod language;
pub mod model;
mod output;
mod search;
mod symbols;
mod workspace;

use anyhow::{Result, anyhow};
use model::{
    BlockReplaceRequest, BlockReplaceResponse, CompressionResponse, EditRequest, EditResponse,
    FileListResponse, FileReadResponse, SearchOptions, SearchResponse, Symbol, SymbolRead,
    SymbolReference,
};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

pub struct NativeEngine {
    root: PathBuf,
    symbol_index: Mutex<symbols::SymbolIndex>,
}

impl NativeEngine {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|e| anyhow!("invalid repository root: {e}"))?;
        Ok(Self {
            root,
            symbol_index: Mutex::new(symbols::SymbolIndex::default()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn search(&self, query: &str, options: SearchOptions) -> Result<SearchResponse> {
        search::search(&self.root, query, options)
    }

    fn symbol_index(&self) -> Result<MutexGuard<'_, symbols::SymbolIndex>> {
        self.symbol_index
            .lock()
            .map_err(|_| anyhow!("native symbol index lock poisoned"))
    }

    pub fn find_symbols(&self, query: &str, limit: usize) -> Result<Vec<Symbol>> {
        self.symbol_index()?.find_symbols(&self.root, query, limit)
    }

    pub fn read_symbol(&self, id: &str) -> Result<Option<SymbolRead>> {
        self.symbol_index()?.read_symbol(&self.root, id)
    }

    pub fn references(&self, id_or_name: &str, limit: usize) -> Result<Vec<SymbolReference>> {
        self.symbol_index()?
            .find_references(&self.root, id_or_name, limit)
    }

    pub fn dependency_edges(
        &self,
        max_symbols: usize,
    ) -> Result<std::collections::HashMap<String, Vec<String>>> {
        self.symbol_index()?
            .dependency_edges(&self.root, max_symbols)
    }

    pub fn replace_symbol(&self, request: EditRequest) -> Result<EditResponse> {
        let response = edit::replace_symbol(&self.root, request)?;
        if response.changed
            && let Ok(mut index) = self.symbol_index.lock()
        {
            index.invalidate(&response.path);
        }
        Ok(response)
    }

    pub fn replace_block(&self, request: BlockReplaceRequest) -> Result<BlockReplaceResponse> {
        let response = edit::replace_block(&self.root, request)?;
        if response.changed
            && let Ok(mut index) = self.symbol_index.lock()
        {
            index.invalidate(&response.path);
        }
        Ok(response)
    }

    pub fn read_file(
        &self,
        path: &str,
        start_line: usize,
        end_line: Option<usize>,
        max_bytes: usize,
        token_budget: usize,
    ) -> Result<FileReadResponse> {
        workspace::read_file(
            &self.root,
            path,
            start_line,
            end_line,
            max_bytes,
            token_budget,
        )
    }

    pub fn list_files(
        &self,
        path: &str,
        limit: usize,
        token_budget: usize,
    ) -> Result<FileListResponse> {
        workspace::list_files(&self.root, path, limit, token_budget)
    }
}

pub fn compress_process_output(
    argv: &[String],
    stdout: &str,
    stderr: &str,
    returncode: i32,
) -> CompressionResponse {
    output::compress(argv, stdout, stderr, returncode)
}

pub fn compress_process_output_with_budget(
    argv: &[String],
    stdout: &str,
    stderr: &str,
    returncode: i32,
    token_budget: usize,
) -> CompressionResponse {
    output::compress_with_budget(argv, stdout, stderr, returncode, token_budget)
}
