// Loaded only by the pinned Vitest 4.1.9 adapter. Constants are prepended by Rust.
import { writeFileSync } from 'node:fs';
import { relative } from 'node:path';

const text = (value) => typeof value === 'string' ? value.slice(0, 2048) : '';
const error = (value) => ({ name: text(value?.name), code: text(value?.code),
  plugin: text(value?.__vitest_rollup_error__?.plugin),
  transformFile: typeof value?.__vitest_rollup_error__?.id === 'string'
    ? relative(MUTIE_ROOT, value.__vitest_rollup_error__.id) : '',
  message: text(value?.message) });
export default class MutieReporter {
  started = false;
  ended = false;
  scheduled = [];
  hooksSeen = false;
  activeHooks = 0;
  onTestRunStart(specifications) {
    if (this.started) throw new Error('Mutie requires exactly one test run');
    this.started = true;
    this.scheduled = specifications.map(spec => relative(MUTIE_ROOT, spec.moduleId));
  }
  onUserConsoleLog(log) {
    (log.type === 'stderr' ? process.stderr : process.stdout).write(log.content);
  }
  onHookStart() { this.hooksSeen = true; this.activeHooks++; }
  onHookEnd() { this.activeHooks--; }
  onTestRunEnd(modules, unhandledErrors, reason) {
    if (!this.started || this.ended) throw new Error('Invalid Mutie completion lifecycle');
    this.ended = true;
    const records = modules.map(module => ({
      path: relative(MUTIE_ROOT, module.moduleId), state: module.state(),
      errors: module.errors().map(error),
      suiteErrors: [...module.children.allSuites()].flatMap(suite => suite.errors().map(error)),
      tests: [...module.children.allTests()].map(test => ({
        lifecycleEvidence: test.meta().mutieLifecycleEvidence === true,
        lifecycleUsed: test.meta().mutieLifecycle === true,
        id: test.id, name: text(test.fullName), state: test.result().state,
        unsupportedOptions: test.options.fails === true || (test.options.retry != null && test.options.retry !== 0)
          || (test.options.repeats != null && test.options.repeats !== 0)
          || (test.options.timeout != null && test.options.timeout !== 0)
          || (test.diagnostic()?.retryCount || 0) > 0
          || (test.diagnostic()?.repeatCount || 0) > 0,
        errors: (test.result().errors || []).map(error),
      })),
    }));
    const report = JSON.stringify({ completed: true, scheduled: this.scheduled,
      reason, hooksSeen: this.hooksSeen, activeHooks: this.activeHooks,
      unhandledErrors: unhandledErrors.map(error), modules: records });
    if (Buffer.byteLength(report) > MUTIE_EVIDENCE_LIMIT)
      throw new Error('Mutie completion evidence exceeds its byte limit');
    writeFileSync(MUTIE_EVIDENCE, report, { flag: 'wx', mode: 0o600 });
    for (const module of records) {
      for (const failure of [...module.errors, ...module.suiteErrors])
        process.stderr.write(`${module.path}: ${failure.name}: ${failure.message}\n`);
      for (const test of module.tests.filter(test => test.state === 'failed'))
        process.stderr.write(`${module.path} > ${test.name}: ${test.errors.map(e => e.message).join('; ')}\n`);
    }
    for (const failure of unhandledErrors)
      process.stderr.write(`Unhandled: ${text(failure.message)}\n`);
  }
}
