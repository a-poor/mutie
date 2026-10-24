from sample import count, weak

def test_count(): assert count(2) == 12

def test_empty(): assert weak(0) == 0
