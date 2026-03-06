use super::{
    languages::{LANGUAGE_REGISTRY, LanguageSpec},
    symbols::{Symbol, compute_content_hash, make_symbol_id},
};
use std::collections::HashMap;
use tree_sitter::{Node, Parser, Tree};

/// Parse source code and extract symbols using tree-sitter.
pub fn parse_file(content: &str, filename: &str, language: &str) -> Vec<Symbol> {
    let Some(spec) = LANGUAGE_REGISTRY.get(language) else {
        return Vec::new();
    };

    let source_bytes = content.as_bytes();

    let mut symbols = if language == "cpp" {
        parse_cpp_symbols(source_bytes, filename)
    } else if language == "elixir" {
        parse_elixir_symbols(source_bytes, filename)
    } else if language == "erlang" {
        parse_erlang_symbols(source_bytes, filename)
    } else {
        parse_with_spec(source_bytes, filename, language, spec)
    };

    disambiguate_overloads(&mut symbols);
    symbols
}

/// Get a tree-sitter parser for a language name.
fn get_parser(ts_language: &str) -> Option<Parser> {
    let mut parser = Parser::new();

    let ok = match ts_language {
        "python" => parser.set_language(&tree_sitter_python::LANGUAGE.into()),
        "javascript" => parser.set_language(&tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into()),
        "go" => parser.set_language(&tree_sitter_go::LANGUAGE.into()),
        "rust" => parser.set_language(&tree_sitter_rust::LANGUAGE.into()),
        "java" => parser.set_language(&tree_sitter_java::LANGUAGE.into()),
        "php" => parser.set_language(&tree_sitter_php::LANGUAGE_PHP.into()),
        "c" => parser.set_language(&tree_sitter_c::LANGUAGE.into()),
        "cpp" => parser.set_language(&tree_sitter_cpp::LANGUAGE.into()),
        "c_sharp" | "csharp" => parser.set_language(&tree_sitter_c_sharp::LANGUAGE.into()),
        "swift" => parser.set_language(&tree_sitter_swift::LANGUAGE.into()),
        "dart" => parser.set_language(&tree_sitter_dart::language()),
        "elixir" => parser.set_language(&tree_sitter_elixir::LANGUAGE.into()),
        "erlang" => parser.set_language(&tree_sitter_erlang::LANGUAGE.into()),
        _ => return None,
    };

    ok.ok().map(|()| parser)
}

fn parse_tree(ts_language: &str, source_bytes: &[u8]) -> Option<Tree> {
    let mut parser = get_parser(ts_language)?;
    parser.parse(source_bytes, None)
}

fn parse_with_spec(
    source_bytes: &[u8],
    filename: &str,
    language: &str,
    spec: &LanguageSpec,
) -> Vec<Symbol> {
    let Some(tree) = parse_tree(spec.ts_language, source_bytes) else {
        return Vec::new();
    };

    let mut symbols = Vec::new();
    walk_tree(
        tree.root_node(),
        spec,
        source_bytes,
        filename,
        language,
        &mut symbols,
        None,
        &[],
        0,
    );
    symbols
}

