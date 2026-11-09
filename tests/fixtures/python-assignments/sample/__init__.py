def checked():
    values = {}
    values['count'] = 1
    return values


def weak(include):
    values = {}
    if include:
        values['count'] = 2
    return values
