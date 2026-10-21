"""Native binary arithmetic, with a deliberately weak fee test."""
def add(left, right):
    return left() + right()


def subtract(left, right):
    return left() - right()


def multiply(left, right):
    return left() * right()


def divide(left, right):
    return left() / right()


def net(amount, fee):
    return amount - fee
