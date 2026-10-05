# Past empty on the porous models: a reversal for the particle, a settle for the split

**Status: built for the `Spm`, 2026-10-05, `SNAPSHOT_VERSION` 21 → 22. The `Dfn` half is
not built; why is below.** Follows ROADMAP H8's "an `Spm` has no physics past empty" and
"a `Dfn` driven past its range by a `Demand::Current`" bullets, and the "Still open"
sections of `spm-end-of-step.md`, `spm-pack-window.md` and `dfn-long-step-holds.md`.

**What this note does not have, said first:** predictions registered before the engine
ran. The slice opened as a spike — the baselines were measured, then three designs were
tried against them in turn, each against the same five cases — and the note was written
after. Its claims are measurements of the commit as shipped, each with the harness that
took it; none is a prediction scored after the fact.

Harnesses: `W:\temp\claude\porous-rev` (the working tree) and `W:\temp\claude\porous-rev-headh`
(a worktree of the parent `e5a60ad` at `W:\temp\claude\porous-rev-head`), the same
`main.rs` compiled against each; and `W:\temp\claude\porous-rev-win` /
`porous-rev-win-head`, the `spm-window` harness from `spm-pack-window.md` against each.
Lithium is read off the snapshot, never off telemetry.

## What was broken, measured on the parent

The parent let a particle carry whatever current it was handed. Past its edge the bulk
ran below empty into concentrations no particle holds, the surface was clamped at the
edge of its OCP table, and `V(i)` went flat there.

| case | parent |
| --- | --- |
| `Spm` 1S1P, 5 A for two hours from 50 % (60 s steps) | flat at 0.504 V for the last 90 minutes, 354.6 K; negative particle's outer shell at a stoichiometry of **−1.26**; rested an hour later at **1.1038 V**, the clamped surface's value |
| `Spm` 1S1P, 3.5 A for an hour from 50 % | 0.50 V at every step length, outer shell at −0.14 |
| `Spm` 1S3P scattered, 20 A through empty (thermal) | **NaN / inf** at 1, 10 and 60 s steps |
| `Dfn` 1S3P, same | inf at 1 s; 1272 and 1343 K at 10 and 60 s |
| `Dfn` 1S3P under a 2 Ω short, `Current(2.0)`, fourth hour | **6.7e74 A** isothermal; 352 K and a negative particle at −1.09 with the network |
| `Current(1e9)` for one second | `Spm` −5.65e5 V; `Dfn` **−1105.6 V**, then −4.7e7 V at rest |
| `Spm` 1S1P energy ledger from the state at both ends, 20 A for 20 min | **−16 446 J at a 1 s step, −16 205 at 10 s** — it does not shrink: energy released at a fixed voltage from lithium that was not there |

The ledger is the separable store below, evaluated at the run's two endpoints; on the
parent's negative bulk it charges the reversal ramp, one definition for both engines.

Where the surface runs out relative to the bulk (1S1P from 50 %, 10 s steps; the outer
shell reaching zero against the bulk SOC flag):

| rate | `Spm`: bulk left when the surface empties | `Dfn` |
| --- | --- | --- |
| ≤ 2 C | none — the bulk empties first | ≤ 0.4 % |
| 3 C | 3.9 % | 29 % (a front node) |
| 5 C | 13.6 % | 42 % |

## What was tried, and what each found

**1. Bulk edge only, the equivalent circuit's rule transplanted** (`spike1_bulk.patch`). A
deficit only once the particle's bulk is at the chemistry's empty, so the position stays
one number and the ledger closes by construction, as in `low-clamp-reversal.md`. Past the
edge, `V = V_particle(edge) − drop(d_end)`. The single cell was fixed — rest at the floor,
ledger 6.5 J at 1 s and linear in `dt` against the parent's 3864 J flat — and the parallel
group was **worse**: a scattered 1S3P at 3–5 C and 60 s steps went from bounded (on the
parent, at 1300 K) to 1e100. Between the surface edge and the bulk edge the curve was still
flat, and next to it the reversal was steep.

