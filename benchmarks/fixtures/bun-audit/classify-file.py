"""Experimental file-backed attribution; not a production Mutie adapter."""
import xml.etree.ElementTree as ET


def _classify(run, observed):
    def error(reason):
        return {'outcome': 'error', 'reason': reason}
    if run.get('outer_timeout') or run.get('exit_code') not in (0, 1):
        return error('command did not complete normally')
    if not observed or observed[0].get('event') != 'init':
        return error('missing observer initialization')
    if observed[0].get('version') != '1.3.11' or observed[0].get('revision') != 'af24e281ebacd6ac77c0f14b4206599cf4ae1c9f':
        return error('wrong runtime identity')
    if any(e.get('sequence') != i for i, e in enumerate(observed, 1)):
        return error('observer sequence gap')
    if sum(e.get('event') == 'init' for e in observed) != 1:
        return error('duplicate observer initialization')
    if any(e.get('event') not in ('init', 'declare', 'enter', 'return', 'throw', 'reject', 'caught', 'unsupported', 'exit') for e in observed):
        return error('unknown observer event')
    if any(e.get('event') in ('unsupported', 'exit') for e in observed):
        return error('unsupported API or explicit exit')
    # Negative evidence only. User output can cause conservative errors, never
    # supply positive failure/completion evidence. Read complete raw captures.
    if 'Unhandled error between tests' in run['stderr']:
        return error('native unhandled error diagnostic')
    xml = run.get('junit')
    if not xml or len(xml.encode()) > 8 * 1024 * 1024 or '<!DOCTYPE' in xml or '<!ENTITY' in xml:
        return error('missing or unsupported JUnit report')
    try:
        root = ET.fromstring(xml)
    except ET.ParseError:
        return error('invalid JUnit report')
    if root.tag != 'testsuites' or any(s.tag != 'testsuite' for s in root):
        return error('invalid JUnit structure')
    cases = list(root.iter('testcase'))
    if not cases or any(c.tag not in ('testcase','testsuite','properties') for s in root.iter('testsuite') for c in s):
        return error('empty or unsupported JUnit suite')
    try:
        failures = sum(bool(list(c)) for c in cases)
        if int(root.get('tests','-1')) != len(cases) or int(root.get('failures','-1')) != failures or int(root.get('skipped','-1')) != 0:
            return error('JUnit aggregate mismatch')
        for suite in root.iter('testsuite'):
            entries = list(suite.iter('testcase'))
            if int(suite.get('tests','-1')) != len(entries) or int(suite.get('failures','-1')) != sum(bool(list(c)) for c in entries) or int(suite.get('skipped','-1')) != 0:
                return error('JUnit suite aggregate mismatch')
    except ValueError:
        return error('invalid JUnit counts')
    declarations = {e['testId']:e['name'] for e in observed if e['event'] == 'declare'}
    if len(declarations) != sum(e['event'] == 'declare' for e in observed):
        return error('duplicate declaration ID')
    if len(declarations) != len(cases):
        return error('declaration/report count mismatch')
    invocations, terminals, bodies, caught = {}, {}, [], {}
    for e in observed:
        event = e['event']
        if event == 'enter':
            if e['id'] in invocations or e['phase'] not in ('body','beforeEach','afterEach','beforeAll','afterAll'):
                return error('invalid invocation')
            invocations[e['id']] = e
            if e['phase'] == 'body':
                bodies.append(e)
        elif event == 'caught':
            begin = invocations.get(e['id'])
            if not begin or e['id'] in caught or e['id'] in terminals or e['kind'] not in ('throw','reject') or e['phase'] != 'body' or any(e[k] != begin[k] for k in ('phase','name')):
                return error('invalid provisional failure')
            caught[e['id']] = e['kind']
        elif event in ('return','throw','reject'):
            begin = invocations.get(e['id'])
            if not begin or e['id'] in terminals or any(e[k] != begin[k] for k in ('phase','name')):
                return error('invalid observer terminal')
            if e['phase'] == 'body' and event in ('throw','reject') and caught.get(e['id']) != event:
                return error('failure checkpoint mismatch')
            terminals[e['id']] = event
            if e['phase'] != 'body' and event != 'return':
                return error('hook failure')
    if len(bodies) != len(cases):
        return error('body/report execution count mismatch')
    if len({b['testId'] for b in bodies}) != len(bodies) or {b['testId']:b['name'] for b in bodies} != declarations:
        return error('registered/body identity mismatch')
    failed, timed_out = 0, False
    for body, case in zip(bodies, cases):
        if body['name'] != case.get('name'):
            return error('body/report name mismatch')
        terminal = terminals.get(body['id'])
        children = list(case)
        if not children and terminal == 'return':
            continue
        if len(children) != 1 or children[0].tag != 'failure':
            return error('unsupported or incomplete test result')
        kind = children[0].get('type')
        if kind == 'TimeoutError' and terminal is None and body['id'] not in caught:
            timed_out = True
        elif kind == 'AssertionError' and terminal in ('throw','reject'):
            failed += 1
        else:
            return error('failure lacks positive body evidence')
    for ident, begin in invocations.items():
        if ident not in terminals and begin['phase'] != 'body':
            return error('incomplete hook')
    if timed_out:
        return {'outcome':'timeout','reason':'native timeout with incomplete body'} if run['exit_code']==1 else error('timeout exit mismatch')
    if failed and run['exit_code']==1:
        return {'outcome':'body_failure','reason':'observed body failures match completed native results'}
    if not failed and run['exit_code']==0:
        return {'outcome':'passed','reason':'all observed bodies and hooks completed'}
    return error('exit status mismatch')


def classify(run, observed):
    try:
        return _classify(run, observed)
    except (KeyError, TypeError, ValueError, AttributeError):
        return {'outcome':'error','reason':'malformed evidence'}
