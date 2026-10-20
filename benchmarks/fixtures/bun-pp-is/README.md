# pp-is Bun JavaScript acceptance fixture

The retained `stronger.test.js` is added as `test/mutie-stronger.test.js` to a
separate copy of pp-is at commit `1d5b3bac804e1e67da289f3d2a14bd0705a383fb`:
https://github.com/carlos-sweb/pp-is

The scoped acceptance root contains the complete, unchanged upstream `src/` and
`test/` directories, plus the upstream package manifest with only `type: module`
added (JSON formatting also changes). The unchanged checkout is NOT currently
accepted: its generated dist files, declaration file and `.mjs` build script are
outside Mutie's audited scope. Packaging/docs are omitted only in the explicitly
adapted temporary root; no runtime source or test is omitted or rewritten.

Use pinned Bun 1.3.11+af24e281e. Run the adapted root with:

```sh
mutie /path/to/scoped-copy --lang bun --timeout 5 --baseline-timeout 15 --ui plain
```

Copy the scoped root separately, add the retained test, then use the source report:

```sh
mutie /path/to/stronger-copy --lang bun --rerun-missed /path/to/source-run --timeout 5 --baseline-timeout 15 --ui plain
```

See `benchmarks/bun-javascript-pp-is.md` and its results JSON for complete input,
adaptation and report preservation evidence. Tests target documented string
predicates and malformed IPv4 octets; they intentionally leave the ambiguous
`isEmpty` fallback and redundant primitive-boolean checks unresolved.

Follow-up: `benchmarks/bun-unchanged-layout.md` records support for the original
checkout with matching outcomes; the adaptation above remains historical evidence.
