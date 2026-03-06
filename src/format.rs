use crate::parser::SymbolNode;
use std::fmt::Write;

/// Format a compact `key: val | key: val` header line.
pub fn format_kv_header(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Quote a TOON value if it contains the delimiter, newlines, or is empty.
pub fn quote_toon(value: &str, delimiter: char) -> String {
    if value.is_empty() || value.contains(delimiter) || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Format a TOON tabular block.
///
/// Produces:
/// ```text
/// [N]{col1|col2}:
///   val1|val2
///   val3|val4
/// ```
pub fn format_toon_table(columns: &[&str], rows: &[Vec<String>], delimiter: char) -> String {
    let mut out = String::new();
    let delim_str = String::from(delimiter);
    let header_cols = columns.join(&delim_str);
    writeln!(out, "[{}]{{{header_cols}}}:", rows.len()).unwrap();
    for row in rows {
        let quoted: Vec<String> = row.iter().map(|v| quote_toon(v, delimiter)).collect();
        writeln!(out, "  {}", quoted.join(&delim_str)).unwrap();
    }
    out
}

/// Render a file tree as plain indented text.
///
/// Expects a nested JSON tree (as built by get_file_tree's tree builder).
pub fn format_file_tree(tree: &[serde_json::Value], indent: usize) -> String {
    let mut out = String::new();
    let prefix = "  ".repeat(indent);
    for node in tree {
        let path = node.get("path").and_then(|v| v.as_str()).unwrap_or("?");
        let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("file");

        if node_type == "dir" {
            // Extract just the directory name from path like "src/"
            let dir_name = path.split('/').rfind(|s| !s.is_empty()).unwrap_or(path);
            writeln!(out, "{prefix}{dir_name}/").unwrap();
            if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                out.push_str(&format_file_tree(children, indent + 1));
            }
        } else {
            let file_name = path.rsplit('/').next().unwrap_or(path);
            let lang = node.get("language").and_then(|v| v.as_str()).unwrap_or("");
            let sym_count = node
                .get("symbol_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if !lang.is_empty() {
                writeln!(out, "{prefix}{file_name} [{lang}, {sym_count} symbols]").unwrap();
            } else {
                writeln!(out, "{prefix}{file_name}").unwrap();
            }
        }
    }
    out
}

/// Render a file outline as plain indented text with hierarchy.
///
/// ```text
/// L9   struct MunchServer
///   L128   fn new(storage_path: Option<String>) -> Self
/// ```
pub fn format_symbol_nodes(nodes: &[SymbolNode], depth: usize) -> String {
    let mut out = String::new();
    let indent = "  ".repeat(depth);
    for node in nodes {
        let s = &node.symbol;
        writeln!(out, "{indent}L{}   {}", s.line, s.signature).unwrap();
        if !node.children.is_empty() {
            out.push_str(&format_symbol_nodes(&node.children, depth + 1));
        }
    }
    out
}

/// Format a single symbol as plain text header + source block.
pub fn format_symbol(
    symbol: &serde_json::Value,
    source: &str,
    context_before: &str,
    context_after: &str,
) -> String {
    let mut out = String::new();
    let get = |key: &str| -> &str { symbol.get(key).and_then(|v| v.as_str()).unwrap_or("") };
    let get_num = |key: &str| -> String {
        symbol
            .get(key)
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string())
            .unwrap_or_default()
    };

    writeln!(out, "id: {}", get("id")).unwrap();
    writeln!(out, "kind: {}", get("kind")).unwrap();
    writeln!(out, "name: {}", get("name")).unwrap();
    writeln!(out, "file: {}", get("file")).unwrap();
    writeln!(out, "line: {}", get_num("line")).unwrap();
    writeln!(out, "end_line: {}", get_num("end_line")).unwrap();
    writeln!(out, "signature: {}", get("signature")).unwrap();

    let docstring = get("docstring");
    if !docstring.is_empty() {
        writeln!(out, "docstring: {docstring}").unwrap();
    }

    if !context_before.is_empty() {
        writeln!(out).unwrap();
        writeln!(out, "---context_before---").unwrap();
        writeln!(out, "{context_before}").unwrap();
        writeln!(out, "---/context_before---").unwrap();
    }

    writeln!(out).unwrap();
    writeln!(out, "---source---").unwrap();
    writeln!(out, "{source}").unwrap();
    write!(out, "---/source---").unwrap();

    if !context_after.is_empty() {
        writeln!(out).unwrap();
        writeln!(out, "---context_after---").unwrap();
        writeln!(out, "{context_after}").unwrap();
        write!(out, "---/context_after---").unwrap();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_kv_header() {
        let h = format_kv_header(&[("repo", "local/munchrs"), ("query", "parse")]);
        assert_eq!(h, "repo: local/munchrs | query: parse");
    }

    #[test]
    fn test_quote_toon_no_quoting() {
        assert_eq!(quote_toon("hello", '|'), "hello");
    }

    #[test]
    fn test_quote_toon_with_delimiter() {
        assert_eq!(quote_toon("a|b", '|'), "\"a|b\"");
    }

    #[test]
    fn test_quote_toon_empty() {
        assert_eq!(quote_toon("", '|'), "\"\"");
    }

    #[test]
    fn test_quote_toon_with_newline() {
        assert_eq!(quote_toon("a\nb", ','), "\"a\nb\"");
    }

    #[test]
    fn test_format_toon_table() {
        let cols = &["file", "line", "text"];
        let rows = vec![
            vec!["src/main.rs".into(), "1".into(), "use std::io;".into()],
            vec!["src/lib.rs".into(), "5".into(), "pub mod foo;".into()],
        ];
        let result = format_toon_table(cols, &rows, '|');
        assert!(result.starts_with("[2]{file|line|text}:\n"));
        assert!(result.contains("  src/main.rs|1|use std::io;\n"));
    }

    #[test]
    fn test_format_toon_table_with_pipes_in_values() {
        let cols = &["name", "sig"];
        let rows = vec![vec!["foo".into(), "fn foo(a: i32) -> Result<A, B>".into()]];
        let result = format_toon_table(cols, &rows, '|');
        // No pipes in values, so no quoting needed
        assert!(result.contains("foo|fn foo(a: i32) -> Result<A, B>"));
    }

    #[test]
    fn test_format_file_tree() {
        let tree = serde_json::json!([
            {
                "path": "src/",
                "type": "dir",
                "children": [
                    {"path": "src/main.rs", "type": "file", "language": "rust", "symbol_count": 2}
                ]
            }
        ]);
        let result = format_file_tree(tree.as_array().unwrap(), 0);
        assert!(result.contains("src/\n"));
        assert!(result.contains("  main.rs [rust, 2 symbols]\n"));
    }

    #[test]
    fn test_format_symbol_nodes() {
        use crate::parser::{Symbol, SymbolNode};
        let parent = SymbolNode {
            symbol: Symbol {
                id: "test::Foo#class".into(),
                file: "test.rs".into(),
                name: "Foo".into(),
                qualified_name: "Foo".into(),
                kind: "class".into(),
                language: "rust".into(),
                signature: "struct Foo".into(),
                docstring: String::new(),
                summary: String::new(),
                decorators: vec![],
                keywords: vec![],
                parent: None,
                line: 9,
                end_line: 20,
                byte_offset: 0,
                byte_length: 100,
                content_hash: String::new(),
            },
            children: vec![SymbolNode {
                symbol: Symbol {
                    id: "test::Foo::new#method".into(),
                    file: "test.rs".into(),
                    name: "new".into(),
                    qualified_name: "Foo::new".into(),
                    kind: "method".into(),
                    language: "rust".into(),
                    signature: "fn new() -> Self".into(),
                    docstring: String::new(),
                    summary: String::new(),
                    decorators: vec![],
                    keywords: vec![],
                    parent: Some("test::Foo#class".into()),
                    line: 12,
                    end_line: 14,
                    byte_offset: 0,
                    byte_length: 50,
                    content_hash: String::new(),
                },
                children: vec![],
            }],
        };
        let result = format_symbol_nodes(&[parent], 0);
        assert!(result.contains("L9   struct Foo\n"));
        assert!(result.contains("  L12   fn new() -> Self\n"));
    }
}
