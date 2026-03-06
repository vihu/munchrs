use std::collections::HashMap;
use std::sync::LazyLock;

/// Specification for extracting symbols from a language's AST.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LanguageSpec {
    /// tree-sitter language name
    pub ts_language: &'static str,
    /// Node types that represent extractable symbols: node_type -> symbol kind
    pub symbol_node_types: HashMap<&'static str, &'static str>,
    /// How to extract the symbol name: node_type -> child field name
    pub name_fields: HashMap<&'static str, &'static str>,
    /// How to extract parameters: node_type -> child field name
    pub param_fields: HashMap<&'static str, &'static str>,
    /// Return type extraction: node_type -> child field name
    pub return_type_fields: HashMap<&'static str, &'static str>,
    /// Docstring extraction strategy
    pub docstring_strategy: &'static str,
    /// Decorator/attribute node type (if any)
    pub decorator_node_type: Option<&'static str>,
    /// Node types that indicate nesting (methods inside classes)
    pub container_node_types: Vec<&'static str>,
    /// Node types for constants
    pub constant_patterns: Vec<&'static str>,
    /// Node types for type definitions
    pub type_patterns: Vec<&'static str>,
    /// If true, decorators are direct children (e.g. C#)
    pub decorator_from_children: bool,
}

fn hm(pairs: &[(&'static str, &'static str)]) -> HashMap<&'static str, &'static str> {
    pairs.iter().copied().collect()
}

fn python_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "python",
        symbol_node_types: hm(&[
            ("function_definition", "function"),
            ("class_definition", "class"),
        ]),
        name_fields: hm(&[
            ("function_definition", "name"),
            ("class_definition", "name"),
        ]),
        param_fields: hm(&[("function_definition", "parameters")]),
        return_type_fields: hm(&[("function_definition", "return_type")]),
        docstring_strategy: "next_sibling_string",
        decorator_node_type: Some("decorator"),
        container_node_types: vec!["class_definition"],
        constant_patterns: vec!["assignment"],
        type_patterns: vec!["type_alias_statement"],
        decorator_from_children: false,
    }
}

fn javascript_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "javascript",
        symbol_node_types: hm(&[
            ("function_declaration", "function"),
            ("class_declaration", "class"),
            ("method_definition", "method"),
            ("generator_function_declaration", "function"),
        ]),
        name_fields: hm(&[
            ("function_declaration", "name"),
            ("class_declaration", "name"),
            ("method_definition", "name"),
        ]),
        param_fields: hm(&[
            ("function_declaration", "parameters"),
            ("method_definition", "parameters"),
            ("arrow_function", "parameters"),
        ]),
        return_type_fields: HashMap::new(),
        docstring_strategy: "preceding_comment",
        decorator_node_type: None,
        container_node_types: vec!["class_declaration", "class"],
        constant_patterns: vec!["lexical_declaration"],
        type_patterns: vec![],
        decorator_from_children: false,
    }
}

fn typescript_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "typescript",
        symbol_node_types: hm(&[
            ("function_declaration", "function"),
            ("class_declaration", "class"),
            ("method_definition", "method"),
            ("interface_declaration", "type"),
            ("type_alias_declaration", "type"),
            ("enum_declaration", "type"),
        ]),
        name_fields: hm(&[
            ("function_declaration", "name"),
            ("class_declaration", "name"),
            ("method_definition", "name"),
            ("interface_declaration", "name"),
            ("type_alias_declaration", "name"),
            ("enum_declaration", "name"),
        ]),
        param_fields: hm(&[
            ("function_declaration", "parameters"),
            ("method_definition", "parameters"),
            ("arrow_function", "parameters"),
        ]),
        return_type_fields: hm(&[
            ("function_declaration", "return_type"),
            ("method_definition", "return_type"),
            ("arrow_function", "return_type"),
        ]),
        docstring_strategy: "preceding_comment",
        decorator_node_type: Some("decorator"),
        container_node_types: vec!["class_declaration", "class"],
        constant_patterns: vec!["lexical_declaration"],
        type_patterns: vec![
            "interface_declaration",
            "type_alias_declaration",
            "enum_declaration",
        ],
        decorator_from_children: false,
    }
}

fn go_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "go",
        symbol_node_types: hm(&[
            ("function_declaration", "function"),
            ("method_declaration", "method"),
            ("type_declaration", "type"),
        ]),
        name_fields: hm(&[
            ("function_declaration", "name"),
            ("method_declaration", "name"),
            ("type_declaration", "name"),
        ]),
        param_fields: hm(&[
            ("function_declaration", "parameters"),
            ("method_declaration", "parameters"),
        ]),
        return_type_fields: hm(&[
            ("function_declaration", "result"),
            ("method_declaration", "result"),
        ]),
        docstring_strategy: "preceding_comment",
        decorator_node_type: None,
        container_node_types: vec![],
        constant_patterns: vec!["const_declaration"],
        type_patterns: vec!["type_declaration"],
        decorator_from_children: false,
    }
}

