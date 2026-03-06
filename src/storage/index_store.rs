use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parser::Symbol;

/// Bump this when the index schema changes in an incompatible way.
pub const INDEX_VERSION: u32 = 2;

/// SHA-256 hash of file content string.
pub fn file_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Get current HEAD commit hash for a git repo, or None.
pub fn get_git_head(repo_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Index for a repository's source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeIndex {
    pub repo: String,
    pub owner: String,
    pub name: String,
    pub indexed_at: String,
    pub source_files: Vec<String>,
    pub languages: HashMap<String, usize>,
    pub symbols: Vec<serde_json::Value>,
    #[serde(default)]
    pub index_version: u32,
    #[serde(default)]
    pub file_hashes: HashMap<String, String>,
    #[serde(default)]
    pub git_head: String,
}

impl CodeIndex {
    /// Find a symbol by ID.
    pub fn get_symbol(&self, symbol_id: &str) -> Option<&serde_json::Value> {
        self.symbols
            .iter()
            .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(symbol_id))
    }

    /// Search symbols with weighted scoring.
    pub fn search(
        &self,
        query: &str,
        kind: Option<&str>,
        file_pattern: Option<&str>,
    ) -> Vec<&serde_json::Value> {
        let query_lower = query.to_lowercase();
        let query_words: std::collections::HashSet<String> =
            query_lower.split_whitespace().map(String::from).collect();

        let mut scored: Vec<(i32, &serde_json::Value)> = Vec::new();

        for sym in &self.symbols {
            if let Some(k) = kind
                && sym.get("kind").and_then(|v| v.as_str()) != Some(k)
            {
                continue;
            }
            if let Some(fp) = file_pattern {
                let file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
                if !match_pattern(file, fp) {
                    continue;
                }
            }

            let score = score_symbol(sym, &query_lower, &query_words);
            if score > 0 {
                scored.push((score, sym));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, sym)| sym).collect()
    }
}

fn match_pattern(file_path: &str, pattern: &str) -> bool {
    glob::Pattern::new(pattern)
        .map(|p| p.matches(file_path))
        .unwrap_or(false)
        || glob::Pattern::new(&format!("*/{pattern}"))
            .map(|p| p.matches(file_path))
            .unwrap_or(false)
}

