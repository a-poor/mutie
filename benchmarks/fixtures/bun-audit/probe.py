#!/usr/bin/env python3
"""Direct pinned-Bun evidence; this is not a Mutie execution adapter."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import xml.etree.ElementTree as ET

CASES = {
    'pass': 'test("body", () => expect(true).toBe(true));',
    'assertion': 'test("body", () => expect(true).toBe(false));',
    'body_throw': 'test("body", () => { throw new Error("same failure"); });',
    'shared_error_body': 'const failure = new Error("same failure"); const fail = () => { throw failure; };' + '\n' * 10 + 'beforeEach(() => {}); test("body", fail);',
    'shared_error_hook': 'const failure = new Error("same failure"); const fail = () => { throw failure; };' + '\n' * 10 + 'beforeEach(fail); test("body", () => {});',
    'body_reject': 'test("body", async () => { throw new Error("same failure"); });',
    'before_each': 'beforeEach(() => { throw new Error("same failure"); }); test("body", () => {});',
    'after_each': 'afterEach(() => { throw new Error("same failure"); }); test("body", () => {});',
    'before_all': 'beforeAll(() => { throw new Error("same failure"); }); test("body", () => {});',
    'after_all': 'afterAll(() => { throw new Error("same failure"); }); test("body", () => {});',
    'body_and_cleanup': 'afterEach(() => { throw new Error("cleanup failure"); }); test("body", () => { throw new Error("body failure"); });',
    'async_cleanup': 'afterEach(async () => { throw new Error("same failure"); }); test("body", () => {});',
    'nested_cleanup': 'describe("nested", () => { afterEach(() => { throw new Error("same failure"); }); test("body", () => {}); });',
    'import_throw': 'throw new Error("import failure"); test("body", () => {});',
    'empty': '',
    'skip': 'test.skip("body", () => {});',
    'todo': 'test.todo("body", () => {});',
    'only': 'test.only("body", () => {}); test("omitted", () => { throw new Error("omitted"); });',
    'exit_zero': 'test("body", () => { process.exit(0); });',
    'late_exception': 'test("body", async () => { setTimeout(() => { throw new Error("late failure"); }, 0); await Bun.sleep(20); });',
    'unhandled_rejection': 'test("body", async () => { Promise.reject(new Error("late failure")); await Bun.sleep(20); });',
    'pending': 'test("body", () => new Promise(() => {}));',
    'plan': 'test("body", () => { expect.assertions(1); });',
    'second_file_import': 'test("body", () => {});',
}
HEADER = 'import { test, expect, describe, beforeEach, afterEach, beforeAll, afterAll } from "bun:test";\n'


def run(bun, directory, timeout, observer=None, timeout_ms=100):
    start = time.monotonic()
    (directory / 'result.xml').unlink(missing_ok=True)
    (directory / 'observer.jsonl').unlink(missing_ok=True)
    env = {k: v for k, v in os.environ.items() if k in ('PATH', 'SYSTEMROOT', 'TMPDIR')}
    env.update(HOME=str(directory), NO_COLOR='1', FORCE_COLOR='0')
    args = [bun, 'test', '--max-concurrency=1', f'--timeout={timeout_ms}', '--reporter=junit', '--reporter-outfile=result.xml']
    if observer:
        args.extend(['--preload', str(observer)])
        env['MUTIE_BUN_OBSERVER'] = str(directory / 'observer.jsonl')
    p = subprocess.Popen(args, cwd=directory, env=env, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    expired = False
    try:
        stdout, stderr = p.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        expired = True
        os.killpg(p.pid, signal.SIGKILL)
        stdout, stderr = p.communicate()
    xml = directory / 'result.xml'
    content = xml.read_text() if xml.exists() else None
    tree = ET.fromstring(content) if content else None
    return {'exit_code': p.returncode, 'outer_timeout': expired,
            'duration_seconds': time.monotonic() - start,
            'stdout': stdout.decode(errors='replace'), 'stderr': stderr.decode(errors='replace'),
            'junit': content, 'suites': [x.attrib for x in tree.iter('testsuite')] if tree is not None else [],
            'cases': [{'attributes': x.attrib, 'children': [{'tag': c.tag, 'attributes': c.attrib, 'text': c.text} for c in x]} for x in tree.iter('testcase')] if tree is not None else []}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bun', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--inspector-node', help='Also capture inspector events using Node 24')
    args = parser.parse_args()
    bun = str(Path(args.bun).resolve())
    revision = subprocess.check_output([bun, '--revision'], text=True).strip()
    if revision != '1.3.11+af24e281e':
        raise SystemExit(f'Expected audited Bun 1.3.11+af24e281e, got {revision}')
    results = {'runtime': revision, 'executable': bun,
               'executable_sha256': hashlib.sha256(Path(bun).read_bytes()).hexdigest(), 'results': []}
    for syntax in ('js', 'ts'):
        for name, body in CASES.items():
            with tempfile.TemporaryDirectory(prefix='mutie-bun-audit-') as temporary:
                directory = Path(temporary)
                (directory / 'package.json').write_text('{"type":"module"}')
                typed = 'const typed: boolean = true;\n' if syntax == 'ts' else ''
                source = HEADER + typed + body + '\n'
                (directory / f'case.test.{syntax}').write_text(source)
                if name == 'second_file_import':
                    (directory / f'other.test.{syntax}').write_text('throw new Error("other import");\n')
                result = run(bun, directory, 5)
                if args.inspector_node:
                    inspector = directory / 'inspector.json'
                    subprocess.run([args.inspector_node, str(Path(__file__).with_name('inspect.mjs').resolve()),
                                    bun, str(directory), str(inspector)], check=True, timeout=10)
                    result['inspector'] = json.loads(inspector.read_text())
                results['results'].append({'syntax': syntax, 'scenario': name, 'source': source, **result})
                print(syntax, name, result['exit_code'], result['outer_timeout'], flush=True)
    Path(args.out).write_text(json.dumps(results, indent=2) + '\n')

if __name__ == '__main__':
    main()
