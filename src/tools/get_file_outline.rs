use crate::{
    format::{format_kv_header, format_symbol_nodes},
    parser::{Symbol, build_symbol_tree},
    storage::IndexStore,
    tools::resolve_repo,
};

pub fn get_file_outline(repo: &str, file_path: &str, storage_path: Option<&str>) -> String {
    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return format!("error: {e}"),
    };

    let store = IndexStore::new(storage_path);
    let index = match store.load_index(&owner, &name) {
        Some(i) => i,
        None => return format!("error: Repository not indexed: {owner}/{name}"),
    };

    let file_symbols: Vec<&serde_json::Value> = index
        .symbols
        .iter()
        .filter(|s| s.get("file").and_then(|v| v.as_str()) == Some(file_path))
        .collect();

    if file_symbols.is_empty() {
        return format!(
            "{}\n\n(no symbols)",
            format_kv_header(&[("repo", &format!("{owner}/{name}")), ("file", file_path),])
        );
    }

    let language = file_symbols[0]
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let symbol_objects: Vec<Symbol> = file_symbols
        .iter()
        .filter_map(|s| dict_to_symbol(s))
        .collect();
    let tree = build_symbol_tree(&symbol_objects);

    let header = format_kv_header(&[
        ("repo", &format!("{owner}/{name}")),
        ("file", file_path),
        ("language", language),
    ]);
    format!("{header}\n\n{}", format_symbol_nodes(&tree, 0))
}

fn dict_to_symbol(d: &serde_json::Value) -> Option<Symbol> {
    Some(Symbol {
        id: d.get("id")?.as_str()?.to_string(),
        file: d.get("file")?.as_str()?.to_string(),
        name: d.get("name")?.as_str()?.to_string(),
        qualified_name: d.get("qualified_name")?.as_str()?.to_string(),
        kind: d.get("kind")?.as_str()?.to_string(),
        language: d.get("language")?.as_str()?.to_string(),
        signature: d.get("signature")?.as_str()?.to_string(),
        docstring: d
            .get("docstring")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        summary: d
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        decorators: d
            .get("decorators")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        keywords: d
            .get("keywords")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        parent: d.get("parent").and_then(|v| v.as_str()).map(String::from),
        line: d.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        end_line: d.get("end_line").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        byte_offset: d.get("byte_offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        byte_length: d.get("byte_length").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        content_hash: d
            .get("content_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}
