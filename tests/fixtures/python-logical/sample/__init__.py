def both(a, b):
    return a and b


def either(a, b):
    return a or b


def nested(a, b, c):
    return a and b or c


def weak_default(value, fallback):
    return value or fallback


def weak_both(a, b):
    return a and b
