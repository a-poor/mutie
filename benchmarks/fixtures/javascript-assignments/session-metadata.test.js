import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { SessionManager } from '../src/session-manager.js';

async function withClockAndManager(body) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mutie-session-assignment-'));
  const originalNow = Date.now;
  let now = new Date(2026, 0, 15, 12).getTime();
  Date.now = () => now;
  try {
    const manager = new SessionManager({
      dataDir: root,
      resetPolicy: { daily: '04:00', idleMs: 60_000 },
    });
    await manager.init();
    await body(manager, (milliseconds) => { now += milliseconds; return now; });
  } finally {
    Date.now = originalNow;
    await fs.rm(root, { recursive: true, force: true });
  }
}

test('resolving an existing session persists its refreshed timestamp', async () => {
  await withClockAndManager(async (manager, advance) => {
    const first = await manager.resolve('cron:refresh', 'main');
    const before = (await manager.listSessions('main'))['cron:refresh'];
    const refreshedAt = advance(1_000);
    const second = await manager.resolve('cron:refresh', 'main');
    assert.equal(second.isNew, false);
    assert.equal(second.sessionId, first.sessionId);
    const after = (await manager.listSessions('main'))['cron:refresh'];
    assert.equal(after.createdAt, before.createdAt);
    assert.equal(after.updatedAt, refreshedAt);
  });
});

test('an idle reset persists the replacement session for subsequent resolves', async () => {
  await withClockAndManager(async (manager, advance) => {
    const first = await manager.resolve('cron:reset', 'main');
    advance(61_000);
    const reset = await manager.resolve('cron:reset', 'main');
    assert.equal(reset.isNew, true);
    assert.notEqual(reset.sessionId, first.sessionId);
    const saved = (await manager.listSessions('main'))['cron:reset'];
    assert.equal(saved.sessionId, reset.sessionId);
    const reused = await manager.resolve('cron:reset', 'main');
    assert.equal(reused.isNew, false);
    assert.equal(reused.sessionId, reset.sessionId);
  });
});