**2. Surface or bulk edge, with the particle paying the deficit back**
(`spike2_surface_payback.patch`). The edge is the lower of the current that takes a
surface to `SURFACE_EDGE·c_max` and the one that takes the bulk to empty. With no deficit
the particle carries up to it; with one it carries exactly it, so what the terminals do not
take repays the deficit. The owner chose this over "the deficit stays until a recharge",
whose flaw was shown before asking: with the particle still holding lithium, either the
depressed voltage turns into heat on every later discharge (the forbidden variant of
`low-clamp-reversal.md`) or `V(i)` jumps.

A deficit beside lithium is two state variables, and the advisor's first argument was that
the ledger then stops closing by construction. It does not have to: book the two channels
separately — `heat = i_p·U_eq + i_d·OCV_d(d) − i·V` with `OCV_d` a function of the deficit
alone — and the store is separable, `E = E_particle(bulk) + E_reversal(d)`. Measured, 20 A
for 20 minutes: **708 → 147 → 15 J at 60, 10, 1 s**; 25 A for 300 s then 1800 s of rest:
1663 → 298 → 30 J. Linear in `dt` on every case.

It found two defects on the way, both now fixed:

* **The report and the step disagreed by 0.3 V.** The pack reports a cell's voltage from
  a zero-length read of its stored state, which divided the current as "no deficit → all
  to the particle, deficit → none". The step had divided it otherwise, so the reported
  `v_terminal` (0.162 V) was not the node the cell advanced at (−0.149 V), and the ledger
  ran ~5 W out exactly while both channels were active. `SpmState::i_reversal_last` stores
  the share the reversal carried; a read where no time passes holds it. It is state for the
  reason `i_last` is.
* **A cell at its edge with a deficit creeps upward by rounding**, about 5e-15 of SOC a
  step (4.3e-14 → 1.0e-13 over eleven ten-minute steps). Its source was not found. The
  guard written for it — a particle share of `edge.max(0)` — was measured with and without
  and changed nothing to the last digit, so it was taken out rather than kept as coverage.

The parallel group still failed, and by more: 3 and 4 C at 1 and 10 s steps went to NaN
once the thermal network was on.

**3. The split, settled where the pass loop cannot settle it.** A per-pass log of the
failing step settled it. Under a current demand the pack current is the caller's, so every
damped trial splits the same group current the same way: the damping search took sixteen
identical halvings per pass while one cell's current went 33 → 27 → 36 A and another's
−0.1 → 12 → −0.3, until it ran away. That is the pass loop, not the cell: a single cell and
an unscattered 1S3P were clean at every rate.

Each cell's end-of-step curve is now strictly decreasing in its current — past the edge the
kinetics of the whole current and the ramp both keep falling — so a group's node voltage is
the root of one decreasing scalar function: the currents its cells carry at that voltage,
less the leakage, less the group current. Where the loop gives up at its cap under a
current or rest demand, `settle_group` finds that root by Newton safeguarded with a bracket,
per cell and per group:

* **Seeded from the previous step's committed split**, not the loop's last pass — which, on
  a step that cycled to the cap, can be billions of amps out (measured: a fallback seeded
  there failed on the first 4 C case it met).
* **The node bracketed by the cells' own curves at an even share.** Some cell carries at
  least the share and some at most, so the node lies between the lowest and highest of the
  cells' voltages there. Without it the search stepped the node to −4.2 V, which a cell past
  its edge reaches only at ~1e12 A (its curve falls logarithmically there).
* **Commit-or-nothing.** Every group is settled into scratch and committed only if all
  settle, so a failed search leaves the loop's answer bit for bit. The first version wrote
  as it went and moved four `Dfn` steps it then failed on.
* **Not on a pack with a `Dfn` cell.** Past its range that model's curve is not monotone,
  and each evaluation is a coupled solve: tried, it settled none of the `Dfn` steps it met
  and made the fingerprint run thirty minutes long. Not under an external short either,
  whose current rides on the node voltages and would need an outer search.

## The design as shipped

`spm::split(edge, i)` returns `(i_p, d_end)`: inside the window `(i, 0.0)`, exactly. Past
it the particle carries `edge` and the deficit grows by
the rest; a charge repays the deficit first. `discharge_edge` is the lower of
`current_window`'s upper end (the surface, unpulled) and `bulk_window`'s.

