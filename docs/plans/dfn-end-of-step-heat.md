# The porous-electrode cell's heat at a long step — read at the end of the step

**Status: built, 2026-09-23.** The `Dfn` bullet `spm-end-of-step.md` left open under ROADMAP
H8: "a `Dfn` books the equilibrium voltage's fall across a long step as heat."

**No predictions were registered before the build.** Like `spm-end-of-step.md`, it was
built measurement-first against an out-of-tree harness (`W:\temp\claude\dfn-heat`), and the
rule the network integrates was chosen by measuring three candidates, below.

## What was wrong

The pack's heat for a `Dfn` cell is `dfn::heat_w`, `i·(U_eq − v_node)` with `U_eq` read
from the **start-of-step** state. `v_node` has been the step's **last** instant since
Phase 7 (the `Dfn`'s probe is a backward-Euler solve). So the estimate held the fall of the
equilibrium voltage across the step, `i·(U_eq,start − U_eq,end)` — stored energy that left
through the terminals — as if it were heat, and it fed that to both the reported
`q_gen_w` and the thermal network. The `Spm` had the same mix until `spm-end-of-step.md`.

Measured at `358dfa9`: shipped LG M50, 10 shells and 10/5/10 nodes, one step of the whole
interval against one-second steps (the reference; one-minute steps sit within 0.3 % of it):

| case | one-second steps | one long step |
| --- | --- | --- |
| 1S1P C/5 from 90 %, one hour | 1.06 K | **3.71 K** |
| 1S1P 1C from 90 %, half an hour | 18.4 K | **55.4 K** |
| 1S1P C/2 charge from 20 %, one hour | 6.1 K | **27.4 K** |
| 1S3P σ = 0.05 C/5 from 90 %, one hour | 1.57 K | **5.44 K** |

The charge case is the worst. The error is `i·ΔU_eq`, and on a charge both factors change
sign, so the booked "heat" is positive either way.

## The fix

`dfn::advance` returns a correction \[V\] beside its flags: the difference between the
end-of-step overpotential read off the cell's own solve, `U_eq,end − V_end`, and the pack's
estimate, `U_eq,start − v_node`. The pack adds `i` times it to the heat it reports and to
the heat the network integrates. That is the same two slots (`Advanced::rc_delta_v`,
`rc_mean_excess_v`) the equivalent circuit and the `Spm` already use. No new state, no
snapshot bump. A zero-length step returns `0.0`, which keeps probe steps bit for bit.

Read off the solve and not off `v_node` for the reason `spm-end-of-step.md` gives: the node
is on the curve only when the pack's solve converged.

### What the network integrates: measured, not carried over

ROADMAP H8 proposed the `Spm`'s rule: a trapezoid through the step's first and last
instants, "at the price of a second solve's worth of readout." Three candidates were built
behind a temporary switch and measured (temperature rise at the longest step, one-minute
reference in brackets):

| case | end of step only | trapezoid, start by a 1 ms solve | trapezoid, start off the stored line |
| --- | --- | --- | --- |
| C/5, 1 h (1.062 K) | **1.028** | 0.864 | 1.167 |
| 1C, 30 min (18.34 K) | **18.87** | 15.44 | 24.67 |
| C/2 charge, 1 h (6.126 K) | **6.458** | 5.229 | 7.160 |
| 1S3P C/5, 1 h (1.568 K) | **1.522** | 1.285 | 1.744 |

End of step only is closest in every case, and it is the only one that adds no solve. The
trapezoid undershoots because a `Dfn`'s overpotential builds within seconds of a current
change, so the step's first instant is unrepresentative of the rest. The stored line
overshoots because it is last step's tangent, taken at last step's length. So the network
is handed the same correction as the report, and the trapezoid proposal is refused on
measurement.

## After

Same harness, fixed engine, one long step against one-minute steps:

| case | rise, long / one-minute | reported heat, long / one-minute |
| --- | --- | --- |
| C/5, 1 h | 1.028 / 1.062 K (−3.2 %) | 197.5 / 199.6 J (−1.1 %) |
| 1C, 30 min | 18.87 / 18.34 K (+2.9 %) | 1943 / 1918 J (+1.3 %) |
| C/2 charge, 1 h | 6.458 / 6.126 K (+5.4 %) | 1241 / 1139 J (+8.9 %) |
| 1S3P C/5, 1 h | 1.522 / 1.568 K (−2.9 %) | 598.7 / 602.8 J (−0.7 %) |

Before, the same columns were off by +185 % to +322 % (rise) and +181 % to +338 % (heat).

**The energy ledger is not closed by this, and this note does not claim it is.** The
reported pair is `v_terminal` and `q_gen_w` at the step's last instant, as it is for the
other two models. At C/5 over an hour, one-second steps give `Σ v·i·dt + Σ q·dt` =
14 337.3 + 199.6 = 14 536.9 J. One hour-long step gives 14 037.1 J of electrical energy on
**both** engines — the end-of-step voltage sits below the step's mean, and this change does
not touch it — plus 712.5 J of heat before (213 J **over**) or 197.5 J now (302 J
**short**). The shortfall was always there; the false heat more than covered it. That is
the existing H10 "mean pair" item.

## The risk the fix carries: unconverged solves

On an unconverged step, the correction reads the voltage of a Newton that did not
converge. The `Dfn` fails to converge on **269 of 405** hour-long voltage holds from a fresh
pack, so that case was measured before the fix was kept. The sweep was 81 targets across the
window × five pack states (1S1P at 2 %, 50 %, 98 %; a scattered 1S3P; a 4S2P; σ = 0.05),
one step each, on both engines:

| one hour-long step, 405 holds | `358dfa9` | this change |
| --- | --- | --- |
| unconverged | 269 | 269 |
| hottest cell above 320 K | 264 | 220 |
| a cell cooled below ambient under load | 2 | 0 |
| NaN | 0 | 0 |

At a 1 s step both engines converge on all 405 holds.

**What that sweep found instead: these holds are not bounded, on either engine.** The
roadmap said "whether those are bounded was not measured." They are not. Single hour-long
steps from a fresh pack reach 2e179 K (old) and 1e183 K (new). A scattered 1S3P held at
3.3 V for consecutive hours reads 3e66 A (old) or −3e173 A (new) on its second hour, and the
new engine reaches NaN on the third. Under an unreachable 40 W power demand the same group
reads −5e69 A (new) or NaN (old) by the fourth hour; on the way the old engine cooled it to
282 K on the first hour and to −4721 K on the second. The heat rule does not cause this. It
is the `Dfn`'s pack solve under `Voltage` and `Power` at a long step, the `Dfn` analogue of
what `spm-end-of-step.md` fixed for the `Spm` with its current window. Logged under ROADMAP
H8, not fixed here.

## One lesson number moved

`the-electrolyte-starves` quoted **22.41 W** at its 500 s mark; the engine now says
**22.39 W**. What moved it is the equilibrium voltage's fall across one of that lesson's
short steps, which the old estimate booked as heat at 15.46 A — **not** the unconverged
solve past 466 s, which was the first explanation written here and was wrong. Measured: with
the correction reading `v_node` instead of the solve's `V_end` (perturbation P4 below) the
claim reads 22.392177510688764 W, the same bits as the fix. On a single cell under a current
demand the node lies on the tangent through the probed point, so the two voltages are one
number. The sentence ("22.39 W here against the twin's 6.33 W") still says what it said.
`web/app.js` and `web/path-claims.toml` are updated together. No other claim left its
tolerance, which is all the claims test can say.

## Tests

`crates/sim-data/tests/dfn_long_step_heat.rs`, four tests on the shipped LG M50, one per row
of the tables above. Each checks the temperature rise within 10 % and the reported heat
within 20 % of the one-minute run. That is about twice the fix's own worst error (5.4 %,
8.9 %) and far inside the defect's (+181 % and more). All four fail on `358dfa9` (a
worktree), on the rise, by +185 % to +322 %.

