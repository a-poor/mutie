// Additional boundary tests for sri-dynamic-loader 370652c304d6fee0c59cfb51bf097e72f6ebf13a.
// Copy into its test/ directory; production source and original tests stay unchanged.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { isValidIntegrity } from '../src/integrity.js';
import { isCrossOrigin, resolveSources } from '../src/sources.js';
import { clearCache, loadStyle } from '../src/loader.js';
import { StubDocument } from './dom-stub.js';

const url = 'https://app.example/local.css';
const integrity = `sha384-${createHash('sha384').update('body{}').digest('base64')}`;
test.beforeEach(() => clearCache());

test('validation wrapper returns false for invalid integrity without throwing', () => {
  assert.equal(isValidIntegrity('not-an-integrity-value'), false);
});
test('boolean crossorigin aliases preserve explicit false and true', () => {
  for (const [crossOrigin, expected] of [[false, null], [true, 'anonymous']]) {
    let source;
    assert.doesNotThrow(() => {
      [source] = resolveSources(url, { integrity, crossOrigin }, 'https://app.example/');
    });
    assert.equal(source.crossOrigin, expected);
  }
});
test('origin comparison has conservative no-base and invalid-base fallbacks', () => {
  assert.equal(isCrossOrigin(url, ''), false);
  assert.equal(isCrossOrigin(url, 'not a URL'), false);
});
test('stylesheet polling does not invent success without a stylesheet collection', async () => {
  const doc = new StubDocument();
  doc.styleSheets = undefined;
  await assert.rejects(() => loadStyle(url, { document: doc, integrity, timeout: 250 }));
});
test('stylesheet polling finds the matching entry when link.sheet is unavailable', async () => {
  const doc = new StubDocument({
    onAppend: element => { doc.styleSheets = [{ href: element.getAttribute('href') }]; },
  });
  await assert.doesNotReject(() => loadStyle(url, { document: doc, integrity, timeout: 250 }));
});
