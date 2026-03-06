pub mod get_file_outline;
pub mod get_file_tree;
pub mod get_repo_outline;
pub mod get_symbol;
pub mod index_folder;
pub mod invalidate_cache;
pub mod list_repos;
pub mod search_symbols;
pub mod search_text;

use crate::storage::IndexStore;

/// Parse "owner/repo" or look up single name. Returns (owner, name).
pub fn resolve_repo(
    repo: &str,
    storage_path: Option<&str>,
) -> std::result::Result<(String, String), String> {
    if let Some((owner, name)) = repo.split_once('/') {
        return Ok((owner.to_string(), name.to_string()));
    }
    let store = IndexStore::new(storage_path);
    let repos = store.list_repos();
    for r in &repos {
        if let Some(repo_name) = r.get("repo").and_then(|v| v.as_str())
            && repo_name.ends_with(&format!("/{repo}"))
            && let Some((owner, name)) = repo_name.split_once('/')
        {
            return Ok((owner.to_string(), name.to_string()));
        }
    }
    Err(format!("Repository not found: {repo}"))
}