fn parse_cpp_symbols(source_bytes: &[u8], filename: &str) -> Vec<Symbol> {
    let cpp_spec = match LANGUAGE_REGISTRY.get("cpp") {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut cpp_symbols = Vec::new();
    let mut cpp_error_nodes: usize = usize::MAX;

    if let Some(tree) = parse_tree("cpp", source_bytes) {
        cpp_error_nodes = count_error_nodes(tree.root_node());
        walk_tree(
            tree.root_node(),
            cpp_spec,
            source_bytes,
            filename,
            "cpp",
            &mut cpp_symbols,
            None,
            &[],
            0,
        );
    }

    // Non-headers are always C++.
    if !filename.to_lowercase().ends_with(".h") {
        return cpp_symbols;
    }

    // Header auto-detection: parse both C++ and C, prefer better parse quality.
    let c_spec = match LANGUAGE_REGISTRY.get("c") {
        Some(s) => s,
        None => return cpp_symbols,
    };

    let mut c_symbols = Vec::new();
    let mut c_error_nodes: usize = usize::MAX;

    if let Some(tree) = parse_tree("c", source_bytes) {
        c_error_nodes = count_error_nodes(tree.root_node());
        walk_tree(
            tree.root_node(),
            c_spec,
            source_bytes,
            filename,
            "c",
            &mut c_symbols,
            None,
            &[],
            0,
        );
    }

    // If only one parser yields symbols, use that.
    if !cpp_symbols.is_empty() && c_symbols.is_empty() {
        return cpp_symbols;
    }
    if !c_symbols.is_empty() && cpp_symbols.is_empty() {
        return c_symbols;
    }
    if cpp_symbols.is_empty() && c_symbols.is_empty() {
        return cpp_symbols;
    }

    // Both yielded: choose fewer parse errors first, then richer output.
    if c_error_nodes < cpp_error_nodes {
        return c_symbols;
    }
    if cpp_error_nodes < c_error_nodes {
        return cpp_symbols;
    }

    // Same error quality: use lexical signal to break ties for `.h`.
    if looks_like_cpp_header(source_bytes) {
        if cpp_symbols.len() >= c_symbols.len() {
            return cpp_symbols;
        }
    } else {
        return c_symbols;
    }

    if c_symbols.len() > cpp_symbols.len() {
        return c_symbols;
    }

    cpp_symbols
}

#[allow(clippy::too_many_arguments)]
fn walk_tree(
    node: Node,
    spec: &LanguageSpec,
    source_bytes: &[u8],
    filename: &str,
    language: &str,
    symbols: &mut Vec<Symbol>,
    parent_symbol: Option<&Symbol>,
    scope_parts: &[String],
    class_scope_depth: usize,
) {
    // Dart: function_signature inside method_signature is handled by method_signature
    if node.kind() == "function_signature"
        && let Some(parent) = node.parent()
        && parent.kind() == "method_signature"
    {
        return;
    }

    let is_cpp = language == "cpp";
    let mut local_scope_parts = scope_parts.to_vec();
    let mut next_parent = parent_symbol;
    let mut next_class_scope_depth = class_scope_depth;
    // Temp storage to hold the symbol while we recurse
    let mut extracted_symbol: Option<Symbol> = None;

    if is_cpp
        && node.kind() == "namespace_definition"
        && let Some(ns_name) = extract_cpp_namespace_name(node, source_bytes)
    {
        local_scope_parts.push(ns_name);
    }

    // Check if this node is a symbol
    if spec.symbol_node_types.contains_key(node.kind()) {
        // C++ declarations: filter non-function declarations
        let skip = is_cpp
            && (node.kind() == "declaration" || node.kind() == "field_declaration")
            && !is_cpp_function_declaration(node);

        if !skip
            && let Some(sym) = extract_symbol(
                node,
                spec,
                source_bytes,
                filename,
                language,
                parent_symbol,
                &local_scope_parts,
                class_scope_depth,
            )
        {
            symbols.push(sym.clone());
            if is_cpp {
                if is_cpp_type_container(node) {
                    extracted_symbol = Some(sym);
                    next_class_scope_depth = class_scope_depth + 1;
                }
            } else {
                extracted_symbol = Some(sym);
            }
        }
    }

    if let Some(ref sym) = extracted_symbol {
        next_parent = Some(sym);
    }

    // Check for arrow/function-expression variable assignments in JS/TS
    if node.kind() == "variable_declarator"
        && (language == "javascript" || language == "typescript")
        && let Some(var_func) =
            extract_variable_function(node, spec, source_bytes, filename, language, parent_symbol)
    {
        symbols.push(var_func);
    }

    // Check for constant patterns (top-level assignments with UPPER_CASE names)
    if spec.constant_patterns.contains(&node.kind())
        && parent_symbol.is_none()
        && let Some(const_symbol) = extract_constant(node, spec, source_bytes, filename, language)
    {
        symbols.push(const_symbol);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree(
            child,
            spec,
            source_bytes,
            filename,
            language,
            symbols,
            next_parent,
            &local_scope_parts,
            next_class_scope_depth,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_symbol(
    node: Node,
    spec: &LanguageSpec,
    source_bytes: &[u8],
    filename: &str,
    language: &str,
    parent_symbol: Option<&Symbol>,
    scope_parts: &[String],
    class_scope_depth: usize,
) -> Option<Symbol> {
    let mut kind = spec.symbol_node_types.get(node.kind())?.to_string();

    if node.has_error() {
        return None;
    }

    let name = extract_name(node, spec, source_bytes)?;
    if name.is_empty() {
        return None;
    }

    let qualified_name;
    if language == "cpp" {
        if let Some(parent) = parent_symbol {
            qualified_name = format!("{}.{}", parent.qualified_name, name);
        } else if !scope_parts.is_empty() {
            let mut parts = scope_parts.to_vec();
            parts.push(name.clone());
            qualified_name = parts.join(".");
        } else {
            qualified_name = name.clone();
        }
        if kind == "function" && class_scope_depth > 0 {
            kind = "method".to_string();
        }
    } else if let Some(parent) = parent_symbol {
        qualified_name = format!("{}.{}", parent.name, name);
        if kind == "function" {
            kind = "method".to_string();
        }
    } else {
        qualified_name = name.clone();
    }

    let signature_node = if language == "cpp" {
        nearest_cpp_template_wrapper(node).unwrap_or(node)
    } else {
        node
    };

    let signature = build_signature(signature_node, source_bytes);
    let docstring = extract_docstring(signature_node, spec, source_bytes);
    let decorators = extract_decorators(node, spec, source_bytes);

    let start_node = signature_node;
    let mut end_byte = node.end_byte();
    let mut end_line_num = node.end_position().row + 1;

    // Dart: function_signature/method_signature have their body as next sibling
    if (node.kind() == "function_signature" || node.kind() == "method_signature")
        && let Some(next_sib) = node.next_named_sibling()
        && next_sib.kind() == "function_body"
    {
        end_byte = next_sib.end_byte();
        end_line_num = next_sib.end_position().row + 1;
    }

    let symbol_bytes = &source_bytes[start_node.start_byte()..end_byte];
    let c_hash = compute_content_hash(symbol_bytes);

    Some(Symbol {
        id: make_symbol_id(filename, &qualified_name, &kind),
        file: filename.to_string(),
        name,
        qualified_name,
        kind,
        language: language.to_string(),
        signature,
        docstring,
        summary: String::new(),
        decorators,
        keywords: Vec::new(),
        parent: parent_symbol.map(|p| p.id.clone()),
        line: start_node.start_position().row + 1,
        end_line: end_line_num,
        byte_offset: start_node.start_byte(),
        byte_length: end_byte - start_node.start_byte(),
        content_hash: c_hash,
    })
}

fn extract_name(node: Node, spec: &LanguageSpec, source_bytes: &[u8]) -> Option<String> {
    // Erlang: fun_decl has name in first function_clause child's first atom
    if node.kind() == "fun_decl" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_clause" {
                // First child of function_clause is the function name atom
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "atom" {
                        return Some(node_text(inner, source_bytes));
                    }
                }
            }
        }
        return None;
    }

    // Erlang: type_alias has name in type_name child's atom
    if node.kind() == "type_alias" || node.kind() == "opaque" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_name" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "atom" {
                        return Some(node_text(inner, source_bytes));
                    }
                }
            }
        }
        return None;
    }

    // Erlang: record_decl has name as direct atom child
    if node.kind() == "record_decl" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "atom" {
                return Some(node_text(child, source_bytes));
            }
        }
        return None;
    }

    // Go: type_declaration name is in type_spec child
    if node.kind() == "type_declaration" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec"
                && let Some(name_node) = child.child_by_field_name("name")
            {
                return Some(node_text(name_node, source_bytes));
            }
        }
        return None;
    }

    // Dart: mixin_declaration has identifier as direct child
    if node.kind() == "mixin_declaration" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Some(node_text(child, source_bytes));
            }
        }
        return None;
    }

    // Dart: method_signature wraps function_signature or getter_signature
    if node.kind() == "method_signature" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if (child.kind() == "function_signature" || child.kind() == "getter_signature")
                && let Some(name_node) = child.child_by_field_name("name")
            {
                return Some(node_text(name_node, source_bytes));
            }
        }
        return None;
    }

    // Dart: type_alias name is the first type_identifier child
    if node.kind() == "type_alias" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                return Some(node_text(child, source_bytes));
            }
        }
        return None;
    }

    let field_name = spec.name_fields.get(node.kind())?;
    let name_node = node.child_by_field_name(field_name)?;

    if spec.ts_language == "cpp" {
        return extract_cpp_name(name_node, source_bytes);
    }

    // C function_definition: declarator is a function_declarator, unwrap
    let mut current = name_node;
    while matches!(
        current.kind(),
        "function_declarator" | "pointer_declarator" | "reference_declarator"
    ) {
        if let Some(inner) = current.child_by_field_name("declarator") {
            current = inner;
        } else {
            break;
        }
    }

    Some(node_text(current, source_bytes))
}

