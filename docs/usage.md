# Usage and configuration

[Documentation index](README.md) · [Project overview](../README.md)

These commands describe the current source checkout. The published 0.1.0 release
uses the previous CLI and flat config format.

## Commands

Running `mutie` alone prints help. It never starts a mutation run.

```sh
cd /path/to/project
mutie init
mutie list
mutie check
mutie run
mutie rerun /path/to/saved/run --missed
mutie rerun /path/to/saved/run --id 9,11 --dry-run
mutie report /path/to/saved/run
mutie runs /path/to/project/.mutie
mutie prune /path/to/project/.mutie --keep-last 5
mutie dismiss list
```

Commands work in the current directory by default. The global `--path DIR` option
changes that working directory before any command paths are resolved; it works
before or after the subcommand, including nested dismissal commands:

```sh
mutie --path /path/to/project run
mutie list --path /path/to/project
mutie --path /path/to/project rerun .mutie/RUN --missed
mutie dismiss list --path /path/to/project
```

Project directories are no longer positional arguments. Saved-run and report-root
arguments still identify command inputs, relative to the selected working directory.
`list` discovers mutations without running tests or saving reports. `check` validates
the project and runs the same isolated baseline used by `run`, then stops without
mutations or saved reports.
Both commands accept `--json` and use the same configuration as `run`.
Bun listing does not launch Bun; Python listing audits without importing project
code. Other runners may need their language tools for discovery.

`run` and `rerun` accept `--jobs N`, `--timeout SECONDS`, `--plain` or `--json`,
and `--verbose`. `rerun` additionally accepts `--missed`, repeatable/comma-separated
`--id`, and `--dry-run` to preview its saved selection. Source filters, operators,
runner settings, output locations and score policies belong in config.

## Configuration

`mutie init` writes a starting `mutie.toml`. It detects supported tooling
without running tests or installing dependencies. Existing configs are preserved
unless you pass `--force`, which replaces the whole file with generated settings.
Use `--dry-run` to print a proposal without writing, even with `--force`, or
`--non-interactive` for automation. Replacement is atomic and refuses symlinks
or other non-regular files; failed detection leaves the existing config unchanged.
Ambiguous setup needs an explicit language and, for JS/TS, a runner:

```sh
mutie init --lang js --runner vitest --non-interactive
mutie init --lang python --python .venv/bin/python
mutie init --force
```

Mutie reads `./mutie.toml` in the selected working directory; it does not search
parents. Use `--config FILE` to select an alternate file. A relative config path
is also resolved from the working directory selected by `--path`:

```sh
mutie run --config ci/mutie.toml
mutie --path /path/to/project run --config ci/mutie.toml
```

Within `mutie.toml`, use language sections to select project subdirectories, such
as `[go].path = "backend"`. All configured paths are relative to the config file's
directory, including language project paths, Python launchers and output directories.
Include/exclude globs are relative to the selected project path. No shell expansion
is performed.

```toml
jobs = 4
# Omit timeout to use max(10 seconds, 5 × baseline duration).
timeout = 300
baseline_timeout = 300
output_dir = ".mutie"

[go]
path = "backend"
exclude = ["internal/generated/**"]
test_scope = "full"

[js]
path = "frontend"
runner = "vitest"
jobs = 1
operators = ["boolean_flip", "comparison_boundary"]
```

A language section enables that language. `enabled = false` disables it while
retaining its settings. With multiple enabled sections, select one explicitly:

```sh
mutie list --lang go
mutie check --lang js
mutie run --lang go
```

Each invocation still handles **one project through one runner**. The example
illustrates directory selection, not general frontend compatibility: JSX/TSX,
browser environments and JS package workspaces remain unsupported. Multiple
projects of the same language and combined cross-language runs are not implemented.
See [runner limits](runners.md).

Precedence is built-in defaults, top-level config, language section, then the
small set of explicit CLI overrides. Arrays replace inherited arrays. An empty
`include` allows all eligible files; an empty `exclude` adds no exclusions; an empty
`operators` selects all supported operators. Unknown keys, invalid values and
unsupported runner settings are errors, including misspelled keys in disabled sections.
Without language sections, Mutie can infer unambiguous project tooling. Explicit
`--lang go|rust|js|python` narrows selection; `[js].runner` distinguishes JS/TS runners.

## Shared settings

These can appear at the top level or in a language section:

| Setting | Default / meaning |
| --- | --- |
| `jobs` | `1`; mutation workers. JS/TS and Python currently require `1`. |
| `timeout` | Automatic; seconds per mutation. |
| `baseline_timeout` | `300`; seconds for baseline and tool discovery. |
| `include`, `exclude` | Empty arrays; project-relative source globs. Exclusions win. |
| `operators` | Empty array; all supported operators. An explicit list selects only those operators. |
| `test_scope` | `"full"`; `"affected"` is supported only for Go. |
| `max_log_bytes` | Unset; unlimited retained bytes per log. A limit keeps the head and tail. |
| `max_capture_bytes` | Unset for Go/Rust/Vitest/Node; 4 MiB for Bun/Python. Stops commands exceeding combined stdout/stderr budget. |
| `output_dir` | `.mutie` under the selected project. An explicit path is relative to the config file. |
| `fail_on_missed` | `false`; fail completed runs with undismissed survivors. |
| `min_score` | Unset; minimum raw mutation score, 0–100 with up to two decimals. |

