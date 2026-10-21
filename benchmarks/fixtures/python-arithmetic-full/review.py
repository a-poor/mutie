"""Retain every candidate disposition, including unresolved numerical behavior."""
import json
from pathlib import Path
import re
import shutil
import sys

output, project, previous_path=map(Path,sys.argv[1:4])
state=json.loads((output/'results.json').read_text())
previous=json.loads(previous_path.read_text())
def identity(m):
    return tuple(m[k] for k in ('file','source_fingerprint','operator','byte_start','byte_end','original','replacement'))
ids={identity(m):m['id'] for m in state['canonical']}
prior_kills={}
for r in previous['records']:
    if r['label']!='stronger-focused':continue
    local={m['id']:ids[identity(m)] for m in r['inventory']['mutations']}
    for o in r['outcomes']['mutations']:
        assert o['state']=='completed' and o['outcome']=='killed'
        prior_kills[local[o['id']]]=dict(report=r['report'],local_id=o['id'])
notes={}
def assign(keys,note):
    for key in keys:notes[key]=note
assign([4,46], 'Open: changed power-search midpoint. Favorite ordering alone is weaker than an independent exponent/accuracy oracle; retained focused outcome records whether the check distinguishes it.')
assign([6], 'Open: power-search total becomes a difference; normalized results do not establish the intended exponent.')
assign([7,30,48,62], 'Open: convergence residual changes, often forcing more bounded iterations; no equivalence claim or exact-output oracle.')
assign([11,13], 'Unequal additive inputs expose wrong equal-margin removal.')
assign([17,18], 'Extreme valid skew reaches the additive floor and requires final normalization; expected-success assertion retains error classification rules.')
assign([19,31,32,50,65], 'Unexercised zero-iteration/zero-z branch: ordinary public calls use 100 iterations with positive midpoints. No general helper equivalence claim.')
assign([20,21,24,25,51,52,54,55,57,58,59], 'Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added.')
assign([26], 'Limited ordinary-path redundancy: total is consumed at z=0, while the default public search keeps z positive. Private/exceptional cases are not declared equivalent.')
assign([27,28,29,60,61], 'Open: Shin search changes; normalized outputs and ordering alone do not validate the intended search or bias.')
assign([36,37,38,39,40,41,42,43,68], 'Exact original-margin field checked independently of the selected method.')
assign([56], 'Inverse-probability arithmetic reverses unequal multi-outcome ordering.')
assign([63,64,67], 'Open/unexercised numerical fallback normalization; normal positive inputs did not trigger this branch. Not declared equivalent.')
assign([104], 'Valid wrong endpoint must be clamped to finite log loss; expected-success assertion catches ValueError.')
assign([107,111,113,114,115,116,117,118,119,121], 'Exact calibration bin boundaries, counts, means, gaps and weighted error distinguish altered arithmetic.')
assign([112], 'Open: exact probability 1 must enter the final calibration bin; the new calibration examples are interior values and do not exercise this clamp.')
assign([155], 'Open: returned raw edge is not checked independently by this stronger selection; original sign/range checks leave the changed subtraction undetected.')
assign([124,125], 'Identical-edge branch must return rates/means, not unnormalized totals.')
assign([126,127,132,133,135,136,137,138,139,140,141,142], 'Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages.')
assign([160,161,162,164,165,166,167], 'Non-even odds and uncapped sizing distinguish Kelly magnitudes.')
assign([171,173,174,176], 'Two- and three-trial public percentiles check adjacent interpolation and weights.')
assign([172], 'Limited public-path redundancy: for ordinary list sizes the fixed 5/95 percentiles keep lo+1 within n-1, so raising the cap does not alter the index. No general helper/huge-list equivalence claim.')
assign([177], 'Open: changed median index can select the wrong neighbor or exceed a two-item list. Not included in the stronger kill selection.')
assign([178,179,180,182,183,184,185,186,187,188,190], 'Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks.')
assign([191], 'Fraction-search results must match standalone runs using the documented incrementing sub-seeds.')
lines=['# All 194 arithmetic candidates','',
'Canonical IDs are arithmetic-only source order, not report-local IDs. The driver translates them by exact source identity. Prior kills are separate-copy evidence from the first 30-site acceptance. All original outcomes remain unchanged; limited redundancy observations are not dismissals or broad equivalence claims.','',
'| ID | Source | Change | Original | Stronger evidence | Review |',
'| ---: | --- | --- | --- | --- | --- |']
errors=[]
error_dir=output/'error-logs';error_dir.mkdir(exist_ok=True)
for m in state['canonical']:
    i=m['id'];a=state['accounting'][str(i)];r=state.get('review_accounting', {}).get(str(i));note=notes.get(i)
    if a['outcome']=='killed':note='Detected by the complete unchanged original suite.'
    if i in prior_kills:note='Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning.'
    if a['outcome']=='error':
        source=Path(a['report'])/'logs'/f"{a['local_id']:06d}.log"
        text=source.read_text();shutil.copy2(source,error_dir/f'{i:03d}.log')
        types=sorted(set(re.findall(r'^E\s+(?:[\w.]+\.)?([A-Za-z_]+(?:Error|Exception)):',text,re.M)))
        failures=[line for line in text.splitlines() if line.startswith('FAILED ')]
        assert types, ('inspect non-native error',i)
        detail=dict(id=i,file=m['file'],line=m['line'],exceptions=types,failed_tests=failures,
                    report=a['report'],local_id=a['local_id'],log=f'error-logs/{i:03d}.log')
        errors.append(detail)
        note='Retained native '+', '.join(types)+'. Full-suite failure evidence is in the error review; no relabeling as a kill.'
    assert note, ('review needed',i,a['outcome'])
    stronger=('new '+r['outcome']) if r else ('prior kill' if i in prior_kills else '—')
    lines.append(f"| {i} | {Path(m['file']).name}:{m['line']} | `{m['original']}` → `{m['replacement']}` | {a['outcome']} | {stronger} | {note} |")
(output/'candidate-review.md').write_text('\n'.join(lines)+'\n')
(output/'error-review.json').write_text(json.dumps(errors,indent=2)+'\n')
(output/'prior-stronger-mapping.json').write_text(json.dumps(prior_kills,indent=2)+'\n')
print(f'Reviewed {len(state["canonical"])} candidates and retained {len(errors)} original error logs.')
