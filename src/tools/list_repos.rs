use crate::{format::format_toon_table, storage::IndexStore};

pub fn list_repos(storage_path: Option<&str>) -> String {
    let store = IndexStore::new(storage_path);
    let repos = store.list_repos();

    if repos.is_empty() {
        return "No indexed repositories found.".to_string();
    }

    let rows: Vec<Vec<String>> = repos
        .iter()
        .map(|r| {
            let get = |key: &str| -> String {
                r.get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let get_num = |key: &str| -> String {
                r.get(key).and_then(|v| v.as_u64()).unwrap_or(0).to_string()
            };
            let languages = r
                .get("languages")
                .and_then(|v| v.as_object())
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| format!("{k}={}", v.as_u64().unwrap_or(0)))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            vec![
                get("repo"),
                get("folder_path"),
                get_num("file_count"),
                get_num("symbol_count"),
                languages,
                get("indexed_at"),
            ]
        })
        .collect();

    format!(
        "{} indexed repositories\n\n{}",
        rows.len(),
        format_toon_table(
            &[
                "repo",
                "folder_path",
                "files",
                "symbols",
                "languages",
                "indexed_at"
            ],
            &rows,
            ','
        )
    )
}
