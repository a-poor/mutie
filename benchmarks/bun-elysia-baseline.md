# Elysia: native baseline and the Bun runtime boundary

The unchanged [Elysia revision](https://github.com/elysiajs/elysia/tree/1cf5ce15dc4de23a96c769b5500a8d6fac8a8373)
from the [compatibility audit](compatibility-audit.md) contains 296 tracked inputs,
including the 40,379,030-byte test video. All remain intact. Dependencies are
installed in a separate complete copy using Bun 1.3.11's frozen lockfile with
installation scripts disabled: 218 packages. Source, tests, manifests and lockfiles
are unchanged, with generated build/install outputs kept outside the repository.

The upstream CI sequence builds before testing. Its package test command is:

```text
bun run test:functionality && bun run test:types && bun run test:node
```

Functionality runs `bun test ./test` and then an explicit import probe. Types run
the original `tsc --project tsconfig.test.json`. Node compatibility installs local
package links in the original CommonJS and ESM test packages and runs both entry
scripts against generated `dist/` outputs. Node is pinned to 24.22.0, npm to
11.19.1; the frozen root tools include TypeScript 5.9.3 and tsup 8.5.1.

| Native stage | Bun 1.3.11 | Bun 1.4.4 |
| --- | --- | --- |
| Build | Pass | Pass; generated output identity checked |
| Functionality | 1,705 pass, two skip, four fail | 1,709 pass, two skip, zero fail |
| Import probe | Pass independently | Pass in complete command |
| Type tests | Pass independently | Pass in complete command, fresh incremental cache |
| Node CJS + ESM | Pass independently | Pass in complete command |

The older complete command stops at functionality, so later independent probes
do not turn it into a passing baseline. The four failures are:

- `stop(false)` leaves a subsequent health fetch able to resolve.
- A server-initiated WebSocket close reports an unclean closure.
- Two redirect-header checks receive a bare authority without the expected `/`.

Changing only the runtime to the installed exact Bun 1.4.4 revision removes those
four failures. Both original Standard Schema skips remain. The full package test
command completes successfully; this is **not** a claim that every declared test
executed. The separate Cloudflare `test:cf` CI step is not part of this command
and was not exercised. No publish or release script runs.

This demonstrated runtime gap led to the [bounded runtime extension](bun-runtime-compatibility.md).
It does not justify lifting Elysia's other admission checks. Full Mutie acceptance
still needs owned dependencies and nested package links, complete build/type/test
execution evidence, fresh mutated build outputs, JSONC/bundler configuration,
positive skip identity and appropriate inert-asset limits. No Elysia mutations
were run and no score was produced.

[Results](fixtures/bun-runtime/results.json) include native counts, command timings,
capture hashes, installation/build inventory digests and original-input checks.
The original Git checkout remains clean. The initial sandbox installation denial
is retained separately from the successful authorized frozen install.
