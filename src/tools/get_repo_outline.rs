use std::collections::HashMap;
use std::time::Instant;

use crate::storage::IndexStore;
use crate::tools::resolve_repo;

pub fn get_repo_outline(repo: &str, storage_path: Option<&str>) -> serde_json::Value {
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

    let mut dir_file_counts: HashMap<String, usize> = HashMap::new();
    for f in &index.source_files {
        let dir = if let Some(pos) = f.find('/') {
            format!("{}/", &f[..pos])
        } else {
            "(root)".to_string()
        };
        *dir_file_counts.entry(dir).or_insert(0) += 1;
    }

    let mut kind_counts: HashMap<String, usize> = HashMap::new();
    for sym in &index.symbols {
        let kind = sym
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        *kind_counts.entry(kind.to_string()).or_insert(0) += 1;
    }

    // Sort by count descending
    let mut dirs: Vec<(String, usize)> = dir_file_counts.into_iter().collect();
    dirs.sort_by(|a, b| b.1.cmp(&a.1));
    let directories: serde_json::Map<String, serde_json::Value> = dirs
        .into_iter()
        .map(|(k, v)| (k, serde_json::json!(v)))
        .collect();

    let mut kinds: Vec<(String, usize)> = kind_counts.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    let symbol_kinds: serde_json::Map<String, serde_json::Value> = kinds
        .into_iter()
        .map(|(k, v)| (k, serde_json::json!(v)))
        .collect();

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    serde_json::json!({
        "repo": format!("{owner}/{name}"),
        "indexed_at": index.indexed_at,
        "file_count": index.source_files.len(),
        "symbol_count": index.symbols.len(),
        "languages": index.languages,
        "directories": directories,
        "symbol_kinds": symbol_kinds,
        "_meta": {
            "timing_ms": (elapsed * 10.0).round() / 10.0,
        },
    })
}
