use crate::{
    format::{format_kv_header, format_toon_table},
    storage::IndexStore,
    tools::resolve_repo,
};

pub fn search_text(
    repo: &str,
    query: &str,
    file_pattern: Option<&str>,
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
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut truncated = false;

    for file_path in &files {
        let full_path = match index.original_file_path(file_path) {
            Some(p) => p,
            None => continue,
        };
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (line_num, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                let text: String = line.trim_end().chars().take(200).collect();
                rows.push(vec![
                    file_path.to_string(),
                    (line_num + 1).to_string(),
                    text,
                ]);
                if rows.len() >= max_results {
                    truncated = true;
                    break;
                }
            }
        }
        if rows.len() >= max_results {
            break;
        }
    }

    let header = format_kv_header(&[
        ("repo", &format!("{owner}/{name}")),
        ("query", query),
        ("results", &rows.len().to_string()),
    ]);
    let mut out = if truncated {
        format!("{header} (truncated)\n\n")
    } else {
        format!("{header}\n\n")
    };
    out.push_str(&format_toon_table(&["file", "line", "text"], &rows, '|'));
    out
}
