use tree_sitter::Node;

fn expression(mut node: Node<'_>) -> Option<Node<'_>> {
    while matches!(
        node.kind(),
        "expression_statement" | "parenthesized_expression"
    ) {
        let mut cursor = node.walk();
        let mut children = node
            .named_children(&mut cursor)
            .filter(|n| n.kind() != "comment");
        let child = children.next()?;
        if children.next().is_some() {
            return None;
        }
        node = child;
    }
    Some(node)
}

/// Skip the whole standalone property/index write, including target lookup and
/// RHS/store effects. An empty statement retains ASI and directive boundaries.
pub(super) fn removable(statement: Node<'_>, source: &str) -> bool {
    if statement.kind() != "expression_statement" {
        return false;
    }
    // Avoid class initialization noise, including nested callbacks. This is a
    // syntactic exclusion, not inference about functions later invoked with new.
    let mut ancestor = statement.parent();
    while let Some(node) = ancestor {
        if matches!(
            node.kind(),
            "class_static_block" | "field_definition" | "public_field_definition"
        ) {
            return false;
        }
        if node.kind() == "method_definition"
            && let Some(name) = node.child_by_field_name("name")
            && (source[name.byte_range()].trim_matches(['\'', '"']) == "constructor"
                || source[name.byte_range()].contains('\\'))
        {
            return false;
        }
        ancestor = node.parent();
    }
    let Some(assignment) = expression(statement).filter(|n| n.kind() == "assignment_expression")
    else {
        return false;
    };
    let Some(target) = assignment.child_by_field_name("left").and_then(expression) else {
        return false;
    };
    if !matches!(target.kind(), "member_expression" | "subscript_expression") {
        return false;
    }
    let mut pending = vec![assignment];
    while let Some(node) = pending.pop() {
        if (node.kind() == "assignment_expression" && node != assignment)
            || matches!(
                node.kind(),
                "augmented_assignment_expression"
                    | "update_expression"
                    | "await_expression"
                    | "yield_expression"
                    | "super"
                    | "private_property_identifier"
            )
            || (node.kind() == "optional_chain" && node.end_byte() <= target.end_byte())
        {
            return false;
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    true
}
