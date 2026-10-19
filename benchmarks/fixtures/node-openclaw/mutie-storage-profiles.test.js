// Review tests for openclaw-cc e323faa446f03b5438fa9edc58781b4ef8dd52df.
// Copy this file and mutie-lock-owner.js into test/. No original files change.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { withLock } from '../src/utils/storage.js';
import { loadWorkspaceContext } from '../src/workspace-loader.js';

async function temporary(body) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mutie-storage-profile-'));
  try { await body(root); }
  finally { await fs.rm(root, { recursive: true, force: true }); }
}

async function assertOwned(lock) {
  const record = JSON.parse(await fs.readFile(lock, 'utf8'));
  assert.equal(record.pid, process.pid, 'callback must own the lock');
  assert.equal(typeof record.ts, 'number');
}

test('storage owns the lock during callbacks and releases it after success or failure', async () => {
  await temporary(async root => {
    const lock = path.join(root, 'data.lock');
    const result = await withLock(lock, async () => { await assertOwned(lock); return 'done'; });
    assert.equal(result, 'done');
    await assert.rejects(fs.access(lock), { code: 'ENOENT' });
    const failure = new Error('callback failure');
    await assert.rejects(withLock(lock, async () => {
      await assertOwned(lock);
      throw failure;
    }), error => error === failure);
    await assert.rejects(fs.access(lock), { code: 'ENOENT' });
    await withLock(lock, () => assertOwned(lock));
  });
});

test('storage waits for an independent process to release a fresh lock', async () => {
  await temporary(async root => {
    const lock = path.join(root, 'data.lock');
    const child = spawn(process.execPath, [fileURLToPath(new URL('./mutie-lock-owner.js', import.meta.url)), lock], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    const exited = new Promise(resolve => {
      child.once('error', error => resolve({ error }));
      child.once('close', (code, signal) => resolve({ code, signal }));
    });
    let diagnostics = '';
    child.stderr.on('data', data => { diagnostics += data; });
    child.stdin.on('error', () => {}); // Exit status is checked below.
    const lines = createInterface({ input: child.stdout });
    let release;
    let watchdog;
    try {
      await Promise.race([
        once(lines, 'line').then(([line]) => assert.equal(line, 'locked')),
        exited.then(status => { throw new Error(`lock owner exited before ready: ${JSON.stringify(status)} ${diagnostics}`); }),
        new Promise((_, reject) => { watchdog = setTimeout(() => reject(new Error('lock owner did not start')), 3000); }),
      ]);
      clearTimeout(watchdog);
      const held = JSON.parse(await fs.readFile(lock, 'utf8'));
      assert.equal(held.pid, child.pid);
      let released = false;
      // The handshake establishes ownership first. This delay only holds it
      // across multiple 50ms retries; ownership assertion also catches bypass
      // if a heavily loaded machine delays the contender past this timer.
      release = setTimeout(() => { released = true; child.stdin.end('release\n'); }, 150);
      await withLock(lock, async () => {
        assert.equal(released, true, 'callback must wait for the owner');
        await assertOwned(lock);
      });
      const status = await exited;
      assert.deepEqual(status, { code: 0, signal: null }, diagnostics);
      await assert.rejects(fs.access(lock), { code: 'ENOENT' });
    } finally {
      clearTimeout(release);
      clearTimeout(watchdog);
      lines.close();
      if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
      await exited;
    }
  });
});

function dateName(date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

for (const profile of ['main', 'cron', 'group']) {
  test(`${profile} workspace profile includes only its permitted context`, async () => {
    await temporary(async root => {
      const files = {
        'SOUL.md': 'core-marker', 'USER.md': 'user-marker',
        'IDENTITY.md': 'identity-marker', 'AGENTS.md': 'agents-marker',
        'TOOLS.md': 'tools-marker', 'MEMORY.md': 'private-marker',
        'GROUP_MEMORY.md': 'group-marker', 'BOOTSTRAP.md': 'bootstrap-marker',
      };
      await fs.mkdir(path.join(root, 'memory'));
      // Cover local midnight rollover without replacing Date or runner internals.
      for (const offset of [-2, -1, 0, 1, 2]) {
        const date = new Date();
        date.setDate(date.getDate() + offset);
        files[`memory/${dateName(date)}.md`] = 'daily-marker';
      }
      for (const [name, content] of Object.entries(files)) await fs.writeFile(path.join(root, name), content);
      const result = await loadWorkspaceContext({ workspacePath: root, sessionType: profile });
      const permitted = {
        main: ['SOUL.md', 'USER.md', 'IDENTITY.md', 'AGENTS.md', 'TOOLS.md', 'MEMORY.md', 'BOOTSTRAP.md'],
        cron: ['SOUL.md', 'IDENTITY.md', 'AGENTS.md'],
        group: ['SOUL.md', 'USER.md', 'IDENTITY.md', 'AGENTS.md', 'GROUP_MEMORY.md'],
      }[profile];
      assert.deepEqual(result.loadedFiles.filter(name => !name.startsWith('memory/')).sort(), [...permitted].sort());
      for (const name of Object.keys(files).filter(name => !name.startsWith('memory/'))) {
        assert.equal(result.context.includes(files[name]), permitted.includes(name), `${profile}: ${name}`);
      }
      assert.equal(result.loadedFiles.filter(name => name.startsWith('memory/')).length, profile === 'cron' ? 0 : 2);
      assert.equal(result.context.includes('daily-marker'), profile !== 'cron');
      assert.equal(result.bootstrapFound, profile === 'main');
    });
  });
}
