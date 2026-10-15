# Vitest scope fixture

This is a planned adapter's direct-runner fixture, not implemented Mutie support.
It separately exercises ESM JavaScript and TypeScript boolean value expressions.
Pinned tools: Node 24.22.0, pnpm 11.10.0, Vitest 4.1.9 with Vite 8.3.4; macOS arm64 evidence.
Root `cargo test` does not run it.

Copy this directory to scratch, then install dependencies there with
`pnpm install --frozen-lockfile --ignore-scripts` using pnpm 11.10.0. Run
`node node_modules/vitest/vitest.mjs run`. The baseline passes four tests.
Dependencies must remain self-contained; don't link to another checkout's
`node_modules`. No dependencies or generated reports should be committed here.

Each source file has three runtime boolean literals. Flipping `enabled`'s return
or the tested `fallback(true)` return fails its test. Flipping the final false
return survives. Adding a test that expects `fallback(false)` to be false kills
that survivor. Both syntax paths must be tested independently.

From the Mutie root, run the complete direct scope probes on that prepared copy:

```sh
python3 benchmarks/vitest-scope-probe.py /path/to/prepared-fixture /tmp/vitest-scope.json /path/to/node
```

The probe creates another disposable copy and checks baseline, separate JS/TS
kill/survive/stronger-test scenarios, transform failure, no tests, unhandled error,
config error and all-skipped execution. See
[the scope record](../../../benchmarks/vitest-initial-scope.md) for acceptance gates.
