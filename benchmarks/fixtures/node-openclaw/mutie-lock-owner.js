// Independent lock owner for the storage contention test; never imports mutated code.
import fs from 'node:fs/promises';
import { once } from 'node:events';

const file = process.argv[2];
const handle = await fs.open(file, 'wx');
try {
  await handle.writeFile(JSON.stringify({ pid: process.pid, ts: Date.now() }));
  console.log('locked');
  await once(process.stdin, 'data');
} finally {
  await handle.close();
  await fs.unlink(file);
  process.stdin.destroy();
}