Perturbations (`W:\temp\claude\dfn-heat\perturb.py`): one wrong edit at a time,
`cargo test -p sim-core -p sim-data --no-fail-fast`, judged by exit code with every red
test named, and the tree's diff hashed before and after (unchanged). P4 was run by hand
after the commit, on the reviewer's prompt.

| # | the wrong edit | exit | what goes red |
| --- | --- | --- | --- |
| P1 | the correction zeroed | 101 | the four tests, and `path_claims::every_claim_matches_the_engine` (the 22.39 W claim) |
| P2 | the network not corrected (reported still is) | 101 | the four tests, each on the rise |
| P3 | the report not corrected (network still is) | 101 | the four tests, each on the reported heat (checked by message: +179 % to +337 %), and the path claim |
| P4 | the correction reads `v_node` instead of the solve's `V_end` | 0 | **nothing.** The `Dfn` twin of `spm-end-of-step.md`'s P6: the rule that the heat is read off the cell's own solve and not off the node rests on that note's argument (on an unconverged step the node is off the curve), not on a test. Every case here converges, and where the lesson claim does not, a 1S1P current demand makes the two voltages equal. |

The whole workspace suite is green (`cargo test --workspace --no-fail-fast`), with the one
path claim updated. `web/pkg` was rebuilt.

## Also measured, not changed: the `Spm`'s trapezoid

The same harness run on the `Spm` shows its trapezoid is not clearly the better rule there
either. At the longest step:

| case | reference | trapezoid (shipped) | end only |
| --- | --- | --- | --- |
| C/5, 1 h | 0.852 K | 0.698 | 0.807 |
| 1C, 30 min | 13.75 K | 11.62 | 13.80 |
| C/2 charge, 1 h | 4.60 K | 4.11 | 5.02 |
| 1S3P C/5, 1 h | 1.266 K | 1.044 | 1.202 |

End only is closer in three cases of four at an hour. It is worse on the charge (+9 %
against −11 %) and slightly worse at ten-minute steps. Mixed, and a different model's rule,
so it is logged under ROADMAP H10 rather than changed here.

## Still open

* **The `Dfn`'s long-step `Voltage` and `Power` solves are unbounded** (above). ROADMAP H8.
* **The `Spm`'s trapezoid vs end-of-step choice** (above). ROADMAP H10.
* **A step-mean reported pair** — the H10 item this change moves the `Dfn` onto, like the
  other two models.
* **Speed.** One extra equilibrium evaluation per `Dfn` cell per step, against a Newton
  solve. Not benched; the `Dfn` has no bench case (ROADMAP H9).
