from sample import positive, negative, fractional, formatting, wrap
def test_positive(): assert positive(7,3) == 1
def test_negative(): assert negative(-7,3) == 2
def test_fractional(): assert fractional(7.5,2.0) == 1.5
def test_formatting(): assert formatting("%d",3) == "3"
def test_wrap_zero(): assert wrap(0,3) == 0