fn extract_cpp_name(name_node: Node, source_bytes: &[u8]) -> Option<String> {
    let wrapper_types = [
        "function_declarator",
        "pointer_declarator",
        "reference_declarator",
        "array_declarator",
        "parenthesized_declarator",
        "attributed_declarator",
        "init_declarator",
    ];

    let mut current = name_node;
    while wrapper_types.contains(&current.kind()) {
        if let Some(inner) = current.child_by_field_name("declarator") {
            current = inner;
        } else {
            break;
        }
    }

    // Prefer typed name children
    if (current.kind() == "qualified_identifier" || current.kind() == "scoped_identifier")
        && let Some(n) = current.child_by_field_name("name")
    {
        let text = node_text(n, source_bytes);
        if !text.is_empty() {
            return Some(text);
        }
    }

    if let Some(name) = find_cpp_name_in_subtree(current, source_bytes) {
        return Some(name);
    }

    let text = node_text(current, source_bytes);
    if text.is_empty() { None } else { Some(text) }
}

fn find_cpp_name_in_subtree(node: Node, source_bytes: &[u8]) -> Option<String> {
    let direct_types = [
        "identifier",
        "field_identifier",
        "operator_name",
        "destructor_name",
        "type_identifier",
    ];

    if direct_types.contains(&node.kind()) {
        let text = node_text(node, source_bytes);
        if !text.is_empty() {
            return Some(text);
        }
        return None;
    }

    if (node.kind() == "qualified_identifier" || node.kind() == "scoped_identifier")
        && let Some(n) = node.child_by_field_name("name")
    {
        return find_cpp_name_in_subtree(n, source_bytes);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named()
            && let Some(found) = find_cpp_name_in_subtree(child, source_bytes)
        {
            return Some(found);
        }
    }
    None
}

fn build_signature(node: Node, source_bytes: &[u8]) -> String {
    let end_byte = if node.kind() == "template_declaration" {
        let inner = node.child_by_field_name("declaration").or_else(|| {
            // fallback: last named child
            let count = node.named_child_count();
            if count > 0 {
                node.named_child((count - 1) as u32)
            } else {
                None
            }
        });

        if let Some(inner) = inner {
            if let Some(body) = inner.child_by_field_name("body") {
                body.start_byte()
            } else {
                inner.end_byte()
            }
        } else {
            node.end_byte()
        }
    } else if let Some(body) = node.child_by_field_name("body") {
        body.start_byte()
    } else {
        node.end_byte()
    };

    let sig_bytes = &source_bytes[node.start_byte()..end_byte];
    let sig_text = String::from_utf8_lossy(sig_bytes).trim().to_string();
    sig_text
        .trim_end_matches(|c: char| "{: \n\t".contains(c))
        .to_string()
}

fn extract_docstring(node: Node, spec: &LanguageSpec, source_bytes: &[u8]) -> String {
    match spec.docstring_strategy {
        "next_sibling_string" => extract_python_docstring(node, source_bytes),
        "preceding_comment" => extract_preceding_comments(node, source_bytes),
        _ => String::new(),
    }
}

