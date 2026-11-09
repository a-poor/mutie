import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { SessionManager } from '../src/session-manager.js';

async function withSession(at, check) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'mutie-session-numbers-'));
  const originalNow = Date.now;
  let now = at;
  Date.now = () => now;
  try {
    const manager = new SessionManager({ dataDir: dir });
    await manager.init();
    await check(manager, value => { now = value; });
  } finally {
    Date.now = originalNow;
    await fs.rm(dir, { recursive: true, force: true });
  }
}

test('fresh sessions start with no consumed tokens', async () => {
  await withSession(new Date(2024, 0, 15, 12).getTime(), async manager => {
    await manager.resolve('numeric:fresh', 'main');
    const sessions = await manager.listSessions('main');
    assert.equal(sessions['numeric:fresh'].totalTokens, 0);
  });
});

test('the default idle window keeps a recent session', async () => {
  const start = new Date(2024, 0, 15, 12).getTime();
  await withSession(start, async (manager, setTime) => {
    const first = await manager.resolve('numeric:idle', 'main');
    setTime(start + 1000);
    const second = await manager.resolve('numeric:idle', 'main');
    assert.equal(second.isNew, false);
    assert.equal(second.sessionId, first.sessionId);
  });
});

test('daily reset starts at the exact minute with no leftover seconds or milliseconds', async () => {
  await withSession(new Date(2024, 0, 15, 3, 30, 12, 345).getTime(), async (manager, setTime) => {
    const first = await manager.resolve('numeric:daily', 'main');
    setTime(new Date(2024, 0, 15, 4, 0, 0, 0).getTime());
    const second = await manager.resolve('numeric:daily', 'main');
    assert.equal(second.isNew, true);
    assert.notEqual(second.sessionId, first.sessionId);
  });
});
