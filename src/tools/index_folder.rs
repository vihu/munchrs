use crate::{
    parser::{LANGUAGE_EXTENSIONS, parse_file},
    security::{
        DEFAULT_MAX_FILE_SIZE, get_max_index_files, is_binary_file, is_secret_file,
        is_symlink_escape, validate_path,
    },
    storage::IndexStore,
    storage::index_store::{file_hash, get_git_head},
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// File patterns to skip.
const SKIP_PATTERNS: &[&str] = &[
    "node_modules/",
    "vendor/",
    "venv/",
    ".venv/",
    "__pycache__/",
    "dist/",
    "build/",
    ".git/",
    ".tox/",
    ".mypy_cache/",
    "target/",
    ".gradle/",
    "test_data/",
    "testdata/",
    "fixtures/",
    "snapshots/",
    "migrations/",
    ".min.js",
    ".min.ts",
    ".bundle.js",
    "package-lock.json",
    "yarn.lock",
    "go.sum",
    "generated/",
    "proto/",
];

fn should_skip_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    SKIP_PATTERNS.iter().any(|p| normalized.contains(p))
}

pub fn discover_local_files(
    folder_path: &Path,
    max_files: Option<usize>,
    max_size: u64,
    extra_ignore_patterns: Option<&[String]>,
    follow_symlinks: bool,
) -> (Vec<PathBuf>, Vec<String>, HashMap<String, usize>) {
    let max_files = get_max_index_files(max_files);
    let mut files = Vec::new();
    let warnings = Vec::new();
    let root = match folder_path.canonicalize() {
        Ok(r) => r,
        Err(_) => folder_path.to_path_buf(),
    };

    let mut skip_counts: HashMap<String, usize> = HashMap::new();
    for key in &[
        "symlink",
        "symlink_escape",
        "path_traversal",
        "skip_pattern",
        "extra_ignore",
        "secret",
        "wrong_extension",
        "too_large",
        "unreadable",
        "binary",
        "file_limit",
    ] {
        skip_counts.insert(key.to_string(), 0);
    }

    // Build extra ignore matcher
    let extra_gitignore = extra_ignore_patterns.and_then(|patterns| {
        let mut builder = ignore::gitignore::GitignoreBuilder::new(&root);
        for p in patterns {
            builder.add_line(None, p).ok();
        }
        builder.build().ok()
    });

    let walker = ignore::WalkBuilder::new(&root)
        .follow_links(follow_symlinks)
        .hidden(false)
        .ignore(false)
        .require_git(false)
        .build()
        .filter_map(|e| e.ok());

    for entry in walker {
        let file_path = entry.path().to_path_buf();
        if !file_path.is_file() {
            continue;
        }

        // Symlink protection
        if !follow_symlinks
            && file_path
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
        {
            *skip_counts.get_mut("symlink").unwrap() += 1;
            continue;
        }
        if file_path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
            && is_symlink_escape(&root, &file_path)
        {
            *skip_counts.get_mut("symlink_escape").unwrap() += 1;
            continue;
        }

        // Path traversal check
        if !validate_path(&root, &file_path) {
            *skip_counts.get_mut("path_traversal").unwrap() += 1;
            continue;
        }

        let rel_path = match file_path.strip_prefix(&root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => {
                *skip_counts.get_mut("path_traversal").unwrap() += 1;
                continue;
            }
        };

        if should_skip_file(&rel_path) {
            *skip_counts.get_mut("skip_pattern").unwrap() += 1;
            continue;
        }

        if let Some(ref egi) = extra_gitignore
            && egi
                .matched_path_or_any_parents(&file_path, false)
                .is_ignore()
        {
            *skip_counts.get_mut("extra_ignore").unwrap() += 1;
            continue;
        }

        if is_secret_file(&rel_path) {
            *skip_counts.get_mut("secret").unwrap() += 1;
            continue;
        }

        let ext = file_path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        if !LANGUAGE_EXTENSIONS.contains_key(ext.as_str()) {
            *skip_counts.get_mut("wrong_extension").unwrap() += 1;
            continue;
        }

        match fs::metadata(&file_path) {
            Ok(meta) if meta.len() > max_size => {
                *skip_counts.get_mut("too_large").unwrap() += 1;
                continue;
            }
            Err(_) => {
                *skip_counts.get_mut("unreadable").unwrap() += 1;
                continue;
            }
            _ => {}
        }

        if is_binary_file(&file_path) {
            *skip_counts.get_mut("binary").unwrap() += 1;
            continue;
        }

        files.push(file_path);
    }

    // File count limit with prioritization
    if files.len() > max_files {
        *skip_counts.get_mut("file_limit").unwrap() = files.len() - max_files;
        let priority_dirs = ["src/", "lib/", "pkg/", "cmd/", "internal/"];

        files.sort_by(|a, b| {
            let rel_a = a
                .strip_prefix(&root)
                .unwrap_or(a)
                .to_string_lossy()
                .replace('\\', "/");
            let rel_b = b
                .strip_prefix(&root)
                .unwrap_or(b)
                .to_string_lossy()
                .replace('\\', "/");

            let pri_a = priority_dirs
                .iter()
                .position(|p| rel_a.starts_with(p))
                .unwrap_or(priority_dirs.len());
            let pri_b = priority_dirs
                .iter()
                .position(|p| rel_b.starts_with(p))
                .unwrap_or(priority_dirs.len());

            pri_a
                .cmp(&pri_b)
                .then_with(|| rel_a.matches('/').count().cmp(&rel_b.matches('/').count()))
                .then_with(|| rel_a.cmp(&rel_b))
        });
        files.truncate(max_files);
    }

    (files, warnings, skip_counts)
}

