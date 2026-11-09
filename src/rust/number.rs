//! One canonical value change per runtime literal, without type inference.
use super::{Node, Parser};

fn signed_literal<'tree>(node: Node<'tree>, source: &str) -> Option<Node<'tree>> {
    if node.kind() != "unary_expression" {
        return None;
    }
    let child = node
        .named_children(&mut node.walk())
        .find(|n| !n.is_extra())?;
    if !matches!(child.kind(), "integer_literal" | "float_literal")
        || !source[node.byte_range()].starts_with('-')
    {
        return None;
    }
    Some(child)
}

pub(super) fn replacement(node: Node<'_>, source: &str) -> Option<String> {
    let literal = if matches!(node.kind(), "integer_literal" | "float_literal") {
        if node
            .parent()
            .is_some_and(|p| signed_literal(p, source).is_some())
        {
            return None;
        }
        node
    } else {
        signed_literal(node, source)?
    };
    // Tuple field selectors and struct field names are not value expressions.
    if node.parent().is_some_and(|p| {
        matches!(p.kind(), "field_expression" | "field_initializer")
            && ["field", "name"].iter().any(|field| {
                p.child_by_field_name(field)
                    .is_some_and(|n| n.id() == node.id())
            })
    }) {
        return None;
    }
    let token = &source[literal.byte_range()];
    let radix = match token.get(..2) {
        Some("0x") => Some(16),
        Some("0o") => Some(8),
        Some("0b") => Some(2),
        _ => None,
    };
    let suffix = [
        "usize", "isize", "u128", "i128", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8",
    ]
    .into_iter()
    .find(|suffix| token.ends_with(suffix))
    .or_else(|| {
        (radix.is_none())
            .then(|| ["f32", "f64"].into_iter().find(|s| token.ends_with(s)))
            .flatten()
    })
    .unwrap_or("");
    let digits = token[..token.len() - suffix.len()].replace('_', "");
    let float = suffix.starts_with('f') || literal.kind() == "float_literal";
    let zero = if let Some(radix) = radix {
        if float {
            return None;
        }
        let body = digits.get(2..)?;
        if body.is_empty() || !body.chars().all(|c| c.is_digit(radix)) {
            return None;
        }
        body.bytes().all(|b| b == b'0')
    } else if float {
        if !suffix.is_empty() && !matches!(suffix, "f32" | "f64") {
            return None;
        }
        let small = digits.parse::<f32>().ok()? == 0.0;
        let large = digits.parse::<f64>().ok()? == 0.0;
        match suffix {
            "f32" => small,
            "f64" => large,
            "" if small == large => large,
            // Unsuffixed literals may infer either width. Do not guess the type
            // and emit a replacement that is unchanged at the other width.
            _ => return None,
        }
    } else {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.bytes().all(|b| b == b'0')
    };
    Some(format!(
        "{}{}{}",
        if zero { "1" } else { "0" },
        if float { ".0" } else { "" },
        suffix
    ))
}

pub(crate) fn valid_replacement(original: &str, changed: &str) -> bool {
    let prefix = "fn __mutie() { let value = ";
    let source = format!("{prefix}{original}; }}");
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .is_err()
    {
        return false;
    }
    let Some(tree) = parser.parse(&source, None) else {
        return false;
    };
    if tree.root_node().has_error() {
        return false;
    }
    let Some(node) = tree
        .root_node()
        .descendant_for_byte_range(prefix.len(), prefix.len() + original.len())
    else {
        return false;
    };
    node.byte_range() == (prefix.len()..prefix.len() + original.len())
        && replacement(node, &source).as_deref() == Some(changed)
}
