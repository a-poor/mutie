# Output modes

[Documentation index](README.md) · [Usage and configuration](usage.md)

```sh
mutie run
mutie run --plain
mutie run --json > events.jsonl
mutie rerun .mutie/RUN --missed --json
```

With no output flag, `run` and `rerun` show a full-window live view on the
alternate screen when stdin and stdout are terminals, `TERM` is not `dumb`, and
`CI` is absent. Otherwise they print plain text. Terminal initialization failures
fall back to plain text.
The live view has overall progress, fixed worker rows, and a rolling mutation
results log. The original terminal screen returns before the final summary.
See [live terminal status](reports.md#live-terminal-status) for details.

`--plain` explicitly selects line-oriented human-readable progress and summaries.
`--json` explicitly selects a JSON event stream, even when attached to a terminal.
The two flags are mutually exclusive. Redirecting stdout does not select JSON.
These flags affect presentation only; mutation selection, outcomes, saved reports
and exit codes are the same in each mode. They are not `mutie.toml` settings.

`init --non-interactive` separately disables setup prompts. The `report` command
remains an interactive browser. `list --json`, `check --json`, and
`rerun --dry-run --json` return their existing single-object previews, without
creating a report or opening a live view.

## JSON events

Run/rerun stdout contains newline-delimited JSON: one object per line, flushed
after each event. There is no enclosing array. Every record has an `event` field
and `elapsed_seconds`, measured from the start of the run. Human diagnostics go
to stderr. Embedded newlines and terminal controls in string values are escaped.

| Event | Fields and meaning |
| --- | --- |
| `run_started` | `language`, `project`, `report`, `rerun`. Paths are absolute; `rerun` is null for a fresh run. |
| `phase` | `phase`: `snapshot`, `baseline`, `discovery`, `inventory`, or `mutations`. A phase may recur. |
| `command_finished` | `phase`, `duration_seconds`, `exit_code`, `timed_out`, `interrupted`, `capture_error`. Describes a completed setup/baseline command; exit zero alone does not establish valid test evidence. |
| `plan` | `total`, `workers`, `timeout_seconds`. The selected mutation inventory is ready. |
| `mutation` | One record per selected mutation: `id`, `file`, `operator`, `source_fingerprint`, `line`, `column`, `byte_start`, `byte_end`, `original`, `replacement`. Reruns also include `source_id` from the source report. |
| `mutation_started` | `id`, `worker`. A worker has been assigned the mutation. |
| `mutation_phase` | `id`, `worker`, `phase`: `preparing`, `running`, `building`, or `testing`. Transient progress, not an outcome. |
| `mutation_finished` | `id`, `worker`, `state`, `outcome`, `duration_seconds`, `error`. Interrupted executions can have a null outcome or duration. |
| `summary` | Final status, counts, score, policies, report location, error and exit code; details below. |

Mutation IDs are local to the current run. Parallel workers can finish out of
inventory order; match records by `id`. Outcomes are `killed`, `missed`, `timeout`,
`build_failed`, or `error`. Consumers should ignore unknown event types and extra
fields to allow additional diagnostics.

`--verbose` adds an `output` string to command and mutation completion records
when captured output is available. This is retained command output, subject to
configured log limits; it is emitted after that command finishes. It is not a
live stream of the underlying test process's stdout/stderr. Full saved logs remain
available through the report, subject to the same retention settings.

## Final results and errors

The final `summary` record includes:

- `status`: `completed`, `failed`, or `interrupted`.
- `total`: planned inventory size, or null if discovery did not finish;
  `completed`, `interrupted`, and `pending` counts.
- `outcomes`: counts for `killed`, `missed`, `timeout`, `build_failed`, and `errors`.
- `score`, `policy`, and `score_policy`: the same structured evaluations as the
  saved report. Failed/interrupted runs do not receive a passing score. These
  fields are null if the report could not be finalized.
- `report`, `report_finalized`, `error`, and `exit_code`.

A completed run can have exit code `3` because a configured findings or score
policy failed. Execution, cancellation, output and persistence errors exit `1`.
Successful runs exit `0`; CLI syntax errors exit `2`.

Check both the final summary and process exit status. A normal handled failure
after startup emits a failed/interrupted summary. Errors before a run starts,
such as invalid configuration or failure to create its report, can produce only
stderr. Abrupt termination or a broken stdout pipe can leave a partial stream
without a summary. A disconnected consumer stops the run through the normal
worker-draining and report-finalization path; it is not treated as success.
