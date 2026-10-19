import { writeFileSync } from 'node:fs';
export default async function* (source) {
  const evidence = { ended: false, summaries: [], failures: [], interrupted: false };
  let size = 0;
  for await (const event of source) {
    const d = event.data;
    if (d.expectFailure || d.details?.attempt !== undefined)
      throw new Error('Node expected failures and retries are outside the supported scope');
    if (event.type === 'test:summary') evidence.summaries.push(d);
    if (event.type === 'test:interrupted') evidence.interrupted = true;
    if (event.type === 'test:fail') {
      const e = d.details?.error;
      evidence.failures.push({ file: d.file, id: d.testId, name: d.name, type: d.details?.type,
        failure: e?.failureType, todo: !!d.todo, message: String(e?.message).slice(0, 2000) });
      yield `${d.file}: ${d.name}: ${String(e?.message)}\n`;
    }
    if (event.type === 'test:stdout' || event.type === 'test:stderr') yield d.message;
    if (event.type === 'test:diagnostic') yield `${d.message}\n`;
    size += 1;
    if (size > 100000 || evidence.failures.length > 10000 || evidence.summaries.length > 10000)
      throw new Error('Node completion evidence limit exceeded');
  }
  evidence.ended = true;
  const text = JSON.stringify(evidence);
  if (Buffer.byteLength(text) > 4 * 1024 * 1024) throw new Error('Node completion evidence limit exceeded');
  writeFileSync(MUTIE_EVIDENCE, text, { flag: 'wx' });
}
