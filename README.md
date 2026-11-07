# mutie

[![Linux CI](https://github.com/a-poor/mutie/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/a-poor/mutie/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/mutie.svg)](https://crates.io/crates/mutie)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![Status: experimental](https://img.shields.io/badge/status-experimental-orange)](#status)

`mutie` is a mutation testing tool that finds gaps in your tests by making small
changes to your code and checking whether the tests catch them.

It's written in Rust, with [Tree-sitter](https://tree-sitter.github.io/tree-sitter/)
to locate mutations in Go, Rust and JS/TS, and
[reflink-copy](https://docs.rs/reflink-copy/latest/reflink_copy/) for copy-on-write
workspace clones on supported filesystems, falling back to ordinary copies elsewhere.
[Ratatui](https://ratatui.rs/) powers the live terminal view and report browser.

Each mutation runs in a temporary copy of your project. You can inspect saved
diffs and test logs, rerun survivors after improving your tests, and set mutation
score thresholds for CI.

> **Early development:** Mutie works on real projects, but compatibility is still
> limited and breaking changes are expected. Check the [supported scope](SUPPORT.md)
> before trying it on your project.

## Installation

Install from crates.io with a current stable Rust toolchain:

```sh
cargo install mutie --locked
```

To build a source checkout, use `cargo install --path . --locked`.

You'll also need your project's toolchain and dependencies installed. Go requires
1.24 or later; the JS/TS and Python runners currently require specific versions.
See [runner setup and limits](docs/runners.md). macOS and Linux are the target
platforms; Windows is not supported yet.

## Getting started

Create a configuration, preview the mutations, then run them:

```sh
mutie init /path/to/project
mutie /path/to/project --list
mutie /path/to/project
```

Mutie first checks that the original tests pass. A **killed** mutation was caught
by the tests; a **missed** mutation survived and may point to a missing assertion.
Some mutations don't change observable behavior, so a survivor needs review.

Open the saved run, improve your tests, and rerun its survivors:

```sh
mutie report /path/to/project/.mutie/RUN
mutie /path/to/project --rerun-missed /path/to/project/.mutie/RUN
```

Start with a small source selection on larger projects. For example:

```sh
mutie /path/to/project --include 'src/score.rs' --operator comparison_boundary
```

Source filters choose where to mutate; they don't narrow the test suite. Runs can
be expensive because the full suite runs for each mutation by default. See
[configuration and CI options](docs/usage.md) and [reports and review](docs/reports.md).

## Status

Go has the broadest validation. Rust/Cargo, Python/pytest, and JavaScript/TypeScript
through Vitest, Node and Bun have working, validated workflows within their
documented limits. Runtime versions, dependencies and project layouts matter;
the [support guide](SUPPORT.md) has the details and [TODO.md](TODO.md) tracks what's next.

✅ Implemented within the supported scope · ⬜ Not implemented

| Mutation | Go | Rust | Python | JS/TS |
| --- | --- | --- | --- | --- |
| Arithmetic: `+ ↔ -`, `* ↔ /` | ✅ | ✅ | ✅ | ✅ |
| Modulo: `% → *` | ✅ | ✅ | ✅ | ✅ |
| Invert comparisons: `== ↔ !=`, `< ↔ >=`, `<= ↔ >` | ✅ | ✅ | ✅ | ✅ |
| Move comparison boundaries: `< ↔ <=`, `> ↔ >=` | ✅ | ✅ | ✅ | ✅ |
| Swap logical operators: `&& ↔ \|\|`, `and ↔ or` | ✅ | ✅ | ✅ | ✅ |
| Flip Boolean literals | ✅ | ✅ | ✅ | ✅ |
| Remove negation: `!x → x`, `not x → x` | ✅ | ✅ | ✅ | ✅ |
| Force an `if` condition true or false | ✅ | ✅ | ✅ | ✅ |
| Remove a standalone function call | ✅ | ⬜ | ⬜ | ✅ |
| Skip a conditional loop | ✅ | ✅ | ✅ | ✅ |
| Replace numeric or string literals | ⬜ | ⬜ | ⬜ | ⬜ |
| Remove arbitrary statements or blocks | ⬜ | ⬜ | ⬜ | ⬜ |

JS/TS entries apply to all three supported runners. Each operator has syntax and
context restrictions; see [operator details](docs/operators.md).

## Development

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Real-toolchain workflow tests are opt-in. See the [development guide](docs/development.md)
for those tests, [DESIGN.md](DESIGN.md) for architecture, and [docs/](docs/README.md)
for the reference documentation.

## License

Mutie is licensed under either the [MIT license](LICENSE-MIT) or the
[Apache License 2.0](LICENSE-APACHE), at your option.

Retained third-party excerpts keep their upstream licenses. See
[third-party notices](THIRD_PARTY_NOTICES.md) for their sources and license texts.
