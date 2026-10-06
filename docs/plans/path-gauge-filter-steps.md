# The lesson for the model-based gauge (ROADMAP H7, the client half)

`docs/plans/model-based-estimator.md` shipped `EstimatorConfig::Ekf` at `SNAPSHOT_VERSION` 24
and four scenario twins, and closed with one item it had deliberately not done:

> **The guided-path lesson.** [...] it should be the LFP pair — the filter that corrects
> slowly beside the same filter on a cell 20 % off its table.

This is that slice. It adds two guided-path steps (32 → 34), no engine code, no scenario file
(the two it teaches already ship), and no change to anything on the wire.

## What the reader is shown

The story the estimator note measured, in the order a reader can see it:

1. **Under load, the wrong model is fifteen points wrong inside a minute.** Step 33 runs
   `lfp_gauge_filter_weak_cell.toml` to a mark 60 s into the discharge. Its control arm is
   `lfp_gauge_filter.toml` — the same file with the one `[[faults]]` entry removed — at the
   same mark: the same filter on a cell its model describes exactly, still sitting on its
   boot error, because on this plateau a correct reading is worth very little. The two
   `terminal` rows are 9 mV apart, and that is the whole of the difference between the runs.
2. **At rest, the voltage is honest again and the wrong filter does not come back.** Step 34
   continues the same run to 3900 s. Both cells now rest at the same terminal voltage — at
   zero current the extra resistance costs nothing — and the exact-model filter reads close
   to the truth while the wrong one is still almost 7 points low. A second arm runs the plain
   counter from steps 30–31 at this step's current: it is the better estimator on this cell.

### Two decisions taken with the owner before any prose

* **The filter's own uncertainty is not put on the panel.** It is the lesson's headline — the
  filter is wrong *and sure of itself* — and the page cannot show it: `Bms::soc_sigma` is read
  by tests and nothing on the wire carries it. Putting it there costs a `Telemetry` field, a
  `WASM_API_VERSION` bump, a `web/pkg` rebuild and the parity re-run. Declined for this slice:
  the prose states it in words and names the test that measures it, and the test gains the
  two sigma readings the prose leans on (below).
* **The current is LFP's own 1 C, `2.303451 A`, not the `1.4558 A` steps 30–32 run.** Every
  number in the estimator note and in the two scenario headers was measured there, and the
  model error the weak cell makes is clearest there (≈ 9 mV against ≈ 6 mV at the lower
  current). The step sets the demand box itself, so the reader changes nothing by hand.

## The numbers, measured in the page's shape

A throwaway probe crate outside the repo (`W:\temp\claude\gauge-lesson\probe`, its own
`[workspace]` and target directory) drove the three files with the page's `Pulse` program
(`2.303451 A`, on 300 s, off 9000 s, `dt` 0.5 s) and read each mark on a zero-length probe.
At 30 s, 300 s and 3900 s it reproduces the scenario headers and `gauge_filter.rs` to every
printed digit, so the page shape and the test shape agree here.

| at | file | `current` | `terminal` | `soc (true)` | `soc (bms)` | sigma [pts] |
| --- | --- | --- | --- | --- | --- | --- |
| 30 s | weak cell, filter | 2.3035 | 3.195032 | 59.1667 | 44.4237 | 1.1719 |
| 60 s | weak cell, filter | 2.3035 | 3.190902 | 58.3333 | 43.8674 | 0.3968 |
| 60 s | exact model, filter | 2.3035 | 3.200193 | 58.3333 | 61.4552 | 3.6326 |
| 300 s | weak cell, filter | 0 | 3.242865 | 51.6667 | 40.2239 | 0.1396 |
| 3900 s | weak cell, filter | 0 | 3.265900 | 51.6667 | 45.0006 | 0.0646 |
| 3900 s | exact model, filter | 0 | 3.265900 | 51.6667 | 52.2953 | 0.2055 |
| 3900 s | counter (`lfp_gauge_declines`) | 0 | 3.265900 | 51.6667 | 53.7241 | — |