`V(i) = U_p(c_s(i_p)) + η_p(i) − U_n(c_s(i_p)) − η_n(i) − i·R_c − drop(d_end)`: the surface
moves with the particle's current, the kinetics and contact resistance carry the cell's,
and `drop(d) = min(v_per_soc·d, U_e − floor_v)` where `U_e` is this model's own equilibrium
voltage at the chemistry's empty, not the equivalent circuit's `OCV(0)`. Continuous at the
edge, strictly decreasing past it. The `[reversal]` section is the shipped one; nothing new
is read.

Heat: the pack's existing `i·(U_eq − V)` plus a new `Advanced::reversal_w`,
`i_d·(OCV_d(d) − U_eq)`, in watts because at rest the particle can be paying the deficit
back at zero terminal current, which no voltage times `i` can express. Exactly `0.0` (and
guarded against `-0.0`) on every cell that is not past its edge.

Over-discharge damage needed no code: the pack's aging accumulator bills `fade_per_ah`
from `CellModel::soc_deficit()` increments, which was model-neutral already. The owner
chose to include it; `over_discharge_damages_an_aging_single_particle_cell` measures it
against a zero-coefficient control arm.

## What it does now

| case | parent | now |
| --- | --- | --- |
| 1S1P 5 A two hours: outer shell | −1.26 | never below 0 (`a_particle_driven_past_empty_holds_no_lithium_it_does_not_have`) |
| rested after it | 1.1038 V | 0.000 V, the floor |
| an hour at −2.5 A after that | charges the negative particle | repays 2.500 Ah of the 7.42 Ah deficit, SOC stays at zero |
| ledger, 20 A 20 min, 1 s / 10 s | −16 446 / −16 205 J | 15 / 147 J |
| 1S3P scattered 20 A through empty | NaN at every `dt` | 340 K, −0.11 V, converged every step |
| 1S3P scattered 3, 4, 5 C, `dt` 1/10/60 | with the network: unconverged steps in 6 of 9 cases (up to 5 each), bounded at 1276–1542 K; isothermal not measured | 0 unconverged, 0 NaN in all 18 (network and isothermal) |
| 3 C single cell, the guided path's run | cut-off at 1060 s, no flag | `SOC_CLAMPED_LOW` at 1052 s (surface dry, 12.3 % inside), cut-off at 1056 s |

**What did not move.** All 276 in-window current and rest fingerprints the `spm-window`
harness computes but two, bit for bit; every zero-length read; every `Dfn` case. The two
are a 1S1P from 20 % at 20 A whose surface runs dry before its bulk does — the parent
never flagged it, and the new engine correctly does.

**The pass cap decides nothing new.** The fallback runs exactly when the loop reaches its
cap, so the cap now chooses when it runs. Re-run at caps of 31, 32 and 33: the 18 fast
over-drive cases are identical at all three, and so are all current and rest fingerprints
of the `Spm`. Two things do move with the cap — the `Spm`'s zero-length reads and some
`Dfn` steps past empty — and both move identically on the parent at the same caps: neither
reaches the fallback (a zero-length step, a `Dfn` pack).

**What moved that is not past empty.** 538 of the 4860 lines of the `spm-window` hold
sweep (`Spm`, voltage and power holds), all at 3600 s steps, all unmet powers landing on
their maximum-power point at the bulk edge. A tangent's difference quotient there now
straddles the reversal on one side; currents moved by at most 1.06e-3 relative (4S2P,
`k6`), delivered power by none at the printed precision (55.03 W both), no flag and no
convergence verdict changed.

**Thermal runaway is not new.** With the network on, the 3–5 C parallel cases run away
near empty on the parent and here alike (1280–1540 K there, 1315–1670 K here). Moving the
surface clamp's margin from 1e-6 to 1e-9 on the commit as shipped moved the onset of
runaway by 5–9 s (648 → 643, 457 → 448, 333 → 324 s at 3, 4, 5 C, one-second steps) and
the peaks by a few percent, so the runaway is the chemistry's reaction once over-drive
heating reaches its onset, not the constant's. It moved the **deepest voltage** of those
over-drives by 0.27–0.79 V (−1.75 → −2.02, −1.64 → −2.41, −1.70 → −2.49 V), and left one
step unsettled at 5 C — see "Still open". Inside runaway, cells hundreds of kelvin apart
redistribute up to about four times the group current at converged steps; the test bounds
that at ten times and isothermal runs at twice.

