# Package-manager audit fixtures

Generated with scripts disabled from the same existing JS/TS Vitest fixture:
Vitest 4.1.9 and Vite 8.1.3, using npm 11.19.1, Yarn 4.18.1 (node-modules,
classic mode), and Bun 1.3.11 (hoisted). These are installer-controlled fixture
variants, not unchanged upstream projects. The source/tests are the existing
`tests/fixtures/vitest/src` and `test` directories.

`installed-package-lock.json` and `installed-yarn-state.yml` preserve diagnostic
installation metadata under names that cannot be mistaken for live node_modules.
No installed dependency code is committed. Reinstall with scripts disabled in a
separate copy; original inputs and generated reports must be retained separately.
Production acceptance is tracked in `benchmarks/package-managers.md`.

`run_development.py` exercises Node and Bun independently for JS and TS using
each prepared installation. It requires pinned runtimes on PATH and an explicit
Mutie binary; `--help` lists inputs. It never installs packages. Each case retains
the development tree in snapshots, checks expected kills/survivors, adds a stronger
test in a separate copy, and verifies original project/report hashes. Use
`--verify-drift` against that output directory to check content-only installation
drift is rejected before a focused run. These fixture manifests are deliberately
adapted and must not be described as unchanged public-project validation.
