# The porous-electrode cell held at a voltage or a power over a long step

**Status: built, 2026-09-30.** The `Dfn` bullet `dfn-end-of-step-heat.md` found and left open
under ROADMAP H8: "voltage holds the model cannot reach inside its range stay unconverged", with
the `Dfn`'s hour-long ones **not bounded**.

**No predictions were registered before the build.** Like the two notes before it, this was
built measurement-first against out-of-tree harnesses (`W:\temp\claude\dfn-heat`, and a
`git worktree` of the parent commit at `W:\temp\claude\dfn-holds-head` for every "before"
number). The design changed three times on measurement, and the record of why is the point of
this note.

## What was wrong

Measured at `d99c7ee`, shipped LG M50, 10 shells and 10/5/10 nodes. The sweep is the one
`spm-end-of-step.md` used: 81 voltage targets across the window × five fresh packs (1S1P at
2 %, 50 % and 98 %; a scattered 1S3P; a 4S2P; σ = 0.05), one step each.

| | before |
| --- | --- |
| one-hour holds unconverged | **269 of 405**; hottest cell 10¹⁷⁹ K |
| one-second holds unconverged | 0 of 405, **but six "converged" 4–45 mV off their target** (below) |
| scattered 1S3P held at 3.3 V, four hours | 27 A, then **3e146 A** and 4e298 K, then infinities |
| 1S3P at an unreachable 40 W, four hours | 12 A, then **−2.5e52 A** |
| cell-steps that do not conserve lithium, twelve four-hour runs | **124** |

The last row is the measure that matters, and it took a correction to get right: a converged
step moves exactly `i·dt` of charge, so each cell's state of charge falls by `i·dt/Q` (`Q`
measured, one converged hour at 1 A). The first version of that check skipped cells reading
exactly empty or full, which is where the worst rows were — a 1S3P at 40 W committing 9.5 A to
three cells that can carry about 2.7 A each read "no error". The corrected check counts the
charge a clamped cell *should* hold and flags it when that lies past the electrode's physical
limit.

Three separate faults produced those numbers.

**1. The cell's own Newton failed on long steps, inside the range it can carry.** Its first
guess is the last step's answer, which is a poor one when an hour moves the particles by a
large fraction of their range. From 98 % a 4 A discharge (0.8C) failed with the cell nowhere
near empty; from a cell just drained to 2.5 V, a one-hour step failed **at every current, rest
included**, while a one-minute step converged. An unconverged solve still advances the state,
and that state does not conserve lithium: after an hour at 10 A from half charge the cell read
19 %.

**2. The pack committed cells to currents their step cannot carry.** A voltage or power demand's
probes were not held to any range, so the search wandered to currents that empty a cell several
times over, and the step was then advanced there.

**3. The pack's damped search stopped where the demand was not met, and called it converged.**
The search scores a trial by how far each cell's curve is from the straight line it was split
on. That line is the tangent the last probe took, so a trial a small step `λ` from that probe
scores near zero *whatever the demand said*. Two consequences, both measured:

* **False convergence.** A 1S1P held at 3.393 V for an hour walked the score down to
  2.5e-10 V at `λ` = 6e-5 and reported "converged" at **4.127 V, charging at 1.56 A** — the
  wrong sign. At a one-second step, six holds from 98 % "converged" off target: 2.504 V for
  2.5 V, 3.119 V for 3.074 V (45 mV), 3.117 V for 3.095 V.
* **A trap.** Once one tiny step is accepted, no real step can beat its near-zero score, and the
  search crawls at `λ` ≈ 3e-5 until its 32-pass cap. A 98 % cell held at 2.5 V spent thirty
  passes creeping from 2.14 to 2.15 A towards an answer near 2.8 A.

## The fix, in five parts

