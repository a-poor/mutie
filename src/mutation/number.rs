//! Bounded integer/float constant edits in value-expression contexts.
use tree_sitter::{Node, Parser};

fn signed_literal(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() != "unary_expression"
        || !matches!(node.child_by_field_name("operator")?.kind(), "+" | "-")
    {
        return None;
    }
    let operand = node.child_by_field_name("operand")?;
    matches!(operand.kind(), "int_literal" | "float_literal").then_some(operand)
}

pub(super) fn replacement(node: Node<'_>, source: &str) -> Option<&'static str> {
    let literal = if matches!(node.kind(), "int_literal" | "float_literal") {
        if node.parent().and_then(signed_literal).is_some() {
            return None;
        }
        node
    } else {
        signed_literal(node)?
    };
    if !super::literal::value_context(node) {
        return None;
    }
    let token = source[literal.byte_range()].replace('_', "");
    let float = literal.kind() == "float_literal";
    let hex = token.starts_with("0x") || token.starts_with("0X");
    let zero = if float {
        let (mantissa, exponent) = token
            .split_once(if hex { ['p', 'P'] } else { ['e', 'E'] })
            .unwrap_or((&token, "0"));
        let mantissa = if hex { mantissa.get(2..)? } else { mantissa };
        let digits = mantissa.replace('.', "");
        let zero = digits.bytes().all(|b| b == b'0');
        if !zero {
            // Go constants are exact. A tiny nonzero constant must not use the
            // zero recipe merely because a host float parser underflows. Skip
            // values that could round to zero at Go's narrowest float width.
            if hex {
                if !hex_nonzero_at_f32(mantissa, exponent)? {
                    return None;
                }
            } else if token.parse::<f32>().ok()? == 0.0 {
                return None;
            }
        }
        zero
    } else {
        let digits = if token.starts_with("0x")
            || token.starts_with("0X")
            || token.starts_with("0o")
            || token.starts_with("0O")
            || token.starts_with("0b")
            || token.starts_with("0B")
        {
            &token[2..]
        } else {
            &token
        };
        digits.bytes().all(|b| b == b'0')
    };
    Some(match (float, zero) {
        (false, true) => "1",
        (false, false) => "0",
        (true, true) => "1.0",
        (true, false) => "0.0",
    })
}

fn hex_nonzero_at_f32(mantissa: &str, exponent: &str) -> Option<bool> {
    let exponent = exponent.parse::<i64>().ok()?;
    let fractional = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    let digits = mantissa.replace('.', "");
    let digits = digits.trim_start_matches('0');
    let first = digits.chars().next()?.to_digit(16)?;
    let top = i64::try_from(digits.len().checked_sub(1)?)
        .ok()?
        .checked_mul(4)?
        .checked_add(i64::from(31 - first.leading_zeros()))?
        .checked_add(exponent)?
        .checked_sub(i64::try_from(fractional).ok()?.checked_mul(4)?)?;
    // Half the smallest f32 subnormal is 2^-150; a tie rounds to zero.
    Some(
        top > -150
            || (top == -150
                && (!first.is_power_of_two() || digits[1..].bytes().any(|b| b != b'0'))),
    )
}

pub(crate) fn valid_replacement(original: &str, changed: &str) -> bool {
    let prefix = "package p\nvar value = ";
    let source = format!("{prefix}{original}\n");
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
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
        && replacement(node, &source) == Some(changed)
}

#[cfg(test)]
#[path = "number_tests.rs"]
mod tests;
