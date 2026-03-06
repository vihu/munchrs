use crate::{
    format::{format_kv_header, format_toon_table},
    storage::IndexStore,
    tools::resolve_repo,
};

pub fn search_symbols(
    repo: &str,
    query: &str,
    kind: Option<&str>,
    file_pattern: Option<&str>,
    language: Option<&str>,
    max_results: usize,
    storage_path: Option<&str>,
) -> String {
    let max_results = max_results.clamp(1, 100);

    let (owner, name) = match resolve_repo(repo, storage_path) {
        Ok(r) => r,
        Err(e) => return format!("error: {e}"),
    };

    let store = IndexStore::new(storage_path);
    let index = match store.load_index(&owner, &name) {
        Some(i) => i,
        None => return format!("error: Repository not indexed: {owner}/{name}"),
    };

    let mut results = index.search(query, kind, file_pattern);

    if let Some(lang) = language {
        results.retain(|s| s.get("language").and_then(|v| v.as_str()) == Some(lang));
    }

    let query_lower = query.to_lowercase();
    let query_words: std::collections::HashSet<String> =
        query_lower.split_whitespace().map(String::from).collect();

    let truncated = results.len() > max_results;
    let total = results.len();

    let rows: Vec<Vec<String>> = results
        .into_iter()
        .take(max_results)
        .map(|sym| {
            let score = calculate_score(sym, &query_lower, &query_words);
            vec![
                sym.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                sym.get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                sym.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                sym.get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                sym.get("line")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    .to_string(),
                sym.get("signature")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                score.to_string(),
            ]
        })
        .collect();

    let header = format_kv_header(&[
        ("repo", &format!("{owner}/{name}")),
        ("query", query),
        ("results", &rows.len().to_string()),
    ]);
    let mut out = if truncated {
        format!("{header} (truncated from {total})\n\n")
    } else {
        format!("{header}\n\n")
    };
    out.push_str(&format_toon_table(
        &["id", "kind", "name", "file", "line", "signature", "score"],
        &rows,
        '|',
    ));
    out
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
