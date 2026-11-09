//! One value change per runtime literal; never evaluate source to discover it.
use tree_sitter::{Node, Parser};

fn signed_number<'tree>(node: Node<'tree>, source: &str) -> Option<Node<'tree>> {
    if node.kind() != "unary_expression" {
        return None;
    }
    let operator = node.child_by_field_name("operator")?;
    if !matches!(&source[operator.byte_range()], "+" | "-") {
        return None;
    }
    node.child_by_field_name("argument")
        .filter(|n| n.kind() == "number")
}

pub(super) fn replacement(node: Node<'_>, source: &str) -> Option<&'static str> {
    let literal = if node.kind() == "number" {
        // The signed parent owns this site, including unsupported +BigInt.
        if node
            .parent()
            .is_some_and(|p| signed_number(p, source).is_some())
        {
            return None;
        }
        node
    } else {
        signed_number(node, source)?
    };
    // Property declarations (including computed names) change API shape. TS
    // types and module syntax are not runtime literal sites in this first slice.
    let mut child = node;
    while let Some(parent) = child.parent() {
        if super::type_only(parent.kind())
            || matches!(
                parent.kind(),
                "computed_property_name"
                    | "import_statement"
                    | "export_clause"
                    | "import_attribute"
                    | "import_attributes"
            )
        {
            return None;
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
                .is_some_and(|key| key.id() == child.id())
        }) {
            return None;
        }
        child = parent;
    }
    let token = source[literal.byte_range()].replace('_', "");
    let bigint = token.ends_with('n');
    if bigint
        && node.kind() == "unary_expression"
        && node
            .child_by_field_name("operator")
            .is_some_and(|op| &source[op.byte_range()] == "+")
    {
        return None;
    }
    let digits = token.strip_suffix('n').unwrap_or(&token);
    let radix = digits.get(..2).and_then(|prefix| match prefix {
        "0x" | "0X" => Some(16),
        "0o" | "0O" => Some(8),
        "0b" | "0B" => Some(2),
        _ => None,
    });
    let zero = if let Some(radix) = radix {
        let body = &digits[2..];
        if body.is_empty() || !body.chars().all(|c| c.is_digit(radix)) {
            return None;
        }
        body.bytes().all(|b| b == b'0')
    } else {
        // Legacy leading-zero forms are not valid in supported ESM packages.
        if digits.starts_with('0') && digits.as_bytes().get(1).is_some_and(u8::is_ascii_digit) {
            return None;
        }
        if bigint {
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            digits.bytes().all(|b| b == b'0')
        } else {
            // IEEE-754 parsing recognizes underflow to zero. Overflow remains a
            // Number (Infinity), which is distinct from the zero replacement.
            digits.parse::<f64>().ok()? == 0.0
        }
    };
    Some(match (bigint, zero) {
        (false, false) => "(0)",
        (false, true) => "(1)",
        (true, false) => "(0n)",
        (true, true) => "(1n)",
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
