// Direct inspector audit. Node 24 controls pinned Bun; no project code is rewritten.
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
const [bun, cwd, outfile] = process.argv.slice(2);
const events = [], commands = [];
let stderr = '', stdout = '', ws, connected = false, expired = false;
const pending = new Map();
let id = 0;
const child = spawn(bun, ['--inspect-wait=127.0.0.1:0', 'test', '--max-concurrency=1', '--timeout=100'], {
  cwd, detached: true, env: { PATH: process.env.PATH, HOME: cwd, NO_COLOR: '1' }
});
const watchdog = setTimeout(() => { expired = true; process.kill(-child.pid, 'SIGKILL'); }, 5000);
function send(method) {
  const request = { id: ++id, method };
  commands.push(request);
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject }); ws.send(JSON.stringify(request));
  });
}
child.stdout.on('data', chunk => { stdout += chunk; });
child.stderr.on('data', chunk => {
  stderr += chunk;
  const url = stderr.match(/ws:\/\/127\.0\.0\.1:\d+\/[^\s]+/);
  if (!url || connected) return;
  connected = true;
  ws = new WebSocket(url[0]);
  ws.onmessage = event => {
    const message = JSON.parse(event.data);
    events.push(message);
    if (message.id && pending.has(message.id)) {
      const p = pending.get(message.id); pending.delete(message.id);
      if (message.error) p.reject(new Error(JSON.stringify(message.error))); else p.resolve(message.result);
    }
  };
  ws.onopen = async () => {
    try {
      await send('Inspector.enable');
      await send('Runtime.enable');
      await send('TestReporter.enable');
      await send('LifecycleReporter.enable');
      await send('Inspector.initialized');
    } catch (error) { events.push({ controllerError: String(error) }); ws.close(); }
  };
  ws.onerror = event => { events.push({ socketError: String(event.message) }); };
});
child.on('close', (code, signal) => {
  clearTimeout(watchdog);
  let saved = false;
  function save(drainTimedOut = false) {
    if (saved) return;
    saved = true;
    writeFileSync(outfile, JSON.stringify({ code, signal, expired, drainTimedOut, commands, events, stdout, stderr }, null, 2) + '\n');
  }
  if (!ws || ws.readyState === WebSocket.CLOSED) { save(); return; }
  // Process exit and WebSocket delivery are independent. Drain the transport first.
  const drain = setTimeout(() => { save(true); ws.close(); }, 1000);
  ws.addEventListener('close', () => { clearTimeout(drain); save(); }, { once: true });
});
