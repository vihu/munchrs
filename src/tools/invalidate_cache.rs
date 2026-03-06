use crate::storage::IndexStore;
use crate::tools::resolve_repo;

pub fn invalidate_cache(repo: &str, storage_path: Option<&str>) -> serde_json::Value {
    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return serde_json::json!({"error": e}),
    };

    let store = IndexStore::new(storage_path);
    let deleted = store.delete_index(&owner, &name);

    if deleted {
        serde_json::json!({
            "success": true,
            "repo": format!("{owner}/{name}"),
            "message": format!("Index and cached files deleted for {owner}/{name}"),
        })
    } else {
        serde_json::json!({
            "success": false,
            "error": format!("No index found for {owner}/{name}"),
        })
    }
}