fn rust_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "rust",
        symbol_node_types: hm(&[
            ("function_item", "function"),
            ("struct_item", "type"),
            ("enum_item", "type"),
            ("trait_item", "type"),
            ("impl_item", "class"),
            ("type_item", "type"),
        ]),
        name_fields: hm(&[
            ("function_item", "name"),
            ("struct_item", "name"),
            ("enum_item", "name"),
            ("trait_item", "name"),
            ("type_item", "name"),
        ]),
        param_fields: hm(&[("function_item", "parameters")]),
        return_type_fields: hm(&[("function_item", "return_type")]),
        docstring_strategy: "preceding_comment",
        decorator_node_type: Some("attribute_item"),
        container_node_types: vec!["impl_item", "trait_item"],
        constant_patterns: vec!["const_item", "static_item"],
        type_patterns: vec!["struct_item", "enum_item", "trait_item", "type_item"],
        decorator_from_children: false,
    }
}

fn java_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "java",
        symbol_node_types: hm(&[
            ("method_declaration", "method"),
            ("constructor_declaration", "method"),
            ("class_declaration", "class"),
            ("interface_declaration", "type"),
            ("enum_declaration", "type"),
        ]),
        name_fields: hm(&[
            ("method_declaration", "name"),
            ("constructor_declaration", "name"),
            ("class_declaration", "name"),
            ("interface_declaration", "name"),
            ("enum_declaration", "name"),
        ]),
        param_fields: hm(&[
            ("method_declaration", "parameters"),
            ("constructor_declaration", "parameters"),
        ]),
        return_type_fields: hm(&[("method_declaration", "type")]),
        docstring_strategy: "preceding_comment",
        decorator_node_type: Some("marker_annotation"),
        container_node_types: vec![
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
        ],
        constant_patterns: vec!["field_declaration"],
        type_patterns: vec!["interface_declaration", "enum_declaration"],
        decorator_from_children: false,
    }
}

fn php_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "php",
        symbol_node_types: hm(&[
            ("function_definition", "function"),
            ("class_declaration", "class"),
            ("method_declaration", "method"),
            ("interface_declaration", "type"),
            ("trait_declaration", "type"),
            ("enum_declaration", "type"),
        ]),
        name_fields: hm(&[
            ("function_definition", "name"),
            ("class_declaration", "name"),
            ("method_declaration", "name"),
            ("interface_declaration", "name"),
            ("trait_declaration", "name"),
            ("enum_declaration", "name"),
        ]),
        param_fields: hm(&[
            ("function_definition", "parameters"),
            ("method_declaration", "parameters"),
        ]),
        return_type_fields: hm(&[
            ("function_definition", "return_type"),
            ("method_declaration", "return_type"),
        ]),
        docstring_strategy: "preceding_comment",
        decorator_node_type: Some("attribute"),
        container_node_types: vec![
            "class_declaration",
            "trait_declaration",
            "interface_declaration",
        ],
        constant_patterns: vec!["const_declaration"],
        type_patterns: vec![
            "interface_declaration",
            "trait_declaration",
            "enum_declaration",
        ],
        decorator_from_children: false,
    }
}

fn dart_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "dart",
        symbol_node_types: hm(&[
            ("function_signature", "function"),
            ("class_definition", "class"),
            ("mixin_declaration", "class"),
            ("enum_declaration", "type"),
            ("extension_declaration", "class"),
            ("method_signature", "method"),
            ("type_alias", "type"),
        ]),
        name_fields: hm(&[
            ("function_signature", "name"),
            ("class_definition", "name"),
            ("enum_declaration", "name"),
            ("extension_declaration", "name"),
        ]),
        param_fields: hm(&[("function_signature", "parameters")]),
        return_type_fields: HashMap::new(),
        docstring_strategy: "preceding_comment",
        decorator_node_type: Some("annotation"),
        container_node_types: vec![
            "class_definition",
            "mixin_declaration",
            "extension_declaration",
        ],
        constant_patterns: vec![],
        type_patterns: vec!["type_alias", "enum_declaration"],
        decorator_from_children: false,
    }
}

fn csharp_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "csharp",
        symbol_node_types: hm(&[
            ("class_declaration", "class"),
            ("record_declaration", "class"),
            ("interface_declaration", "type"),
            ("enum_declaration", "type"),
            ("struct_declaration", "type"),
            ("delegate_declaration", "type"),
            ("method_declaration", "method"),
            ("constructor_declaration", "method"),
        ]),
        name_fields: hm(&[
            ("class_declaration", "name"),
            ("record_declaration", "name"),
            ("interface_declaration", "name"),
            ("enum_declaration", "name"),
            ("struct_declaration", "name"),
            ("delegate_declaration", "name"),
            ("method_declaration", "name"),
            ("constructor_declaration", "name"),
        ]),
        param_fields: hm(&[
            ("method_declaration", "parameters"),
            ("constructor_declaration", "parameters"),
            ("delegate_declaration", "parameters"),
        ]),
        return_type_fields: hm(&[("method_declaration", "returns")]),
        docstring_strategy: "preceding_comment",
        decorator_node_type: Some("attribute_list"),
        container_node_types: vec![
            "class_declaration",
            "struct_declaration",
            "record_declaration",
            "interface_declaration",
        ],
        constant_patterns: vec![],
        type_patterns: vec![
            "interface_declaration",
            "enum_declaration",
            "struct_declaration",
            "delegate_declaration",
            "record_declaration",
        ],
        decorator_from_children: true,
    }
}

