def compare(a, b):
    return a == b, a != b, a < b, a <= b, a > b, a >= b


def negate(x):
    return not (x  # Preserve operand trivia and multiline coordinates.
                )


def nested(x):
    return not not x


def weak_compare(a, b):
    return a < b


def weak_negate(x):
    return not x