fn score_symbol(
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

/// Storage for code indexes with byte-offset content retrieval.
pub struct IndexStore {
    pub base_path: PathBuf,
}

impl IndexStore {
    pub fn new(base_path: Option<&str>) -> Self {
        let path = match base_path {
            Some(p) => PathBuf::from(p),
            None => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".code-index"),
        };
        fs::create_dir_all(&path).ok();
        Self { base_path: path }
    }

    fn safe_repo_component(value: &str) -> Result<&str, String> {
        if value.is_empty() || value == "." || value == ".." {
            return Err(format!("Invalid component: {value:?}"));
        }
        if value.contains('/') || value.contains('\\') {
            return Err(format!("Invalid component: {value:?}"));
        }
        let re = Regex::new(r"^[A-Za-z0-9._-]+$").unwrap();
        if !re.is_match(value) {
            return Err(format!("Invalid component: {value:?}"));
        }
        Ok(value)
    }

    fn repo_slug(owner: &str, name: &str) -> Result<String, String> {
        let safe_owner = Self::safe_repo_component(owner)?;
        let safe_name = Self::safe_repo_component(name)?;
        Ok(format!("{safe_owner}-{safe_name}"))
    }

    fn index_path(&self, owner: &str, name: &str) -> Result<PathBuf, String> {
        let slug = Self::repo_slug(owner, name)?;
        Ok(self.base_path.join(format!("{slug}.json")))
    }

    pub fn content_dir(&self, owner: &str, name: &str) -> Result<PathBuf, String> {
        let slug = Self::repo_slug(owner, name)?;
        Ok(self.base_path.join(slug))
    }

    fn safe_content_path(content_dir: &Path, relative_path: &str) -> Option<PathBuf> {
        let candidate = content_dir.join(relative_path);
        let base = match content_dir.canonicalize() {
            Ok(b) => b,
            Err(_) => {
                // If content_dir doesn't exist yet, just check for traversal
                if relative_path.contains("..") {
                    return None;
                }
                return Some(candidate);
            }
        };
        match candidate.canonicalize() {
            Ok(resolved) => {
                if resolved.starts_with(&base) {
                    Some(resolved)
                } else {
                    None
                }
            }
            Err(_) => {
                // File doesn't exist yet — check parent
                if relative_path.contains("..") {
                    None
                } else {
                    Some(candidate)
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_index(
        &self,
        owner: &str,
        name: &str,
        source_files: &[String],
        symbols: &[Symbol],
        raw_files: &HashMap<String, String>,
        languages: &HashMap<String, usize>,
        file_hashes: Option<&HashMap<String, String>>,
        git_head: &str,
    ) -> Result<CodeIndex, String> {
        let computed_hashes: HashMap<String, String>;
        let hashes = match file_hashes {
            Some(h) => h,
            None => {
                computed_hashes = raw_files
                    .iter()
                    .map(|(fp, content)| (fp.clone(), file_hash(content)))
                    .collect();
                &computed_hashes
            }
        };

        let index = CodeIndex {
            repo: format!("{owner}/{name}"),
            owner: owner.to_string(),
            name: name.to_string(),
            indexed_at: Utc::now().to_rfc3339(),
            source_files: source_files.to_vec(),
            languages: languages.clone(),
            symbols: symbols.iter().map(symbol_to_json).collect(),
            index_version: INDEX_VERSION,
            file_hashes: hashes.clone(),
            git_head: git_head.to_string(),
        };

        // Save index JSON atomically
        let index_path = self.index_path(owner, name)?;
        let tmp_path = index_path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())?;
        fs::write(&tmp_path, &json).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &index_path).map_err(|e| e.to_string())?;

        // Save raw files
        let content_dir = self.content_dir(owner, name)?;
        fs::create_dir_all(&content_dir).map_err(|e| e.to_string())?;

        for (file_path, content) in raw_files {
            let dest = Self::safe_content_path(&content_dir, file_path)
                .ok_or_else(|| format!("Unsafe file path: {file_path}"))?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&dest, content).map_err(|e| e.to_string())?;
        }

        Ok(index)
    }

    pub fn load_index(&self, owner: &str, name: &str) -> Option<CodeIndex> {
        let index_path = self.index_path(owner, name).ok()?;
        if !index_path.exists() {
            return None;
        }
        let data = fs::read_to_string(&index_path).ok()?;
        let index: CodeIndex = serde_json::from_str(&data).ok()?;
        if index.index_version > INDEX_VERSION {
            return None;
        }
        Some(index)
    }

    pub fn get_symbol_content(&self, owner: &str, name: &str, symbol_id: &str) -> Option<String> {
        let index = self.load_index(owner, name)?;
        let symbol = index.get_symbol(symbol_id)?;

        let file = symbol.get("file")?.as_str()?;
        let byte_offset = symbol.get("byte_offset")?.as_u64()? as usize;
        let byte_length = symbol.get("byte_length")?.as_u64()? as usize;

        let content_dir = self.content_dir(owner, name).ok()?;
        let file_path = Self::safe_content_path(&content_dir, file)?;
        if !file_path.exists() {
            return None;
        }

        let data = fs::read(&file_path).ok()?;
        if byte_offset + byte_length > data.len() {
            return None;
        }
        let source_bytes = &data[byte_offset..byte_offset + byte_length];
        Some(String::from_utf8_lossy(source_bytes).to_string())
    }

    pub fn detect_changes(
        &self,
        owner: &str,
        name: &str,
        current_files: &HashMap<String, String>,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let index = match self.load_index(owner, name) {
            Some(idx) => idx,
            None => {
                return (
                    Vec::new(),
                    current_files.keys().cloned().collect(),
                    Vec::new(),
                );
            }
        };

        let old_hashes = &index.file_hashes;
        let current_hashes: HashMap<String, String> = current_files
            .iter()
            .map(|(fp, content)| (fp.clone(), file_hash(content)))
            .collect();

        let old_set: std::collections::HashSet<&String> = old_hashes.keys().collect();
        let new_set: std::collections::HashSet<&String> = current_hashes.keys().collect();

        let new_files: Vec<String> = new_set.difference(&old_set).map(|s| (*s).clone()).collect();
        let deleted_files: Vec<String> =
            old_set.difference(&new_set).map(|s| (*s).clone()).collect();
        let changed_files: Vec<String> = old_set
            .intersection(&new_set)
            .filter(|fp| old_hashes[**fp] != current_hashes[**fp])
            .map(|s| (*s).clone())
            .collect();

        (changed_files, new_files, deleted_files)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn incremental_save(
        &self,
        owner: &str,
        name: &str,
        changed_files: &[String],
        new_files: &[String],
        deleted_files: &[String],
        new_symbols: &[Symbol],
        raw_files: &HashMap<String, String>,
        languages: &HashMap<String, usize>,
        git_head: &str,
    ) -> Option<CodeIndex> {
        let index = self.load_index(owner, name)?;

        let files_to_remove: std::collections::HashSet<&str> = deleted_files
            .iter()
            .chain(changed_files.iter())
            .map(|s| s.as_str())
            .collect();

        let mut kept_symbols: Vec<serde_json::Value> = index
            .symbols
            .into_iter()
            .filter(|s| {
                s.get("file")
                    .and_then(|v| v.as_str())
                    .map(|f| !files_to_remove.contains(f))
                    .unwrap_or(true)
            })
            .collect();

        for sym in new_symbols {
            kept_symbols.push(symbol_to_json(sym));
        }

        let recomputed_languages = languages_from_symbols(&kept_symbols);
        let final_languages = if recomputed_languages.is_empty() && !languages.is_empty() {
            languages.clone()
        } else {
            recomputed_languages
        };

        let mut source_files: std::collections::HashSet<String> =
            index.source_files.into_iter().collect();
        for f in deleted_files {
            source_files.remove(f);
        }
        for f in new_files {
            source_files.insert(f.clone());
        }
        for f in changed_files {
            source_files.insert(f.clone());
        }

        let mut fh = index.file_hashes;
        for f in deleted_files {
            fh.remove(f);
        }
        for (fp, content) in raw_files {
            fh.insert(fp.clone(), file_hash(content));
        }

        let mut sorted_files: Vec<String> = source_files.into_iter().collect();
        sorted_files.sort();

        let updated = CodeIndex {
            repo: format!("{owner}/{name}"),
            owner: owner.to_string(),
            name: name.to_string(),
            indexed_at: Utc::now().to_rfc3339(),
            source_files: sorted_files,
            languages: final_languages,
            symbols: kept_symbols,
            index_version: INDEX_VERSION,
            file_hashes: fh,
            git_head: git_head.to_string(),
        };

        // Save atomically
        if let Ok(index_path) = self.index_path(owner, name) {
            let tmp_path = index_path.with_extension("json.tmp");
            if let Ok(json) = serde_json::to_string_pretty(&updated) {
                let _ = fs::write(&tmp_path, &json);
                let _ = fs::rename(&tmp_path, &index_path);
            }
        }

        // Update raw files
        if let Ok(content_dir) = self.content_dir(owner, name) {
            let _ = fs::create_dir_all(&content_dir);

            for fp in deleted_files {
                if let Some(dead) = Self::safe_content_path(&content_dir, fp) {
                    let _ = fs::remove_file(dead);
                }
            }

            for (fp, content) in raw_files {
                if let Some(dest) = Self::safe_content_path(&content_dir, fp) {
                    if let Some(parent) = dest.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(&dest, content);
                }
            }
        }

        Some(updated)
    }

    pub fn list_repos(&self) -> Vec<serde_json::Value> {
        let mut repos = Vec::new();
        let entries = match fs::read_dir(&self.base_path) {
            Ok(e) => e,
            Err(_) => return repos,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false)
                && let Ok(data) = fs::read_to_string(&path)
                && let Ok(index) = serde_json::from_str::<serde_json::Value>(&data)
            {
                repos.push(serde_json::json!({
                            "repo": index.get("repo").and_then(|v| v.as_str()).unwrap_or(""),
                            "indexed_at": index.get("indexed_at").and_then(|v| v.as_str()).unwrap_or(""),
                            "symbol_count": index.get("symbols").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                            "file_count": index.get("source_files").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                            "languages": index.get("languages").cloned().unwrap_or(serde_json::json!({})),
                            "index_version": index.get("index_version").and_then(|v| v.as_u64()).unwrap_or(1),
                        }));
            }
        }
        repos
    }

    pub fn delete_index(&self, owner: &str, name: &str) -> bool {
        let mut deleted = false;

        if let Ok(index_path) = self.index_path(owner, name)
            && index_path.exists()
        {
            let _ = fs::remove_file(&index_path);
            deleted = true;
        }

        if let Ok(content_dir) = self.content_dir(owner, name)
            && content_dir.exists()
        {
            let _ = fs::remove_dir_all(&content_dir);
            deleted = true;
        }

        deleted
    }
}

fn symbol_to_json(symbol: &Symbol) -> serde_json::Value {
    serde_json::json!({
        "id": symbol.id,
        "file": symbol.file,
        "name": symbol.name,
        "qualified_name": symbol.qualified_name,
        "kind": symbol.kind,
        "language": symbol.language,
        "signature": symbol.signature,
        "docstring": symbol.docstring,
        "summary": symbol.summary,
        "decorators": symbol.decorators,
        "keywords": symbol.keywords,
        "parent": symbol.parent,
        "line": symbol.line,
        "end_line": symbol.end_line,
        "byte_offset": symbol.byte_offset,
        "byte_length": symbol.byte_length,
        "content_hash": symbol.content_hash,
    })
}

fn languages_from_symbols(symbols: &[serde_json::Value]) -> HashMap<String, usize> {
    let mut file_languages: HashMap<String, String> = HashMap::new();
    for sym in symbols {
        let file_path = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let language = sym.get("language").and_then(|v| v.as_str()).unwrap_or("");
        if !file_path.is_empty() && !language.is_empty() {
            file_languages
                .entry(file_path.to_string())
                .or_insert_with(|| language.to_string());
        }
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    for language in file_languages.values() {
        *counts.entry(language.clone()).or_insert(0) += 1;
    }
    counts
}