fn c_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "c",
        symbol_node_types: hm(&[
            ("function_definition", "function"),
            ("struct_specifier", "type"),
            ("enum_specifier", "type"),
            ("union_specifier", "type"),
            ("type_definition", "type"),
        ]),
        name_fields: hm(&[
            ("function_definition", "declarator"),
            ("struct_specifier", "name"),
            ("enum_specifier", "name"),
            ("union_specifier", "name"),
            ("type_definition", "declarator"),
        ]),
        param_fields: hm(&[("function_definition", "declarator")]),
        return_type_fields: hm(&[("function_definition", "type")]),
        docstring_strategy: "preceding_comment",
        decorator_node_type: None,
        container_node_types: vec![],
        constant_patterns: vec!["preproc_def"],
        type_patterns: vec![
            "type_definition",
            "enum_specifier",
            "struct_specifier",
            "union_specifier",
        ],
        decorator_from_children: false,
    }
}

fn swift_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "swift",
        symbol_node_types: hm(&[
            ("function_declaration", "function"),
            ("class_declaration", "class"),
            ("protocol_declaration", "type"),
            ("init_declaration", "method"),
        ]),
        name_fields: hm(&[
            ("function_declaration", "name"),
            ("class_declaration", "name"),
            ("protocol_declaration", "name"),
            ("init_declaration", "name"),
        ]),
        param_fields: HashMap::new(),
        return_type_fields: HashMap::new(),
        docstring_strategy: "preceding_comment",
        decorator_node_type: None,
        container_node_types: vec!["class_declaration", "protocol_declaration"],
        constant_patterns: vec!["property_declaration"],
        type_patterns: vec!["protocol_declaration"],
        decorator_from_children: false,
    }
}

fn cpp_spec() -> LanguageSpec {
    LanguageSpec {
        ts_language: "cpp",
        symbol_node_types: hm(&[
            ("class_specifier", "class"),
            ("struct_specifier", "type"),
            ("union_specifier", "type"),
            ("enum_specifier", "type"),
            ("type_definition", "type"),
            ("alias_declaration", "type"),
            ("function_definition", "function"),
            ("declaration", "function"),
            ("field_declaration", "function"),
        ]),
        name_fields: hm(&[
            ("class_specifier", "name"),
            ("struct_specifier", "name"),
            ("union_specifier", "name"),
            ("enum_specifier", "name"),
            ("type_definition", "declarator"),
            ("alias_declaration", "name"),
            ("function_definition", "declarator"),
            ("declaration", "declarator"),
            ("field_declaration", "declarator"),
        ]),
        param_fields: hm(&[
            ("function_definition", "declarator"),
            ("declaration", "declarator"),
            ("field_declaration", "declarator"),
        ]),
        return_type_fields: hm(&[
            ("function_definition", "type"),
            ("declaration", "type"),
            ("field_declaration", "type"),
        ]),
        docstring_strategy: "preceding_comment",
        decorator_node_type: None,
        container_node_types: vec!["class_specifier", "struct_specifier", "union_specifier"],
        constant_patterns: vec!["preproc_def"],
        type_patterns: vec![
            "class_specifier",
            "struct_specifier",
            "union_specifier",
            "enum_specifier",
            "type_definition",
            "alias_declaration",
        ],
        decorator_from_children: false,
    }
}

/// File extension to language mapping.
pub static LANGUAGE_EXTENSIONS: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            (".py", "python"),
            (".js", "javascript"),
            (".jsx", "javascript"),
            (".ts", "typescript"),
            (".tsx", "typescript"),
            (".go", "go"),
            (".rs", "rust"),
            (".java", "java"),
            (".php", "php"),
            (".dart", "dart"),
            (".cs", "csharp"),
            (".c", "c"),
            (".h", "cpp"),
            (".cpp", "cpp"),
            (".cc", "cpp"),
            (".cxx", "cpp"),
            (".hpp", "cpp"),
            (".hh", "cpp"),
            (".hxx", "cpp"),
            (".swift", "swift"),
        ])
    });

/// Language registry mapping language names to their specs.
pub static LANGUAGE_REGISTRY: LazyLock<HashMap<&'static str, LanguageSpec>> = LazyLock::new(|| {
    HashMap::from([
        ("python", python_spec()),
        ("javascript", javascript_spec()),
        ("typescript", typescript_spec()),
        ("go", go_spec()),
        ("rust", rust_spec()),
        ("java", java_spec()),
        ("php", php_spec()),
        ("dart", dart_spec()),
        ("csharp", csharp_spec()),
        ("c", c_spec()),
        ("swift", swift_spec()),
        ("cpp", cpp_spec()),
    ])
});
