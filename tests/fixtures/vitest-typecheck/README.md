# Vitest typecheck fixture

Copy this directory to a temporary location, install its frozen lockfile using
pnpm 10.33.2 with `--ignore-scripts`, and use Node 24.22.0. Mutie never installs
these dependencies itself.

The fixture pins Vitest 4.1.5, Vite 8.0.10, TypeScript 6.0.3, ESLint 10.2.1,
eslint-config-unjs 0.6.2 and Prettier 3.8.3. It has no package `type` declaration.
Its test command includes both lint and typechecking. The source-only tsconfig
intentionally reproduces UFO's missing type-test compilation; Mutie's isolated
supplemental config must include `test/types.test-d.ts` and verify compiler coverage.

Set `MUTIE_VITEST_RECIPE_FIXTURE` to the installed copy and run:

```sh
cargo test --locked --bin mutie vitest::recipe_tests:: -- --ignored --test-threads=1
```

These controls cover type assertions, source errors, mixed failures, compiler
exit/signal/timeout/output, ignored package-manager launchers, lint failures and
configuration drift. They preserve the input fixture and dependency store.
