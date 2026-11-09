//! Go single-target selector/index stores. No type inference or reference repair.
use tree_sitter::{Node, Parser};

use super::unparenthesized;

pub(super) fn eligible(node: Node<'_>) -> bool {
    node.kind() == "assignment_statement"
        && node.parent().is_some_and(|p| p.kind() == "statement_list")
        && node
            .child_by_field_name("operator")
            .is_some_and(|op| op.kind() == "=")
        && node
            .child_by_field_name("left")
            .and_then(single_expression)
            .is_some_and(|lhs| {
                matches!(
                    unparenthesized(lhs).kind(),
                    "selector_expression" | "index_expression"
                )
            })
        && node
            .child_by_field_name("right")
            .and_then(single_expression)
            .is_some()
}

fn single_expression(list: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = list.walk();
    let mut children = list.named_children(&mut cursor).filter(|n| !n.is_extra());
    let first = children.next()?;
    children.next().is_none().then_some(first)
}

/// Validate the exact standalone recipe; reruns also rediscover the original
/// file, so a valid snippet forged into a header or label is still rejected.
pub(crate) fn valid_replacement(original: &str, replacement: &str) -> bool {
    if !replacement.is_empty() {
        return false;
    }
    let prefix = "package p\nfunc f() {\n";
    let source = format!("{prefix}{original}\n}}\n");
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
    let root = tree.root_node();
    if root.has_error() {
        return false;
    }
    let Some(node) = root.descendant_for_byte_range(prefix.len(), prefix.len() + original.len())
    else {
        return false;
    };
    eligible(node) && node.byte_range() == (prefix.len()..prefix.len() + original.len())
}

#[cfg(test)]
#[path = "assignment_tests.rs"]
mod tests;
