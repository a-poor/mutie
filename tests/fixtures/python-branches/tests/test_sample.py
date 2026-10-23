from sample import choose, weak

def test_true(): assert choose([1]) == 1
def test_false(): assert choose([]) == 2
def test_weak(): assert weak([1]) == 3
