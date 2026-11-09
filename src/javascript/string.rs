//! Ordinary string values only. Discovery never evaluates source text.
use tree_sitter::{Node, Parser};

fn value_context(node: Node<'_>) -> bool {
    let mut expression = node;
    while let Some(parent) = expression.parent() {
        if parent.kind() != "parenthesized_expression" {
            // Conservatively exclude all standalone string statements, including
            // directive prologues and strings outside a directive position.
            if parent.kind() == "expression_statement" {
                return false;
            }
            break;
        }
        expression = parent;
    }
    let mut child = node;
    while let Some(parent) = child.parent() {
        if super::type_only(parent.kind())
            || matches!(
                parent.kind(),
                "computed_property_name"
                    | "import_statement"
                    | "export_clause"
                    | "namespace_export"
                    | "import_attribute"
                    | "import_attributes"
                    | "template_string"
            )
        {
            return false;
        }
        if parent.kind() == "export_statement"
            && parent
                .child_by_field_name("source")
                .is_some_and(|n| n.id() == child.id())
        {
            return false;
        }
        if parent.kind() == "call_expression"
            && parent
                .child_by_field_name("function")
                .is_some_and(|n| n.kind() == "import")
        {
            return false;
        }
        if matches!(
            parent.kind(),
            "pair"
                | "pair_pattern"
                | "method_definition"
                | "field_definition"
                | "public_field_definition"
                | "property_signature"
                | "method_signature"
        ) && ["key", "name", "property"].iter().any(|name| {
            parent
                .child_by_field_name(name)
                .is_some_and(|n| n.id() == child.id())
        }) {
            return false;
        }
        child = parent;
    }
    true
}

// Only emptiness matters. Every valid character/escape emits at least one UTF-16
// code unit except a line continuation. Validate strict-mode escape forms without
// decoding into Rust strings (which cannot represent lone UTF-16 surrogates).
fn nonempty(token: &str) -> Option<bool> {
    let quote = token.chars().next()?;
    if !matches!(quote, '\'' | '"') || token.len() < 2 || !token.ends_with(quote) {
        return None;
    }
    let mut chars = token[1..token.len() - 1].chars().peekable();
    let mut nonempty = false;
    while let Some(c) = chars.next() {
        if c == quote || matches!(c, '\r' | '\n') {
            return None;
        }
        if c == '\\' {
            match chars.next()? {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    continue;
                }
                '\n' | '\u{2028}' | '\u{2029}' => continue,
                '0' if chars.peek().is_some_and(char::is_ascii_digit) => return None,
                '1'..='9' => return None,
                'x' => {
                    for _ in 0..2 {
                        if !chars.next()?.is_ascii_hexdigit() {
                            return None;
                        }
                    }
                }
                'u' if chars.peek() == Some(&'{') => {
                    chars.next();
                    let mut value = 0u32;
                    let mut digits = 0;
                    loop {
                        let c = chars.next()?;
                        if c == '}' {
                            break;
                        }
                        value = value.checked_mul(16)?.checked_add(c.to_digit(16)?)?;
                        if value > 0x10ffff {
                            return None;
                        }
                        digits += 1;
                    }
                    if digits == 0 {
                        return None;
                    }
                }
                'u' => {
                    for _ in 0..4 {
                        if !chars.next()?.is_ascii_hexdigit() {
                            return None;
                        }
                    }
                }
                _ => {}
            }
        }
        nonempty = true;
    }
    Some(nonempty)
}

pub(super) fn replacement(node: Node<'_>, source: &str) -> Option<&'static str> {
    if node.kind() != "string" || !value_context(node) {
        return None;
    }
    Some(if nonempty(&source[node.byte_range()])? {
        "(\"\")"
    } else {
        "(\"x\")"
    })
}

pub(super) fn valid_replacement(original: &str, changed: &str, typescript: bool) -> bool {
    let prefix = "const value = ";
    let source = format!("{prefix}{original};");
    let mut parser = Parser::new();
    let language = if typescript {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    } else {
        tree_sitter_javascript::LANGUAGE
    };
    if parser.set_language(&language.into()).is_err() {
        return false;
    }
    let Ok(tree) = super::parse(&mut parser, &source) else {
        return false;
    };
    let Some(node) = tree
        .root_node()
        .descendant_for_byte_range(prefix.len(), prefix.len() + original.len())
    else {
        return false;
    };
    node.byte_range() == (prefix.len()..prefix.len() + original.len())
        && replacement(node, &source) == Some(changed)
}
