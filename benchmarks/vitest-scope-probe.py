"""Direct Vitest scope probes; not a Mutie adapter acceptance test.

Usage: python3 benchmarks/vitest-scope-probe.py PREPARED_FIXTURE OUTPUT_JSON [NODE]
PREPARED_FIXTURE is tests/fixtures/vitest with dependencies installed.
All changes and executions happen in a disposable copy.
"""
import json, subprocess, time, shutil, sys, tempfile
from pathlib import Path
scratch=tempfile.TemporaryDirectory(prefix='mutie-vitest-scope-')
p=Path(scratch.name)/'project'
shutil.copytree(Path(sys.argv[1]).resolve(), p, symlinks=True)
node=sys.argv[3] if len(sys.argv)>3 else 'node'
original={str(f.relative_to(p)): f.read_text() for d in ['src','test'] for f in (p/d).glob('*')}
results=[]
def run(name,changes={},extra=[]):
 for f,s in original.items(): (p/f).write_text(s)
 for f in ['test/probe.test.js','vitest.config.mjs']:
  (p/f).unlink(missing_ok=True)
 for f,s in changes.items(): (p/f).write_text(s)
 out=p/(name+'.json'); out.unlink(missing_ok=True)
 start=time.monotonic()
 r=subprocess.run([node,'node_modules/vitest/vitest.mjs','run','--reporter=json','--outputFile='+str(out),*extra],cwd=p,capture_output=True,text=True,timeout=30)
 d=json.loads(out.read_text()) if out.exists() else {}
 results.append({'case':name,'exit':r.returncode,'seconds':round(time.monotonic()-start,3),'report_present':out.exists(),**{k:d.get(k) for k in ['success','numTotalTests','numPassedTests','numFailedTests']},'files':[{'name':Path(f['name']).name,'status':f['status'],'message':f['message'],'failures':[t['fullName'] for t in f['assertionResults'] if t['status']=='failed']} for f in d.get('testResults',[])],'stderr':r.stderr})
run('baseline')
for ext in ['js','ts']:
 key='src/flags.'+ext
 run(ext+'-killed',{key:original[key].replace('return true','return false',1)})
 survivor=original[key].replace('return false','return true')
 run(ext+'-missed',{key:survivor})
 test='test/flags.'+ext+'.test.'+ext
 run(ext+'-stronger',{key:survivor,test:original[test]+'\ntest("fallback rejects false", () => { expect(fallback(false)).toBe(false); });\n'})
run('transform-error',{'src/flags.ts':'export function invalid( {\n'})
run('no-tests',extra=['no-such-test-filter'])
run('unhandled',{'test/probe.test.js':'import {test,expect} from "vitest"; test("unhandled", async () => { Promise.reject(new Error("unhandled-probe")); await new Promise(r => setTimeout(r, 30)); expect(true).toBe(true); });\n'})
run('config-error',{'vitest.config.mjs':'throw new Error("config-probe");\n'})
run('all-skipped',{f:s.replace('test(', 'test.skip(') for f,s in original.items() if f.startswith('test/')})
for f,s in original.items(): (p/f).write_text(s)
for f in ['test/probe.test.js','vitest.config.mjs']: (p/f).unlink(missing_ok=True)
Path(sys.argv[2]).write_text(json.dumps(results,indent=2).replace(str(p), '<fixture>')+'\n')
expected = {
 'baseline': (0, 4, 0), 'js-killed': (1, 3, 1), 'js-missed': (0, 4, 0),
 'js-stronger': (1, 4, 1), 'ts-killed': (1, 3, 1), 'ts-missed': (0, 4, 0),
 'ts-stronger': (1, 4, 1), 'transform-error': (1, 2, 0),
 'no-tests': (1, 0, 0), 'unhandled': (1, 5, 0),
 'config-error': (1, None, None), 'all-skipped': (0, 0, 0),
}
for result in results:
 assert (result['exit'], result['numPassedTests'], result['numFailedTests']) == expected[result['case']], result
scratch.cleanup()
print(json.dumps([{k:r[k] for k in ['case','exit','success','numPassedTests','numFailedTests']} for r in results],indent=2))
