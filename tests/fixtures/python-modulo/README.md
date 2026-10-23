# Python modulo fixture

Five native `% → *` sites cover positive/negative integers, floats, numeric
string formatting and a zero-input wrap survivor. The five original tests kill
four sites; `wrap(0, 3)` survives because both results are zero. Adding
`assert wrap(7, 3) == 1` kills it in a six-test focused rerun.

This is a syntactic operator: formatting with an integer becomes repetition;
formatting with a string/tuple/mapping can raise a native TypeError. Independent
workflow controls preserve formatting and custom multiplication failures as
errors, not kills. Tests and `%=` assignments are not mutation sites.

Run the opt-in workflows with the pinned CPython/pytest environment:

```sh
MUTIE_PYTHON=/path/to/venv/bin/python cargo test --test python_workflow modulo_ -- --ignored --test-threads=1
```
