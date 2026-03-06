use crate::{storage::IndexStore, tools::resolve_repo};
use std::time::Instant;

pub fn search_text(
    repo: &str,
    query: &str,
    file_pattern: Option<&str>,
    max_results: usize,
    storage_path: Option<&str>,
) -> serde_json::Value {
    let start = Instant::now();
    let max_results = max_results.clamp(1, 100);

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

    let files: Vec<&String> = if let Some(fp) = file_pattern {
        index
            .source_files
            .iter()
            .filter(|f| {
                glob::Pattern::new(fp)
                    .map(|p| p.matches(f))
                    .unwrap_or(false)
                    || glob::Pattern::new(&format!("*/{fp}"))
                        .map(|p| p.matches(f))
                        .unwrap_or(false)
            })
            .collect()
    } else {
        index.source_files.iter().collect()
    };

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();
    let mut files_searched = 0;

    for file_path in &files {
        let full_path = match index.original_file_path(file_path) {
            Some(p) => p,
            None => continue,
        };
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        files_searched += 1;
        for (line_num, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                let text: String = line.trim_end().chars().take(200).collect();
                matches.push(serde_json::json!({
                    "file": file_path,
                    "line": line_num + 1,
                    "text": text,
                }));
                if matches.len() >= max_results {
                    break;
                }
            }
        }
        if matches.len() >= max_results {
            break;
        }
    }

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    serde_json::json!({
        "repo": format!("{owner}/{name}"),
        "query": query,
        "result_count": matches.len(),
        "results": matches,
        "_meta": {
            "timing_ms": (elapsed * 10.0).round() / 10.0,
            "files_searched": files_searched,
            "truncated": matches.len() >= max_results,
        },
    })
}
