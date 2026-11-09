import { test } from 'node:test';
import assert from 'node:assert/strict';
import { buildSessionKey, parseSessionKey } from '../src/utils/session-key.js';

test('missing routing fields have an actionable diagnostic', () => {
  for (const options of [{ channel: 'telegram' }, { agentId: 'assistant' }]) {
    assert.throws(() => buildSessionKey(options), /agentId and channel are required/);
  }
});

test('cron parsing preserves the type and a job ID containing colons', () => {
  const key = buildSessionKey({ cronJobId: 'nightly:assistant' });
  assert.deepEqual(parseSessionKey(key), {
    type: 'cron', cronJobId: 'nightly:assistant',
  });
});

test('direct-message parsing preserves each routing field', () => {
  const key = buildSessionKey({
    agentId: 'assistant', channel: 'telegram', peerKind: 'dm', peerId: '42',
  });
  assert.deepEqual(parseSessionKey(key), {
    type: 'agent', agentId: 'assistant', channel: 'telegram', peerKind: 'dm', peerId: '42',
  });
});

test('group parsing includes its topic', () => {
  const key = buildSessionKey({
    agentId: 'assistant', channel: 'telegram', peerKind: 'group', peerId: '-42', topicId: '7',
  });
  assert.deepEqual(parseSessionKey(key), {
    type: 'agent', agentId: 'assistant', channel: 'telegram', peerKind: 'group', peerId: '-42', topicId: '7',
  });
});

test('unknown session keys retain their raw text and classification', () => {
  assert.deepEqual(parseSessionKey('future:opaque:key'), {
    type: 'unknown', raw: 'future:opaque:key',
  });
});