fn extract_python_docstring(node: Node, source_bytes: &[u8]) -> String {
    let body = match node.child_by_field_name("body") {
        Some(b) => b,
        None => return String::new(),
    };

    if body.child_count() == 0 {
        return String::new();
    }

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "expression_statement" {
            // Check field "expression"
            if let Some(expr) = child.child_by_field_name("expression")
                && expr.kind() == "string"
            {
                let doc = node_text(expr, source_bytes);
                return strip_quotes(&doc);
            }
            // Handle tree-sitter-python 0.21+ string format
            if child.child_count() > 0
                && let Some(first) = child.child(0)
                && (first.kind() == "string" || first.kind() == "concatenated_string")
            {
                let doc = node_text(first, source_bytes);
                return strip_quotes(&doc);
            }
        } else if child.kind() == "string" {
            let doc = node_text(child, source_bytes);
            return strip_quotes(&doc);
        }
    }

    String::new()
}

fn strip_quotes(text: &str) -> String {
    let t = text.trim();
    if t.len() >= 6 && (t.starts_with("\"\"\"") && t.ends_with("\"\"\"")) {
        return t[3..t.len() - 3].trim().to_string();
    }
    if t.len() >= 6 && (t.starts_with("'''") && t.ends_with("'''")) {
        return t[3..t.len() - 3].trim().to_string();
    }
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        return t[1..t.len() - 1].trim().to_string();
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        return t[1..t.len() - 1].trim().to_string();
    }
    t.to_string()
}

fn extract_preceding_comments(node: Node, source_bytes: &[u8]) -> String {
    let mut comments = Vec::new();

    // Walk backwards through siblings, skipping annotations/decorators
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        if p.kind() == "annotation" || p.kind() == "marker_annotation" {
            prev = p.prev_named_sibling();
            continue;
        }
        break;
    }

    while let Some(p) = prev {
        if matches!(
            p.kind(),
            "comment" | "line_comment" | "block_comment" | "documentation_comment"
        ) {
            let comment_text = node_text(p, source_bytes);
            comments.insert(0, comment_text);
            prev = p.prev_named_sibling();
        } else {
            break;
        }
    }

    if comments.is_empty() {
        return String::new();
    }

    let docstring = comments.join("\n");
    clean_comment_markers(&docstring)
}

fn clean_comment_markers(text: &str) -> String {
    let mut cleaned = Vec::new();
    for line in text.lines() {
        let mut l = line.trim().to_string();
        if l.starts_with("/**") {
            l = l[3..].to_string();
        } else if l.starts_with("/*") {
            l = l[2..].to_string();
        } else if l.starts_with("///") || l.starts_with("//!") {
            l = l[3..].to_string();
        } else if l.starts_with("//") || l.starts_with("%%") {
            l = l[2..].to_string();
        } else if l.starts_with('%') || l.starts_with('*') {
            l = l[1..].to_string();
        }
        if l.ends_with("*/") {
            l = l[..l.len() - 2].to_string();
        }
        cleaned.push(l.trim().to_string());
    }
    cleaned.join("\n").trim().to_string()
}

fn extract_decorators(node: Node, spec: &LanguageSpec, source_bytes: &[u8]) -> Vec<String> {
    let decorator_type = match spec.decorator_node_type {
        Some(dt) => dt,
        None => return Vec::new(),
    };

    let mut decorators = Vec::new();

    if spec.decorator_from_children {
        // C#: attribute_list nodes are direct children of the declaration
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == decorator_type {
                decorators.push(node_text(child, source_bytes));
            }
        }
    } else {
        // Other languages: decorators are preceding siblings
        let mut prev = node.prev_named_sibling();
        while let Some(p) = prev {
            if p.kind() == decorator_type {
                decorators.insert(0, node_text(p, source_bytes));
                prev = p.prev_named_sibling();
            } else {
                break;
            }
        }
    }

    decorators
}

const VARIABLE_FUNCTION_TYPES: &[&str] = &[
    "arrow_function",
    "function_expression",
    "generator_function",
];

fn extract_variable_function(
    node: Node,
    spec: &LanguageSpec,
    source_bytes: &[u8],
    filename: &str,
    language: &str,
    parent_symbol: Option<&Symbol>,
) -> Option<Symbol> {
    // node is a variable_declarator
    let name_node = node.child_by_field_name("name")?;
    if name_node.kind() != "identifier" {
        return None;
    }

    let value_node = node.child_by_field_name("value")?;
    if !VARIABLE_FUNCTION_TYPES.contains(&value_node.kind()) {
        return None;
    }

    let name = node_text(name_node, source_bytes);

    let (kind, qualified_name) = if let Some(parent) = parent_symbol {
        ("method".to_string(), format!("{}.{}", parent.name, name))
    } else {
        ("function".to_string(), name.clone())
    };

    // Signature: use the full declaration statement
    let mut sig_node = node;
    if let Some(parent) = node.parent()
        && matches!(
            parent.kind(),
            "lexical_declaration" | "export_statement" | "variable_declaration"
        )
    {
        sig_node = parent;
    }
    // Walk up through export_statement wrapper
    if let Some(parent) = sig_node.parent()
        && parent.kind() == "export_statement"
    {
        sig_node = parent;
    }

    let signature = build_signature(sig_node, source_bytes);
    let docstring = extract_docstring(sig_node, spec, source_bytes);

    let start_byte = sig_node.start_byte();
    let end_byte = sig_node.end_byte();
    let symbol_bytes = &source_bytes[start_byte..end_byte];
    let c_hash = compute_content_hash(symbol_bytes);

    Some(Symbol {
        id: make_symbol_id(filename, &qualified_name, &kind),
        file: filename.to_string(),
        name,
        qualified_name,
        kind,
        language: language.to_string(),
        signature,
        docstring,
        summary: String::new(),
        decorators: Vec::new(),
        keywords: Vec::new(),
        parent: parent_symbol.map(|p| p.id.clone()),
        line: sig_node.start_position().row + 1,
        end_line: sig_node.end_position().row + 1,
        byte_offset: start_byte,
        byte_length: end_byte - start_byte,
        content_hash: c_hash,
    })
}

