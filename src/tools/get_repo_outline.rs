use crate::{storage::IndexStore, tools::resolve_repo};
use std::collections::HashMap;

pub fn get_repo_outline(repo: &str, storage_path: Option<&str>) -> String {
    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return format!("error: {e}"),
    };

    let store = IndexStore::new(storage_path);
    let index = match store.load_index(&owner, &name) {
        Some(i) => i,
        None => return format!("error: Repository not indexed: {owner}/{name}"),
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

    let mut dirs: Vec<(String, usize)> = dir_file_counts.into_iter().collect();
    dirs.sort_by(|a, b| b.1.cmp(&a.1));
    let dirs_str = dirs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");

    let mut kinds: Vec<(String, usize)> = kind_counts.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    let kinds_str = kinds
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");

    let languages_str = index
        .languages
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "repo: {owner}/{name} | indexed: {}\nfiles: {} | symbols: {}\nlanguages: {languages_str}\nkinds: {kinds_str}\ndirectories: {dirs_str}",
        index.indexed_at,
        index.source_files.len(),
        index.symbols.len(),
    )
}
