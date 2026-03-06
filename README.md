# munchrs

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
| `OPENROUTER_API_KEY` | API key for AI-powered symbol summarization             |

## Credits

- All credit for the original python implementation goes to: https://github.com/jgravelle/jcodemunch-mcp

### Differences from [jcodemunch-mcp](https://github.com/jgravelle/jcodemunch-mcp)

- **Language:** Rust (single static binary) vs Python
- **Storage:** reads from original files on disk — no file copies in the index
- **Index layout:** nested `~/.munchrs/<owner>/<name>/` vs flat `~/.code-index/<slug>`
- **Additional languages:** Elixir and Erlang support
- **Summarization:** OpenRouter as the LLM gateway vs direct OpenAI/Gemini APIs
- **Output format:** compact toon tables vs JSON with metadata envelopes

## License

[MIT](LICENSE)
