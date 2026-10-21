"""Render the complete source-identity review, keeping unresolved cases explicit."""
import json
from pathlib import Path
import shutil
import sys
out=Path(sys.argv[1]);state=json.loads((out/'results.json').read_text())
notes={
1:'Unresolved: exact convergence-tolerance equality; no distinguishing float input demonstrated.',
2:'Equality is caught by the preceding convergence check for ordinary finite inputs; not a general numeric-type equivalence claim.',
3:'Unreachable equality in this function: z is a midpoint of bounds initially [0, 0.5], and updates remain within those bounds.',
4:'Unresolved: exact Shin convergence-tolerance equality.',
5:'Equality is caught by the preceding convergence check for ordinary finite inputs.',
6:'At z=0 the helper returns the same normalization; limited to ordinary numeric inputs with valid total.',
7:'Unresolved: zero normalization guard and exceptional/extreme numeric inputs.',
8:'Unresolved: exact multi-outcome convergence-tolerance equality.',
9:'Equality is caught by the preceding convergence check for ordinary finite inputs.',
10:'Unresolved: exact multi-outcome Shin convergence-tolerance equality.',
11:'Equality is caught by the preceding convergence check for ordinary finite inputs.',
12:'At z=0 the helper returns the same normalization; limited to ordinary numeric inputs with valid total.',
13:'Unresolved: zero multi-outcome normalization guard and exceptional/extreme inputs.',
14:'Existing two-outcome and unsupported-method tests raise unexpected ValueError; retained error.',
16:'Valid zero endpoint rejected with InvalidProbabilityError; explicit expected-success assertion in separate copy.',
17:'Valid one endpoint rejected with InvalidProbabilityError; explicit expected-success assertion in separate copy.',
20:'Full takeout must return zero without division by zero.',
21:'For finite valid other operands, pool_size=0 gives zero through either path; infinities/NaNs not established equivalent.',
23:'For finite valid other operands, impact_limit=0 gives zero through either path; infinities/NaNs not established equivalent.',
25:'No-edge result must not claim a pool reduction.',
26:'No-edge result must not claim a bankroll cap.',
27:'Unconstrained result must not claim a pool reduction.',
28:'Zero pool size is ignored by the current optional pool constraint.',
29:'Exact pool limit must not claim a reduction.',
31:'Unconstrained result must not claim a bankroll cap.',
32:'Exact bankroll cap must not claim a reduction.',
34:'Exact minimum bet remains accepted.',
37:'Exact aggregate exposure limit must not claim scaling.',
38:'Unresolved: zero-total secondary guard; nonnegative exposure limit suppresses it, broader parameter values remain unproved.',
39:'Race-level reduction must set capped flag.',
40:'Existing perfect/worst-prediction tests raise InvalidProbabilityError at zero; retained error.',
41:'Existing perfect/worst-prediction tests raise InvalidProbabilityError at one; retained error.',
42:'Length check precedes zip: redundant strictness for ordinary unchanged lists; no claim for effectful custom sequences.',
43:'Length check precedes zip: redundant strictness for ordinary unchanged lists; no claim for effectful custom sequences.',
44:'Existing single-bin tests raise ValueError; retained error.',
45:'Length check precedes zip: redundant strictness for ordinary unchanged lists; no claim for effectful custom sequences.',
46:'Probability one must clamp into the final calibration bin; prior finding reconfirmed.',
47:'Opening probability zero is a valid closed endpoint; prior finding reconfirmed.',
48:'Opening probability one is a valid closed endpoint; prior finding reconfirmed.',
49:'Closing probability zero is a valid closed endpoint; prior finding reconfirmed.',
50:'Closing probability one is a valid closed endpoint; prior finding reconfirmed.',
51:'One edge-analysis bin remains valid; prior finding reconfirmed.',
52:'Length checks precede zip: redundant strictness for ordinary unchanged lists; no claim for effectful custom sequences.',
53:'Existing edge-bucket test raises IndexError at maximum edge; retained error.',
54:'Open finding: valid zero probability endpoint rejected; no stronger rerun in this review.',
55:'Open finding: valid one probability endpoint rejected; no stronger rerun in this review.',
56:'Mixed expected-exception assertion and ZeroDivisionError; error precedence remains intact.',
57:'Zero already rejected by American-odds validation; equality unreachable for ordinary integer input.',
59:'Zero bypasses expected validation and raises ZeroDivisionError; retained error.',
60:'Closed probability-one endpoint returns decimal odds 1.0.',
61:'At zero Kelly, min(kelly * 0.5, 0.25) also returns zero for ordinary float inputs.',
62:'Zero Kelly must not select a bet even when minimum edge is zero.',
63:'Exact minimum edge remains admissible.',
66:'Simulation result must remain frozen.',
71:'Open finding: one bet is documented valid; no focused rerun of this candidate.',
72:'Open finding: one trial is documented valid; no focused rerun of this candidate.',
74:'Unresolved seeded random-draw equality; rare does not imply equivalent.',
75:'Exact ruin threshold counts as ruin.',
77:'Equal br/peak assignment preserves the ordinary numeric value; limited numeric-domain redundancy.',
78:'Positive initial bankroll keeps peak positive for ordinary valid finite inputs; no broader numeric-domain claim.',
79:'Unresolved zero-bankroll log guard: ordinary ruin handling precedes it, unusual ruin thresholds remain unproved.',
80:'Positive initial bankroll already validated for ordinary finite inputs.',
81:'Open finding: outer edge validation can matter with an empty fractions list; delegated validation otherwise rejects zero. No stronger rerun.',
82:'Open finding: outer odds validation can matter with an empty fractions list; delegated validation otherwise rejects one. No stronger rerun.',
83:'Two-outcome result must remain frozen.',
84:'Unresolved: exact normalization-tolerance equality and representable inputs.',
85:'Multi-outcome result must remain frozen.',
86:'Unresolved: exact normalization-tolerance equality and representable inputs.',
87:'Edge result must remain frozen.',
}
lines=['# Complete betting-math-kit candidate review','',
'IDs refer to the full 87-site inventory, not report-local selection IDs. Domain-limited redundancy observations are not saved dismissals or broad equivalence claims. Original outcomes remain unchanged.','',
'| ID | Source | Change | Original | Stronger rerun | Review |',
'| ---: | --- | --- | --- | --- | --- |']
error_dir=out/'review-error-logs';error_dir.mkdir(exist_ok=True)
for m in state['records'][0]['inventory']['mutations']:
 i=m['id'];a=state['accounting'][str(i)];review=state['review_accounting'].get(str(i))
 note=notes.get(i)
 if a['outcome']=='killed':note='Detected by unchanged original suite.'
 if a['outcome']=='error':
  source=Path(a['report'])/'logs'/f"{a['local_id']:06d}.log"
  shutil.copy2(source,error_dir/f'{i:03d}.log')
  assert note, ('unreviewed error',i)
 assert note, ('unreviewed survivor',i)
 lines.append(f"| {i} | {Path(m['file']).name}:{m['line']} | `{m['original']}` → `{m['replacement']}` | {a['outcome']} | {review['outcome'] if review else '—'} | {note} |")
(out/'candidate-review.md').write_text('\n'.join(lines)+'\n')
