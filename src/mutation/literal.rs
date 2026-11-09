//! Shared value-expression boundary for Go numeric and string literals.
use tree_sitter::Node;

pub(super) fn value_context(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "import_declaration"
                | "import_spec"
                | "const_declaration"
                | "const_spec"
                | "type_spec"
                | "type_alias"
                | "type_arguments"
                | "type_elem"
        ) || parent.kind().ends_with("_type")
            || (parent.kind() == "keyed_element"
                && parent
                    .child_by_field_name("key")
                    .is_some_and(|key| key.id() == node.id()))
            || (parent.kind() == "expression_case"
                && parent
                    .child_by_field_name("value")
                    .is_some_and(|value| value.id() == node.id()))
        {
            return false;
        }
        node = parent;
    }
    true
}