fn extract_constant(
    node: Node,
    _spec: &LanguageSpec,
    source_bytes: &[u8],
    filename: &str,
    language: &str,
) -> Option<Symbol> {
    fn is_upper_case_name(name: &str) -> bool {
        name.chars()
            .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
            || (name.len() > 1
                && name.chars().next().is_some_and(|c| c.is_uppercase())
                && name.contains('_'))
    }

    // Python: assignment
    if node.kind() == "assignment" {
        let left = node.child_by_field_name("left")?;
        if left.kind() != "identifier" {
            return None;
        }
        let name = node_text(left, source_bytes);
        if !is_upper_case_name(&name) {
            return None;
        }
        let sig = node_text(node, source_bytes);
        let sig = if sig.len() > 100 { &sig[..100] } else { &sig };
        let const_bytes = &source_bytes[node.start_byte()..node.end_byte()];
        let c_hash = compute_content_hash(const_bytes);
        return Some(Symbol {
            id: make_symbol_id(filename, &name, "constant"),
            file: filename.to_string(),
            name: name.clone(),
            qualified_name: name,
            kind: "constant".to_string(),
            language: language.to_string(),
            signature: sig.to_string(),
            docstring: String::new(),
            summary: String::new(),
            decorators: Vec::new(),
            keywords: Vec::new(),
            parent: None,
            line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            byte_offset: node.start_byte(),
            byte_length: node.end_byte() - node.start_byte(),
            content_hash: c_hash,
        });
    }

    // C preprocessor #define macros
    if node.kind() == "preproc_def" {
        let name_node = node.child_by_field_name("name")?;
        let name = node_text(name_node, source_bytes);
        if !is_upper_case_name(&name) {
            return None;
        }
        let sig = node_text(node, source_bytes);
        let sig = if sig.len() > 100 { &sig[..100] } else { &sig };
        let const_bytes = &source_bytes[node.start_byte()..node.end_byte()];
        let c_hash = compute_content_hash(const_bytes);
        return Some(Symbol {
            id: make_symbol_id(filename, &name, "constant"),
            file: filename.to_string(),
            name: name.clone(),
            qualified_name: name,
            kind: "constant".to_string(),
            language: language.to_string(),
            signature: sig.to_string(),
            docstring: String::new(),
            summary: String::new(),
            decorators: Vec::new(),
            keywords: Vec::new(),
            parent: None,
            line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            byte_offset: node.start_byte(),
            byte_length: node.end_byte() - node.start_byte(),
            content_hash: c_hash,
        });
    }

    // Swift: property_declaration with let binding
    if node.kind() == "property_declaration" {
        let mut binding = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "value_binding_pattern" {
                binding = Some(child);
                break;
            }
        }
        let binding = binding?;
        let mutability = binding.child_by_field_name("mutability")?;
        if node_text(mutability, source_bytes) != "let" {
            return None;
        }
        let pattern = node.child_by_field_name("name")?;
        let name_node = pattern
            .child_by_field_name("bound_identifier")
            .or_else(|| {
                let mut c = pattern.walk();
                pattern
                    .children(&mut c)
                    .find(|ch| ch.kind() == "simple_identifier")
            })?;
        let name = node_text(name_node, source_bytes);
        if !is_upper_case_name(&name) {
            return None;
        }
        let sig = node_text(node, source_bytes);
        let sig = if sig.len() > 100 { &sig[..100] } else { &sig };
        let const_bytes = &source_bytes[node.start_byte()..node.end_byte()];
        let c_hash = compute_content_hash(const_bytes);
        return Some(Symbol {
            id: make_symbol_id(filename, &name, "constant"),
            file: filename.to_string(),
            name: name.clone(),
            qualified_name: name,
            kind: "constant".to_string(),
            language: language.to_string(),
            signature: sig.to_string(),
            docstring: String::new(),
            summary: String::new(),
            decorators: Vec::new(),
            keywords: Vec::new(),
            parent: None,
            line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            byte_offset: node.start_byte(),
            byte_length: node.end_byte() - node.start_byte(),
            content_hash: c_hash,
        });
    }

    None
}

fn disambiguate_overloads(symbols: &mut [Symbol]) {
    let mut id_counts: HashMap<String, usize> = HashMap::new();
    for sym in symbols.iter() {
        *id_counts.entry(sym.id.clone()).or_insert(0) += 1;
    }

    let duplicated: std::collections::HashSet<String> = id_counts
        .into_iter()
        .filter(|&(_, count)| count > 1)
        .map(|(id, _)| id)
        .collect();

    if duplicated.is_empty() {
        return;
    }

    let mut ordinals: HashMap<String, usize> = HashMap::new();
    for sym in symbols.iter_mut() {
        if duplicated.contains(&sym.id) {
            let ordinal = ordinals.entry(sym.id.clone()).or_insert(0);
            *ordinal += 1;
            sym.id = format!("{}~{}", sym.id, ordinal);
        }
    }
}

// Elixir extraction

fn parse_elixir_symbols(source_bytes: &[u8], filename: &str) -> Vec<Symbol> {
    let Some(tree) = parse_tree("elixir", source_bytes) else {
        return Vec::new();
    };
    let mut symbols = Vec::new();
    walk_elixir(tree.root_node(), source_bytes, filename, &mut symbols, None);
    symbols
}

