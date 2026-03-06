use crate::{storage::IndexStore, tools::resolve_repo};

pub fn invalidate_cache(repo: &str, storage_path: Option<&str>) -> String {
    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return format!("error: {e}"),
    };

    let store = IndexStore::new(storage_path);
    let deleted = store.delete_index(&owner, &name);

    if deleted {
        format!("Index deleted for {owner}/{name}")
    } else {
        format!("error: No index found for {owner}/{name}")
    }
}
