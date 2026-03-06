use crate::tools;
use rmcp::{
    ErrorData as McpError, ServerHandler, handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;

#[derive(Clone)]
pub struct MunchServer {
    storage_path: Option<String>,
    tool_router: ToolRouter<MunchServer>,
}

// Parameter structs for each tool

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IndexFolderParams {
    /// Path to local folder (absolute or relative, supports ~ for home directory)
    pub path: String,
    /// Use AI to generate symbol summaries (requires OPENROUTER_API_KEY)
    #[serde(default = "default_true")]
    pub use_ai_summaries: bool,
    /// Additional gitignore-style patterns to exclude from indexing
    #[serde(default)]
    pub extra_ignore_patterns: Option<Vec<String>>,
    /// Whether to follow symlinks. Default false for security.
    #[serde(default)]
    pub follow_symlinks: bool,
    /// When true and an existing index exists, only re-index changed files.
    #[serde(default)]
    pub incremental: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RepoParams {
    /// Repository identifier (owner/repo or just repo name)
    pub repo: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetFileTreeParams {
    /// Repository identifier (owner/repo or just repo name)
    pub repo: String,
    /// Optional path prefix to filter (e.g., 'src/utils')
    #[serde(default)]
    pub path_prefix: String,
    /// Include per-file summaries in the tree output
    #[serde(default)]
    pub include_summaries: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetFileOutlineParams {
    /// Repository identifier (owner/repo or just repo name)
    pub repo: String,
    /// Path to the file within the repository (e.g., 'src/main.py')
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolParams {
    /// Repository identifier (owner/repo or just repo name)
    pub repo: String,
    /// Symbol ID from get_file_outline or search_symbols
    pub symbol_id: String,
    /// Verify content hash matches stored hash (detects source drift)
    #[serde(default)]
    pub verify: bool,
    /// Number of lines before/after symbol to include for context
    #[serde(default)]
    pub context_lines: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolsParams {
    /// Repository identifier (owner/repo or just repo name)
    pub repo: String,
    /// List of symbol IDs to retrieve
    pub symbol_ids: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSymbolsParams {
    /// Repository identifier (owner/repo or just repo name)
    pub repo: String,
    /// Search query (matches symbol names, signatures, summaries, docstrings)
    pub query: String,
    /// Optional filter by symbol kind
    #[serde(default)]
    pub kind: Option<String>,
    /// Optional glob pattern to filter files (e.g., 'src/**/*.py')
    #[serde(default)]
    pub file_pattern: Option<String>,
    /// Optional filter by language
    #[serde(default)]
    pub language: Option<String>,
    /// Maximum number of results to return
    #[serde(default = "default_10")]
    pub max_results: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchTextParams {
    /// Repository identifier (owner/repo or just repo name)
    pub repo: String,
    /// Text to search for (case-insensitive substring match)
    pub query: String,
    /// Optional glob pattern to filter files (e.g., '*.py')
    #[serde(default)]
    pub file_pattern: Option<String>,
    /// Maximum number of matching lines to return
    #[serde(default = "default_20")]
    pub max_results: usize,
}

fn default_true() -> bool {
    true
}
fn default_10() -> usize {
    10
}
fn default_20() -> usize {
    20
}

#[tool_router]
impl MunchServer {
    pub fn new(storage_path: Option<String>) -> Self {
        Self {
            storage_path: storage_path.clone(),
            tool_router: Self::tool_router(),
        }
    }

    fn sp(&self) -> Option<&str> {
        self.storage_path.as_deref()
    }

    /// Index a local folder containing source code. Response includes `discovery_skip_counts` (files filtered per reason), `no_symbols_count`/`no_symbols_files` (files with no extractable symbols) for diagnosing missing files.
    #[tool(name = "index_folder")]
    async fn index_folder(
        &self,
        Parameters(params): Parameters<IndexFolderParams>,
    ) -> Result<CallToolResult, McpError> {
        let sp = self.storage_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            tools::index_folder::index_folder(
                &params.path,
                params.use_ai_summaries,
                sp.as_deref(),
                params.extra_ignore_patterns.as_deref(),
                params.follow_symlinks,
                params.incremental,
            )
        })
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// List all indexed repositories.
    #[tool(name = "list_repos")]
    fn list_repos(&self) -> Result<CallToolResult, McpError> {
        let result = tools::list_repos::list_repos(self.sp());
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// Get the file tree of an indexed repository, optionally filtered by path prefix.
    #[tool(name = "get_file_tree")]
    fn get_file_tree(
        &self,
        Parameters(params): Parameters<GetFileTreeParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::get_file_tree::get_file_tree(
            &params.repo,
            &params.path_prefix,
            params.include_summaries,
            self.sp(),
        );
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// Get all symbols (functions, classes, methods) in a file with signatures and summaries.
    #[tool(name = "get_file_outline")]
    fn get_file_outline(
        &self,
        Parameters(params): Parameters<GetFileOutlineParams>,
    ) -> Result<CallToolResult, McpError> {
        let result =
            tools::get_file_outline::get_file_outline(&params.repo, &params.file_path, self.sp());
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// Get the full source code of a specific symbol. Use after identifying relevant symbols via get_file_outline or search_symbols.
    #[tool(name = "get_symbol")]
    fn get_symbol(
        &self,
        Parameters(params): Parameters<GetSymbolParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::get_symbol::get_symbol(
            &params.repo,
            &params.symbol_id,
            params.verify,
            params.context_lines,
            self.sp(),
        );
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// Get full source code of multiple symbols in one call. Efficient for loading related symbols.
    #[tool(name = "get_symbols")]
    fn get_symbols(
        &self,
        Parameters(params): Parameters<GetSymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::get_symbol::get_symbols(&params.repo, &params.symbol_ids, self.sp());
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// Search for symbols matching a query across the entire indexed repository. Returns matches with signatures and summaries.
    #[tool(name = "search_symbols")]
    fn search_symbols(
        &self,
        Parameters(params): Parameters<SearchSymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::search_symbols::search_symbols(
            &params.repo,
            &params.query,
            params.kind.as_deref(),
            params.file_pattern.as_deref(),
            params.language.as_deref(),
            params.max_results,
            self.sp(),
        );
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// Full-text search across indexed file contents. Useful when symbol search misses (e.g., string literals, comments, config values).
    #[tool(name = "search_text")]
    fn search_text(
        &self,
        Parameters(params): Parameters<SearchTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::search_text::search_text(
            &params.repo,
            &params.query,
            params.file_pattern.as_deref(),
            params.max_results,
            self.sp(),
        );
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// Get a high-level overview of an indexed repository: directories, file counts, language breakdown, symbol counts.
    #[tool(name = "get_repo_outline")]
    fn get_repo_outline(
        &self,
        Parameters(params): Parameters<RepoParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::get_repo_outline::get_repo_outline(&params.repo, self.sp());
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    /// Delete the index and cached files for a repository. Forces a full re-index on next index_folder call.
    #[tool(name = "invalidate_cache")]
    fn invalidate_cache(
        &self,
        Parameters(params): Parameters<RepoParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::invalidate_cache::invalidate_cache(&params.repo, self.sp());
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for MunchServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("munchrs", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "munchrs indexes codebases using tree-sitter AST parsing and exposes tools for \
             token-efficient symbol discovery and retrieval. Use index_folder to index a \
             codebase, then use search_symbols, get_file_outline, get_symbol, etc. to explore it."
                    .to_string(),
            )
    }
}
