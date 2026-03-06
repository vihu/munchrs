use crate::storage::IndexStore;
use std::time::Instant;

pub fn list_repos(storage_path: Option<&str>) -> serde_json::Value {
    let start = Instant::now();
    let store = IndexStore::new(storage_path);
    let repos = store.list_repos();
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    serde_json::json!({
        "count": repos.len(),
        "repos": repos,
        "_meta": {
            "timing_ms": (elapsed * 10.0).round() / 10.0,
        },
    })
}
