// Observe the pinned loaders without replacing their functions or interpreting
// exception messages. A deferred dependency import is still module evaluation,
// even when its rejection reaches an ordinary test body or is caught there.
import { tracingChannel } from 'node:diagnostics_channel';
import { writeFileSync } from 'node:fs';
if (process.env.NODE_TEST_CONTEXT === 'child-v8') {
  const active = new Set();
  const evidence = { file: process.argv[1], started: 0, completed: 0,
    errors: 0, overflow: false, samples: [] };
  let exiting = false;
  const persist = () => writeFileSync(`${MUTIE_OWNED}/modules-${process.pid}.json`, JSON.stringify(evidence));
  for (const [name, completion] of [['module.require', 'end'], ['module.import', 'asyncEnd']]) {
    tracingChannel(name).subscribe({
      start(context) {
        if (evidence.started >= 100000) evidence.overflow = true;
        else { active.add(context); evidence.started++; }
        if (exiting) persist();
      },
      [completion](context) {
        // The observer's own import can finish after subscriptions are enabled;
        // it has no observed start and must not manufacture completion credit.
        if (active.delete(context)) evidence.completed++;
        if (exiting) persist();
      },
      error(context) {
        if (evidence.errors >= 100000) evidence.overflow = true;
        else evidence.errors++;
        if (evidence.samples.length < 4) {
          evidence.samples.push(`${name}: ${context.id ?? context.url ?? ''}: ${String(context.error?.message)}`.slice(0, 500));
        }
        // Keep late failures after exit listeners visible, including caught
        // loader errors that never produce an unhandled-rejection event.
        persist();
      },
    });
  }
  const emit = process.emit;
  process.emit = function (event, ...args) {
    if (event === 'exit') exiting = true;
    try { return Reflect.apply(emit, this, [event, ...args]); }
    finally { if (event === 'exit') persist(); }
  };
}
