use std::path::Path;
use std::{env, fs};

/// Check that target path resolves within root directory.
pub fn validate_path(root: &Path, target: &Path) -> bool {
    match (root.canonicalize(), target.canonicalize()) {
        (Ok(resolved_root), Ok(resolved)) => resolved.starts_with(&resolved_root),
        _ => false,
    }
}

/// Check if a symlink points outside the root directory.
pub fn is_symlink_escape(root: &Path, path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            match (root.canonicalize(), path.canonicalize()) {
                (Ok(resolved_root), Ok(resolved)) => !resolved.starts_with(&resolved_root),
                _ => true,
            }
        }
        _ => false,
    }
}

/// Known secret file patterns.
const SECRET_PATTERNS: &[&str] = &[
    "*.env",
    ".env",
    ".env.*",
    "*.pem",
    "*.key",
    "*.p12",
    "*.pfx",
    "*.credentials",
    "*.keystore",
    "*.jks",
    "*.token",
    "*secret*",
    "id_rsa",
    "id_rsa.*",
    "id_ed25519",
    "id_ed25519.*",
    "id_dsa",
    "id_ecdsa",
    ".htpasswd",
    ".netrc",
    ".npmrc",
    ".pypirc",
    "credentials.json",
    "service-account*.json",
    "*.secrets",
];

/// Check if a file path matches known secret file patterns.
pub fn is_secret_file(file_path: &str) -> bool {
    let name = Path::new(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let path_lower = file_path.to_lowercase();

    for pattern in SECRET_PATTERNS {
        if glob_match(pattern, &name) || glob_match(pattern, &path_lower) {
            return true;
        }
    }
    false
}

/// Simple glob matching (supports * and ? wildcards).
fn glob_match(pattern: &str, text: &str) -> bool {
    glob::Pattern::new(pattern)
        .map(|p| p.matches(text))
        .unwrap_or(false)
}

/// Known binary file extensions.
const BINARY_EXTENSIONS: &[&str] = &[
    ".exe", ".dll", ".so", ".dylib", ".bin", ".out", ".o", ".obj", ".a", ".lib", ".zip", ".tar",
    ".gz", ".bz2", ".xz", ".7z", ".rar", ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".svg",
    ".webp", ".tiff", ".tif", ".mp3", ".mp4", ".avi", ".mov", ".mkv", ".wav", ".flac", ".ogg",
    ".webm", ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".pyc", ".pyo", ".class",
    ".wasm", ".db", ".sqlite", ".sqlite3", ".ttf", ".otf", ".woff", ".woff2", ".eot", ".jar",
    ".war", ".ear",
];

/// Check if a file has a known binary extension.
pub fn is_binary_extension(file_path: &str) -> bool {
    let lower = file_path.to_lowercase();
    BINARY_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

/// Detect binary content by checking for null bytes.
pub fn is_binary_content(data: &[u8], check_size: usize) -> bool {
    let sample = if data.len() > check_size {
        &data[..check_size]
    } else {
        data
    };
    sample.contains(&0)
}

/// Check if a file is binary using extension check + content sniffing.
pub fn is_binary_file(file_path: &Path) -> bool {
    if is_binary_extension(&file_path.to_string_lossy()) {
        return true;
    }
    match fs::read(file_path) {
        Ok(data) => is_binary_content(&data, 8192),
        Err(_) => true,
    }
}

pub const DEFAULT_MAX_FILE_SIZE: u64 = 500 * 1024; // 500KB
pub const DEFAULT_MAX_INDEX_FILES: usize = 500;

/// Resolve the maximum indexed file count from arg or environment.
pub fn get_max_index_files(max_files: Option<usize>) -> usize {
    if let Some(mf) = max_files {
        if mf == 0 {
            return DEFAULT_MAX_INDEX_FILES;
        }
        return mf;
    }
    match env::var("MUNCHRS_MAX_INDEX_FILES") {
        Ok(val) => val
            .parse::<usize>()
            .unwrap_or(DEFAULT_MAX_INDEX_FILES)
            .max(1),
        Err(_) => DEFAULT_MAX_INDEX_FILES,
    }
}

/// Run all security checks on a file. Returns reason string if excluded, None if ok.
#[allow(dead_code)]
pub fn should_exclude_file(
    file_path: &Path,
    root: &Path,
    max_file_size: u64,
    check_secrets: bool,
    check_binary: bool,
    check_symlinks: bool,
) -> Option<&'static str> {
    if check_symlinks && is_symlink_escape(root, file_path) {
        return Some("symlink_escape");
    }
    if !validate_path(root, file_path) {
        return Some("path_traversal");
    }

    let rel_path = match file_path.strip_prefix(root) {
        Ok(r) => r.to_string_lossy().replace('\\', "/"),
        Err(_) => return Some("outside_root"),
    };

    if check_secrets && is_secret_file(&rel_path) {
        return Some("secret_file");
    }

    match fs::metadata(file_path) {
        Ok(meta) => {
            if meta.len() > max_file_size {
                return Some("file_too_large");
            }
        }
        Err(_) => return Some("unreadable"),
    }

    if check_binary && is_binary_extension(&rel_path) {
        return Some("binary_extension");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_secret_file() {
        assert!(is_secret_file(".env"));
        assert!(is_secret_file("config/.env.local"));
        assert!(is_secret_file("id_rsa"));
        assert!(is_secret_file("server.pem"));
        assert!(!is_secret_file("main.py"));
        assert!(!is_secret_file("README.md"));
    }

    #[test]
    fn test_is_binary_extension() {
        assert!(is_binary_extension("image.png"));
        assert!(is_binary_extension("binary.exe"));
        assert!(!is_binary_extension("code.py"));
        assert!(!is_binary_extension("data.json"));
    }

    #[test]
    fn test_is_binary_content() {
        assert!(is_binary_content(b"hello\x00world", 8192));
        assert!(!is_binary_content(b"hello world", 8192));
    }
}
