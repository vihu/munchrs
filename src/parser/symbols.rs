use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A code symbol extracted from source via tree-sitter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Unique ID: "file_path::QualifiedName#kind"
    pub id: String,
    /// Source file path (e.g., "src/main.py")
    pub file: String,
    /// Symbol name (e.g., "login")
    pub name: String,
    /// Fully qualified (e.g., "MyClass.login")
    pub qualified_name: String,
    /// "function" | "class" | "method" | "constant" | "type"
    pub kind: String,
    /// "python" | "javascript" | "typescript" | "go" | "rust" | "java" | "c" | "cpp" etc.
    pub language: String,
    /// Full signature line(s)
    pub signature: String,
    /// Extracted docstring (language-specific)
    #[serde(default)]
    pub docstring: String,
    /// One-line summary
    #[serde(default)]
    pub summary: String,
    /// Decorators/attributes
    #[serde(default)]
    pub decorators: Vec<String>,
    /// Extracted search keywords
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Parent symbol ID (for methods -> class)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Start line number (1-indexed)
    #[serde(default)]
    pub line: usize,
    /// End line number (1-indexed)
    #[serde(default)]
    pub end_line: usize,
    /// Start byte in raw file
    #[serde(default)]
    pub byte_offset: usize,
    /// Byte length of full source
    #[serde(default)]
    pub byte_length: usize,
    /// SHA-256 of symbol source bytes (for drift detection)
    #[serde(default)]
    pub content_hash: String,
}

/// Generate unique symbol ID.
///
/// Format: {file_path}::{qualified_name}#{kind}
/// Example: src/main.py::MyClass.login#method
pub fn make_symbol_id(file_path: &str, qualified_name: &str, kind: &str) -> String {
    if kind.is_empty() {
        format!("{file_path}::{qualified_name}")
    } else {
        format!("{file_path}::{qualified_name}#{kind}")
    }
}

/// Compute SHA-256 hash of symbol source bytes for drift detection.
pub fn compute_content_hash(source_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_symbol_id_with_kind() {
        assert_eq!(
            make_symbol_id("src/main.py", "MyClass.login", "method"),
            "src/main.py::MyClass.login#method"
        );
    }

    #[test]
    fn test_make_symbol_id_without_kind() {
        assert_eq!(
            make_symbol_id("src/main.py", "MyClass", ""),
            "src/main.py::MyClass"
        );
    }

    #[test]
    fn test_compute_content_hash() {
        let hash = compute_content_hash(b"def hello(): pass");
        assert_eq!(hash.len(), 64); // SHA-256 hex length
        // Same input should produce same hash
        assert_eq!(hash, compute_content_hash(b"def hello(): pass"));
        // Different input should produce different hash
        assert_ne!(hash, compute_content_hash(b"def world(): pass"));
    }
}