fn walk_elixir(
    node: Node,
    source_bytes: &[u8],
    filename: &str,
    symbols: &mut Vec<Symbol>,
    parent: Option<&Symbol>,
) {
    // Only process `call` nodes (all Elixir definitions are macro calls)
    if node.kind() == "call"
        && let Some(first_child) = node.child(0)
        && first_child.kind() == "identifier"
    {
        let macro_name = node_text(first_child, source_bytes);
        match macro_name.as_str() {
            "defmodule" => {
                if let Some(sym) = extract_elixir_module(node, source_bytes, filename, parent) {
                    symbols.push(sym.clone());
                    if let Some(do_block) = find_child_by_kind(node, "do_block") {
                        let mut cursor = do_block.walk();
                        for child in do_block.children(&mut cursor) {
                            walk_elixir(child, source_bytes, filename, symbols, Some(&sym));
                        }
                    }
                }
                return;
            }
            "def" | "defp" | "defmacro" | "defmacrop" | "defdelegate" | "defguard"
            | "defguardp" => {
                if let Some(sym) =
                    extract_elixir_function(node, source_bytes, filename, parent, &macro_name)
                {
                    symbols.push(sym);
                }
                return;
            }
            "defstruct" | "defimpl" | "defprotocol" => {
                if let Some(sym) =
                    extract_elixir_type(node, source_bytes, filename, parent, &macro_name)
                {
                    let sym_clone = sym.clone();
                    symbols.push(sym);
                    if macro_name == "defimpl"
                        && let Some(do_block) = find_child_by_kind(node, "do_block")
                    {
                        let mut cursor = do_block.walk();
                        for child in do_block.children(&mut cursor) {
                            walk_elixir(child, source_bytes, filename, symbols, Some(&sym_clone));
                        }
                    }
                }
                return;
            }
            _ => {}
        }
    }

    // Recurse into children for non-definition nodes
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_elixir(child, source_bytes, filename, symbols, parent);
    }
}

fn extract_elixir_module(
    node: Node,
    source_bytes: &[u8],
    filename: &str,
    parent: Option<&Symbol>,
) -> Option<Symbol> {
    let args = find_child_by_kind(node, "arguments")?;
    let alias = find_child_by_kind(args, "alias")?;
    let name = node_text(alias, source_bytes);

    let qualified_name = if let Some(p) = parent {
        format!("{}.{}", p.qualified_name, name)
    } else {
        name.clone()
    };

    let signature = format!("defmodule {}", name);
    let docstring = extract_elixir_docstring(node, source_bytes, "moduledoc")
        .or_else(|| extract_elixir_docstring(node, source_bytes, "doc"))
        .unwrap_or_default();

    let c_hash = compute_content_hash(&source_bytes[node.start_byte()..node.end_byte()]);

    Some(Symbol {
        id: make_symbol_id(filename, &qualified_name, "class"),
        file: filename.to_string(),
        name,
        qualified_name,
        kind: "class".to_string(),
        language: "elixir".to_string(),
        signature,
        docstring,
        summary: String::new(),
        decorators: Vec::new(),
        keywords: Vec::new(),
        parent: parent.map(|p| p.id.clone()),
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        byte_offset: node.start_byte(),
        byte_length: node.end_byte() - node.start_byte(),
        content_hash: c_hash,
    })
}

fn extract_elixir_function(
    node: Node,
    source_bytes: &[u8],
    filename: &str,
    parent: Option<&Symbol>,
    macro_name: &str,
) -> Option<Symbol> {
    let args = find_child_by_kind(node, "arguments")?;

    // Name extraction: first arg is a call node (function head) or binary_operator (defguard)
    let name = extract_elixir_def_name(args, source_bytes)?;

    let (kind, qualified_name) = if let Some(p) = parent {
        (
            "method".to_string(),
            format!("{}.{}", p.qualified_name, name),
        )
    } else {
        ("function".to_string(), name.clone())
    };

    // Signature: macro_name + the arguments text (up to do block)
    let args_text = node_text(args, source_bytes);
    // Strip trailing do: ... or keyword list from inline defs
    let sig_text = args_text.split(", do:").next().unwrap_or(&args_text);
    let signature = format!("{} {}", macro_name, sig_text);

    let docstring = extract_elixir_docstring(node, source_bytes, "doc").unwrap_or_default();
    let c_hash = compute_content_hash(&source_bytes[node.start_byte()..node.end_byte()]);

    Some(Symbol {
        id: make_symbol_id(filename, &qualified_name, &kind),
        file: filename.to_string(),
        name,
        qualified_name,
        kind,
        language: "elixir".to_string(),
        signature,
        docstring,
        summary: String::new(),
        decorators: Vec::new(),
        keywords: Vec::new(),
        parent: parent.map(|p| p.id.clone()),
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        byte_offset: node.start_byte(),
        byte_length: node.end_byte() - node.start_byte(),
        content_hash: c_hash,
    })
}

