"""Native comparison operators; no operand duplication or chain rewriting."""
def less(left, right):
    return left() < right()


def less_equal(left, right):
    return left() <= right()


def greater(left, right):
    return left() > right()


def greater_equal(left, right):
    return left() >= right()


def within_budget(cost, cap):
    return cost <= cap


def chained(a, b, c):
    return a < b <= c
