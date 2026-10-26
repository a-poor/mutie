import { test } from 'node:test';
import assert from 'node:assert/strict';
import { chunkText } from '../src/channel-adapters/telegram.js';

test('chunking retains every full chunk and the final remainder', () => {
  assert.deepEqual(chunkText('abcdefghij', 4), ['abcd', 'efgh', 'ij']);
  assert.deepEqual(chunkText('abcdefgh', 4), ['abcd', 'efgh']);
  assert.deepEqual(chunkText('ab\ncdefg', 4), ['ab', 'cdef', 'g']);
  assert.deepEqual(chunkText('abc', 4), ['abc']);
});