fn extract_elixir_type(
    node: Node,
    source_bytes: &[u8],
    filename: &str,
    parent: Option<&Symbol>,
    macro_name: &str,
) -> Option<Symbol> {
    let args = find_child_by_kind(node, "arguments")?;

    let name = if macro_name == "defstruct" {
        "defstruct".to_string()
    } else {
        // defimpl/defprotocol: first arg is an alias
        find_child_by_kind(args, "alias")
            .map(|a| node_text(a, source_bytes))
            .unwrap_or_else(|| macro_name.to_string())
    };

    let qualified_name = if let Some(p) = parent {
        format!("{}.{}", p.qualified_name, name)
    } else {
        name.clone()
    };

    let args_text = node_text(args, source_bytes);
    let signature = format!("{} {}", macro_name, args_text);

    let c_hash = compute_content_hash(&source_bytes[node.start_byte()..node.end_byte()]);

    Some(Symbol {
        id: make_symbol_id(filename, &qualified_name, "type"),
        file: filename.to_string(),
        name,
        qualified_name,
        kind: "type".to_string(),
        language: "elixir".to_string(),
        signature,
        docstring: String::new(),
        summary: String::new(),
        decorators: Vec::new(),
        keywords: Vec::new(),
        parent: parent.map(|p| p.id.clone()),
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        byte_offset: node.start_byte(),
        byte_length: node.end_byte() - node.start_byte(),
        content_hash: c_hash,
    })
}

fn extract_elixir_def_name(args: Node, source_bytes: &[u8]) -> Option<String> {
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        match child.kind() {
            // def create_user(attrs) — the function head is a call node
            "call" => {
                if let Some(id) = child.child(0)
                    && id.kind() == "identifier"
                {
                    return Some(node_text(id, source_bytes));
                }
            }
            // def to_string(account), do: ... — identifier directly
            "identifier" => return Some(node_text(child, source_bytes)),
            // defguard is_admin(user) when ... — binary_operator wrapping a call
            "binary_operator" => {
                if let Some(call) = find_child_by_kind(child, "call")
                    && let Some(id) = call.child(0)
                    && id.kind() == "identifier"
                {
                    return Some(node_text(id, source_bytes));
                }
            }
            _ => {}
        }
    }
    None
}

/// Look for @doc or @moduledoc attribute preceding this node in the do_block.
fn extract_elixir_docstring(node: Node, source_bytes: &[u8], attr_name: &str) -> Option<String> {
    // For modules, look inside the do_block for @moduledoc
    if attr_name == "moduledoc" {
        let do_block = find_child_by_kind(node, "do_block")?;
        let mut cursor = do_block.walk();
        for child in do_block.children(&mut cursor) {
            if child.kind() == "unary_operator"
                && let Some(doc) = try_extract_elixir_attr(child, source_bytes, attr_name)
            {
                return Some(doc);
            }
        }
        return None;
    }

    // For functions, look at preceding siblings for @doc
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        if p.kind() == "unary_operator" {
            if let Some(doc) = try_extract_elixir_attr(p, source_bytes, attr_name) {
                return Some(doc);
            }
            break; // Only check the immediately preceding attribute
        }
        if p.kind() != "comment" {
            break;
        }
        prev = p.prev_named_sibling();
    }
    None
}

fn try_extract_elixir_attr(node: Node, source_bytes: &[u8], attr_name: &str) -> Option<String> {
    // unary_operator -> call -> identifier (should match attr_name)
    let call = find_child_by_kind(node, "call")?;
    let id = call.child(0)?;
    if id.kind() != "identifier" || node_text(id, source_bytes) != attr_name {
        return None;
    }

    let args = find_child_by_kind(call, "arguments")?;
    let string_node = find_child_by_kind(args, "string")?;

    // Find quoted_content inside the string
    if let Some(content) = find_child_by_kind(string_node, "quoted_content") {
        return Some(node_text(content, source_bytes));
    }

    // Fallback: strip triple quotes from the string text
    let text = node_text(string_node, source_bytes);
    Some(strip_quotes(&text))
}

fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|c| c.kind() == kind)
}

// Erlang extraction

fn parse_erlang_symbols(source_bytes: &[u8], filename: &str) -> Vec<Symbol> {
    let Some(spec) = LANGUAGE_REGISTRY.get("erlang") else {
        return Vec::new();
    };
    let Some(tree) = parse_tree("erlang", source_bytes) else {
        return Vec::new();
    };

    // First pass: extract module name from module_attribute
    let mut module_name = None;
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "module_attribute"
            && let Some(atom) = find_child_by_kind(child, "atom")
        {
            module_name = Some(node_text(atom, source_bytes));
        }
    }

    // Second pass: extract symbols using standard spec-based walk
    let mut symbols = Vec::new();
    walk_tree(
        root,
        spec,
        source_bytes,
        filename,
        "erlang",
        &mut symbols,
        None,
        &[],
        0,
    );

    // Qualify function names with module name
    if let Some(ref module) = module_name {
        for sym in &mut symbols {
            if sym.kind == "function" {
                sym.qualified_name = format!("{}.{}", module, sym.name);
                sym.id = make_symbol_id(filename, &sym.qualified_name, &sym.kind);
            }
        }
    }

    symbols
}

// C++ helpers

fn nearest_cpp_template_wrapper(node: Node) -> Option<Node> {
    let mut current = node;
    let mut wrapper = None;
    while let Some(parent) = current.parent() {
        if parent.kind() == "template_declaration" {
            wrapper = Some(parent);
            current = parent;
        } else {
            break;
        }
    }
    wrapper
}

fn is_cpp_type_container(node: Node) -> bool {
    matches!(
        node.kind(),
        "class_specifier" | "struct_specifier" | "union_specifier"
    )
}

fn is_cpp_function_declaration(node: Node) -> bool {
    if node.kind() != "declaration" && node.kind() != "field_declaration" {
        return true;
    }
    if let Some(declarator) = node.child_by_field_name("declarator") {
        has_function_declarator(declarator)
    } else {
        false
    }
}

