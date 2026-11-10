// The pinned serial runner emits every worker's events before that worker's
// file summary. Bind failures to that explicit completion boundary, retaining
// their separate declaration locations (which may be imported test helpers).
import { writeFileSync } from 'node:fs';
export default async function* (source) {
  const evidence = { ended: false, summaries: [], failures: [], interrupted: false };
  let pending = [];
  let completed = 0;
  let events = 0;
  for await (const event of source) {
    const d = event.data;
    if (d.expectFailure || d.details?.attempt !== undefined)
      throw new Error('Node expected failures and retries are outside the supported scope');
    if (event.type === 'test:summary') {
      if (d.file !== undefined) {
        if (d.file !== MUTIE_FILES[completed])
          throw new Error('Node serial worker completion order disagrees with selected files');
        for (const failure of pending) failure.owner = d.file;
        pending = [];
        completed++;
      } else if (completed !== MUTIE_FILES.length || pending.length !== 0) {
        throw new Error('Node global completion precedes complete worker evidence');
      }
      evidence.summaries.push(d);
    }
    if (event.type === 'test:interrupted') evidence.interrupted = true;
    if (event.type === 'test:fail') {
      const error = d.details?.error;
      const failure = { file: d.file, owner: undefined, id: d.testId, name: d.name,
        type: d.details?.type, failure: error?.failureType, todo: !!d.todo,
        message: String(error?.message).slice(0, 2000) };
      pending.push(failure);
      evidence.failures.push(failure);
      yield `${d.file}: ${d.name}: ${String(error?.message)}\n`;
    }
    if (event.type === 'test:stdout' || event.type === 'test:stderr') yield d.message;
    if (event.type === 'test:diagnostic') yield `${d.message}\n`;
    if (++events > 100000 || evidence.failures.length > 10000 || evidence.summaries.length > 10000)
      throw new Error('Node completion evidence limit exceeded');
  }
  if (completed !== MUTIE_FILES.length || pending.length !== 0)
    throw new Error('Node worker completion evidence is incomplete');
  evidence.ended = true;
  const text = JSON.stringify(evidence);
  if (Buffer.byteLength(text) > 4 * 1024 * 1024) throw new Error('Node completion evidence limit exceeded');
  writeFileSync(MUTIE_EVIDENCE, text, { flag: 'wx' });
}