## The guided path

Three lessons moved, all on the single-particle file at 3 C:

* **Step 15** (`looks-fine-from-outside`): the continuation's cut-off is 1056 s, not 1060,
  with 12.0 % showing; and it now raises one flag, at 1052 s, which the sentence says. "Pins
  near 0.3 V" past the cut-off was the clamp; it now settles a fraction of a volt below
  zero, on the reversal.
* **Step 20** (`the-gradient-itself`): its pulse is 1060 s, now 4 s past the cut-off, so the
  gaps are read at 1050 s for the plateau and the dip is a sentence of its own (5.67 at
  1060). The rest pays the 0.05-point deficit back, so `soc (true)` moves by 0.05 points
  through a rest it used to sit still for — still `11.7 %` on the panel. The rebound's floor
  is now this step's own claim (2.442 V at 1060 s) rather than step 15's cut-off.
* **Step 14** (`three-times-the-current`): the particle no longer clamps with the circuit.
  It clamps at 11 262 s, mid-tooth, when its surface runs dry; the circuit at 11 880. The
  floor under every tooth is −0.241 V, the reversal, not 0.3095 V, "a hole in the model".

Step 16's "596 seconds still to run" is 592 (a `Tie::Difference` of the two cut-offs; its
ratio, 2.28, did not move). The ledger's self-counts moved with all of it. The browser's
`past empty` row is `over-drained` now, at the owner's choice: on this model the deficit can
coexist with charge still inside.

## Tests

`crates/sim-data/tests/porous_reversal.rs`, all eight red on the parent by exit code (101):

| test | what it holds |
| --- | --- |
| `a_particle_driven_past_empty_holds_no_lithium_it_does_not_have` | every shell in `[0, 1]`; the deficit is the charge drawn past empty, to 1e-6 Ah |
| `a_rested_over_drained_cell_reads_the_reversal_floor` | 0.000 V after an hour's rest |
| `a_charge_repays_the_deficit_before_the_particle` | 2.5 Ah repaid, SOC below 1e-12 throughout |
| `a_fast_over_drive_is_paid_back_at_rest` | a deficit with SOC above 5 %; zero after rest; above 3.2 V |
| `a_scattered_group_driven_through_empty_settles` | 3 C, no `SOLVE_UNCONVERGED`, finite, bounded branch currents |
| `the_energy_ledger_of_an_over_drive_closes_with_the_step` | under 50 J at 1 s and under a fifth of the 10 s figure; the parent is 16 446 J out |
| `a_snapshot_mid_reversal_round_trips_through_json` | bit-identical continuation from a real serialization, surface-run-dry state |
| `over_discharge_damages_an_aging_single_particle_cell` | the capacity lost against a zero-coefficient control is `fade_per_ah` per Ah past empty, within −20/+5 % |

Plus `snapshot_version.rs::a_v21_shaped_spm_state_misparses_at_v22` (below), the version
pair moved to 21 → 22, `cell_footprint`'s `SpmState` 64 → 80 B, and `spm_long_step.rs`'s
scattered C/5 test, which now stops on the `SOC_CLAMPED_LOW` flag rather than on
`soc_true == 0.0` — the particle lands on its edge to rounding (4e-14 of capacity), not to
the bit.

## Perturbations

Each piece deleted in turn in a worktree of the finished change, `sim-core` and `sim-data`
run whole in debug (`--no-fail-fast`, below-normal priority), red decided by the exit code
and the failing tests named. Driver: `W:\temp\claude\porous-rev\perturb.py`; the
unperturbed baseline exits 0.