**1. Two retries in `dfn::solve`, only when the Newton failed** — first in current (`i/4`,
`i/2`, `3i/4`, then `i`), then in step length (`dt/8` … `dt`, and again from `dt/64`), each
stage started from the last stage's answer. Only the last stage is the real system; the others
are starting points. A solve that converges at the first attempt never reaches either retry and
is bit for bit what it was. Measured separately: the step-length retry alone left 2 one-second
and 1 one-hour hold unconverged, which the current retry then fixed, so both are kept.

**2. A range for the cell** (`dfn::current_window`): the currents over which the step leaves both
electrodes' bulk lithium between the stoichiometries the chemistry declares as 0 % and 100 %.
Closed form from conservation — a converged step moves exactly `i·dt/F` moles out of one solid
and into the other, however the reaction is spread — so each electrode's end-of-step bulk is
affine in `i`.

The edge was chosen on measurement. Conservation's own edge, the whole solid emptied, was
built first. It is a point the cell's Newton cannot converge on, and an unmeetable demand stops
exactly there: a 98 % cell held at 2.5 V for an hour was committed at that edge's 5.45 A and
came back non-conserving. The chemistry's empty and full are also the right physics for the
two demands this is used on: a voltage hold is clamped into `[v_min, v_max]`, and under load the
terminal reaches those before the bulk reaches the stoichiometry at which the resting voltage
does, so the range never excludes a hold's answer.

**3. A voltage or power demand's probes held to that range** (`dfn::probe_at`'s `hold`), as the
`Spm`'s already were to its own. A current demand's probes are not: its caller chose the current.

**4. The same demands' pack current held to the range every cell can carry**
(`pack_current_window`). Each cell's current is affine in the pack current through the pass's
lines, so each cell's range maps to one interval of pack current, and the pack's is their
intersection, widened to contain zero. The demand's current is clamped into it before
protection, and so is every damped trial. The range is widened to take zero in: that is what
keeps protection's last word and the damping's "cannot un-refuse a refused current" argument
true, and it is what a cell at an edge needs — one a step ended exactly at empty sits a
rounding hair either side of it, and may rest there but not be moved further out. Measured
before the widening existed, the range excluded zero by 1e-16 A (on one cell `E_g` is
`E_k·G/G`, a hair off `E_k`), the hold switched off, and an unmet 10 W demand pushed an empty
cell 0.25 A further.

**5. On a pass the range is in force on, the score includes the demand's miss** — how far
the trial current is from the demand's own answer on its line, in volts of terminal
(`|i_full − i|·R_pack`). A full step misses by exactly `0.0`, so every pass that takes one
scores as it always did. Since "converged" is the score under `SOLVE_TOL_V`, a held pass can
no longer converge off its demand. A separate stop rule saying the same was built first and
deleted when perturbation P7 showed it could not fail while the score carried the miss. A pass
that stops on the range's edge rather than on the demand raises `SOLVE_UNCONVERGED`: the demand
was not met, which is the `Spm`'s verdict for the same case.

**Scoped to held passes, and that is a scope, not a claim that the rest is sound.** Turned on
everywhere, it moved the `Spm`: its unmet 10 W hour, which the old search stops on a damped pass
at 1.6 A (4.7 W, flagged), was refused that stop and wandered onto the flat curve past empty at
3.4 A. The `Spm` declares no range to the pack yet, so nothing keeps its search inside. Scoped,
the `Spm` is bit for bit unchanged. Carried under ROADMAP H8.

No snapshot bump: no state was added or reinterpreted.

## After

Same harnesses, same cases.

