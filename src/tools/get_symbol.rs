use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::storage::IndexStore;
use crate::tools::resolve_repo;

pub fn get_symbol(
    repo: &str,
    symbol_id: &str,
    verify: bool,
    context_lines: usize,
    storage_path: Option<&str>,
) -> serde_json::Value {
    let start = Instant::now();
    let context_lines = context_lines.min(50);

    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return serde_json::json!({"error": e}),
    };

    let store = IndexStore::new(storage_path);
    let index = match store.load_index(&owner, &name) {
        Some(i) => i,
        None => {
            return serde_json::json!({"error": format!("Repository not indexed: {owner}/{name}")});
        }
    };

    let symbol = match index.get_symbol(symbol_id) {
        Some(s) => s,
        None => return serde_json::json!({"error": format!("Symbol not found: {symbol_id}")}),
    };

    let source = store
        .get_symbol_content(&owner, &name, symbol_id)
        .unwrap_or_default();

    let mut context_before = String::new();
    let mut context_after = String::new();
    if context_lines > 0
        && !source.is_empty()
        && let Ok(content_dir) = store.content_dir(&owner, &name)
    {
        let file = symbol.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let file_path = content_dir.join(file);
        if let Ok(all_text) = std::fs::read_to_string(&file_path) {
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

    let mut meta = serde_json::Map::new();
    if verify && !source.is_empty() {
        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        let actual_hash = format!("{:x}", hasher.finalize());
        let stored_hash = symbol
            .get("content_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !stored_hash.is_empty() {
            meta.insert(
                "content_verified".to_string(),
                serde_json::json!(actual_hash == stored_hash),
            );
        }
    }

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    meta.insert(
        "timing_ms".to_string(),
        serde_json::json!((elapsed * 10.0).round() / 10.0),
    );

    let mut result = serde_json::json!({
        "id": symbol.get("id"),
        "kind": symbol.get("kind"),
        "name": symbol.get("name"),
        "file": symbol.get("file"),
        "line": symbol.get("line"),
        "end_line": symbol.get("end_line"),
        "signature": symbol.get("signature"),
        "decorators": symbol.get("decorators"),
        "docstring": symbol.get("docstring"),
        "content_hash": symbol.get("content_hash"),
        "source": source,
        "_meta": meta,
    });

    if !context_before.is_empty() {
        result["context_before"] = serde_json::json!(context_before);
    }
    if !context_after.is_empty() {
        result["context_after"] = serde_json::json!(context_after);
    }

    result
}

pub fn get_symbols(
    repo: &str,
    symbol_ids: &[String],
    storage_path: Option<&str>,
) -> serde_json::Value {
    let start = Instant::now();

    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return serde_json::json!({"error": e}),
    };

    let store = IndexStore::new(storage_path);
    let index = match store.load_index(&owner, &name) {
        Some(i) => i,
        None => {
            return serde_json::json!({"error": format!("Repository not indexed: {owner}/{name}")});
        }
    };

    let mut symbols = Vec::new();
    let mut errors = Vec::new();

    for symbol_id in symbol_ids {
        match index.get_symbol(symbol_id) {
            Some(symbol) => {
                let source = store
                    .get_symbol_content(&owner, &name, symbol_id)
                    .unwrap_or_default();

                symbols.push(serde_json::json!({
                    "id": symbol.get("id"),
                    "kind": symbol.get("kind"),
                    "name": symbol.get("name"),
                    "file": symbol.get("file"),
                    "line": symbol.get("line"),
                    "end_line": symbol.get("end_line"),
                    "signature": symbol.get("signature"),
                    "decorators": symbol.get("decorators"),
                    "docstring": symbol.get("docstring"),
                    "content_hash": symbol.get("content_hash"),
                    "source": source,
                }));
            }
            None => {
                errors.push(serde_json::json!({
                    "id": symbol_id,
                    "error": format!("Symbol not found: {symbol_id}"),
                }));
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    serde_json::json!({
        "symbols": symbols,
        "errors": errors,
        "_meta": {
            "timing_ms": (elapsed * 10.0).round() / 10.0,
            "symbol_count": symbols.len(),
        },
    })
}
