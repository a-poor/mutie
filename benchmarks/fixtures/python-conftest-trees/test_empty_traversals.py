"""Public empty-tree boundaries, added only to the separate review copy."""
import pytest
from trees.binary_trees.avl_tree import AVLTree
from trees.binary_trees.binary_search_tree import BinarySearchTree
from trees.binary_trees import traversal


@pytest.mark.parametrize('tree_type', [BinarySearchTree, AVLTree])
@pytest.mark.parametrize('walk', [traversal.inorder_traverse,
                                  traversal.preorder_traverse,
                                  traversal.postorder_traverse,
                                  traversal.reverse_inorder_traverse])
def test_default_traversal_of_empty_tree(tree_type, walk):
    try:
        pairs = list(walk(tree_type()))
    except RuntimeError as error:
        pytest.fail(f'Empty traversal must finish normally: {error}')
    assert pairs == []


def test_avl_empty_before_and_after_insert(basic_tree):
    tree = AVLTree()
    assert tree.empty is True
    key, data = basic_tree[0]
    tree.insert(key=key, data=data)
    assert tree.empty is False