| | before | after |
| --- | --- | --- |
| one-hour holds unconverged | 269 of 405, hottest 10¹⁷⁹ K | **0 of 405**, hottest 341.8 K |
| one-second holds unconverged | 0 of 405 (six falsely) | 0 of 405, every one on target |
| cell-steps that do not conserve lithium, twelve four-hour runs | 124 | **0** |
| scattered 1S3P at 3.3 V, four hours | 27 A, 3e146 A, ∞ | 5.61, 0.52, 0.05, 0.006 A; all at 3.300 V |
| 4S2P at 10 V, four hours | 63 A at 382 K, then 76 A, then 6e9 A | 4.97 A at 10.000 V, then rests at 11.2 V, flagged |
| 1S3P at 40 W, four hours | 12 A, then −2.5e52 A | 7.2 A at 2.82 V, 0.58 A, then rest; flagged every hour; ≤ 312.2 K |
| 1S1P at 10 W from 35 %, four hours | 1.7 A at 2.88 V, then 1.5 A at 0.05 V, "converged", past empty | 1.89 A at 2.17 V, then rest at 2.50 V; flagged every hour |
| drained 98 % cell, one-hour steps at −0.4 … +0.3 A | all 15 unconverged, 1.4–12.9 V | all 15 converged, a smooth monotone curve |

All of this table was re-measured on the commit as shipped, after the two deletions the
perturbation table below records, and came out identical — as did the fingerprints, the
930-record diff and the `Spm` checks.

