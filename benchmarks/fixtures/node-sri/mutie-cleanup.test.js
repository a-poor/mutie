// Review probes for sri-dynamic-loader 370652c304d6fee0c59cfb51bf097e72f6ebf13a.
// Copy into test/. These exercise public behavior; no captured/removed callback is invoked.
import test from 'node:test';
import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { clearCache, loadScript, loadStyle } from '../src/loader.js';
import { StubDocument } from './dom-stub.js';

const url = 'https://app.example/review.js';
test.beforeEach(() => clearCache());
for (const kind of ['script', 'style']) {
  test(`${kind}: successful completion survives later error, abort and deadline`, async () => {
    const controller = new AbortController();
    let element;
    const doc = new StubDocument({ onAppend: el => {
      element = el;
      queueMicrotask(() => el.emitLoad());
    } });
    const load = kind === 'script' ? loadScript : loadStyle;
    const result = await load(url, {
      document: doc, allowUnverified: true, signal: controller.signal, timeout: 40,
    });
    element.emitError();
    controller.abort();
    await delay(80);
    assert.equal(result.element, element);
    assert.equal(element.parentNode, doc.head);
    assert.equal(doc.appended.length, 1);
    assert.equal(element.listeners.get('load')?.length, 0);
    assert.equal(element.listeners.get('error')?.length, 0);
  });
}
for (const outcome of ['error', 'abort', 'timeout']) {
  test(`${outcome}: failed load remains removed after later load/error events`, async () => {
    const controller = new AbortController();
    let element;
    const doc = new StubDocument({ onAppend: el => {
      element = el;
      queueMicrotask(() => {
        if (outcome === 'error') el.emitError();
        if (outcome === 'abort') controller.abort();
      });
    } });
    await assert.rejects(loadScript(url, {
      document: doc, allowUnverified: true, signal: controller.signal, timeout: 40,
    }));
    element.emitLoad();
    element.emitError();
    controller.abort();
    await delay(80);
    assert.equal(element.parentNode, null);
    assert.equal(doc.appended.length, 0);
    assert.equal(element.listeners.get('load')?.length, 0);
    assert.equal(element.listeners.get('error')?.length, 0);
  });
}
