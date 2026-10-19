// Additional tests for openclaw-cc e323faa446f03b5438fa9edc58781b4ef8dd52df.
// Copy into test/. All service interactions are injected fakes; state is temporary.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { TelegramPoller } from '../src/channel-receivers/telegram-poller.js';
import { setLogLevel } from '../src/utils/logger.js';

setLogLevel('error');
const update = {
  update_id: 1,
  message: { text: 'hello', chat: { id: 123, type: 'private' }, from: { first_name: 'Test' } },
};
async function withPoller(options, body) {
  const stateDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mutie-poller-'));
  const calls = [];
  const sends = [];
  const poller = new TelegramPoller({
    botToken: 'fake', stateDir, allowedChatIds: ['123'],
    agentRunner: { run: async args => { calls.push(args); return { text: 'reply' }; } },
    sessionManager: { resolve: async () => ({ sessionId: 'fake-session', isNew: true }) },
    sendMessage: async (...args) => { sends.push(args); },
    fetchImpl: async url => ({ json: async () => ({ ok: true, result: url.includes('getUpdates') ? [update] : true }) }),
    ...options,
  });
  try { await body(poller, calls, sends); }
  finally { await poller.stop(); await fs.rm(stateDir, { recursive: true, force: true }); }
}

test('empty allowedChatIds rejects messages before agent execution or sending', async () => {
  await withPoller({ allowedChatIds: [] }, async (poller, calls, sends) => {
    await poller.pollOnce();
    assert.equal(calls.length, 0);
    assert.equal(sends.length, 0);
  });
});

test('explicit sessionPersistence false requests stateless agent execution', async () => {
  await withPoller({ agentConfig: { sessionPersistence: false } }, async (poller, calls) => {
    await poller.pollOnce();
    assert.equal(calls.length, 1);
    assert.equal(calls[0].noSessionPersistence, true);
  });
});

test('overlapping polls issue only one outstanding request', async () => {
  let release;
  let count = 0;
  const pending = new Promise(resolve => { release = resolve; });
  await withPoller({ fetchImpl: async () => {
    count++;
    await pending;
    return { json: async () => ({ ok: true, result: [] }) };
  } }, async poller => {
    const first = poller.pollOnce();
    const second = poller.pollOnce();
    try { assert.equal(count, 1); }
    finally { release(); await Promise.all([first, second]); }
  });
});