Worker counts, timeouts and byte limits must be positive integers. Static validation
and runtime identity probes retain their own fixed internal bounds. Bun/Python always
retain a bounded raw capture budget. Source filters choose mutation locations;
they never remove tests from the audited suite. See [operators](operators.md) for
supported names and behavior.

The baseline always tests the full supported project. Go's opt-in `affected` scope
uses pristine import/test dependencies to select mutation test packages, then
refreshes that selection on focused reruns. Other runners require full-suite testing.

## Language settings

```toml
[rust]
path = "."
packages = ["core"] # workspace members to mutate; [] means all members
build_cache = "worker" # default: "fresh"

[python]
enabled = false
path = "tools/service"
executable = "tools/service/.venv/bin/python"
continue_on_error = 2 # optional; stop at this many verified candidate errors
```

Rust always tests all supported workspace members, even when `packages` narrows
mutation locations. `worker` opts into worker-local Cargo dependency reuse; local
packages are rebuilt and identity checks remain mandatory.

Python's executable is its pinned virtual-environment launcher. Mutie preserves
that launcher symlink rather than resolving it to the base interpreter. Init does
not create/install the environment. `continue_on_error` only permits the existing
verified Python candidate-error continuation; lifecycle, collection and process
failures keep their strict classification and precedence.

`[js]` covers JavaScript and TypeScript. Its `runner` is `"vitest"`, `"node"` or
`"bun"`; omission permits unambiguous inference from audited package metadata.
Package-manager installation and test-runner selection are independent. Arbitrary
test commands, flags and unsupported runtime versions cannot be configured around
inventory or failure-attribution checks.

## Presentation and CI

Presentation is invocation-only; `plain`, `json`, `ui` and `verbose` are not config keys.

```sh
mutie run --plain
mutie run --json > events.jsonl
mutie run --jobs 2 --timeout 30 --verbose
```

With neither flag, runs use the live view on suitable terminals and plain text in
CI or when redirected. `--plain` and `--json` are mutually exclusive. JSON runs and
reruns emit one event per line as work proceeds, followed by a final summary.
`--verbose` includes captured command output in text or JSON results; live views
keep it in the saved logs. Listing/checking never open the live run view and retain
their single-object JSON responses. See [output modes and event fields](output.md)
and [reports and review](reports.md).

Set CI policies in config:

```toml
fail_on_missed = true
min_score = 80.5

[go]
```

A completed run exits `3` when either configured policy fails. Dismissals affect
`fail_on_missed` but never change raw outcomes or the raw mutation score. Filtered
runs score only their selected mutations; reruns evaluate their new selected run.
Errors, interruption, incomplete execution and persistence failures take precedence
and do not yield a passing score.

| Exit code | Meaning |
| --- | --- |
| `0` | Successful command; enabled policies passed. |
| `1` | Configuration, execution, reporting or other operation failure, including handled interruption. |
| `2` | Invalid CLI syntax. |
| `3` | Completed mutation run failed a findings or score policy. |

## Migrating from 0.1.0

There are no legacy command aliases or flat config compatibility mode.

- `mutie PATH` and `mutie run PATH` become `mutie --path PATH run`, or just
  `mutie run` from that directory.
- `mutie PATH --list` and runner-specific `*-list` become `mutie list`
  (use global `--path PATH` when needed).
- Runner-specific `*-check` become `mutie check`.
- `--rerun RUN` becomes `mutie rerun RUN`; `--rerun-missed RUN` becomes
  `mutie rerun RUN --missed`. Preview with `--dry-run`.
- Replace `lang = "go"` with `[go]`, and `lang = "node"`/`"bun"`/`"vitest"`
  with `[js]` and its `runner` field.
- Move `rust_packages` to `[rust].packages`, `rust_build_cache` to
  `[rust].build_cache`, and `python` to `[python].executable`.
- Remove config `ui`/`verbose`. Replace `--ui plain` with `--plain`; omit the flag
  for automatic terminal selection. Use `--json` for machine-readable output.
- Replace source/operator/policy flags and `--clear-*` flags with config edits or
  an alternate `--config FILE`. `mutie init --dry-run` previews the new format.
- Dismissal project selection uses `--path` and accepts `--config`/`--lang`.

Saved reports keep their schema and remain readable. Reruns validate source and
execution identity as before. Saved runner, Python launcher and Rust member
selection provide defaults when not set in current config; current settings and
runtime drift still undergo validation. A moved checkout can be supplied as the
optional rerun project path or through the report browser's `--module` option.

## Isolation and limits

Mutation workspaces are temporary copies, using copy-on-write where supported
and ordinary copies otherwise. Workers restore from a pristine snapshot between
mutations. Report directories are excluded from execution copies, and exclusions
cannot hide audited source or tests. Originals and prior reports remain unchanged.

Captured output is classified before optional log retention. Capture overflow,
timeout and cancellation stop owned process groups and remain distinct from test
kills. Per-file limits used by scoped runners are not a total disk quota. Projects
and toolchains must still fit the supported runner scope; see [SUPPORT.md](../SUPPORT.md).
