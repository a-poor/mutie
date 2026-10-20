"""Alternate full-content tree scans in one pinned interpreter (warm filesystem)."""
import argparse
import importlib.util
import json
from pathlib import Path
import sysconfig
import time

parser = argparse.ArgumentParser()
parser.add_argument('--before', type=Path, required=True)
parser.add_argument('--after', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
modules = {}
for name in ('before', 'after'):
    spec = importlib.util.spec_from_file_location(name, getattr(args, name))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    modules[name] = module
assert modules['before'].identify() == modules['after'].identify()
roots = [('stdlib', Path(sysconfig.get_path('stdlib')), False),
         ('packages', Path(sysconfig.get_path('purelib')), True)]
expected = {name: modules['before'].fingerprint_tree(root, site=site)
            for name, root, site in roots}
records = []
for repeat in range(1, 6):
    for mode in (['before', 'after'] if repeat % 2 else ['after', 'before']):
        phases = {}
        for name, root, site in roots:
            start = time.perf_counter()
            actual = modules[mode].fingerprint_tree(root, site=site)
            phases[name] = time.perf_counter() - start
            assert actual == expected[name], 'runtime changed or fingerprint incompatible'
        records.append(dict(mode=mode, repeat=repeat, phases=phases,
                            seconds=sum(phases.values())))
args.output.write_text(json.dumps(dict(fingerprints=expected, records=records), indent=2) + '\n')
