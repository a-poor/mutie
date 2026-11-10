// Constants TSC, BUILD_INFO and OUTPUT_LIMIT are supplied by the owned launcher.
// Vitest captures compiler output internally. Bound it before forwarding it.
import { spawn } from 'node:child_process';

let bytes = 0;
let overflow = false;
const args = process.argv.slice(2);
const buildInfo = args.indexOf('--tsBuildInfoFile');
if (buildInfo < 0 || buildInfo + 1 >= args.length)
  throw new Error('Unexpected pinned Vitest typechecker arguments');
args[buildInfo + 1] = BUILD_INFO;
const child = spawn(process.execPath, [TSC, ...args, '--listFiles'], {
  stdio: ['ignore', 'pipe', 'pipe'],
  env: { ...process.env, NODE_OPTIONS: '', NODE_PATH: '' },
});
const forward = (stream, chunk) => {
  bytes += chunk.length;
  if (bytes > OUTPUT_LIMIT) {
    if (!overflow) {
      overflow = true;
      process.stderr.write('Mutie typechecker output exceeded its byte limit\n');
      child.kill('SIGKILL');
    }
    return;
  }
  stream.write(chunk);
};
child.stdout.on('data', chunk => forward(process.stdout, chunk));
child.stderr.on('data', chunk => forward(process.stderr, chunk));
child.on('error', error => {
  process.stderr.write(`Mutie typechecker could not start: ${error.message}\n`);
  process.exitCode = 193;
});
child.on('close', code => { process.exitCode = overflow ? 194 : (code ?? 195); });