pub fn index_folder(
    path: &str,
    _use_ai_summaries: bool,
    storage_path: Option<&str>,
    extra_ignore_patterns: Option<&[String]>,
    follow_symlinks: bool,
    incremental: bool,
) -> serde_json::Value {
    let folder_path = match shellexpand::tilde(path) {
        std::borrow::Cow::Borrowed(p) => PathBuf::from(p),
        std::borrow::Cow::Owned(p) => PathBuf::from(p),
    };
    let folder_path = match folder_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return serde_json::json!({
                "success": false,
                "error": format!("Folder not found: {path}")
            });
        }
    };

    if !folder_path.is_dir() {
        return serde_json::json!({
            "success": false,
            "error": format!("Path is not a directory: {path}")
        });
    }

    let max_files = get_max_index_files(None);

    let (source_files, warnings, skip_counts) = discover_local_files(
        &folder_path,
        Some(max_files),
        DEFAULT_MAX_FILE_SIZE,
        extra_ignore_patterns,
        follow_symlinks,
    );

    if source_files.is_empty() {
        return serde_json::json!({
            "success": false,
            "error": "No source files found"
        });
    }

    let repo_name = folder_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let owner = "local";
    let store = IndexStore::new(storage_path);

    // Read all files
    let mut current_files: HashMap<String, String> = HashMap::new();
    for file_path in &source_files {
        if !validate_path(&folder_path, file_path) {
            continue;
        }
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rel_path = match file_path.strip_prefix(&folder_path) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        let ext = file_path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        if !LANGUAGE_EXTENSIONS.contains_key(ext.as_str()) {
            continue;
        }
        current_files.insert(rel_path, content);
    }

    // Incremental path
    if incremental && store.load_index(owner, &repo_name).is_some() {
        let (changed, new, deleted) = store.detect_changes(owner, &repo_name, &current_files);

        if changed.is_empty() && new.is_empty() && deleted.is_empty() {
            return serde_json::json!({
                "success": true,
                "message": "No changes detected",
                "repo": format!("{owner}/{repo_name}"),
                "folder_path": folder_path.to_string_lossy(),
                "changed": 0, "new": 0, "deleted": 0,
            });
        }

        let files_to_parse: std::collections::HashSet<&String> =
            changed.iter().chain(new.iter()).collect();
        let mut new_symbols = Vec::new();
        let mut file_hashes_subset: HashMap<String, String> = HashMap::new();

        for rel_path in &files_to_parse {
            if let Some(content) = current_files.get(*rel_path) {
                file_hashes_subset.insert((*rel_path).clone(), file_hash(content));
                let ext_pos = rel_path.rfind('.');
                let ext = ext_pos.map(|p| &rel_path[p..]).unwrap_or("");
                if let Some(&language) = LANGUAGE_EXTENSIONS.get(ext) {
                    let symbols = parse_file(content, rel_path, language);
                    new_symbols.extend(symbols);
                }
            }
        }

        // Summarize synchronously (no AI for now in blocking context)
        crate::summarizer::summarize_symbols_simple(&mut new_symbols);

        let git_head = get_git_head(&folder_path).unwrap_or_default();

        let updated = store.incremental_save(
            owner,
            &repo_name,
            &changed,
            &new,
            &deleted,
            &new_symbols,
            &file_hashes_subset,
            &HashMap::new(),
            &git_head,
        );

        return serde_json::json!({
            "success": true,
            "repo": format!("{owner}/{repo_name}"),
            "folder_path": folder_path.to_string_lossy(),
            "incremental": true,
            "changed": changed.len(),
            "new": new.len(),
            "deleted": deleted.len(),
            "symbol_count": updated.map(|u| u.symbols.len()).unwrap_or(0),
            "discovery_skip_counts": skip_counts,
        });
    }

    // Full index path
    let mut all_symbols = Vec::new();
    let mut languages: HashMap<String, usize> = HashMap::new();
    let mut parsed_files = Vec::new();
    let mut no_symbols_files = Vec::new();

    for (rel_path, content) in &current_files {
        let ext_pos = rel_path.rfind('.');
        let ext = ext_pos.map(|p| &rel_path[p..]).unwrap_or("");
        let language = match LANGUAGE_EXTENSIONS.get(ext) {
            Some(&l) => l,
            None => continue,
        };

        let symbols = parse_file(content, rel_path, language);
        if !symbols.is_empty() {
            let file_language = symbols[0].language.clone();
            *languages.entry(file_language).or_insert(0) += 1;
            parsed_files.push(rel_path.clone());
            all_symbols.extend(symbols);
        } else {
            no_symbols_files.push(rel_path.clone());
        }
    }

    if all_symbols.is_empty() {
        return serde_json::json!({
            "success": false,
            "error": "No symbols extracted from files"
        });
    }

    // Summarize
    crate::summarizer::summarize_symbols_simple(&mut all_symbols);

    // Compute file hashes for all discovered files
    let file_hashes: HashMap<String, String> = current_files
        .iter()
        .map(|(fp, content)| (fp.clone(), file_hash(content)))
        .collect();

    let result = store.save_index(
        owner,
        &repo_name,
        &folder_path.to_string_lossy(),
        &parsed_files,
        &all_symbols,
        &languages,
        &file_hashes,
        &get_git_head(&folder_path).unwrap_or_default(),
    );

    match result {
        Ok(index) => {
            let mut response = serde_json::json!({
                "success": true,
                "repo": format!("{owner}/{repo_name}"),
                "folder_path": folder_path.to_string_lossy(),
                "indexed_at": index.indexed_at,
                "file_count": parsed_files.len(),
                "symbol_count": all_symbols.len(),
                "languages": languages,
                "files": &parsed_files[..parsed_files.len().min(20)],
                "discovery_skip_counts": skip_counts,
                "no_symbols_count": no_symbols_files.len(),
                "no_symbols_files": &no_symbols_files[..no_symbols_files.len().min(50)],
            });

            if !warnings.is_empty() {
                response["warnings"] = serde_json::json!(warnings);
            }
            if skip_counts.get("file_limit").copied().unwrap_or(0) > 0 {
                response["note"] =
                    serde_json::json!(format!("Folder has many files; indexed first {max_files}"));
            }

            response
        }
        Err(e) => serde_json::json!({
            "success": false,
            "error": format!("Indexing failed: {e}")
        }),
    }
}
