#!/usr/bin/env python3
"""Check the retained runner-audit gates without invoking or installing Bun."""
import json
from pathlib import Path
import sys

record = json.loads(Path(sys.argv[1]).read_text())
assert record['runtime'] == '1.3.11+af24e281e'
assert len(record['results']) == 48
for syntax in ('js', 'ts'):
    cases = {r['scenario']: r for r in record['results'] if r['syntax'] == syntax}
    assert len(cases) == 24
    for case in cases.values():
        assert not case['outer_timeout']
        inspector = case['inspector']
        assert not inspector['expired']
        assert not inspector['drainTimedOut']
        assert inspector['code'] == case['exit_code']
        assert len(inspector['commands']) == 5
        assert not any('controllerError' in e or 'error' in e for e in inspector['events'])
    def events(name):
        return [e for e in cases[name]['inspector']['events'] if e.get('method', '').startswith(('TestReporter.', 'LifecycleReporter.'))]
    def shape(name):
        return [(e['method'], e['params'].get('status')) for e in events(name)]
    def failure_type(name):
        return cases[name]['cases'][0]['children'][0]['attributes']['type']
    for name in ('body_throw', 'body_reject', 'before_each', 'after_each', 'async_cleanup', 'unhandled_rejection', 'late_exception'):
        assert shape(name) == shape('body_throw')
        assert failure_type(name) == 'AssertionError'
    assert shape('shared_error_body') == shape('shared_error_hook')
    assert cases['pass']['exit_code'] == 0
    assert shape('pass')[-1] == ('TestReporter.end', 'pass')
    assert shape('exit_zero')[-1][0] == 'TestReporter.start'
    assert cases['exit_zero']['exit_code'] == 0
    assert shape('empty') == []
    assert sum(e['method'] == 'TestReporter.found' for e in events('only')) == 2
    assert sum(e['method'] == 'TestReporter.end' for e in events('only')) == 1
    assert shape('pending')[-1] == ('TestReporter.end', 'timeout')
    assert failure_type('pending') == 'TimeoutError'
    assert sum(e['method'] == 'LifecycleReporter.error' for e in events('body_and_cleanup')) == 2
    assert shape('after_all')[-1][0] == 'LifecycleReporter.error'
print('Verified 48 JS/TS cases: ambiguous failure attribution, completion and timeout controls.')
