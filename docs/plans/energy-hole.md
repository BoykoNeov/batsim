# The energy hole: charge that vanishes at the clamp, and charge that appears

A cell-model fix that Phase 3 found, named precisely, and deliberately left open. Not a
numbered phase and not a client slice: it is one commit against the most golden-tested
path in the tree, and its whole content is making two ledgers close through a SOC clamp.

The defect, in `docs/plans/phase-3-aging-faults.md:838`, written by the slice that found
it and quoted here rather than paraphrased:

> Charge pushed into a cell whose SOC is clamped at 1.0 currently vanishes: it is not
> stored, and it generates no heat beyond `I^2*R0`. That is silent energy destruction,
> and the honest shape is `OCV * (rejected charge)/dt` driven from the *rejected
> fraction* `(raw_soc - 1.0)*capacity_as/dt`, not from a boolean "clamped" — the boolean
> version passes tests and is wrong at every clamp entry. The discharge side has the
> mirror image: at `soc = 0` the cell keeps sourcing current at `OCV(0)` forever.

Everything above the implementation notes was written before the work, from measurements
taken on the shipped engine at `1365808`. Where a later note contradicts it, the note is
the measurement and wins.

## Scope, settled by measurement rather than by symmetry

**The hole is ECM-only.** `crates/sim-core/src/spm.rs:729` is explicit that the
single-particle model does not truncate anything:

> The concentration profile is **never** clamped: an overcharged particle keeps the
> lithium it was pushed, so the flag says the *readout* has run past its window rather
> than that state was discarded. Getting that lithium back out is then a discharge, which
> is the physical answer and a more honest one than the equivalent circuit's hard SOC
> clamp.

`dfn::advance` inherits that contract. So `SOC_CLAMPED_HIGH`/`_LOW` mean two different
things depending on the cell model — *state was discarded* for an ECM, *the readout left
its window* for a porous-electrode cell — and only the first is a conservation defect.
One flag, two meanings, and the fix touches exactly one of them. That asymmetry is worth
a doc comment on the flags themselves, because it is not guessable from the name.

## The measurements

An out-of-tree probe at `M:\claud_projects\temp\energy-hole-probe`, path-depending on
`sim-core` and `sim-data` so the half it shares with the engine *is* the engine — the
same arrangement Phases 5, 6 and 7 used for their spikes. It copies `properties.rs`'s
`chem()` / `flat_chem()` fixtures verbatim so its accounting is the property test's own.

### 1. The instrument this fix was about to be built against cannot see the defect

`crates/sim-core/tests/properties.rs:369` — the pack energy balance — and `:559` — charge
conservation through an internal short — both open with a `prop_assert!` that **no SOC
clamp occurred**. That reads like the hole's exact negative image, and the obvious exit
criterion is "delete the exclusions and watch them fail". It is the wrong criterion, and
the probe says so in two independent ways.

**It is an algebraic identity.** The test computes its chemical term from *current*
(`FLAT_V0 * (S·i_actual + i_balancing) * dt`), never from ΔSOC. Per cell,
`v_start = V0 − i·r0 − ΣV_rc` and `q_gen = i·(i·r0 + ΣV_rc)`, so
`chemical − electrical − heat ≡ 0` term by term whatever SOC does. Driven deliberately
into each clamp:

| probe run (flat chem, 1S1P, dt = 0.5 s, 60 steps) | imbalance before first clamp | imbalance over the whole run |
| --- | --- | --- |
| high clamp, soc₀ = 0.95, i = −40 A (clamps at step 22) | −4.55e−13 J | **+4.55e−13 J** |
| low clamp, soc₀ = 0.05, i = +40 A (clamps at step 22) | 0 J | **−4.55e−13 J** |
| control, soc₀ = 0.50, i = −4 A (never clamps) | — | −2.61e−13 J |

Rounding, on both sides of the clamp, indistinguishable from the control. **Deleting the
exclusion would have gone green on unfixed code**, and the fix would have shipped with a
gate that proved nothing.