| # | deleted | exit | turned red |
| --- | --- | --- | --- |
| P1 | the split (the particle carries everything again) | 101 | seven of the eight new tests and `every_claim_matches_the_engine` — not the settle test: the fallback alone keeps that case bounded |
| P2 | `i_reversal_last` (a zero-length read divides as "deficit → none to the particle") | 101 | `the_energy_ledger_of_an_over_drive_closes_with_the_step`, `every_claim_matches_the_engine` |
| P3 | the reversal channel's heat (`reversal_w`) | 101 | `the_energy_ledger_of_an_over_drive_closes_with_the_step` |
| P4 | the settle fallback | 101 | `a_scattered_group_driven_through_empty_settles` |
| P5 | commit-or-nothing (commit a failed settle) | **0** | **nothing** — no case in the suite has a settle that fails on an `Spm` pack; the one found (5 C, network, at a margin of 1e-9) is not the shipped margin |
| P6 | the `Dfn` exclusion | **0** | **nothing, and nothing can**: with commit-or-nothing in place a failed settle changes no value, only the cost (the fingerprint run went from minutes to half an hour) |
| P7 | the surface edge (bulk only) | 101 | `a_fast_over_drive_is_paid_back_at_rest`, `a_snapshot_mid_reversal_round_trips_through_json`, `every_claim_matches_the_engine` |
| P8 | `edge.max(0)` | **0** | **nothing** — and it changed nothing measured either, so it was removed (above) |
| P9 | the node fenced by the cells' curves | **0** | **nothing**. It fires — 18 bisections forced over the 18 fast cases and the 20 A runs — but no measured case fails without it, alone or together with P10 |
| P10 | seeding from the previous step (an even share instead) | **0** | **nothing**; either seed settles every case measured. Seeding from the loop's last pass is what failed, and nothing seeds from there now |

P9 and P10 were each added to fix a measured failure, and each fixed it — at that commit.
Commit-or-nothing and the move away from the loop's seed landed after them and removed the
conditions they were fixing, which is the `spm-pack-window.md` shape ("a guard added during
tracing can be dead once a sibling guard lands"). They are kept because they are not
guards against a value but a choice of where to search — a seed and a bracket — and both
are what the search would want anyway; this table says plainly that no test holds them.

## The snapshot bump

Two `f64`s appended to `SpmState`. A v21 single-particle cell read at v22 takes the cell's
own `capacity_factor` and `r0_factor` — the next two bytes-worth in a `Cell` — as its
deficit and its reversal share: a scatter-free cell restored 100 % over-drained, quietly.
The version check is what refuses it. The fixture pack is equivalent-circuit and unchanged.

## Still open

* **The `Dfn`.** Its bulk-edge spike (`spike3_dfn_bulk.patch`) fixed the moderate cases
  (ledger 6.5 J at 1 s, linear) and left two defects: past the edge the voltage continued
  along the solve's own tangent, whose slope at a long step carries the OCV's fall across
  the step, so the voltage past empty depended on `dt` (−7.0 V at an hour, −0.39 V at a
  second); and its surface region — a front node empty with up to 42 % of the bulk left at
  5 C — swung by ±1e6 J. It needs its own edge, which has no closed form, and its own slope.
  ROADMAP H8.
* **Past full on the `Spm`.** The charge side is still the clamp and the flat curve.
* **The reversal's kinetics are the intercalation kinetics at an emptied surface**, whose
  exchange current is set by `SURFACE_EDGE`. Between margins of 1e-6 and 1e-9 that moved the
  deepest voltage of a 3–5 C over-drive by up to 0.79 V and the onset of runaway by up to
  9 s, and at 1e-9 one step of the 5 C network case at one second stayed unsettled. In the
  region past the surface's edge, the guard constant is still part of the physics — less
  than it was (the parent's state ran to negative lithium there), but not nothing. A side
  reaction with kinetics of its own would need a parameter nobody has measured.
* **The runaway integrator's sub-step cap** trips its debug assertion on a scattered 1S3P at
  3 C past empty from a 10 s step up — on the parent too (1280 K). The settle test's thermal
  arm runs at 1 s for that reason.
* **Absurd currents** (`Current(1e9)`) are absurd on both engines.
* **The settle under an external short**, and its cost: it runs only at the cap, after the
  loop's 32 passes, and was not benched (H9).
* **The 3 C gradient lesson's pulse runs 4 s past the cut-off it says it runs to** "just
  past". Shortening the pulse moves every rest-phase claim; not done.