What the panel prints from that: step 33 — `2.303 A`, `3.191 V`, `58.3 %`, `43.9 %`; its
arm `3.200 V`, `58.3 %`, `61.5 %`; the two printed terminals 9 mV apart and the estimate
14.4 points under the truth. Step 34 — `0.000 A`, `3.266 V`, `51.7 %`, `45.0 %`; the exact
arm `3.266 V` and `52.3 %`; the counter arm `53.7 %`.

No flag is raised on any of the three trajectories.

### What the prose must not say

* **That the estimate "stopped" because it is confident.** From 1800 s on it sits on 45.00 %,
  which is the `[ocv]` table's 0.45 node, held there by the 20 mA sensor offset —
  `on_lfp_the_wrong_model_is_held_at_a_table_node_by_the_offset` measures both halves. The
  confidence is what makes it *slow*; the node and the offset are what make it *stop*. The
  step says both, in that order.
* **What the same weak cell costs on sodium-ion.** The estimator note predicted "under half a
  point" and measured −1.33: sodium-ion's `R0` is three times LFP's. No sentence compares the
  chemistries' millivolts.
* **That the counter on this weak cell reads what the counter arm reads, unless a test says
  so.** The arm runs `lfp_gauge_declines`, which has no weak cell. The equality is asserted
  in `on_lfp_a_wrong_model_makes_the_filter_confidently_wrong` (the counter on this file's
  weak cell reproduces the counter row to four decimals), and the prose names that test.

## Predictions, registered before the checks run

Each case edits one thing, runs `cargo test --workspace --no-fail-fast` below normal
priority, and records the names of the tests that redden, read from the log.

| case | predicted to redden |
| --- | --- |
| **A** the weak cell's `r0_factor` `1.2` → `1.1` | `every_claim_matches_the_engine` (both steps' `soc (bms)`, step 33's `terminal`), the ledger (the prose's `1.2` is no longer the file's), and `on_lfp_a_wrong_model_makes_the_filter_confidently_wrong` |
| **B** the weak-cell file's estimator section deleted (what the page would run if it dropped the section) | `every_claim_matches_the_engine` on both steps; `gauge_filter.rs` |
| **C** the new lessons' blocks deleted | many, structurally — read as integration only |
| **D** step 33's subtraction stated wrong *consistently* (prose, literal and `spells` moved together) | `every_derivation_is_a_sentence_doing_arithmetic`, alone |
| **E** control: an unclaimed sentence reworded, no number touched | nothing |
| **F** the sigma assertions added to `gauge_filter.rs`: the filter's `voltage_sigma_v` `0.010` → `0.030` in the weak-cell file | `gauge_filter.rs` on the sigma readings, and the claims |

## What lands

| file | what |
| --- | --- |
| `web/app.js` | steps 33 and 34 |
| `web/index.html` | the two filter files in the picker's fallback list; the start-button count |
| `web/path-claims.toml` | three arms, the claims, two `[[derived]]`, both `[ledger]` partitions |
| `crates/sim-data/tests/path_claims.rs` | the ledger rules, the tallies the new steps move |
| `crates/sim-data/tests/gauge_filter.rs` | the two sigma readings the prose leans on |

## Results

Landed 2026-10-06 as steps 33 (`a-model-that-is-wrong`) and 34 (`wrong-and-sure-of-it`):
three arms, 16 claims, two `[[derived]]`, two new ledger rules (the weak cell's factor and the
`[ocv]` node) plus step 30's pulse and boot-error rules reused, both steps ledgered whole
(13 and 10 numerals) and in `spelled` at 0. Every claim passed against the engine on the
first run; the reds on the way were all bookkeeping — the scanner reads the `0` inside
`r0_factor` as a numeral (the prose names the factor instead of the field), and every self-count
the two files state that depends on the path's length moved with it, driven to a fixpoint by a script that parses
each check's "should say" line. `HEADER_WORDS` learned `thirty-three` and `thirty-four`.

