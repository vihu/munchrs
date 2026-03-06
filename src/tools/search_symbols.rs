use crate::{storage::IndexStore, tools::resolve_repo};
use std::time::Instant;

pub fn search_symbols(
    repo: &str,
    query: &str,
    kind: Option<&str>,
    file_pattern: Option<&str>,
    language: Option<&str>,
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

    let mut results = index.search(query, kind, file_pattern);

    if let Some(lang) = language {
        results.retain(|s| s.get("language").and_then(|v| v.as_str()) == Some(lang));
    }

    let query_lower = query.to_lowercase();
    let query_words: std::collections::HashSet<String> =
        query_lower.split_whitespace().map(String::from).collect();

    let truncated = results.len() > max_results;
    let scored_results: Vec<serde_json::Value> = results
        .into_iter()
        .take(max_results)
        .map(|sym| {
            let score = calculate_score(sym, &query_lower, &query_words);
            serde_json::json!({
                "id": sym.get("id"),
                "kind": sym.get("kind"),
                "name": sym.get("name"),
                "file": sym.get("file"),
                "line": sym.get("line"),
                "signature": sym.get("signature"),
                "summary": sym.get("summary"),
                "score": score,
            })
        })
        .collect();

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    serde_json::json!({
        "repo": format!("{owner}/{name}"),
        "query": query,
        "result_count": scored_results.len(),
        "results": scored_results,
        "_meta": {
            "timing_ms": (elapsed * 10.0).round() / 10.0,
            "total_symbols": index.symbols.len(),
            "truncated": truncated,
        },
    })
}

fn calculate_score(
    sym: &serde_json::Value,
    query_lower: &str,
    query_words: &std::collections::HashSet<String>,
) -> i32 {
    let mut score = 0i32;

    let name_lower = sym
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    if query_lower == name_lower {
        score += 20;
    } else if name_lower.contains(query_lower) {
        score += 10;
    }
    for word in query_words {
        if name_lower.contains(word.as_str()) {
            score += 5;
        }
    }

    let sig_lower = sym
        .get("signature")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    if sig_lower.contains(query_lower) {
        score += 8;
    }
    for word in query_words {
        if sig_lower.contains(word.as_str()) {
            score += 2;
        }
    }

    let summary_lower = sym
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    if summary_lower.contains(query_lower) {
        score += 5;
    }
    for word in query_words {
        if summary_lower.contains(word.as_str()) {
            score += 1;
        }
    }

    if let Some(keywords) = sym.get("keywords").and_then(|v| v.as_array()) {
        for kw in keywords {
            if let Some(kw_str) = kw.as_str()
                && query_words.contains(kw_str)
            {
                score += 3;
            }
        }
    }

    let doc_lower = sym
        .get("docstring")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    for word in query_words {
        if doc_lower.contains(word.as_str()) {
            score += 1;
        }
    }

    score
}
