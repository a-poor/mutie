# Rust loop fixture

Three loop candidates: one kill, one survivor, one definite-initialization build
failure. Adding `weak(2) == 2` kills the survivor on a focused rerun. The skipped
condition must not earn a kill when its missing assignment prevents compilation.
