use std::time::Instant;

use crate::storage::IndexStore;
use crate::tools::resolve_repo;

pub fn get_file_tree(
    repo: &str,
    path_prefix: &str,
    _include_summaries: bool,
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

    let files: Vec<&String> = index
        .source_files
        .iter()
        .filter(|f| f.starts_with(path_prefix))
        .collect();

    if files.is_empty() {
        return serde_json::json!({
            "repo": format!("{owner}/{name}"),
            "path_prefix": path_prefix,
            "tree": [],
        });
    }

    // Build file->language map from symbols
    let mut file_languages: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for sym in &index.symbols {
        let file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let lang = sym.get("language").and_then(|v| v.as_str()).unwrap_or("");
        if !file.is_empty() && !lang.is_empty() && !file_languages.contains_key(file) {
            file_languages.insert(file.to_string(), lang.to_string());
        }
    }

    // Build nested tree
    let mut root: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

    for file_path in &files {
        let rel = file_path
            .strip_prefix(path_prefix)
            .unwrap_or(file_path)
            .trim_start_matches('/');
        let parts: Vec<&str> = rel.split('/').collect();
        let mut current = &mut root;

        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;
            if is_last {
                let symbol_count = index
                    .symbols
                    .iter()
                    .filter(|s| s.get("file").and_then(|v| v.as_str()) == Some(file_path.as_str()))
                    .count();

                let lang = file_languages
                    .get(file_path.as_str())
                    .cloned()
                    .unwrap_or_default();
                let node = serde_json::json!({
                    "path": file_path,
                    "type": "file",
                    "language": lang,
                    "symbol_count": symbol_count,
                });
                current.insert(part.to_string(), node);
            } else {
                if !current.contains_key(*part) {
                    current.insert(
                        part.to_string(),
                        serde_json::json!({"type": "dir", "children": {}}),
                    );
                }
                let entry = current.get_mut(*part).unwrap();
                current = entry
                    .get_mut("children")
                    .and_then(|v| v.as_object_mut())
                    .unwrap();
            }
        }
    }

    let tree = dict_to_list(&root);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    serde_json::json!({
        "repo": format!("{owner}/{name}"),
        "path_prefix": path_prefix,
        "tree": tree,
        "_meta": {
            "timing_ms": (elapsed * 10.0).round() / 10.0,
            "file_count": files.len(),
        },
    })
}

fn dict_to_list(node_dict: &serde_json::Map<String, serde_json::Value>) -> Vec<serde_json::Value> {
    let mut result = Vec::new();
    let mut keys: Vec<&String> = node_dict.keys().collect();
    keys.sort();

    for key in keys {
        let node = &node_dict[key];
        if node.get("type").and_then(|v| v.as_str()) == Some("file") {
            result.push(node.clone());
        } else {
            let children = node
                .get("children")
                .and_then(|v| v.as_object())
                .map(dict_to_list)
                .unwrap_or_default();
            result.push(serde_json::json!({
                "path": format!("{key}/"),
                "type": "dir",
                "children": children,
            }));
        }
    }
    result
}