Gates: `cargo test --workspace --no-fail-fast` 744 passed, 0 failed, across 88 test
binaries; clippy `-D warnings` and `cargo fmt --check` clean.

**Walked in a real page.** `web/pkg` rebuilt from this tree, `sim-server` on its own port,
a headless Chrome on its own profile, driven over CDP from **Start** through every step to
33 and 34 (`W:\temp\claude\gauge-lesson\walk.mjs`). The panel at step 33's mark read `60s`,
`3.191 V`, `2.303 A`, `58.3 %`, `43.9 %`; at step 34's, `65m`, `3.266 V`, `0.000 A`,
`51.7 %`, `45.0 %` — every claimed string. The `43.9 %` is the check that matters: a page
that had dropped the scenario's estimator section would have run the counter and shown
`61.3 %`. Both processes were stopped by the PIDs recorded at launch.

### Predictions, scored

| case | reddened | against the prediction |
| --- | --- | --- |
| **A** `r0_factor` `1.2` → `1.1` | `every_claim_matches_the_engine` (step 33's `soc (bms)`), `every_numeral_in_a_ledgered_step_is_accounted_for` (the prose's `1.2`), `on_lfp_a_wrong_model_makes_the_filter_confidently_wrong`, `on_lfp_the_wrong_model_is_held_at_a_table_node_by_the_offset` | held, plus the node test: at 1.1 the estimate no longer comes to rest on 0.45 (47.58 %) |
| **B** the weak-cell file's estimator section deleted | `each_twin_is_its_partner_with_the_estimator_alone_changed`, `every_claim_matches_the_engine`, and both weak-cell tests in `gauge_filter.rs` | held |
| **C** both lessons deleted | 16 tests, all in `path_claims` and all structural (`no lesson`, the ledger partitions, the tallies) | held; integration, not values |
| **D** step 33's `14.4` → `14.5` in prose, literals and `spells` together | `every_derivation_is_a_sentence_doing_arithmetic` — and `every_number_in_a_claimed_literal_is_accounted_for` and `every_numeral_in_a_ledgered_step_is_accounted_for` | **not alone**, as predicted: a derived figure that fails its arithmetic is not accounted for either, so the two accounting scans fire with it. The arithmetic check's message is the one naming the cause |
| **E** control: "comes back" → "returns" in an unclaimed sentence | nothing | held |
| **F** the weak-cell filter's `voltage_sigma_v` `0.010` → `0.030` | `each_twin_...` (the twins' tuning now differs), `every_claim_matches_the_engine`, both weak-cell tests | held, but the sigma assertions this slice added were **not** what reddened: the test fails on the error at 30 s first. No case here reaches the two new sigma lines alone; they hold on the shipped run and pin values, which is all they claim |

