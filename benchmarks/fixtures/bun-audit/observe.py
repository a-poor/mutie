#!/usr/bin/env python3
"""Compare native Bun controls with the experimental phase observer."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
probe = runpy.run_path(str(HERE/'probe.py'))
classify = runpy.run_path(str(HERE/'classify-file.py'))['classify']
CASES = dict(probe['CASES'])
CASES.update({
 'hooks_pass': 'beforeEach(() => {}); afterEach(() => {}); test("body", () => {});',
 'async_pass': 'test("body", async () => { await Bun.sleep(1); expect(true).toBe(true); });',
 'body_failure_clean_hooks': 'beforeEach(() => {}); afterEach(() => {}); test("body", () => { throw new Error("body"); });',
 'async_hooks_pass': 'beforeEach(async () => { await Bun.sleep(1); }); afterEach(async () => { await Bun.sleep(1); }); test("body", async () => { await Bun.sleep(1); });',
 'async_body_and_cleanup': 'afterEach(async () => { throw new Error("cleanup"); }); test("body", async () => { throw new Error("body"); });',
 'hook_assertion': 'afterEach(() => expect(true).toBe(false)); test("body", () => {});',
 'two_bodies': 'test("first", () => {throw new Error("body")}); test("second", () => {});',
 'duplicate_names': 'test("body", () => {}); test("body", () => {throw new Error("body")});',
 'nested_body': 'describe("nested", () => {test("body", () => {throw new Error("body")});});',
 'aliased_it': 'const {it: example} = await import("bun:test"); example("body", () => {throw new Error("body")});',
 'primitive_throw': 'test("body", () => { throw "primitive"; });',
 'same_error_process': 'const failure = new Error("shared"); test("body", async () => { Promise.reject(failure); await Bun.sleep(10); throw failure; });',
 'late_cleanup_process': 'afterEach(async () => { Promise.reject(new Error("cleanup")); await Bun.sleep(10); }); test("body", () => {throw new Error("body")});',
 'done': 'test("body", done => { done(); });',
 'thenable': 'test("body", () => ({then(resolve) { resolve(); }}));',
 'each': 'test.each([1,2])("body %i", value => {expect(value).toBeGreaterThan(0);});',
 'concurrent': 'test.concurrent("body", async () => {await Bun.sleep(1)});',
 'retry': 'test("body", () => {}, {retry: 1});',
 'describe_throw': 'describe("suite", () => {throw new Error("declaration")});',
 'process_exit_after_pass': 'test("body", () => {}); afterAll(() => process.exit(0));',
 'body_after_all_process': 'afterAll(async () => { Promise.reject(new Error("cleanup")); await Bun.sleep(10); }); test("body", () => {throw new Error("body")});',
 'body_and_rejection': 'test("body", () => {Promise.reject(new Error("process")); throw new Error("body")});',
 'body_and_timer': 'afterAll(async () => {await Bun.sleep(20)}); test("body", () => {setTimeout(() => {throw new Error("process")}, 0); throw new Error("body")});',
 'shared_error_both': 'const failure=new Error("same"); test("body", () => {Promise.reject(failure); throw failure});',
 'top_throw_after_declare': 'test("body", () => {throw new Error("body")}); throw new Error("import");',
 'nested_hooks_pass': 'describe("outer", () => {beforeEach(() => {}); describe("inner", () => {afterEach(() => {}); test("body", () => {});});});',
 'timer_order': 'let state=false; afterEach(()=>expect(state).toBe(false)); test("body",()=>{setTimeout(()=>state=true,0)});',
 'microtask_order': 'let state=false; afterEach(()=>expect(state).toBe(true)); test("body",()=>{queueMicrotask(()=>state=true)});',
 'before_each_timer': 'let state=false; beforeEach(()=>{setTimeout(()=>state=true,0)}); test("body",()=>expect(state).toBe(false));',
 'checkpoint_during_each_cleanup': 'afterEach(async()=>{await Bun.sleep(20)}); test("body",()=>{Promise.reject(new Error("process")); throw new Error("body");});',
 'checkpoint_during_all_cleanup': 'afterAll(async()=>{await Bun.sleep(20)}); test("body",()=>{Promise.reject(new Error("process")); throw new Error("body");});',
 'mock_api': 'const {mock} = await import("bun:test"); test("body", () => {mock(() => {})();});',
})
POSITIVE = {'assertion','body_throw','body_reject','shared_error_body','body_failure_clean_hooks',
            'two_bodies','duplicate_names','nested_body','aliased_it','primitive_throw'}
PASS = {'pass','hooks_pass','async_pass','async_hooks_pass','nested_hooks_pass','timer_order','microtask_order','before_each_timer'}


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--bun',required=True);p.add_argument('--out',required=True)
    args=p.parse_args();bun=str(Path(args.bun).resolve())
    assert subprocess.check_output([bun,'--revision'],text=True).strip()=='1.3.11+af24e281e'
    results={'runtime':'1.3.11+af24e281e','observer_sha256':hashlib.sha256((HERE/'observer.mjs').read_bytes()).hexdigest(),'recipe_files_sha256':{name:hashlib.sha256((HERE/name).read_bytes()).hexdigest() for name in ('observer.mjs','classify-file.py','probe.py')},'results':[]}
    for syntax in ('js','ts'):
        for name,body in CASES.items():
            with tempfile.TemporaryDirectory(prefix='mutie-bun-observe-') as temp:
                root=Path(temp)
                (root/'package.json').write_text('{"type":"module"}')
                source=probe['HEADER']+('const typed: boolean = true;\n' if syntax=='ts' else '')+body+'\n'
                (root/f'case.test.{syntax}').write_text(source)
                if name=='second_file_import':(root/f'other.test.{syntax}').write_text('throw new Error("other import");\n')
                control=probe['run'](bun,root,5)
                run=probe['run'](bun,root,5,HERE/'observer.mjs')
                log=root/'observer.jsonl'
                events=[json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
                outcome=classify(run,events)
                expected='body_failure' if name in POSITIVE else 'passed' if name in PASS else 'timeout' if name=='pending' else 'error'
                results['results'].append({'syntax':syntax,'scenario':name,'source':source,'control':control,
                    'run':run,'observer':events,'classification':outcome,'expected':expected})
                print(syntax,name,outcome, 'exit',control['exit_code'],run['exit_code'],flush=True)
    Path(args.out).write_text(json.dumps(results,indent=2)+'\n')
    failures=[(r['syntax'],r['scenario'],r['classification']) for r in results['results'] if r['classification']['outcome']!=r['expected'] or r['control']['exit_code']!=r['run']['exit_code']]
    if failures:raise SystemExit(str(failures))

if __name__=='__main__':main()
