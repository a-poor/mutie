# Bun JavaScript audit fixture

Independent dependency-free ESM fixture for pinned Bun **1.3.11+af24e281e**.
Run `bun test` in this directory: both tests pass.

- Flipping `enabled`'s return value fails its body assertion.
- Flipping `fallback`'s empty-input value passes the original tests.
- Adding an empty-input assertion exposes that second change.

`benchmarks/fixtures/bun-audit/real-project.py` runs these controls on temporary
copies for both syntaxes. The root Cargo suite does not execute this fixture.
These are direct Bun probes, not implemented Mutie Bun support or scored outcomes.
See `benchmarks/bun-initial-scope.md` for the failure-attribution gate.
