use crate::{format::format_symbol, storage::IndexStore, tools::resolve_repo};
use sha2::{Digest, Sha256};

pub fn get_symbol(
    repo: &str,
    symbol_id: &str,
    verify: bool,
    context_lines: usize,
    storage_path: Option<&str>,
) -> String {
    let context_lines = context_lines.min(50);

    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return format!("error: {e}"),
    };

    let store = IndexStore::new(storage_path);
    let index = match store.load_index(&owner, &name) {
        Some(i) => i,
        None => return format!("error: Repository not indexed: {owner}/{name}"),
    };

    let symbol = match index.get_symbol(symbol_id) {
        Some(s) => s,
        None => return format!("error: Symbol not found: {symbol_id}"),
    };

    let source = store
        .get_symbol_content(&owner, &name, symbol_id)
        .unwrap_or_default();

    let mut context_before = String::new();
    let mut context_after = String::new();
    if context_lines > 0 && !source.is_empty() {
        let file = symbol.get("file").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(file_path) = index.original_file_path(file)
            && let Ok(all_text) = std::fs::read_to_string(&file_path)
        {
            let all_lines: Vec<&str> = all_text.split('\n').collect();
            let start_line = symbol.get("line").and_then(|v| v.as_u64()).unwrap_or(1) as usize - 1;
            let end_line = symbol.get("end_line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let before_start = start_line.saturating_sub(context_lines);
            let after_end = (end_line + context_lines).min(all_lines.len());
            if before_start < start_line {
                context_before = all_lines[before_start..start_line].join("\n");
            }
            if end_line < after_end {
                context_after = all_lines[end_line..after_end].join("\n");
            }
        }
    }

    let mut out = String::new();

    if verify && !source.is_empty() {
        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        let actual_hash = format!("{:x}", hasher.finalize());
        let stored_hash = symbol
            .get("content_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !stored_hash.is_empty() && actual_hash != stored_hash {
            out.push_str(
                "WARNING: content hash mismatch (source may have changed since indexing)\n\n",
            );
        }
    }

    out.push_str(&format_symbol(
        symbol,
        &source,
        &context_before,
        &context_after,
    ));
    out
}

pub fn get_symbols(repo: &str, symbol_ids: &[String], storage_path: Option<&str>) -> String {
    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return format!("error: {e}"),
    };

    let store = IndexStore::new(storage_path);
    let index = match store.load_index(&owner, &name) {
        Some(i) => i,
        None => return format!("error: Repository not indexed: {owner}/{name}"),
    };

    let mut sections = Vec::new();

    for symbol_id in symbol_ids {
        match index.get_symbol(symbol_id) {
            Some(symbol) => {
                let source = store
                    .get_symbol_content(&owner, &name, symbol_id)
                    .unwrap_or_default();
                sections.push(format_symbol(symbol, &source, "", ""));
            }
            None => {
                sections.push(format!("error: Symbol not found: {symbol_id}"));
            }
        }
    }

    format!(
        "{} symbols\n\n{}",
        sections.len(),
        sections.join("\n\n---\n\n")
    )
}
