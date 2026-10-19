// Loaded in each isolated test process before its entry module. The Rust adapter
// pins the internal implementation whose TestHook resources are observed here.
import { createHook } from 'node:async_hooks';
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { isPromise } from 'node:util/types';
if (process.env.NODE_TEST_CONTEXT === 'child-v8') {
  const hash = createHash('sha256').update(process.binding('natives')['internal/test_runner/test']).digest('hex');
  if (process.version !== 'v24.22.0' || hash !== MUTIE_HOOK_HASH) throw new Error('unaudited Node test implementation');
  const evidence = { file: process.argv[1], started: 0, completed: 0, failed: 0, bodies: [], process_errors: 0 };
  const observer = createHook({ init(_id, type, _trigger, resource) {
    if (type !== 'Test') return;
    const kind = resource?.constructor?.name;
    if (kind === 'Test' && resource.run) {
      const run = resource.run;
      resource.run = async function (...args) {
        const observed = [];
        // Proxy preserves function arity: Node uses it to select callback mode.
        const body = this.fn;
        this.fn = new Proxy(body, { apply(target, receiver, bodyArgs) {
          if (body.length === 2 && typeof bodyArgs[1] === 'function') {
            const done = bodyArgs[1];
            bodyArgs[1] = function (...values) {
              if (values[0]) observed.push(values[0]);
              return Reflect.apply(done, this, values);
            };
          }
          let result;
          try { result = Reflect.apply(target, receiver, bodyArgs); }
          catch (error) { observed.push(error); throw error; }
          if (isPromise(result)) {
            return Promise.prototype.then.call(result, undefined, error => {
              observed.push(error);
              throw error;
            });
          }
          return result;
        } });
        try { return await Reflect.apply(run, this, args); }
        finally {
          this.fn = body;
          // Plans, harness failures and hooks run outside fn. Require the exact
          // thrown/rejected value, not merely a matching message or error code.
          if (this.error?.failureType === 'testCodeFailure' &&
              observed.some(error => Object.is(error, this.error.cause))) {
            evidence.bodies.push(this.testId);
          }
        }
      };
      return;
    }
    if (kind !== 'TestHook') return;
    // Record failure at fail(), before a concurrent invocation can reset error.
    const fail = resource.fail;
    resource.fail = function (...args) {
      evidence.failed++;
      return Reflect.apply(fail, this, args);
    };
    const run = resource.run;
    resource.run = async function (...args) {
      evidence.started++;
      let threw = false;
      try { return await Reflect.apply(run, this, args); }
      catch (error) { threw = true; throw error; }
      finally {
        evidence.completed++;
        if (threw || this.error) evidence.failed++;
      }
    };
  } });
  observer.enable();
  const persist = () => writeFileSync(`${MUTIE_OWNED}/hook-${process.pid}.json`, JSON.stringify(evidence));
  // Observe emission rather than adding an error handler: listener counts,
  // default fatal behavior, return values and thrown errors remain Node's own.
  // Persist after exit listeners, including ones that throw after a failed body.
  const emit = process.emit;
  process.emit = function (event, ...args) {
    const unhandled = event === 'uncaughtExceptionMonitor' || event === 'unhandledRejection';
    if (unhandled) evidence.process_errors++;
    try { return Reflect.apply(emit, this, [event, ...args]); }
    catch (error) {
      if (event === 'exit') evidence.process_errors++;
      throw error;
    } finally {
      if (event === 'exit' || unhandled) persist();
    }
  };
}