**And the exclusion is vacuous anyway.** `CAP_AH = 2.5` is 9000 As; the strategy's worst
case is `|i| = 4 A × dt = 0.5 s × 200 steps` = 400 As from `soc = 0.5`, i.e. ΔSOC ≤ 0.044.
No case that proptest can generate reaches either clamp, so the `prop_assert!` has never
fired in its life. Two reasons for the same green, and neither is the physics.

Recorded in its general form because it is the third phase running to pay this:
**an exclusion is not evidence that what it excludes was ever dangerous.** The way to find
out is to force the excluded case and measure, not to read the comment.

### 2. The charge ledger *is* the sensitive instrument

`properties.rs:559`'s quantity — `∫(i_actual + i_short) dt` against `3600·Δremaining` —
diverges hard the moment a clamp bites:

| probe run | ∫I dt | 3600·Δremaining | gap |
| --- | --- | --- | --- |
| high clamp | −1200.000 As | −450.000 As | **−750.000 As** (2475 J at V0) |
| low clamp | +1200.000 As | +450.000 As | **+750.000 As** (2475 J fabricated) |
| control | −120.000 As | −120.000 As | 0.000 As |

That is the ledger to build against. It also settles a design question: **a heat-only fix
closes nothing here.** The coulombs stay unstored — correctly, if a side reaction ate
them — so unless the rejected charge is *reported*, no invariant can be written that
distinguishes a cell that rejected 750 As from one that miscounted.

### 3. The hole is not a correction term; it dominates

Shipped `lfp_26650_generic`, 1S1P, no BMS, live thermal network, charged from 90 %:

