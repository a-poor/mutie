from trees.binary_trees.avl_tree import AVLTree


def test_left_rotation_updates_public_root():
    tree = AVLTree()
    for key in [1, 0, 3, 2, 4, 5]:
        tree.insert(key, str(key))
    assert tree.root.key == 3
    assert tree.root.parent is None


def test_left_rotation_reparents_moved_subtree():
    tree = AVLTree()
    for key in [1, 0, 3, 2, 4, 5]:
        tree.insert(key, str(key))
    assert tree.root.key == 3
    parent = tree.search(1)
    moved = tree.search(2)
    assert parent.right is moved
    assert moved.parent is parent
