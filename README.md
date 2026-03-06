# munchrs

![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/vihu/munchrs/ci.yml)
![GitHub Release](https://img.shields.io/github/v/release/vihu/munchrs)

A fast, token-efficient MCP server for codebase indexing and symbol retrieval.

**munchrs** is a Rust binary that indexes codebases using tree-sitter AST parsing and exposes tools over the Model Context Protocol (MCP) via stdio transport. It gives AI assistants structured access to your codebase — symbols, outlines, and full-text search — without dumping entire files into context.

## Install

Download a prebuilt binary from [Releases](https://github.com/vihu/munchrs/releases), or build from source:

```sh
git clone https://github.com/vihu/munchrs.git
cd munchrs
just build
# Binary at target/release/munchrs
```

## Token Savings

Instead of dumping entire files into an LLM context window, munchrs returns only the structure and symbols you need. Measured on the munchrs codebase itself (22 Rust files, 5,309 lines, ~42k tokens):

| Approach              | What's sent to the LLM                                     | Est. tokens |
| --------------------- | ---------------------------------------------------------- | ----------- |
| Read all source files | 22 files verbatim                                          | ~42,000     |
| Read 2 relevant files | `server.rs` + `extractor.rs`                               | ~15,000     |
| munchrs tools         | repo outline + file tree + 2 file outlines + symbol search | ~1,900      |

A typical "understand the codebase and find a function" workflow uses **~95% fewer tokens** compared to reading all files, or **~87% fewer** compared to reading just the right files.

## MCP Configuration

### Claude Code

```sh
claude mcp add munchrs /path/to/munchrs
```

### With `.mcp.json`

Add the following to your `.mcp.json` (user/project scoped):

```
  "mcpServers": {
    "munchrs": {
      "type": "stdio",
      "command": "/path/to/munchrs",
      "args": [],
      "env": {}
    }
  }
```

## Tools

| Tool               | Description                                     |
| ------------------ | ----------------------------------------------- |
| `index_folder`     | Index a local folder containing source code     |
| `list_repos`       | List all indexed repositories                   |
| `get_file_tree`    | Get the file tree of an indexed repository      |
| `get_file_outline` | Get all symbols in a file with signatures       |
| `get_symbol`       | Get full source code of a specific symbol       |
| `get_symbols`      | Get source code of multiple symbols in one call |
| `search_symbols`   | Search for symbols matching a query             |
| `search_text`      | Full-text search across indexed file contents   |
| `get_repo_outline` | High-level overview of a repository             |
| `invalidate_cache` | Delete index, forcing re-index                  |

## Supported Languages

Python, JavaScript, TypeScript, Go, Rust, Java, PHP, Dart, C#, C, C++, Swift, Elixir, Erlang

## Environment Variables

| Variable             | Description                                             |
| -------------------- | ------------------------------------------------------- |
| `MUNCHRS_LOG_LEVEL`  | Log level (default: `info`)                             |
| `MUNCHRS_LOG_FILE`   | Path to log file (logs to stderr if unset)              |
| `CODE_INDEX_PATH`    | Custom index storage directory (default: `~/.munchrs/`) |
| `OPENROUTER_API_KEY` | API key for AI-powered symbol summarization (optional)  |

## Recommended CLAUDE.md Instructions

Add the following to your project's `CLAUDE.md` to teach it how to use munchrs efficiently:

```markdown
## Codebase Exploration

When a `munchrs` MCP server is available, prefer it over raw file reads for codebase exploration:

- Use `list_repos` to check if the repo is already indexed; if not, use `index_folder` to index it
- Use `get_repo_outline` for a high-level overview of the codebase (directories, languages, symbol counts)
- Use `get_file_tree` to browse the project structure before diving into specific files
- Use `get_file_outline` to see all symbols in a file with their signatures — avoid reading entire files
- Use `search_symbols` to find functions, classes, or types by name across the repo
- Use `get_symbol` to fetch the full source of a specific symbol instead of reading the whole file
- Use `get_symbols` to batch-fetch multiple symbols in one call when you need several
- Use `search_text` as a fallback for grep-like searches (string literals, comments, config values)
- Use `invalidate_cache` only when the index seems stale or corrupt — it forces a full re-index
```

## Credits

- All credit for the original python implementation goes to: https://github.com/jgravelle/jcodemunch-mcp

### Differences from [jcodemunch-mcp](https://github.com/jgravelle/jcodemunch-mcp)

- **Language:** Rust (single static binary) vs Python
- **Storage:** reads from original files on disk — no file copies in the index
- **Index layout:** nested `~/.munchrs/<owner>/<name>/` vs flat `~/.code-index/<slug>`
- **Additional languages:** Elixir and Erlang support
- **Summarization:** OpenRouter as the LLM gateway vs direct OpenAI/Gemini APIs
- **Output format:** compact [toon](https://github.com/toon-format/toon) fmt vs JSON with metadata envelopes

## License

[MIT](LICENSE)
