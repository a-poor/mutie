# Project configuration and init acceptance

2026-10-22. This is onboarding acceptance, not expansion of any execution adapter.

## Contract

`mutie init [PATH]` writes a minimal `mutie.toml`. `--lang` selects the existing
execution adapter (Go/go test, Rust/Cargo, Python/pytest, Vitest, Node or Bun).
This deliberately retains the existing serialized `lang` key: it denotes an
adapter, not merely source syntax. JavaScript versus TypeScript remains source
syntax within each JS runner, not a package-manager choice. No schema/version
change is needed to clarify these concepts.

Manifest presence supplies Go/Rust/Python candidates; recognized test-script
prefixes supply Node/Bun/Vitest candidates, with declared Vitest dependencies
also contributing a candidate. Mixed candidates require explicit choice. A
terminal prompts; nonterminal input or `--non-interactive` never prompts. An
unknown/custom test script needs `--lang`, not shell execution. No manifest is
executed. No runtime is launched or installed by setup.

Package-manager declarations and lockfile names are diagnostic metadata, not
runner selection. Vitest currently requires its existing pnpm 11.10.0 declaration;
other managers receive an explicit unsupported diagnostic before writing. Node
and Bun setup describes their dependency-free execution boundary. Runtime version
requirements are displayed, not inferred from loose project engines constraints.
The adapter still performs the authoritative runtime/project checks on a run.

Existing files and symlinks are refused; create-new writing also rejects a race
that creates the destination after the initial check. `--dry-run` writes nothing
and permits inspecting a proposal alongside an existing file. No force/overwrite
mode exists. Python launcher paths become invocation-relative absolute paths
without resolving the executable symlink (preserves venv identity).

## Configuration semantics

Existing precedence remains defaults < TOML < explicit CLI settings. Source
include/exclude filters restrict mutation locations, not test discovery. Full
adapter test inventory remains the default. Go's existing affected-package scope
is distinct from arbitrary user test filtering.

Operator inclusion uses the existing `operators` list; users currently exclude
operators by listing those they want. A future explicit exclusion option should
subtract from the selected supported catalog after inclusion, validate unknown
names, preserve each list's CLI replacement/clear behavior, and record effective
selection in reports. It must not change pristine discovery used for dismissals.
It is specified here, not implemented or emitted by init.

Future runner-option overrides must be structured per adapter and allowlisted
according to semantics, with configuration/CLI precedence and execution identity.
Options affecting test inventory require explicit scope and pristine/snapshot
inventory checks; options affecting reporting, observer loading, concurrency or
failure attribution cannot be passed through unchecked. Unknown or unsupported
flags must error before execution. Arbitrary shell commands, environment injection
and generic extra-arguments strings are not introduced by this goal. Existing
supported settings (budgets, workers, full/affected scope, filters) remain usable.

## Audit and validation

Inspected existing configuration/dispatch and fixture manifests for every adapter.
Also inspected copied unchanged public manifests for dependency-atlas (explicit
native TypeScript Node test script) and Tinycss2 1.2.1 (Flit metadata, pytest tool
settings, webencodings dependency). Both initialize successfully in disposable
manifest copies; original projects are not modified. Their previous execution
acceptance remains separate evidence. Package-manager/layout expansion belongs
to the next roadmap goal, not this setup command.

- All binary unit tests: 206 passed, 28 ignored.
- Existing configuration integration suite: seven passed (includes precedence,
  selection, saved settings and unchanged execution behavior).
- Init CLI integration tests cover ambiguity, existing-file preservation,
  dangling symlinks, unsupported manager rejection, seven fixture manifests and
  Python launcher symlink preservation without execution.
- Manual bounded pseudo-terminal probe selects Go from a mixed Go/Rust root;
  the generated file records that choice. Noninteractive ambiguity errors.
- Smoke probes initialize seven fixture-manifest copies and both public-manifest
  copies. These test setup, not execution compatibility or a new full-suite claim.
- Formatting, all-target clippy with warnings denied and diff whitespace checks
  pass. The deferred host startup investigation remains unresolved.
