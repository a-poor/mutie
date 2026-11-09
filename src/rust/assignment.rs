//! Standalone field/index assignment deletion, without ownership/type repair.
use super::{Node, Parser, unwrapped_condition};

pub(super) fn eligible(statement: Node<'_>) -> bool {
    if statement.kind() != "expression_statement"
        || statement.parent().is_none_or(|p| p.kind() != "block")
        || statement
            .child(statement.child_count().saturating_sub(1))
            .is_none_or(|n| n.kind() != ";")
    {
        return false;
    }
    let Some(expression) = statement
        .named_children(&mut statement.walk())
        .find(|n| !n.is_extra())
    else {
        return false;
    };
    let assignment = unwrapped_condition(expression);
    if assignment.kind() != "assignment_expression"
        || assignment.child_by_field_name("left").is_none_or(|lhs| {
            !matches!(
                unwrapped_condition(lhs).kind(),
                "field_expression" | "index_expression"
            )
        })
    {
        return false;
    }
    // Require a fully visible subtree, with no nested assignment, suspension,
    // propagation or explicit control transfer, even inside nested closures.
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if (node.kind() == "assignment_expression" && node.id() != assignment.id())
            || matches!(
                node.kind(),
                "compound_assignment_expr"
                    | "await_expression"
                    | "try_expression"
                    | "yield_expression"
                    | "return_expression"
                    | "break_expression"
                    | "continue_expression"
                    | "macro_invocation"
                    | "attribute_item"
                    | "inner_attribute_item"
            )
        {
            return false;
        }
        pending.extend(node.named_children(&mut node.walk()));
    }
    true
}

pub(crate) fn valid_replacement(original: &str, replacement: &str) -> bool {
    if replacement != "{}" {
        return false;
    }
    let prefix = "fn __mutie() { ";
    let source = format!("{prefix}{original} }}");
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
    tree.root_node()
        .named_child(0)
        .and_then(|f| f.child_by_field_name("body"))
        .and_then(|b| b.named_child(0))
        .is_some_and(|statement| {
            statement.byte_range() == (prefix.len()..prefix.len() + original.len())
                && eligible(statement)
        })
}
