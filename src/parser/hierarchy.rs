use super::symbols::Symbol;
use std::collections::HashMap;

/// A node in the symbol tree with children.
#[derive(Debug, Clone)]
pub struct SymbolNode {
    pub symbol: Symbol,
    pub children: Vec<SymbolNode>,
}

/// Build a hierarchical tree from flat symbol list.
///
/// Methods become children of their parent classes.
/// Returns top-level symbols (classes and standalone functions).
pub fn build_symbol_tree(symbols: &[Symbol]) -> Vec<SymbolNode> {
    let mut node_map: HashMap<&str, SymbolNode> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();

    for s in symbols {
        node_map.insert(
            &s.id,
            SymbolNode {
                symbol: s.clone(),
                children: Vec::new(),
            },
        );
        order.push(&s.id);
    }

    // Collect parent-child relationships
    let mut child_to_parent: Vec<(String, String)> = Vec::new();
    for s in symbols {
        if let Some(ref parent_id) = s.parent
            && node_map.contains_key(parent_id.as_str())
        {
            child_to_parent.push((s.id.clone(), parent_id.clone()));
        }
    }

    // Move children into parents
    for (child_id, parent_id) in &child_to_parent {
        if let Some(child_node) = node_map.remove(child_id.as_str()) {
            if let Some(parent_node) = node_map.get_mut(parent_id.as_str()) {
                parent_node.children.push(child_node);
            } else {
                // Parent was already moved; put child back as root
                node_map.insert(child_id, child_node);
            }
        }
    }

    // Collect roots in original order
    let mut roots = Vec::new();
    for id in &order {
        if let Some(node) = node_map.remove(*id) {
            roots.push(node);
        }
    }
    roots
}

/// Flatten symbol tree with depth information.
///
/// Returns list of (symbol, depth) tuples for indentation.
#[allow(dead_code)]
pub fn flatten_tree(nodes: &[SymbolNode], depth: usize) -> Vec<(Symbol, usize)> {
    let mut result = Vec::new();
    for node in nodes {
        result.push((node.symbol.clone(), depth));
        result.extend(flatten_tree(&node.children, depth + 1));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::symbols::make_symbol_id;

    fn make_test_symbol(name: &str, kind: &str, parent: Option<&str>) -> Symbol {
        Symbol {
            id: make_symbol_id("test.py", name, kind),
            file: "test.py".to_string(),
            name: name.to_string(),
            qualified_name: name.to_string(),
            kind: kind.to_string(),
            language: "python".to_string(),
            signature: format!("def {name}()"),
            docstring: String::new(),
            summary: String::new(),
            decorators: Vec::new(),
            keywords: Vec::new(),
            parent: parent.map(|p| p.to_string()),
            line: 1,
            end_line: 5,
            byte_offset: 0,
            byte_length: 100,
            content_hash: String::new(),
        }
    }

    #[test]
    fn test_build_symbol_tree() {
        let class_id = make_symbol_id("test.py", "MyClass", "class");
        let symbols = vec![
            make_test_symbol("MyClass", "class", None),
            make_test_symbol("method", "method", Some(&class_id)),
            make_test_symbol("standalone", "function", None),
        ];

        let tree = build_symbol_tree(&symbols);
        assert_eq!(tree.len(), 2); // MyClass + standalone
        assert_eq!(tree[0].symbol.name, "MyClass");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].symbol.name, "method");
        assert_eq!(tree[1].symbol.name, "standalone");
    }

    #[test]
    fn test_flatten_tree() {
        let class_id = make_symbol_id("test.py", "MyClass", "class");
        let symbols = vec![
            make_test_symbol("MyClass", "class", None),
            make_test_symbol("method", "method", Some(&class_id)),
        ];

        let tree = build_symbol_tree(&symbols);
        let flat = flatten_tree(&tree, 0);
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].0.name, "MyClass");
        assert_eq!(flat[0].1, 0);
        assert_eq!(flat[1].0.name, "method");
        assert_eq!(flat[1].1, 1);
    }
}
