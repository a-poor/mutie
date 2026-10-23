import { test } from 'node:test';
import assert from 'node:assert/strict';
import { chunkText } from '../src/channel-adapters/telegram.js';

test('long Telegram messages retain every chunk', () => {
  assert.deepEqual(chunkText('short'), ['short']);
  assert.deepEqual(chunkText('abcdefghij', 4), ['abcd', 'efgh', 'ij']);
  assert.deepEqual(chunkText('abc\ndefghij', 4), ['abc', 'defg', 'hij']);
  assert.deepEqual(chunkText('x'.repeat(4097)), ['x'.repeat(4096), 'x']);
});