| run | rejected charge | energy destroyed | heat the run generates | destroyed / heat |
| --- | --- | --- | --- | --- |
| 1C for 2000 s | 3777.7 As | **13 600 J** (45.6 % of the cell's nominal 29.9 kJ) | 327 J | **41.6×** |
| 20C for 400 s | 17 598.4 As | 63 354 J (212 % of nominal) | 22 654 J | 2.80× |

At 1C the missing heat is **forty-one times** everything the engine currently reports, and
the cell ends the run at 298.6 K — a pack being held at 1C into a hard clamp for
half an hour, and the simulator says it is at room temperature. Overcharge heating is not
a refinement to the runaway path; on the ECM it is *the* runaway path, and it is absent.

### 4. The trajectory gate has the wrong half of the case covered

The out-of-tree instrument at `M:\claud_projects\temp\phase6-baseline`, anchor
`after-sliceD-p7.txt`, nine cases. Tallying the clamp bits across every sampled step:

| case | HIGH | LOW |
| --- | --- | --- |
| `lfp_2s3p_scatter_thermal_nobms` | 0 | 32 |
| `lfp_2s1p_cold_plating_extshort` | 0 | 33 |
| `lfp_2s2p_hot_runaway_nobms` | 0 | 29 |
| `nmc_1s1p_2rc_isothermal` | 0 | 3 |
| `soft_short_under_a_lying_sensor` | 0 | 2 |
| `lgm50_2s2p_spm_scatter_thermal_aging` | 1 | 0 |

**Five of nine ECM cases already clamp low. No ECM case ever clamps high**, and the single
HIGH belongs to an SPM case, where the flag is a readout notice and no state was
discarded. So the gate is blind to precisely the side this fix changes — slice D's
"no DFN case at all" hole, one phase later — and it must gain an ECM overcharge case
*before* a byte-identical run means anything.

## The design

One new reported quantity, one new heat term, and a deliberate refusal.

### `Telemetry::i_rejected_a` — the side channel that makes the ledger writable

Discharge-positive \[A\], like every other current in the API: the part of `i_actual` that
crossed the terminals without changing stored charge. Negative while charge is being
refused at the top, positive while charge is being invented at the bottom, exactly zero on
any step where no ECM cell clamped.

This follows the shape the engine already uses twice — `i_balancing_a` and
`i_internal_short_a` exist because an invariant needed a term to name, and their doc
comments say so. The charge ledger becomes, exactly and through clamps:

    ∫(i_actual + i_internal_short_a − i_rejected_a) dt  =  3600 · Δ(stored charge)

Driven from the **rejected fraction**, per Phase 3's warning: `(raw − 1.0)·capacity_as/dt`
on the high side and `(0.0 − raw)·capacity_as/dt` on the low side, never from a boolean.
On the step where a clamp is first entered, only part of the step's charge is refused, and
a boolean would over-report the whole of it while still passing every test in the suite.

### The overcharge heat term

`Q_reject = OCV(1.0) · (rejected charge)/dt`, added to the cell's `q` and therefore into
both `q_gen_w` and the vector that drives the thermal network. `OCV(1.0)` is the
**endpoint** of the table, not `OCV(soc_start)`: the charge is being pushed into a cell
that is at the top of its window. On LFP the difference is not academic — the last 2 % of
that table climbs 180 mV.

It is folded into `q_gen_w` rather than reported separately, and the precedent that
decides it is `q_runaway_w`'s own doc comment, which explains why *it* is kept out:

> this heat comes from the decomposition of the cell's own materials, a separate reservoir
> (`runaway_energy_j`) that the OCV knows nothing about. Adding the two would make the
> four-term balance that closes exactly stop closing.

Rejected-charge heat is the opposite case on every clause: it comes from the electrical
energy the ledger is already tracking, out of the reservoir the OCV *does* describe, and
folding it in is what keeps the balance closing. A reader who wants it separately can
recover it from `i_rejected_a` and the chemistry.

### No cooling term on the discharge side — three independent reasons

The mirror term falls out of the same algebra with the opposite sign: `−OCV(0)·i_rejected`
is what would make the energy ledger close at the bottom clamp. It is not being added.

1. **It is a wrong-signed drive into a real integrator.** `heat_w` feeds `thermal.rs`. A
   cell that is over-delivering would *cool itself*, and in the one regime where that
   matters — a hot pack being driven flat — it would suppress the runaway physics. A
   silent hole is better than an active lie in the opposite direction.
2. **It forfeits bit-identity on five of nine baseline cases** (table above), and buys
   nothing measurable for them.
3. **The energy is fabricated, not misplaced.** Overcharge has a real mechanism — side
   reactions consume the charge and dissipate it — so converting it to heat is physics.
   Over-discharge has none: no reaction makes electrons. The honest fix is for the cell to
   *stop sourcing*, which is a solve-side change, and that is out of scope for the reason
   below.

Instead the low side is **reported and pinned**: `i_rejected_a` makes the fabricated
charge visible, and the test suite asserts the energy ledger's residual is *exactly*
`OCV(0)·∫i_rejected_a dt` there — a defect with a number on it, bounded by an invariant,
rather than a silent violation. That is the same move as the previous commit's "an
invariant that bounds a proxy".

### What is deliberately not attempted, and why it is not a judgement call

Making the cell refuse to over-deliver — capping the deliverable current, or collapsing
`E` at empty — makes an ECM's terminal voltage nonlinear in `i` within a step. That
forfeits `CellModel::is_linear() == true` (`crates/sim-core/src/ecm.rs:308`), which is the
flag the pack branches on, and which every ECM bit-identity claim from Phase 1 through
Phase 7 rests on:

> When every cell answers `true` the aggregated Thévenin is exact too, the closed-form
> solve is the whole answer, and the iteration exits on its first pass having done
> literally the arithmetic Phase 1 did.

It also collides with `CLAUDE.md`'s "with `bms: None`, demands pass through unclamped".
So the solve-side fix is a separate slice with its own argument, priced here and not
smuggled in: it is the only way to close the low side honestly, and it costs the
equivalent circuit's exactness.

## Exit criteria

| criterion | carried by |
| --- | --- |
| **1. Both ledgers close through a clamp.** The charge invariant holds exactly across high and low clamps with the `i_rejected_a` term, and the energy balance closes exactly at the high clamp. Both properties keep their teeth: the clamp is *forced*, not left to a strategy that cannot reach it. | `properties.rs`, new clamp-driven cases |
| **2. The low side's residual is pinned, not hidden — amended, see the perturbation table below.** A test asserts the energy ledger's residual at the bottom clamp equals `OCV(0)·∫i_rejected_a dt` ~~to rounding~~ *to accumulation error, with its chemical side taken from ground-truth state*. As first written it took that side from telemetry, which made the assertion an identity in the very quantity it was checking; the perturbation caught it. | `properties.rs` |
| **3. Nothing that did not clamp high moved.** Every committed golden and every existing baseline case is byte-identical, the eighteenth telemetry column excepted. | the trajectory instrument, extended |
| **4. The gate can see the change.** The instrument gains an ECM overcharge case that reaches `SOC_CLAMPED_HIGH`, and that case's `q_gen_w` moves. A green sweep over a hole is not a pass. | the instrument, re-anchored |

## Predicted blast radius, to be checked against the measurement

Written before running anything, so that a surprise is visible as a surprise.

| site | prediction |
| --- | --- |
| `tests/golden/**` (all discharge or pulse; no charge leg) | **unmoved** — the low side changes no number |
| `analytic_golden.rs:342` (asserts `SOC_CLAMPED_HIGH`) | trajectory **moves**; assertion is on a flag, so it should still pass |
| `scenario_runaway.rs:392` (`SOC_CLAMPED_HIGH \| THERMAL_RUNAWAY`) | **moves hard** — reaches runaway *sooner*. Check for a timing window rather than a reached-state |
| `scenario_protection.rs:228/252/307` | `:252`'s BMS-off arm moves; the BMS-on arms should not clamp at all |
| `faults.rs:481/630`, `scenario_weak_cell.rs:105` | low-clamp only — **unmoved** |
| `sim-core` snapshot layout | **no new state**; the rejected amount is computed and consumed inside the step. **No `SNAPSHOT_VERSION` bump** |
| `API_VERSION` / `WASM_API_VERSION` | an 18th telemetry field is a wire change. **Read each constant's own doc before paying a bump** — Phase 6 planned to move them together and they had already parted |

## Slices

| slice | scope |
| --- | --- |
| **A** | `sim-core`: `coulomb_step` returns the rejected amount, `CellModel::advance` carries it out, `pack.rs` tallies heat *after* `advance` (it currently tallies at `:1682` and advances at `:1704`), `Telemetry::i_rejected_a`, the flag doc-comment split, both invariants, the instrument's 18th column and its new overcharge case. Carries every exit criterion. |
| **B** | adapters and client: the field through `sim-server` / `sim-wasm` / `sim-godot`, version constants, and the pedagogy payoff — "the charge you pushed in that went nowhere" is a lesson the path has no way to show today. **In scope, by owner decision**, so the wire change and the engine change land together rather than leaving six entry points with no caller. |

`cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` stay
clean at every commit.

### One reordering hazard worth naming in advance

Moving the `q_gen_w += q` tally below `advance` must not perturb a cell that did not
clamp. `q + 0.0` is bit-identical to `q` for every value **except `-0.0`**, which becomes
`+0.0`. The addition is therefore gated on a non-zero rejection rather than written
unconditionally, and the baseline diff is what confirms it.

---

## What landed

One commit, both slices, `SNAPSHOT_VERSION` unmoved at 11. Everything below is measured
on this box against the shipped engine; where it contradicts the pre-work text above,
this is the measurement and wins.

### The three exit criteria, and the one that needed a new instrument

**1 and 2 — both ledgers.** `properties.rs` gains three clamp-driven properties:
`charge_conserved_through_a_soc_clamp`, `overcharge_heat_closes_the_energy_ledger`, and
`the_bottom_of_the_window_fabricates_exactly_what_it_reports`. All three carry a
**coverage assertion** (`clamped_steps > 0`), and that was not decoration: the first
draft's strategies produced cases — `parallel = 2, amps = 20, dt = 0.25, nsteps = 40`
moves 100 As of 9000 — that never reached the clamp they were aimed at, and the coverage
assertion is what said so instead of passing quietly. The shipped bounds are sized so the
weakest corner still moves 400 As against the 270 As that separates `soc0` from the bound.

The third property is the unusual one: **it asserts the model's own defect**, pinning the
ledger residual at exactly `OCV(0)·∫i_rejected_a dt`. It is written to fail if the
fabrication is ever fixed properly, and says so — a solve-side fix should announce itself
in a test that names it, rather than as a golden moving by an amount nobody can attribute.

**3 — the floor did not move, with no exception.** With the new 18th column stripped, the
1532 lines of the instrument's nine pre-existing cases are **byte-identical** to
`after-sliceD-p7.txt`; the only differing line is the header that declares the field
count. That also settles two hazards that were argued in advance rather than measured:
the low clamp adds no heat (five of the nine cases clamp low and would have moved), and
moving the heat tally below `advance` perturbs nothing.

**4 — the gate can see the change**, which it could not before. The instrument had *no
ECM case that clamps high* — five clamp low, 99 sampled steps, and the single
`SOC_CLAMPED_HIGH` in the whole dump belongs to an SPM case where nothing is discarded.
The new `lfp_2s2p_overcharge_clamp` case reaches the high clamp on 5 sampled steps with
`i_rejected_a` at −10.0 A and `q_gen_w` up to 36.75 W. It needed **its own legs**: the
shared `schedule()` cannot reach the upper clamp on any chemistry in the file, because
its discharge leg moves more charge than its charge leg.

### The runaway fixture was sized against a heat budget that no longer exists

Three tests failed, and **all three were controls** rather than the assertions they
guard — which is the useful shape, because a control that stops discriminating is
supposed to be loud.

`scenario_runaway.rs` picked `CHARGE_A = -60.0` so that *ohmic heating alone* would
plateau between onset and vent. At 20 A per cell the ohmic term is ~11 W and the
rejected-charge term is `3.6 V × 20 A = 72 W`, so the same fixture now runs to 1321 K and
vents on the charger alone: the no-reaction control vents, and the propagation control
cooks the centre cell's neighbour to 1114 K, so neither can attribute anything to the
reaction any more.

The constant was **re-derived rather than the assertions relaxed**, by sweeping the
control arm for the plateau it was originally chosen to produce:

| pack current | per cell | control plateau | verdict |
| --- | --- | --- | --- |
| −60 A (old) | 20.00 A | 1321.70 K | vents on the charger alone |
| −12 A | 4.00 A | 481.50 K | vents |
| −10 A | 3.33 A | 450.20 K | between onset and vent |
| **−9 A (shipped)** | **3.00 A** | **434.66 K** | **between onset and vent** |
| −8 A | 2.67 A | 419.20 K | below onset — nothing could ignite |

At −9 A every bracket the fixture needs holds: centre 434.66 K (+11.5 above onset, 18.5
below vent), its four neighbours 407.05 K (16.1 *below* onset, so propagation stays
attributable), corners 385.68 K. **And the live arm reproduces the old timings almost
exactly** — centre vents at 2921 s against the old fixture's 2918 s, neighbours 3366 s
against 3409 s, corners 3599 s against 3652 s. The same fire on the same clock, reached
with a sixth of the current: 1.3 C against a chemistry whose `max_charge_c` is 1.0, which
is a far more plausible abuse than the 8.7 C this scenario used to need.

### The protected arm exposed a limit cycle that has been there since Phase 2

`the_same_abuse_through_a_bms_never_gets_warm` failed at 333.17 K against a bound of
323.15 K, and the name is now wrong rather than the number: the pack no longer stays at
room temperature. It is still saved — 90 K short of onset, no vent, no runaway, contactor
never opened — but it is saved by the **over-temperature rung**, which this scenario had
never reached before. The test is renamed
`the_same_abuse_through_a_bms_is_stopped_at_its_temperature_limit` and now asserts `OT`,
asserts the absence of `CONTACTOR_OPEN`, and bounds the temperature against the
chemistry's own `t_max_k` rather than a round number.

Why it warms at all is the part worth recording. Protection's over-voltage response is
memoryless, so at the top of charge it is a **two-step limit cycle**: one step admits the
full derated 6.91 A and raises `SOC_CLAMPED_HIGH`, the group voltage lands above `v_max`,
the next step derates to zero, the voltage falls back with no load, repeat.

This was very nearly misdiagnosed. Sampling every 20 steps showed `i_actual = 0` and
`q_gen_w ≈ 0` at every sample while the pack climbed 298 → 333 K — heat with no source —
because a 20-step stride **aliases exactly onto the zero-current phase of a two-step
cycle**. Printing every step in the window is what produced the actual numbers.

It is not this commit's doing, and that was checked rather than argued: a `git worktree`
at `1365808` with the same probe shows the **identical** cycle, same currents, same flags,
same steps. What changed is the price of an admitted step — **1.32 W to 73.57 W**, a
factor of 55 — and a 50 % duty cycle on that is what walks the pack to its temperature
limit. Hysteresis on the protection comparators is a separate change with its own
argument; the chatter is recorded here because it stopped being cosmetic.

**Transferable: when a periodic system shows an impossible quantity, suspect the sampling
stride before the physics.** The engine was fine; the instrument was reading one phase of
a two-phase cycle.

### The perturbation table, and the two properties that did not bite

`docs/plans/phase-7-dfn.md`'s rule — *"tabulate which tests actually catch the
perturbation rather than assuming the suite does"* — was run against the obvious
perturbation: **multiply `rejected_as` by 2** in `coulomb_step`, on both clamps.

| test | doubled `rejected_as` |
| --- | --- |
| `energy_hole::the_rejected_charge_burns_at_the_windows_endpoint` | fails |
| `energy_hole::only_the_fraction_that_did_not_fit_is_rejected` | fails |
| `energy_hole::the_bottom_of_the_window_adds_no_heat` | fails |
| `properties::charge_conserved_through_a_soc_clamp` | fails |
| `scenario_runaway`'s two controls | fail |
| **`properties::overcharge_heat_closes_the_energy_ledger`** | **passes** |
| **`properties::the_bottom_of_the_window_fabricates_exactly_what_it_reports`** | **passes** |

The two properties written specifically to be this commit's exit criteria were the two
that could not see a doubled rejection, and both had a doc claiming otherwise — the plan
text above said the second one "pins the ledger residual" and "is written to fail if the
fabrication is ever fixed properly". Neither was true as first written.

**Why, and it is worth being precise because the mistake is easy to repeat.** The first
draft took the chemical side from telemetry: `V0·(S·i_actual − i_rejected_a)·dt`. The heat
side already contains `−V0·i_rejected_a`, from the rejection term. So the residual is

```text
[V0·(S·i − i_rej)] − v_start·i − [q_ohmic − V0·i_rej]
    = (V0·S·i − v_start·i − q_ohmic) − V0·i_rej + V0·i_rej
    = 0     for any value of i_rej whatsoever
```

`i_rejected_a` appears on both sides with opposite signs and cancels. The property was an
identity in exactly the quantity it was written to check — the same failure as
`electrical_and_heat_energy_balance`'s clamp exclusion at the top of this document, which
this document had already diagnosed, reached again one level down.

**The fix is to source the chemical side from ground-truth state** —
`FLAT_V0 · 3600 · Δ(remaining Ah)` via `total_remaining_ah`, which the file already had —
so `i_rejected_a` appears on one side only. Re-run, both now fail under the same
perturbation. The tolerance loosens from `1e-12` to `1e-9` relative in the process, and
that is honest rather than a concession: the two sides now accumulate differently (one
telescopes through the SOC state, the other sums a few hundred per-step products), so they
agree to accumulation error rather than to a single rounding.

**Transferable, and sharper than the version at the top of this document: a conservation
test that draws every term from the same reported quantities is checking arithmetic, not
physics.** At least one side has to come from state. "Is this an identity?" is answerable
in five lines of algebra and is worth asking of any ledger before trusting it — but the
thing that actually caught it was running the perturbation.

### The prose this commit falsified

Closing a hole that three documents describe as open means those descriptions become
wrong, and one of them was user-facing:

* **`web/app.js`, the guided path's `leg-that-is-not-there` step** told the reader that
  after `SOC_CLAMPED_HIGH` "the charge counter stops but the current does not, because
  this engine models no overcharge chemistry and the energy simply goes nowhere. That is
  a hole in the model." Rewritten against a measured run of that exact step: `heat` goes
  from 0.041 W to **4.181 W** (1.15 A at the 3.60 V ceiling is 4.14 W of side reaction
  against 41 mW of ohmic loss), the `clamp` readout reads `refused 1.150 A`, and the
  **entry step at 5769 s reads `refused 0.822 A`** — only the part that did not fit, which
  is the fraction-not-boolean rule visible on a page. The temperature does *not* move, and
  the new prose says why: that scenario is isothermal, so heat is reported and never
  integrated.
* **`docs/plans/phase-3-aging-faults.md`** and **`docs/plans/protection-escalation.md`**
  are amended in place with a pointer here, in the house style slice A used. The second
  needed care: its measurements are about the *discharge* half, which is still open
  exactly as it describes, so the amendment says the two faces have parted rather than
  that the section is superseded.

### Adapters: the two version constants parted again, for the second time

Read each constant's own doc before paying a bump, which is the mistake Phase 6 recorded
and this is the second time it has paid:

* **`sim_server::API_VERSION` stays at 2.** Its rule is explicit — "adding a field or an
  error code does not bump it" — and `i_rejected_a` is an added field. Precedent: Phase 6
  slice B's added `"spm":null` was exempted by the same clause.
* **`sim_wasm::WASM_API_VERSION` moves 3 to 4**, not because of a rename but because of
  what that constant is actually for: `web/pkg` is a gitignored build artifact loaded
  separately from the JS that calls it, `web/app.js` now reads `i_rejected_a` in two
  render paths, and against a v3 bundle that field is `undefined` — a `TypeError` from
  inside a draw call, naming neither cause nor fix. `WASM_API_MIN` moves with it, which is
  what makes the bump load-bearing rather than decoration.
* **`sim-godot` is unchanged, deliberately.** Its telemetry surface is a *curated* set of
  `#[func]` accessors that already omits `q_gen_w`, `i_internal_short_a` and
  `q_balancing_w`. Adding one for `i_rejected_a` would be a seventh entry point with zero
  callers, which is exactly what the UI-pedagogy slice recorded as a mistake.

The three transport bit-dumps (`sim-server/tests/common/mod.rs`,
`sim-server/tests/snapshot_json.rs`, `sim-wasm/tests/engine.rs`) each claim to carry
"every `f64` a `Telemetry` carries" and were extended, or the claim would have become
false the moment the field landed.

### The client

One history channel, one plot trace on the existing current panel, one readout row. The
readout returns `null` — an em dash — when nothing was rejected, on the same terms as
`soc (bms)`: a row reading `0.000 A` for an entire ordinary run teaches nothing. When it
does fire it spells the direction out (`refused` / `invented`) rather than relying on the
sign convention, because "charge going in and being refused" is the case that makes heat
and the sign alone does not carry that.

### Predicted blast radius versus measured

| site | predicted | measured |
| --- | --- | --- |
| `tests/golden/**` | unmoved | **unmoved** — all discharge or pulse, and the low side changes no number |
| `analytic_golden.rs:342` | moves, still passes | **exactly that** — it charges into the clamp, so `q_gen_w` moved; it asserts on SOC and a flag |
| `scenario_runaway.rs` | moves hard, check for timing windows | **moved hard, and all three failures were controls** — see above |
| `scenario_protection.rs`, `faults.rs`, `scenario_weak_cell.rs` | low-clamp only, unmoved | **unmoved** |
| `SNAPSHOT_VERSION` | no bump | **no bump** — no new state |
| adapter versions | read each doc first | **they parted** — server 2, wasm 3 to 4 |

## Still open, and priced

* **The low clamp still fabricates energy.** Reported and pinned, not fixed. The
  solve-side fix costs `CellModel::is_linear() == true` for the equivalent circuit and
  collides with `CLAUDE.md`'s "with `bms: None`, demands pass through unclamped"; it wants
  its own slice and its own argument.
* **Protection chatters at the top of charge.** Pre-existing, measured at `1365808`, and
  now expensive: a 50 % duty cycle depositing 73.57 W. Hysteresis on the comparators is
  the obvious fix and is not free — it is state, so it is snapshot layout.
