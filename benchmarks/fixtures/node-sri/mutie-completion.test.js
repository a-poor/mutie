// Standalone diagnostic: run only this file to bound an otherwise pending promise.
// The complete original suite still times out for mutation 8; this is not a scored kill.
import test from 'node:test';
import assert from 'node:assert/strict';
import { loadScript, clearCache } from '../src/loader.js';
import { StubDocument } from './dom-stub.js';

test('an immediate load event completes before the diagnostic deadline', async () => {
  clearCache();
  let timer;
  const doc = new StubDocument({ onAppend: el => queueMicrotask(() => el.emitLoad()) });
  const load = loadScript('https://app.example/completion.js', {
    document: doc, allowUnverified: true, timeout: 20,
  }).then(() => 'loaded', () => 'rejected');
  try {
    const result = await Promise.race([
      load,
      new Promise(resolve => { timer = setTimeout(() => resolve('still pending'), 200); }),
    ]);
    assert.equal(result, 'loaded');
  } finally {
    clearTimeout(timer);
  }
});
