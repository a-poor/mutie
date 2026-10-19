// Loaded in each isolated test process before its entry module. The Rust adapter
// pins the internal implementation whose TestHook resources are observed here.
import { createHook } from 'node:async_hooks';
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
if (process.env.NODE_TEST_CONTEXT === 'child-v8') {
  const hash = createHash('sha256').update(process.binding('natives')['internal/test_runner/test']).digest('hex');
  if (process.version !== 'v24.22.0' || hash !== MUTIE_HOOK_HASH) throw new Error('unaudited Node test implementation');
  const evidence = { file: process.argv[1], started: 0, completed: 0, failed: 0 };
  const observer = createHook({ init(_id, type, _trigger, resource) {
    if (type !== 'Test' || resource?.constructor?.name !== 'TestHook') return;
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
  process.on('exit', () => {
    writeFileSync(`${MUTIE_OWNED}/hook-${process.pid}.json`, JSON.stringify(evidence), { flag: 'wx' });
  });
}
