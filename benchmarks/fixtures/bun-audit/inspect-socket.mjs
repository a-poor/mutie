// Pinned Bun's length-prefixed Unix inspector transport (internal/debugger.ts).
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
const [bun, cwd, outfile, observer] = process.argv.slice(2);
const dir = mkdtempSync('/tmp/mutie-bun-socket-');
const socketPath = join(dir, 'inspect.sock');
const events = [], commands = [], pending = new Map();
let id = 0, socket, child, stdout = '', stderr = '', expired = false;
let received = Buffer.alloc(0), socketClosed = false;
const server = createServer(connection => {
  if (socket) { connection.destroy(); events.push({ controllerError: 'extra inspector connection' }); return; }
  socket = connection;
  function send(method) {
    const request = { id: ++id, method };
    commands.push(request);
    const body = Buffer.from(JSON.stringify(request));
    const size = Buffer.alloc(4); size.writeUInt32BE(body.length);
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject }); socket.write(Buffer.concat([size, body]));
    });
  }
  socket.on('data', data => {
    received = Buffer.concat([received, data]);
    while (received.length >= 4) {
      const size = received.readUInt32BE();
      if (size > 8 * 1024 * 1024) { events.push({ controllerError: 'oversized frame' }); socket.destroy(); return; }
      if (received.length < size + 4) break;
      const message = JSON.parse(received.subarray(4, size + 4));
      received = received.subarray(size + 4);
      events.push(message);
      if (message.id && pending.has(message.id)) {
        const p = pending.get(message.id); pending.delete(message.id);
        if (message.error) p.reject(new Error(JSON.stringify(message.error))); else p.resolve(message.result);
      }
    }
  });
  socket.on('error', error => events.push({ socketError: String(error) }));
  socket.on('close', () => { socketClosed = true; });
  (async () => {
    try {
      for (const method of ['Inspector.enable', 'Runtime.enable', 'TestReporter.enable', 'LifecycleReporter.enable', 'Inspector.initialized']) await send(method);
    } catch (error) { events.push({ controllerError: String(error) }); }
  })();
});
await new Promise((resolve, reject) => { server.once('error', reject); server.listen(socketPath, resolve); });
child = spawn(bun, ['--inspect-wait=unix://' + socketPath, 'test', '--max-concurrency=1', '--timeout=100', ...(observer ? ['--preload', observer] : [])], {
  cwd, detached: true, env: { PATH: process.env.PATH, HOME: cwd, NO_COLOR: '1',
    ...(observer ? { MUTIE_BUN_OBSERVER: outfile + '.observer.jsonl' } : {}) }
});
const watchdog = setTimeout(() => { expired = true; process.kill(-child.pid, 'SIGKILL'); }, 5000);
child.stdout.on('data', data => { stdout += data; });
child.stderr.on('data', data => { stderr += data; });
child.on('close', async (code, signal) => {
  clearTimeout(watchdog);
  let drainTimedOut = false;
  if (socket && !socketClosed) await new Promise(resolve => {
    const timer = setTimeout(() => { drainTimedOut = true; socket.destroy(); resolve(); }, 1000);
    socket.once('close', () => { clearTimeout(timer); resolve(); });
  });
  if (received.length) events.push({ controllerError: 'truncated frame' });
  writeFileSync(outfile, JSON.stringify({ code, signal, expired, drainTimedOut, commands, events, stdout, stderr }, null, 2) + '\n');
  server.close(); rmSync(dir, { recursive: true, force: true });
});