An unmeetable power demand now stops at the chemistry's empty, at about `v_min`, instead of on
the collapse past it (0.47 V before part 2's edge was chosen).

### What moved, and what did not

Every record the fingerprint covers was diffed line by line against the parent commit (930
records: the one-second voltage and power holds from all five packs, and current demands at 1 s,
60 s and 3600 s):

* **Zero-length reads: bit for bit.** A zero-length step never takes the retries (the probe
  answers with the stored line) or the holds (all gated on `dt > 0`).
* **Current demands:** 11 records moved, and every one had been unconverged at the parent or
  follows one that had in the same run — the retries converging a step that used to fail.
* **One-second holds:** 7 moved that the parent reported converged. Six are the false
  convergences above, now on target to 1e-15 V; the seventh moved in its last two digits
  (3.05249999999999977 → 3.05250000000000021 V), both converged.
* **The `Spm`, bit for bit** on the parent's four-hour cases, iteration counts, 738 zero-length
  reads fresh and warmed, and its own sweep of 405 holds at 1 s and 3600 s.
* **The equivalent circuit** never reaches any of it: a linear pack leaves the loop before the
  new rules, and its `current_window` is `None`.

## Tests

`crates/sim-data/tests/dfn_long_step_holds.rs`, seven tests on the shipped LG M50. **All seven
fail on the parent** (run in the worktree), and all pass here.

| test | answers to |
| --- | --- |
| `a_one_hour_voltage_hold_meets_its_target` | the false convergence at 4.127 V |
| `a_one_second_voltage_hold_meets_its_target` | the six short-step ones |
| `a_long_discharge_from_full_converges` | the Newton failing inside the range (current retry) |
| `a_just_drained_cell_can_rest_for_an_hour` | the Newton failing at every current (step-length retry) |
| `parallel_packs_hold_a_voltage_hour_after_hour` | four held hours on three 1S3P targets and a 4S2P, conserving every hour |
| `an_unreachable_power_stops_at_empty_and_says_so` | the range's edge, the flag, and never past empty |
| `hour_long_holds_across_the_window_converge` | eleven of the sweep's targets on the scattered 1S3P |

Perturbations (`W:\temp\claude\dfn-heat\perturb_holds.py`): one wrong edit at a time,
`cargo test -p sim-core -p sim-data --no-fail-fast`, judged by exit code with every red test
named, and the tree's diff hashed before and after the run (unchanged). Run before the two
deletions below; P7 and P9 are the reason for them.

| # | the wrong edit | exit | what goes red |
| --- | --- | --- | --- |
| P1 | no retry in current | 101 | `a_one_second_voltage_hold_meets_its_target` |
| P2 | no retry in step length | 101 | `a_just_drained_cell_can_rest_for_an_hour`, `parallel_packs_hold_a_voltage_hour_after_hour` |
| P3 | the probe not held | 0 | **nothing.** With the pack current held, every split current is already inside its cell's range, so the probe's clamp acts only where the pack widens its range to take zero in, and no case measured is decided there. Kept as the `Spm`'s pattern and the fallback when the pack's range is empty; argued, not tested. |
| P4 | the demand's current not held | 101 | `an_unreachable_power_stops_at_empty_and_says_so` |
| P5 | the damped trial not held | 0 | **nothing**, and nothing in the harness moves either (the sweep, the twelve four-hour runs, the 10 W case). A held pass's range barely moves between passes, so a trial between two held currents stays inside. Kept because it makes "a held step is committed inside this pass's range" true by construction rather than by that observation. |
| P6 | the held score without the demand's miss | 101 | `an_unreachable_power_stops_at_empty_and_says_so` |
| P7 | a separate "demand met" stop rule removed | 0 | **nothing — so the rule was deleted.** The score already carries the miss, so a score under tolerance implies the demand is met. |
| P8 | the range's edges at conservation's `(0, c_max)` | 101 | `a_just_drained_cell_can_rest_for_an_hour`, `an_unreachable_power_stops_at_empty_and_says_so`, `parallel_packs_hold_a_voltage_hour_after_hour` |
| P9 | the cell's own range not widened to contain zero | 0 | **nothing, and nothing in the harness moves — so that widening was deleted.** The pack's widening (P10) does the same work. |
| P10 | the pack's range not widened to contain zero | 0, then 101 | **nothing on the first run** — the cell's own widening (P9) was still in place and covered it. The harness's scattered 1S1P at 10 W was pushed 0.5 mA past empty with `SOC_CLAMPED_LOW` all the same, so that case was added to the power test. Re-run by hand after P9's widening was deleted: `an_unreachable_power_stops_at_empty_and_says_so` goes red, on the unscattered cell's second hour — pushed 0.24 A past empty, to 300.3 K. |
| P11 | an edge stop not flagged | 101 | `an_unreachable_power_stops_at_empty_and_says_so` |

## Still open

* ~~**The same false convergence off held passes.**~~ and ~~**the score's trivial
  minimum**~~ — **closed 2026-09-30** by `spm-pack-window.md`: the `Spm` declares a range to
  the pack, and every damped trial on a step with time in it is scored on the step the next
  pass would take from it, which is zero only at a fixed point. Measuring that found the
  held-pass miss this note built had its own fault: `an_unreachable_power_stops_at_empty_and_says_so`
  passed because a cycling search happened to be on its "empty" pass at the 32nd; at a cap of
  30, 33 or 60 the same step landed at 1.07 A and 3.36 V, and 243 of the 405 hour-long
  discharge-power solves in this note's sweep moved with the cap. That test now checks that
  the landing is the most the cell can give over the hour.
* **Current demands past empty.** A current demand's caller chose its current, so nothing here
  holds it: a `Current(3.5)` hour from half charge still drives the cell past its physical
  limit, and the step does not conserve lithium. This is wider than H8's "absurd current"
  bullet, which spoke only of absurd magnitudes: 3.5 A is 0.7C.
* **An empty pack range** — a group circulating so hard that no pack current keeps every cell
  inside — falls back to held probes alone. Not met on any case measured.
* **An external short under a held demand** is not held: the short's current rides on the
  load's and would need its own mapping. Not measured.
* **Cost, measured coarsely.** A retry costs up to fifteen solves, on the failing path only,
  and the failing path is now rare. The 405-hold sweep at 1 s and 3600 s ran in **8.9 s and
  20.6 s** here against **588 s and 553 s** on the parent, whose failing solves each ran the
  Newton to its cap inside a pack search run to its own. A converging workload (four current
  demands at five step lengths, its output bit-identical on both engines) ran in 3.4–7.5 s on
  both, interleaved, with no order between them: this machine's noise, not a difference. The
  `Dfn` still has no bench case (ROADMAP H9).