fn has_function_declarator(node: Node) -> bool {
    if node.kind() == "function_declarator" || node.kind() == "abstract_function_declarator" {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() && has_function_declarator(child) {
            return true;
        }
    }
    false
}

fn extract_cpp_namespace_name(node: Node, source_bytes: &[u8]) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = node_text(name_node, source_bytes);
        if !name.is_empty() {
            return Some(name);
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "namespace_identifier" || child.kind() == "identifier" {
            let name = node_text(child, source_bytes);
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn looks_like_cpp_header(source_bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(source_bytes);
    let cpp_markers = [
        "namespace ",
        "class ",
        "template<",
        "template <",
        "constexpr",
        "noexcept",
        "[[",
        "std::",
        "using ",
        "::",
        "public:",
        "private:",
        "protected:",
        "operator",
        "typename",
    ];
    cpp_markers.iter().any(|marker| text.contains(marker))
}

fn count_error_nodes(node: Node) -> usize {
    let mut count = if node.kind() == "ERROR" { 1 } else { 0 };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_error_nodes(child);
    }
    count
}

/// Helper to extract text from a node
fn node_text(node: Node, source_bytes: &[u8]) -> String {
    String::from_utf8_lossy(&source_bytes[node.start_byte()..node.end_byte()])
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elixir_module_with_functions() {
        let code = r#"
defmodule MyApp.Accounts do
  @doc """
  Creates a user.
  """
  def create_user(attrs \\ %{}) do
    Repo.insert(attrs)
  end

  defp validate(changeset) do
    changeset
  end
end
"#;
        let symbols = parse_file(code, "lib/accounts.ex", "elixir");

        let module = symbols.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(module.name, "MyApp.Accounts");
        assert_eq!(module.signature, "defmodule MyApp.Accounts");

        let create = symbols.iter().find(|s| s.name == "create_user").unwrap();
        assert_eq!(create.kind, "method");
        assert_eq!(create.qualified_name, "MyApp.Accounts.create_user");
        assert!(create.signature.starts_with("def create_user"));
        assert!(create.docstring.contains("Creates a user."));

        let validate = symbols.iter().find(|s| s.name == "validate").unwrap();
        assert_eq!(validate.kind, "method");
        assert_eq!(validate.qualified_name, "MyApp.Accounts.validate");
        assert!(validate.signature.starts_with("defp validate"));
    }

    #[test]
    fn test_elixir_defstruct_and_defimpl() {
        let code = r#"
defmodule User do
  defstruct [:name, :email]

  defimpl String.Chars, for: __MODULE__ do
    def to_string(user), do: user.name
  end
end
"#;
        let symbols = parse_file(code, "lib/user.ex", "elixir");

        assert!(
            symbols
                .iter()
                .any(|s| s.kind == "type" && s.name == "defstruct")
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.kind == "type" && s.name == "String.Chars")
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.kind == "method" && s.name == "to_string")
        );
    }

    #[test]
    fn test_elixir_defmacro_defdelegate_defguard() {
        let code = r#"
defmodule MyApp do
  defmacro __using__(opts) do
    quote do: use(MyApp.Base)
  end

  defdelegate count(list), to: Enum

  defguard is_admin(user) when user.role == :admin
end
"#;
        let symbols = parse_file(code, "lib/my_app.ex", "elixir");

        let mac = symbols.iter().find(|s| s.name == "__using__").unwrap();
        assert_eq!(mac.kind, "method");

        let deleg = symbols.iter().find(|s| s.name == "count").unwrap();
        assert_eq!(deleg.kind, "method");

        let guard = symbols.iter().find(|s| s.name == "is_admin").unwrap();
        assert_eq!(guard.kind, "method");
    }

    #[test]
    fn test_elixir_moduledoc() {
        let code = r#"
defmodule MyApp.Accounts do
  @moduledoc """
  Handles accounts.
  """
end
"#;
        let symbols = parse_file(code, "lib/accounts.ex", "elixir");
        let module = symbols.iter().find(|s| s.kind == "class").unwrap();
        assert!(module.docstring.contains("Handles accounts."));
    }

    #[test]
    fn test_erlang_functions_and_types() {
        let code = r#"
-module(my_server).
-export([start_link/1, init/1]).

-type state() :: #{count => integer()}.

-record(config, {port, host}).

%% Starts the server.
start_link(Args) ->
    gen_server:start_link(?MODULE, Args, []).

init(Args) ->
    {ok, #config{port=8080}}.
"#;
        let symbols = parse_file(code, "src/my_server.erl", "erlang");

        let start_link = symbols.iter().find(|s| s.name == "start_link").unwrap();
        assert_eq!(start_link.kind, "function");
        assert_eq!(start_link.qualified_name, "my_server.start_link");
        assert!(start_link.docstring.contains("Starts the server."));

        let init = symbols.iter().find(|s| s.name == "init").unwrap();
        assert_eq!(init.kind, "function");
        assert_eq!(init.qualified_name, "my_server.init");

        let state_type = symbols.iter().find(|s| s.name == "state").unwrap();
        assert_eq!(state_type.kind, "type");

        let record = symbols.iter().find(|s| s.name == "config").unwrap();
        assert_eq!(record.kind, "type");
    }

    #[test]
    fn test_erlang_comment_markers() {
        assert_eq!(
            clean_comment_markers("%% @doc Starts the server."),
            "@doc Starts the server."
        );
        assert_eq!(clean_comment_markers("% simple comment"), "simple comment");
    }
}
