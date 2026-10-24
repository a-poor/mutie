from trees.binary_trees.avl_tree import AVLTree


def test_deletion_updates_ancestor_heights():
    tree = AVLTree()
    for key in (2, 1, 3):
        tree.insert(key, str(key))
    tree.delete(1)
    tree.delete(3)
    assert tree.root.key == 2
    assert tree.root.left is None
    assert tree.root.right is None
    assert tree.root.height == 0
