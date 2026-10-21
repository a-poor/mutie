# All 194 arithmetic candidates

Canonical IDs are arithmetic-only source order, not report-local IDs. The driver translates them by exact source identity. Prior kills are separate-copy evidence from the first 30-site acceptance. All original outcomes remain unchanged; limited redundancy observations are not dismissals or broad equivalence claims.

| ID | Source | Change | Original | Stronger evidence | Review |
| ---: | --- | --- | --- | --- | --- |
| 1 | devig.py:58 | `+` → `-` | error | — | Retained native ValueError, ZeroDivisionError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 2 | devig.py:59 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 3 | devig.py:59 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 4 | devig.py:68 | `+` → `-` | missed | new missed | Open: changed power-search midpoint. Favorite ordering alone is weaker than an independent exponent/accuracy oracle; retained focused outcome records whether the check distinguishes it. |
| 5 | devig.py:68 | `/` → `*` | error | — | Retained native ZeroDivisionError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 6 | devig.py:69 | `+` → `-` | missed | — | Open: power-search total becomes a difference; normalized results do not establish the intended exponent. |
| 7 | devig.py:70 | `-` → `+` | missed | — | Open: convergence residual changes, often forcing more bounded iterations; no equivalence claim or exact-output oracle. |
| 8 | devig.py:79 | `+` → `-` | error | — | Retained native ValueError, ZeroDivisionError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 9 | devig.py:80 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 10 | devig.py:80 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 11 | devig.py:84 | `+` → `-` | missed | new killed | Unequal additive inputs expose wrong equal-margin removal. |
| 12 | devig.py:85 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 13 | devig.py:85 | `/` → `*` | missed | new killed | Unequal additive inputs expose wrong equal-margin removal. |
| 14 | devig.py:87 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 15 | devig.py:88 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 16 | devig.py:89 | `+` → `-` | error | — | Retained native ValueError, ZeroDivisionError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 17 | devig.py:90 | `/` → `*` | missed | new killed | Extreme valid skew reaches the additive floor and requires final normalization; expected-success assertion retains error classification rules. |
| 18 | devig.py:90 | `/` → `*` | missed | new killed | Extreme valid skew reaches the additive floor and requires final normalization; expected-success assertion retains error classification rules. |
| 19 | devig.py:104 | `/` → `*` | missed | — | Unexercised zero-iteration/zero-z branch: ordinary public calls use 100 iterations with positive midpoints. No general helper equivalence claim. |
| 20 | devig.py:105 | `*` → `/` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 21 | devig.py:105 | `-` → `+` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 22 | devig.py:107 | `+` → `-` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 23 | devig.py:107 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 24 | devig.py:107 | `-` → `+` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 25 | devig.py:108 | `/` → `*` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 26 | devig.py:114 | `+` → `-` | missed | — | Limited ordinary-path redundancy: total is consumed at z=0, while the default public search keeps z positive. Private/exceptional cases are not declared equivalent. |
| 27 | devig.py:119 | `+` → `-` | missed | — | Open: Shin search changes; normalized outputs and ordering alone do not validate the intended search or bias. |
| 28 | devig.py:119 | `/` → `*` | missed | — | Open: Shin search changes; normalized outputs and ordering alone do not validate the intended search or bias. |
| 29 | devig.py:126 | `+` → `-` | missed | — | Open: Shin search changes; normalized outputs and ordering alone do not validate the intended search or bias. |
| 30 | devig.py:127 | `-` → `+` | missed | — | Open: convergence residual changes, often forcing more bounded iterations; no equivalence claim or exact-output oracle. |
| 31 | devig.py:140 | `/` → `*` | missed | — | Unexercised zero-iteration/zero-z branch: ordinary public calls use 100 iterations with positive midpoints. No general helper equivalence claim. |
| 32 | devig.py:141 | `/` → `*` | missed | — | Unexercised zero-iteration/zero-z branch: ordinary public calls use 100 iterations with positive midpoints. No general helper equivalence claim. |
| 33 | devig.py:143 | `+` → `-` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 34 | devig.py:145 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 35 | devig.py:145 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 36 | devig.py:188 | `+` → `-` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 37 | devig.py:188 | `-` → `+` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 38 | devig.py:213 | `+` → `-` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 39 | devig.py:213 | `-` → `+` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 40 | devig.py:234 | `+` → `-` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 41 | devig.py:234 | `-` → `+` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 42 | devig.py:257 | `+` → `-` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 43 | devig.py:257 | `-` → `+` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 44 | devig.py:301 | `+` → `-` | killed | — | Detected by the complete unchanged original suite. |
| 45 | devig.py:301 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 46 | devig.py:327 | `+` → `-` | missed | new missed | Open: changed power-search midpoint. Favorite ordering alone is weaker than an independent exponent/accuracy oracle; retained focused outcome records whether the check distinguishes it. |
| 47 | devig.py:327 | `/` → `*` | error | — | Retained native ZeroDivisionError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 48 | devig.py:329 | `-` → `+` | missed | — | Open: convergence residual changes, often forcing more bounded iterations; no equivalence claim or exact-output oracle. |
| 49 | devig.py:338 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 50 | devig.py:367 | `/` → `*` | missed | — | Unexercised zero-iteration/zero-z branch: ordinary public calls use 100 iterations with positive midpoints. No general helper equivalence claim. |
| 51 | devig.py:368 | `*` → `/` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 52 | devig.py:368 | `-` → `+` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 53 | devig.py:369 | `+` → `-` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 54 | devig.py:369 | `*` → `/` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 55 | devig.py:369 | `-` → `+` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 56 | devig.py:369 | `*` → `/` | missed | new killed | Inverse-probability arithmetic reverses unequal multi-outcome ordering. |
| 57 | devig.py:369 | `/` → `*` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 58 | devig.py:370 | `-` → `+` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 59 | devig.py:370 | `/` → `*` | missed | — | Open: Shin numerator/denominator arithmetic changes. Final normalization may hide changed search or probability shape; no independent accuracy oracle added. |
| 60 | devig.py:376 | `+` → `-` | missed | — | Open: Shin search changes; normalized outputs and ordering alone do not validate the intended search or bias. |
| 61 | devig.py:376 | `/` → `*` | missed | — | Open: Shin search changes; normalized outputs and ordering alone do not validate the intended search or bias. |
| 62 | devig.py:380 | `-` → `+` | missed | — | Open: convergence residual changes, often forcing more bounded iterations; no equivalence claim or exact-output oracle. |
| 63 | devig.py:388 | `/` → `*` | missed | — | Open/unexercised numerical fallback normalization; normal positive inputs did not trigger this branch. Not declared equivalent. |
| 64 | devig.py:394 | `/` → `*` | missed | — | Open/unexercised numerical fallback normalization; normal positive inputs did not trigger this branch. Not declared equivalent. |
| 65 | devig.py:396 | `/` → `*` | missed | — | Unexercised zero-iteration/zero-z branch: ordinary public calls use 100 iterations with positive midpoints. No general helper equivalence claim. |
| 66 | devig.py:400 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 67 | devig.py:401 | `/` → `*` | missed | — | Open/unexercised numerical fallback normalization; normal positive inputs did not trigger this branch. Not declared equivalent. |
| 68 | devig.py:455 | `-` → `+` | missed | new killed | Exact original-margin field checked independently of the selected method. |
| 69 | devig.py:458 | `/` → `*` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 70 | devig.py:496 | `+` → `-` | killed | — | Detected by the complete unchanged original suite. |
| 71 | devig.py:496 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 72 | kelly.py:138 | `+` → `-` | killed | — | Detected by the complete unchanged original suite. |
| 73 | kelly.py:138 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 74 | kelly.py:138 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 75 | kelly.py:138 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 76 | kelly.py:139 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 77 | kelly.py:143 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 78 | kelly.py:144 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 79 | kelly.py:144 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 80 | kelly.py:144 | `/` → `*` | missed | prior kill | Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning. |
| 81 | kelly.py:173 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 82 | kelly.py:173 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 83 | kelly.py:187 | `+` → `-` | killed | — | Detected by the complete unchanged original suite. |
| 84 | kelly.py:187 | `-` → `+` | missed | prior kill | Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning. |
| 85 | kelly.py:187 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 86 | kelly.py:187 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 87 | kelly.py:188 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 88 | kelly.py:188 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 89 | kelly.py:253 | `-` → `+` | missed | prior kill | Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning. |
| 90 | kelly.py:253 | `/` → `*` | missed | prior kill | Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning. |
| 91 | kelly.py:260 | `*` → `/` | missed | prior kill | Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning. |
| 92 | kelly.py:261 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 93 | kelly.py:271 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 94 | kelly.py:282 | `-` → `+` | missed | prior kill | Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning. |
| 95 | kelly.py:282 | `/` → `*` | missed | prior kill | Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning. |
| 96 | kelly.py:292 | `-` → `+` | missed | prior kill | Previously demonstrated Kelly magnitude/edge gap; prior stronger kill retained without rerunning. |
| 97 | kelly.py:292 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 98 | kelly.py:362 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 99 | kelly.py:365 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 100 | kelly.py:368 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 101 | kelly.py:379 | `+` → `-` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 102 | metrics.py:93 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 103 | metrics.py:94 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 104 | metrics.py:130 | `-` → `+` | missed | new killed | Valid wrong endpoint must be clamped to finite log loss; expected-success assertion catches ValueError. |
| 105 | metrics.py:134 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 106 | metrics.py:135 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 107 | metrics.py:184 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 108 | metrics.py:187 | `*` → `/` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 109 | metrics.py:188 | `*` → `/` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 110 | metrics.py:189 | `*` → `/` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 111 | metrics.py:192 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 112 | metrics.py:195 | `-` → `+` | missed | — | Open: exact probability 1 must enter the final calibration bin; the new calibration examples are interior values and do not exercise this clamp. |
| 113 | metrics.py:204 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 114 | metrics.py:205 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 115 | metrics.py:208 | `*` → `/` | missed | new killed | Exact calibration bin boundaries, counts, means, gaps and weighted error distinguish altered arithmetic. |
| 116 | metrics.py:209 | `+` → `-` | missed | new killed | Exact calibration bin boundaries, counts, means, gaps and weighted error distinguish altered arithmetic. |
| 117 | metrics.py:209 | `*` → `/` | missed | new killed | Exact calibration bin boundaries, counts, means, gaps and weighted error distinguish altered arithmetic. |
| 118 | metrics.py:213 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 119 | metrics.py:250 | `/` → `*` | missed | new killed | Exact calibration bin boundaries, counts, means, gaps and weighted error distinguish altered arithmetic. |
| 120 | metrics.py:251 | `*` → `/` | error | — | Retained native ZeroDivisionError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 121 | metrics.py:251 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 122 | metrics.py:290 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 123 | metrics.py:316 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 124 | metrics.py:388 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 125 | metrics.py:389 | `/` → `*` | missed | new killed | Identical-edge branch must return rates/means, not unnormalized totals. |
| 126 | metrics.py:393 | `-` → `+` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 127 | metrics.py:393 | `/` → `*` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 128 | metrics.py:396 | `*` → `/` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 129 | metrics.py:397 | `*` → `/` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 130 | metrics.py:398 | `*` → `/` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 131 | metrics.py:399 | `*` → `/` | error | — | Retained native TypeError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 132 | metrics.py:402 | `-` → `+` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 133 | metrics.py:402 | `/` → `*` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 134 | metrics.py:404 | `-` → `+` | error | — | Retained native IndexError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 135 | metrics.py:417 | `+` → `-` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 136 | metrics.py:417 | `*` → `/` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 137 | metrics.py:418 | `+` → `-` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 138 | metrics.py:418 | `+` → `-` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 139 | metrics.py:418 | `*` → `/` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 140 | metrics.py:420 | `/` → `*` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 141 | metrics.py:421 | `/` → `*` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 142 | metrics.py:422 | `/` → `*` | missed | new killed | Nonzero edge offsets and repeated items distinguish bin placement, boundaries and averages. |
| 143 | odds.py:67 | `+` → `-` | error | — | Retained native InvalidOddsError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 144 | odds.py:67 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 145 | odds.py:69 | `+` → `-` | error | — | Retained native InvalidOddsError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 146 | odds.py:69 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 147 | odds.py:92 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 148 | odds.py:92 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 149 | odds.py:94 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 150 | odds.py:94 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 151 | odds.py:114 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 152 | odds.py:135 | `/` → `*` | error | — | Retained native InvalidOddsError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 153 | odds.py:163 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 154 | odds.py:219 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 155 | odds.py:221 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 156 | odds.py:259 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 157 | odds.py:260 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 158 | odds.py:261 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 159 | odds.py:261 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 160 | odds.py:261 | `/` → `*` | missed | new killed | Non-even odds and uncapped sizing distinguish Kelly magnitudes. |
| 161 | odds.py:265 | `*` → `/` | missed | new killed | Non-even odds and uncapped sizing distinguish Kelly magnitudes. |
| 162 | odds.py:313 | `-` → `+` | missed | new killed | Non-even odds and uncapped sizing distinguish Kelly magnitudes. |
| 163 | odds.py:315 | `-` → `+` | killed | — | Detected by the complete unchanged original suite. |
| 164 | odds.py:316 | `*` → `/` | missed | new killed | Non-even odds and uncapped sizing distinguish Kelly magnitudes. |
| 165 | odds.py:316 | `-` → `+` | missed | new killed | Non-even odds and uncapped sizing distinguish Kelly magnitudes. |
| 166 | odds.py:316 | `/` → `*` | missed | new killed | Non-even odds and uncapped sizing distinguish Kelly magnitudes. |
| 167 | odds.py:321 | `*` → `/` | missed | new killed | Non-even odds and uncapped sizing distinguish Kelly magnitudes. |
| 168 | simulation.py:97 | `/` → `*` | error | — | Retained native IndexError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 169 | simulation.py:97 | `*` → `/` | killed | — | Detected by the complete unchanged original suite. |
| 170 | simulation.py:97 | `-` → `+` | error | — | Retained native IndexError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 171 | simulation.py:99 | `+` → `-` | missed | new killed | Two- and three-trial public percentiles check adjacent interpolation and weights. |
| 172 | simulation.py:99 | `-` → `+` | missed | — | Limited public-path redundancy: for ordinary list sizes the fixed 5/95 percentiles keep lo+1 within n-1, so raising the cap does not alter the index. No general helper/huge-list equivalence claim. |
| 173 | simulation.py:100 | `-` → `+` | missed | new killed | Two- and three-trial public percentiles check adjacent interpolation and weights. |
| 174 | simulation.py:101 | `+` → `-` | missed | new killed | Two- and three-trial public percentiles check adjacent interpolation and weights. |
| 175 | simulation.py:101 | `*` → `/` | error | — | Retained native ZeroDivisionError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 176 | simulation.py:101 | `-` → `+` | missed | new killed | Two- and three-trial public percentiles check adjacent interpolation and weights. |
| 177 | simulation.py:113 | `-` → `+` | missed | — | Open: changed median index can select the wrong neighbor or exceed a two-item list. Not included in the stronger kill selection. |
| 178 | simulation.py:113 | `+` → `-` | killed | — | Detected by the complete unchanged original suite. |
| 179 | simulation.py:113 | `/` → `*` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 180 | simulation.py:176 | `/` → `*` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 181 | simulation.py:177 | `+` → `-` | killed | — | Detected by the complete unchanged original suite. |
| 182 | simulation.py:181 | `-` → `+` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 183 | simulation.py:196 | `*` → `/` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 184 | simulation.py:198 | `*` → `/` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 185 | simulation.py:217 | `-` → `+` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 186 | simulation.py:217 | `/` → `*` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 187 | simulation.py:220 | `/` → `*` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 188 | simulation.py:220 | `/` → `*` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 189 | simulation.py:231 | `/` → `*` | killed | — | Detected by the complete unchanged original suite. |
| 190 | simulation.py:233 | `/` → `*` | missed | new killed | Exact seeded trajectories, means, medians, drawdown and growth distinguish arithmetic hidden by broad range checks. |
| 191 | simulation.py:348 | `+` → `-` | missed | new killed | Fraction-search results must match standalone runs using the documented incrementing sub-seeds. |
| 192 | types.py:43 | `+` → `-` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 193 | types.py:44 | `-` → `+` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
| 194 | types.py:66 | `-` → `+` | error | — | Retained native ValueError. Full-suite failure evidence is in the error review; no relabeling as a kill. |