Case F is also a measurement the estimator note listed as open ("a filter whose
`voltage_sigma_v` is too small for its model error is how 'confidently wrong' happens"):
told to expect 30 mV of model error instead of 10, the same filter on the same weak cell is
**+0.48** points at 30 s instead of −14.74 — but −9.86 at 300 s and −5.96 at 3900 s, against
−11.44 and −6.67. The first draft of this paragraph read the 30 s figure as "trusting the
voltage less saves it"; the later readings say it only falls more slowly and lands about as
far out. One setting, not a sweep — the sweep is the next section.

### The headline rides on a hand-picked setting

A review after the slice landed pointed out that step 33's fifteen points rest on
`voltage_sigma_v` = 0.010, which the scenario header labels a hand-picked placeholder, and that
neither step said so. Measured at 5, 10, 15, 20, 30, 50 and 100 mV on the weak-cell file
(estimate minus truth, points, at step 33's 60 s mark / the end of the pulse / the end of the
rest):

| `voltage_sigma_v` | 60 s | 300 s | 3900 s | sigma at 3900 s |
| --- | --- | --- | --- | --- |
| 0.005 | −14.74 | −11.69 | −6.57 | 0.036 |
| **0.010** (shipped) | **−14.47** | **−11.44** | **−6.67** | 0.065 |
| 0.015 | −13.93 | −11.09 | −6.65 | 0.103 |
| 0.020 | −9.74 | −10.69 | −6.51 | 0.150 |
| 0.030 | −1.83 | −9.86 | −5.96 | 0.280 |
| 0.050 | +1.15 | −8.23 | −2.78 | 0.793 |
| 0.100 | +2.53 | −0.42 | +0.31 | 1.893 |

(The +2.5 at 60 s on the loosest row is the three-point boot error not yet corrected, not a
fall.) So up to about three times the shipped value a less trusting filter only goes wrong
*later* and ends the rest about as far out — the reviewer's "half a point off at the same
moment" was the 30 s reading, not the mark's — and at ten times it never goes far wrong, ends
inside its own error bar and beats the counter (+2.06). What loosening costs: on the
exact-model `lfp_gauge_filter` almost nothing by the end of the rest (+0.63 → +0.83), though
about a point before that (+1.95 → +2.88 at the end of the pulse, +1.05 → +1.89 at 900 s),
which no step claims; on the sodium-ion filter pair (`na_ion_gauge_filter`,
`na_ion_gauge_low_filter` — not the counter files steps 30–32 run) something, both landing further out at rest (−0.56 → −1.05 mid-range,
−1.00 → −1.50 near empty). Why the sodium-ion pair pays is not measured here. So the shipped
10 mV is close to the worst setting for the weak cell and buys almost nothing on its healthy
twin; the lesson's result is a fact about a filter that trusts its voltage more than its
model deserves, which is what step 33 now says.

Both steps now say so in words, with no numerals: the reader cannot change the setting on
the page, so a figure for another setting would be right and unreachable. Step 33 adds the
caveat after "a flat curve multiplies every millivolt the model gets wrong" (the sentence a
reader who stops there would overgeneralise); step 34 scopes its counter comparison and its
closing "trade" to this tuning. `the_lesson_rides_on_how_far_the_filter_trusts_the_voltage`
in `gauge_filter.rs` pins the 10 / 30 / 100 mV rows (the loosest over every step, for
step 33's "never"), the counter comparison, and both costs,
and both steps name it. A numeral perturbation of the new step 33 sentence reddened
`every_numeral_in_a_ledgered_step_is_accounted_for`, so the new prose is under the scan.

### The mechanism the first draft got wrong

Step 34 first said the filter grew sure of itself because the wrong model made every reading
disagree with it in the same direction, and it "read that consistency as information". The
final review caught it, and the code settles it: the covariance update in
`sim-core/src/bms.rs` reads the curve's slope at the **estimate**, the filter's settings and
its own covariance, and never the innovation. Measured with the probe, the exact-model file
with **no** boot error — every reading agreeing — has sigma 4.1611 points at 30 s started on
the plateau at 60 % (the shipped run, three points of disagreement, also 4.1611) and
**0.4236** started at 44 %, below the `[ocv]` table's 0.45 node, where the curve is several
times steeper: further down than the wrong model's 1.1719. So the wrong model makes the filter
sure of itself by dragging its estimate onto the steep part of the curve, not by disagreeing
with it. The prose now says that, the same node is the one the footnote says it comes to rest
on, and `the_filter_grows_sure_of_itself_where_the_curve_is_steep` pins the control. The
comment the first commit put in `gauge_filter.rs` ("the shrinking is the wrong model's") had
the same error and is gone.

The first commit also named "the test named above" for the counter-on-the-weak-cell claim
where the nearest test named above is the node test; it now names
`on_lfp_a_wrong_model_makes_the_filter_confidently_wrong`. And the weak cell's factor scales
`R0` alone (`CLAUDE.md`: static multipliers scale R0, not the RC pairs), so the prose says
"ohmic resistance" rather than "resistance".

### Found and not fixed

`README.md`'s browser-demo section says the guided path "walks twenty-four steps" and
narrates those twenty-four. It has been short since the LTO lessons (step 25) and no check
reads it. Left for a slice that rewrites that paragraph rather than bumping a count inside a
description that would still stop at step 24.
