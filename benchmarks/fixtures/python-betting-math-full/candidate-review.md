# Complete betting-math-kit candidate review

IDs refer to the full 87-site inventory, not report-local selection IDs. Domain-limited redundancy observations are not saved dismissals or broad equivalence claims. Original outcomes remain unchanged.

| ID | Source | Change | Original | Stronger rerun | Review |
| ---: | --- | --- | --- | --- | --- |
| 1 | devig.py:70 | `<` → `<=` | missed | — | Unresolved: exact convergence-tolerance equality; no distinguishing float input demonstrated. |
| 2 | devig.py:72 | `>` → `>=` | missed | — | Equality is caught by the preceding convergence check for ordinary finite inputs; not a general numeric-type equivalence claim. |
| 3 | devig.py:120 | `>=` → `>` | missed | — | Unreachable equality in this function: z is a midpoint of bounds initially [0, 0.5], and updates remain within those bounds. |
| 4 | devig.py:127 | `<` → `<=` | missed | — | Unresolved: exact Shin convergence-tolerance equality. |
| 5 | devig.py:129 | `>` → `>=` | missed | — | Equality is caught by the preceding convergence check for ordinary finite inputs. |
| 6 | devig.py:136 | `>` → `>=` | missed | — | At z=0 the helper returns the same normalization; limited to ordinary numeric inputs with valid total. |
| 7 | devig.py:144 | `>` → `>=` | missed | — | Unresolved: zero normalization guard and exceptional/extreme numeric inputs. |
| 8 | devig.py:329 | `<` → `<=` | missed | — | Unresolved: exact multi-outcome convergence-tolerance equality. |
| 9 | devig.py:331 | `>` → `>=` | missed | — | Equality is caught by the preceding convergence check for ordinary finite inputs. |
| 10 | devig.py:380 | `<` → `<=` | missed | — | Unresolved: exact multi-outcome Shin convergence-tolerance equality. |
| 11 | devig.py:382 | `>` → `>=` | missed | — | Equality is caught by the preceding convergence check for ordinary finite inputs. |
| 12 | devig.py:390 | `>` → `>=` | missed | — | At z=0 the helper returns the same normalization; limited to ordinary numeric inputs with valid total. |
| 13 | devig.py:399 | `>` → `>=` | missed | — | Unresolved: zero multi-outcome normalization guard and exceptional/extreme inputs. |
| 14 | devig.py:434 | `<` → `<=` | error | — | Existing two-outcome and unsupported-method tests raise unexpected ValueError; retained error. |
| 15 | kelly.py:59 | `True` → `False` | killed | — | Detected by unchanged original suite. |
| 16 | kelly.py:95 | `<` → `<=` | error | killed | Valid zero endpoint rejected with InvalidProbabilityError; explicit expected-success assertion in separate copy. |
| 17 | kelly.py:95 | `>` → `>=` | error | killed | Valid one endpoint rejected with InvalidProbabilityError; explicit expected-success assertion in separate copy. |
| 18 | kelly.py:97 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 19 | kelly.py:99 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 20 | kelly.py:140 | `<=` → `<` | missed | killed | Full takeout must return zero without division by zero. |
| 21 | kelly.py:171 | `<=` → `<` | missed | — | For finite valid other operands, pool_size=0 gives zero through either path; infinities/NaNs not established equivalent. |
| 22 | kelly.py:171 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 23 | kelly.py:171 | `<=` → `<` | missed | — | For finite valid other operands, impact_limit=0 gives zero through either path; infinities/NaNs not established equivalent. |
| 24 | kelly.py:247 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 25 | kelly.py:255 | `False` → `True` | missed | killed | No-edge result must not claim a pool reduction. |
| 26 | kelly.py:256 | `False` → `True` | missed | killed | No-edge result must not claim a bankroll cap. |
| 27 | kelly.py:263 | `False` → `True` | missed | killed | Unconstrained result must not claim a pool reduction. |
| 28 | kelly.py:264 | `>` → `>=` | missed | killed | Zero pool size is ignored by the current optional pool constraint. |
| 29 | kelly.py:266 | `>` → `>=` | missed | killed | Exact pool limit must not claim a reduction. |
| 30 | kelly.py:268 | `True` → `False` | killed | — | Detected by unchanged original suite. |
| 31 | kelly.py:270 | `False` → `True` | missed | killed | Unconstrained result must not claim a bankroll cap. |
| 32 | kelly.py:272 | `>` → `>=` | missed | killed | Exact bankroll cap must not claim a reduction. |
| 33 | kelly.py:274 | `True` → `False` | killed | — | Detected by unchanged original suite. |
| 34 | kelly.py:276 | `<` → `<=` | missed | killed | Exact minimum bet remains accepted. |
| 35 | kelly.py:344 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 36 | kelly.py:358 | `>` → `>=` | killed | — | Detected by unchanged original suite. |
| 37 | kelly.py:364 | `>` → `>=` | missed | killed | Exact aggregate exposure limit must not claim scaling. |
| 38 | kelly.py:364 | `>` → `>=` | missed | — | Unresolved: zero-total secondary guard; nonnegative exposure limit suppresses it, broader parameter values remain unproved. |
| 39 | kelly.py:378 | `True` → `False` | missed | killed | Race-level reduction must set capped flag. |
| 40 | metrics.py:24 | `<` → `<=` | error | — | Existing perfect/worst-prediction tests raise InvalidProbabilityError at zero; retained error. |
| 41 | metrics.py:24 | `>` → `>=` | error | — | Existing perfect/worst-prediction tests raise InvalidProbabilityError at one; retained error. |
| 42 | metrics.py:92 | `False` → `True` | missed | — | Length check precedes zip: redundant strictness for ordinary unchanged lists; no claim for effectful custom sequences. |
| 43 | metrics.py:129 | `False` → `True` | missed | — | Length check precedes zip: redundant strictness for ordinary unchanged lists; no claim for effectful custom sequences. |
| 44 | metrics.py:181 | `<` → `<=` | error | — | Existing single-bin tests raise ValueError; retained error. |
| 45 | metrics.py:191 | `False` → `True` | missed | — | Length check precedes zip: redundant strictness for ordinary unchanged lists; no claim for effectful custom sequences. |
| 46 | metrics.py:194 | `>=` → `>` | missed | killed | Probability one must clamp into the final calibration bin; prior finding reconfirmed. |
| 47 | metrics.py:282 | `<` → `<=` | missed | killed | Opening probability zero is a valid closed endpoint; prior finding reconfirmed. |
| 48 | metrics.py:282 | `>` → `>=` | missed | killed | Opening probability one is a valid closed endpoint; prior finding reconfirmed. |
| 49 | metrics.py:286 | `<` → `<=` | missed | killed | Closing probability zero is a valid closed endpoint; prior finding reconfirmed. |
| 50 | metrics.py:286 | `>` → `>=` | missed | killed | Closing probability one is a valid closed endpoint; prior finding reconfirmed. |
| 51 | metrics.py:371 | `<` → `<=` | missed | killed | One edge-analysis bin remains valid; prior finding reconfirmed. |
| 52 | metrics.py:401 | `False` → `True` | missed | — | Length checks precede zip: redundant strictness for ordinary unchanged lists; no claim for effectful custom sequences. |
| 53 | metrics.py:403 | `>=` → `>` | error | — | Existing edge-bucket test raises IndexError at maximum edge; retained error. |
| 54 | odds.py:32 | `<` → `<=` | missed | — | Open finding: valid zero probability endpoint rejected; no stronger rerun in this review. |
| 55 | odds.py:32 | `>` → `>=` | missed | — | Open finding: valid one probability endpoint rejected; no stronger rerun in this review. |
| 56 | odds.py:38 | `<=` → `<` | error | — | Mixed expected-exception assertion and ZeroDivisionError; error precedence remains intact. |
| 57 | odds.py:66 | `>` → `>=` | missed | — | Zero already rejected by American-odds validation; equality unreachable for ordinary integer input. |
| 58 | odds.py:91 | `>=` → `>` | killed | — | Detected by unchanged original suite. |
| 59 | odds.py:133 | `<=` → `<` | error | — | Zero bypasses expected validation and raises ZeroDivisionError; retained error. |
| 60 | odds.py:133 | `>` → `>=` | missed | killed | Closed probability-one endpoint returns decimal odds 1.0. |
| 61 | odds.py:263 | `<=` → `<` | missed | — | At zero Kelly, min(kelly * 0.5, 0.25) also returns zero for ordinary float inputs. |
| 62 | odds.py:318 | `<=` → `<` | missed | killed | Zero Kelly must not select a bet even when minimum edge is zero. |
| 63 | odds.py:318 | `<` → `<=` | missed | killed | Exact minimum edge remains admissible. |
| 64 | odds.py:319 | `False` → `True` | killed | — | Detected by unchanged original suite. |
| 65 | odds.py:322 | `True` → `False` | killed | — | Detected by unchanged original suite. |
| 66 | simulation.py:37 | `True` → `False` | killed | — | Detected by unchanged original suite. |
| 67 | simulation.py:76 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 68 | simulation.py:78 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 69 | simulation.py:80 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 70 | simulation.py:82 | `<=` → `<` | killed | — | Detected by unchanged original suite. |
| 71 | simulation.py:171 | `<` → `<=` | missed | — | Open finding: one bet is documented valid; no focused rerun of this candidate. |
| 72 | simulation.py:173 | `<` → `<=` | missed | — | Open finding: one trial is documented valid; no focused rerun of this candidate. |
| 73 | simulation.py:193 | `False` → `True` | killed | — | Detected by unchanged original suite. |
| 74 | simulation.py:197 | `<` → `<=` | missed | — | Unresolved seeded random-draw equality; rare does not imply equivalent. |
| 75 | simulation.py:202 | `<=` → `<` | missed | killed | Exact ruin threshold counts as ruin. |
| 76 | simulation.py:204 | `True` → `False` | killed | — | Detected by unchanged original suite. |
| 77 | simulation.py:207 | `>` → `>=` | missed | — | Equal br/peak assignment preserves the ordinary numeric value; limited numeric-domain redundancy. |
| 78 | simulation.py:217 | `>` → `>=` | missed | — | Positive initial bankroll keeps peak positive for ordinary valid finite inputs; no broader numeric-domain claim. |
| 79 | simulation.py:219 | `>` → `>=` | missed | — | Unresolved zero-bankroll log guard: ordinary ruin handling precedes it, unusual ruin thresholds remain unproved. |
| 80 | simulation.py:219 | `>` → `>=` | missed | — | Positive initial bankroll already validated for ordinary finite inputs. |
| 81 | simulation.py:339 | `<=` → `<` | missed | — | Open finding: outer edge validation can matter with an empty fractions list; delegated validation otherwise rejects zero. No stronger rerun. |
| 82 | simulation.py:341 | `<=` → `<` | missed | — | Open finding: outer odds validation can matter with an empty fractions list; delegated validation otherwise rejects one. No stronger rerun. |
| 83 | types.py:25 | `True` → `False` | missed | killed | Two-outcome result must remain frozen. |
| 84 | types.py:44 | `>` → `>=` | missed | — | Unresolved: exact normalization-tolerance equality and representable inputs. |
| 85 | types.py:48 | `True` → `False` | missed | killed | Multi-outcome result must remain frozen. |
| 86 | types.py:66 | `>` → `>=` | missed | — | Unresolved: exact normalization-tolerance equality and representable inputs. |
| 87 | types.py:70 | `True` → `False` | missed | killed | Edge result must remain frozen. |
