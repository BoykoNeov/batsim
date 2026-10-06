# Roadmap — the scientific hurdles, and the phases after 8

Phases 0–8 are complete and each is pinned by a committed test (see the README's status
table). A hundred and fifteen design notes under `docs/plans/` record what each slice measured,
built, and deliberately did not build, and most of them end with a list of what is still
open. This file reads across all of them and puts those lists in one place, ranked by how
much they limit what the engine can honestly claim, with what each would cost. It was
assembled on 2026-09-02 from the notes as they stood then; a note's own "Still open"
section is the authority where they disagree, and an item here should be struck through
here when the note that closes it lands.

The standing rules from `CLAUDE.md` apply to everything below: chemistry is data, no
unlabeled constant ships, snapshot layout changes cost a version bump with a pair test,
predictions are registered before a run, and a phase is done when its exit criterion's
test passes — not when the list of interesting things runs out.

---

## 1. Where the engine stands

Three cell models behind one API (`Ecm`, `Spm`, `Dfn`), seven chemistries, a thermal
network, a sensor-limited BMS, four capacity-fade mechanisms with matching resistance
growth, a fault queue, emergent plating and runaway, snapshots at `SNAPSHOT_VERSION` 24,
and four clients (server, browser, Godot, an example script). Against grid-converged
PyBaMM references the SPM tracks to 2–7 mV over a discharge and the DFN to 5.8 mV at 1 C.

What the engine cannot yet honestly claim is the shorter list, and it is the subject of
this file.

---

## 2. Open scientific hurdles, ranked

Each entry: what is missing, why it matters, the evidence in the notes, a proposed
approach, and the cost. **Rank is by how much the gap limits a claim the project wants to
make**, not by effort.

### H1. LFP has no porous-electrode model, and it is the teaching chemistry

**Gap.** `[spm]` and `[dfn]` are NMC-only by decision: lithium iron phosphate intercalates
through a moving phase boundary, and a single particle with Fickian diffusion is the wrong
physics for its flat plateau (`phase-6-porous-electrodes.md`, README "`[spm]` and `[dfn]`
are NMC-only on purpose"). So the chemistry the guided path opens with, and the one whose
flat curve every estimator lesson turns on, can only ever be run through an equivalent
circuit here. The plateau, the voltage hysteresis, the path dependence of its OCV and the
rate dependence of its knee are all fitted, never produced.

**Approach.** A fourth `CellModel` variant rather than a parameter set: a **multi-particle
single-particle model** — an ensemble of `N` particles per electrode sharing one electrolyte
node, each with its own radius drawn from the seeded RNG, and a **non-monotone
(double-well) open-circuit potential** for the phase-separating electrode. That is the
minimal model in which the plateau *emerges*: particles fill one at a time (the mosaic
picture), the ensemble OCV is flat while individual particles sit on either side of the
spinodal, and charge/discharge branches separate. It reuses `spm.rs` (radial diffusion,
Butler–Volmer, the exact-bits tests) and the pack's nonlinear solve; what is new is the
per-particle bookkeeping, the OCP shape, and a stability treatment of the spinodal region
(a regularised OCP or a small intra-particle mixing term, chosen by measurement). A
shrinking-core model is the cheaper alternative and reproduces the plateau but not the
hysteresis; a phase-field (Cahn–Hilliard) particle reproduces everything and costs a PDE
per particle, which the 141× DFN already shows is too slow for a pack.

**Reference.** PyBaMM's `Prada2013` LFP set gives a DFN with a *fitted monotone* OCP, so a
golden pipeline exists for the rate behaviour but not for the phase separation. The
phase-separation claims (plateau flatness, GITT relaxation, branch separation at rest)
need a literature reference stated with its tolerance, or they stay qualitative and the
lesson is written about a number the model does produce.

**Cost.** A phase (proposed as **Phase 9**, §3). `CellModel` gains a variant (snapshot bump;
enum dispatch is designed for this). `[spm]` for LFP needs extracted parameters with the
OCP replaced — a `tools/reference/` extension. Per-cell cost lands between SPM and DFN.

### H2. Aging is semi-empirical on every model, and the porous models cannot age their pores

**Gap.** Calendar and cycle fade are `sqrt(t)` and throughput laws with placeholder
coefficients in every shipped file (`phase-3-aging-faults.md` §"no test asserts an
end-of-life number"). On `Spm`/`Dfn` aging is applied as the same multipliers on
capacity and resistance; nothing grows an SEI layer, consumes electrolyte, loses active
material by mechanism, or changes porosity (`phase-7-dfn.md` "DFN aging cannot age
porosity"; `dfn-aging-gap.md`). The RC and `R0` growth share one coefficient
(`rc-resistance-growth.md`), aging does not reach `[diffusion]`
(`diffusion-overpotential.md`), and `Telemetry::soh_resistance` is an `R0` ratio only.

**Why it matters.** "Model capacity fade with matching resistance growth" is a
non-negotiable principle, and today both are curves that were *drawn*, not consequences of
anything. A student who asks *why* a cell ages gets a coefficient.

**Approach.** An `AgingModel` enum beside `CellModel` — the semi-empirical law stays as one
variant, and a **physics-based SEI variant** is added for the porous models: a
reaction-limited or diffusion-limited SEI growth law (Single 2018 / the family PyBaMM ships
as `SEI: "reaction limited"` and `"solvent-diffusion limited"`) that consumes cyclable
lithium (LLI) and adds film resistance, so `sqrt(t)` calendar fade and the resistance rise
*fall out* rather than being coefficients. PyBaMM can generate goldens for exactly this,
which is what makes it buildable under the testing strategy. Loss of active material and
porosity change are second-order and should be scoped only after LLI validates.

**Cost.** A phase (proposed as **Phase 10**, §3). Aging state grows (snapshot bump). The
SEI parameters for LG M50 are published (OKane 2022 extends Chen 2020), so the extraction
script gains a section rather than a fit.

### H3. Nearly every constant that is not extracted from PyBaMM is a labelled placeholder, and there is no fitting pipeline

**Gap.** `tools/reference/` generates goldens and *extracts* parameters; it never fits
(`phase-8-chemistries.md` "No fitting pipeline"). So: all `[aging]` coefficients in every
file; the ECM half of LG M50 (its `R0`/RC beside extracted `[spm]`/`[dfn]`); all three
`[reversal]` constants in every file; NiMH's `gamma`, RC split and derived `dU/dT`;
lead-acid's `[r0]` rise toward empty and a three-parameter `[diffusion]` fit against a
seven-point table with a degenerate valley; sodium-ion's plating gate. Entropic
coefficients (`docv_dt_v_per_k`) are absent on every lithium file, so the reversible heat
term — half the thermal physics — is off wherever it would matter most.

**Approach.** A `tools/reference/fit_ecm.py` that fits `R0(soc, T)` and the RC pairs to a
PyBaMM DFN pulse/relaxation set by least squares and prints a TOML block with the fit's
residual in its provenance; a `fit_entropic.py` that extracts `dU/dT` where a set publishes
it (Ai 2020 does for its cell; check OKane 2022 for LG M50) rather than inventing it; and a
rule that a placeholder may only be replaced by a number whose provenance names the fit
and its residual. **This turns the provenance rule from a labelling discipline into a
measurement discipline**, and it is the first slice that makes the LG M50 "same cell
through both models" comparison an honest one.

**Cost.** Python, no engine change, one file at a time; but every fitted number moves a
trajectory, and on LG M50 the ECM pulse-train lessons (steps 12–14) quote numbers that
would move with it. Budget the claims re-measurement into the slice.

### H4. Charge acceptance is one number, and the knee it competes with was placed by hand

**Gap.** `[charge_acceptance]` (`charge-acceptance.md`, `SNAPSHOT_VERSION` 21) is a linear
taper with one onset, stated at one rate and one temperature. Real oxygen evolution is a
kinetic competition: acceptance falls earlier at higher current and when hot. And the
slice's finding is that the taper and the file's hand-placed OCV knee are *not
independent*: the knee stood in for the oxygen-evolution overpotential, and the −ΔV signal
at the charger's instant is decided by how the two are sized against each other. The
shipped onset is bounded by measurement (a wider taper erases the signal; a narrower one
is the corner again) but it is a placeholder.

**Approach.** Replace the knee-plus-taper with the thing they stand in for: an explicit
**oxygen-evolution overpotential** on the positive electrode — a Butler–Volmer side
reaction whose exchange current has an Arrhenius temperature dependence, so the peak's
height, its rate dependence and its temperature coefficient all follow from three cited
numbers instead of from a table's last segment. That is the model NiCd needs too, and
NiCd is "nearly free" once it exists (`phase-8-chemistries.md` names it and deliberately
does not schedule it).

**Cost.** One slice; a snapshot bump only if the side reaction carries state (it need not).
The lesson numbers in steps 27–28 move again.

### ~~H5. Cross-platform determinism is not promised, and the browser and the server may already disagree~~ — **CLOSED** for native Windows and wasm

**Gap.** `CLAUDE.md` promises same-binary determinism and explicitly not bit-exactness
across platforms, because `exp`, `ln` and `powf` come from the platform's libm. The
consequence nobody has measured: the wasm build and the native server are *different
binaries with different libm implementations*, so the guided path in the tab and the same
scenario over the socket may not produce identical trajectories, and the committed
trajectory instrument was declined as a repo test on exactly this ground
(`phase-6-porous-electrodes.md` "Committing the PyBaMM trajectory baseline").

**Approach.** Route every transcendental call in `sim-core` through one `math` module
backed by a pure-Rust implementation (the `libm` crate, which is what wasm32 already
compiles against). Then bit-exactness *is* claimable across native and wasm, the
trajectory instrument can be committed as a test, and "snapshot in the browser, restore
on the server" becomes a promise. Measure first: count the call sites, price the
`exp`/`powf` cost on the 100S10P bench, and expect the goldens to move by ULPs and need
re-pinning under a declared numerical change (the precedent is `pack-step-perf.md`'s
refusal of multiply-by-reciprocal: this is the same class of change, taken deliberately).

**Cost.** Small in code, large in re-pinned exact-bit tests; one slice with a perturbation
table.

**Measured 2026-10-06** (`cross-platform-math.md`): they do disagree — 33 of 54 runs part
in the last bit — but every field stays within ~10⁻¹⁴ of its value, and the RNG state and
all 184 flag transitions are identical. The `libm` crate matches wasm's maths bit for
bit, and routing the 24 calls through it makes all 54 runs, and a snapshot handed either
way, agree exactly. The re-pinning cost above was **wrong**: no test moved. Its cost read
5–6 % of the native step on a saturated box (a quiet-box reading is owed), probably most
of it a per-cell RC decay the pack already computes.

**Closed 2026-10-06, on the owner's choice of "the reuse first, then `libm`".** Each cell
now takes the pack's RC decay (13–15 % off the step, bit for bit), and every
transcendental in `sim-core` goes through `crate::math` on `libm`, pinned exactly and
enforced by a clippy `disallowed-methods` list. The two builds now compile the same source
for those functions, so the native build moved to the wasm build's bits — the browser did
not move at all — and `tools/wasm-parity/parity.mjs` checks every scenario plus a
snapshot handoff against the built `web/pkg`: 108 of 108. No test moved and no snapshot
bump. What stays open: other targets (Linux, macOS) are unmeasured, and the trajectory
instrument the Phase 6 note declined could now come in — as a parity check, not as a
committed baseline, since `cargo test` cannot see wasm.

### ~~H6. Thermal integration stops being valid above a 1.7-hour step, which the aging fast-forward exceeds~~ — **CLOSED**

**Gap.** The thermal network is explicit Euler with sub-stepping, and the sub-step cap
binds above `dt ≈ 1.7 h` (`phase-2-thermal-bms.md`). Months-long aging fast-forward is a
stated use of the engine.

**Approach.** The network is linear in `T` over a step (conductances and `h·A` fixed,
`Q_gen` piecewise constant), so the exact update is a matrix exponential — or, cheaper and
unconditionally stable, backward Euler with one banded solve per step, the same shape as
the SPM's radial solver. "Raise an integrator, not the cap", as the note says.

**Cost.** One slice in `thermal.rs`; no snapshot change; every thermal trajectory moves by
integration error, so the goldens that assert on temperature need their tolerances
re-derived rather than loosened.

**Closed 2026-09-08 by `docs/plans/thermal-implicit-integrator.md`**, which took the
backward-Euler option and refused the "replace the integrator" framing on measurement: the
implicit path is gated to the `dt` where the explicit one runs out of work budget, so the
ordinary path keeps its bits and its performance and **no thermal trajectory moved at
all** — the cost sentence above was wrong about its own price. Two corrections the slice
measured: the cap binding is not the `dt` at which explicit Euler diverges (a factor of
~2.6 separates them, so there is a band above the gate where the old path was merely
inaccurate), and nothing in the tree ever reached either — every aging fast-forward in the
repo is isothermal. What a coarse `dt` still costs is *not* the integrator, and of the two
things it does cost, one is now closed as well. **Runaway ignition lagging a whole step was
closed on 2026-09-08 by `docs/plans/runaway-inside-a-coarse-step.md`**: above the same gate
the ignition test is re-evaluated between sub-steps, stretches with no live reaction go to
the linear integrator, and the reacting sub-step drops a stability bound it no longer needs
— so a live `[safety]` section and a day-long `dt` now belong in the same run. That slice
also **measured** the one still open: heat held constant across the step costs 6.6 K of a
20 K temperature rise on a fresh pack at a day-long `dt`, five orders larger than any
integration error either slice found. **That was closed on the physics side the same day
by `docs/plans/step-mean-heat.md`** — the network now integrates the exact step mean of the
RC overpotentials, the day-long comparison agrees to 1.5 mK with no warm-up, and unlike its
two predecessors this one is *ungated* and therefore reached: it moved ten guided-path
claims and two lessons' prose, which is the first time any of the three changed a number a
reader sees. What it deliberately did not move is `Telemetry::q_gen_w`; that half stays in
H10.

### ~~H7. The BMS can only coulomb-count, so it cannot teach what a real one does~~ — **CLOSED 2026-10-06** (engine and lesson)

**Closed in the engine** (`model-based-estimator.md`, v24): `EstimatorConfig::Ekf`, an
extended Kalman filter over an equivalent-circuit cell built from the chemistry's tables and
read at the probe temperature, predicting over exactly the interval its current was measured
over. Four scenario twins (`na_ion_gauge_filter`, `na_ion_gauge_low_filter`,
`lfp_gauge_filter`, `lfp_gauge_filter_weak_cell`). The measured story is not the one the
approach below predicted: the filter fixes a boot error on sodium-ion in half a minute and
then lands where the counter lands, because the hysteresis fools both the same way; on LFP
with an exact model it corrects, slowly; and on LFP with the cell's resistance 20 % off the
table it is the **worse** estimator — 15 points low under load, 6.7 low after an hour's
rest, with a self-reported sigma of 0.065 points (where it stops is the `[ocv]` table's
0.45 node, held there by the sensor offset; without the offset it creeps on, still six
points low after the hour). **The lesson landed the same day** as guided-path steps 33 and
34 (`path-gauge-filter-steps.md`): the wrong-model filter fifteen points low under load beside
the exact-model one still on its boot error, then still almost seven low after the rest while
the counter beats it. The filter's own uncertainty is stated in words, not shown — it is not on
the wire. **Both results ride on the filter's hand-picked `voltage_sigma_v`** (10 mV, a
placeholder): up to about three times that it only goes wrong later and ends the rest about
as far out, and at ten times it never goes far wrong and beats the counter. The steps say so
in words (`path-gauge-filter-steps.md`, "The headline rides on a hand-picked setting"). Still
open: a control on the page for that setting, and a lesson on choosing it. What follows is
the entry as written.


**Gap.** The estimator is coulomb counting on an imperfect sensor with an OCV correction at
rest (`bms.rs`). That is enough to show drift and hysteresis bias — and the lessons do —
but every production BMS closes the loop with a model-based observer, and the repo has
nothing to show *why* that helps or where it fails (LFP's flat curve makes the observer's
gain small mid-range; hysteresis fools it in a different way than it fools the rest read).

**Approach.** An `Estimator` enum in `bms.rs`: `CoulombCount` (today) and an `Ekf` over a
1-RC ECM the BMS *owns* — a copy of the chemistry's tables, not the engine's cell state,
so principle 8 holds. Its covariance is state (snapshot bump). The lesson is the estimate
converging where coulomb counting drifts, and diverging on the sodium-ion cell's hysteresis
where the rest-OCV gate refused to correct.

**Cost.** One slice; the BMS tests gain a control arm per estimator.

### H8. The pack solve has three open soft spots

* ~~**An `Spm` pack diverges at a long step**~~ — **closed 2026-09-23**
  (`spm-end-of-step.md`): the `Spm`'s curve is now its end-of-step one, and the solve's
  first pass starts from it. What that slice left open is the next bullet.
* ~~**An `Spm` has no physics past empty**~~ — **closed 2026-10-05**
  (`porous-reversal.md`, v22): past the current that takes a particle's surface or bulk to
  empty over the step the particle carries no more, and the rest goes down the chemistry's
  `[reversal]` ramp into a deficit of its own, repaid by a charge and, once the surface has
  room, by the particle's lithium. Particles never hold impossible concentrations (the old
  engine reached −1.26 of a stoichiometry), a rested over-drained cell reads its floor
  (0.00 V, was 1.10), and the energy ledger closes linearly in `dt` (the old engine's was
  ~16 kJ out at any step length).
* **An `Spm` has no physics past FULL within a step — and nor does a `Dfn`.** Once a step would drive a particle's
  surface past `c_max`, the clamp holds it at the edge and `V(i)` goes flat.
  `spm::current_window` states that range in closed form, and since 2026-09-30
  (`spm-pack-window.md`) the `Spm` also declares a range to the pack — that surface range
  intersected with the bulk range the `Dfn` uses, the chemistry's empty and full as edges —
  so a power or voltage demand's current is held to it: the unreachable 10 W hour that ran
  6.8 A and 372 K on its third hour (38 A and 900 K before that) now draws 0 A there, the
  cell empty and the step flagged. A **current** demand that drives a cell out there was
  solved on the flat curve until the reversal above; past full it still is. The `Dfn` has
  the same gap, found 2026-10-06 (`dfn-electrolyte-limit.md`): a charge driven on past the
  4.2 V ceiling puts a particle surface past full 220 s later at 3 C and twenty-four minutes
  later at 1 C, which `SURFACE_OUT_OF_RANGE` now flags.
* ~~**A `Dfn` books the equilibrium voltage's fall across a long step as heat.**~~ —
  **closed 2026-09-23** (`dfn-end-of-step-heat.md`): the heat is read at the end of the
  step off the cell's own solve, for the report and the network both (1.03 K against
  1.06 K, was 3.71 K). The `Spm`'s trapezoid was measured for it and refused: it
  undershoots, and costs a second solve.
* **Voltage holds the model cannot reach inside its range stay unconverged.**
  `voltage-target-blowup.md` counted 11 of 810 in-window solves on a scattered 1S3P SPM
  holding a voltage on its own knee; re-measured 2026-09-23 (`spm-end-of-step.md`, σ = 0.05
  guessed, the original's not recorded) the `Spm` count is 28 of 405 at a 1 s step and 1
  of 405 at an hour, all bounded — they now stop at the edge of the particle's range.
  ~~The `Dfn` fails 269 of 405 at an hour, unbounded~~ — **closed 2026-09-30**
  (`dfn-long-step-holds.md`): 0 of 405 at an hour and at 1 s, hottest 341.8 K, and 0
  cell-steps that fail to conserve lithium across twelve four-hour held runs (124 before).
  Two retries in the cell's own Newton, a range for the cell from conservation with the
  chemistry's empty and full as edges, and a voltage or power demand's pack current held
  to it. Bracketing was declined because the residual is not a scalar monotone one.
* ~~**The pack's damped search can stop where the demand was not met, and call it
  converged**~~ — **closed 2026-09-30** (`spm-pack-window.md`). A damped trial is now
  scored on the step the next pass would take from it, computed from the trial's own probes
  (bit-identical to that pass wherever protection binds at no new bound), so the score is
  zero only at a fixed point — a met demand, or an unreachable power's maximum-power point —
  on every model and every pass, held or not. Once trials of both signs of that step are
  known, the search narrows the bracket (regula falsi, Illinois, bisection fallback) instead
  of halving: the maximum power sits on a steep knee or, on a `Dfn`, on a corner of the
  curve, where halving crawled or never settled. Over the 4860-solve isothermal sweep, no
  answer now changes between pass caps of 31, 32 and 33 (243 `Dfn` and 244 `Spm` did on the
  parent), no solve reaches the cap, and no step reports converged while missing its demand
  (120 `Spm` did). An unreachable power that lands on its maximum says so with
  `SOLVE_UNCONVERGED`, on the porous models. (The previous slice declined bracketing inside
  the `Dfn`'s own Newton, whose residual is a vector; this one brackets the pack's single
  current, whose next step is a scalar.)
* ~~**A `Dfn` driven past empty by a `Demand::Current` does not conserve lithium**~~ —
  **closed 2026-10-05** (`porous-reversal.md`, v23): past the current that takes its bulk to
  the chemistry's empty the solid carries no more, the rest goes down the `[reversal]` ramp
  into a deficit, and the curve keeps falling by the electrodes' own kinetics at empty, which
  depend on no step length (the solve's tangent did: −0.85 V at a minute against −0.50 at ten
  seconds). The 1S3P under a 2 Ω short that reached 1e74 A on its fourth hour is bounded and
  converged; the 1S3P driven through empty at 20 A that reached inf is converged at 350–360 K;
  a rested over-drained cell reads its floor; the ledger closes linearly in `dt`. Bulk edge
  only — the `Dfn` has no closed-form surface edge — and not the next bullet, which is a
  different defect the first spike had mistaken for this one.
* **The `Dfn` has no physics past its rate limit — now diagnosed, and flagged.**
  Re-measured 2026-10-06 (`dfn-electrolyte-limit.md`), which corrects what follows: the
  failure is always at or after the cell's own 2.5 V cut-off (2–5 C from full, half and a
  fifth), so a protected pack, capped at 1.5 C, never reaches it. It is not the
  electrolyte's charge equation going singular — the conductivity floor is inert from 1e-12
  to 1e-6 — but a pinched positive electrode: full particles beside the separator, where
  there is still electrolyte, and room only deeper in, where there is none. The logarithm
  plan below is struck. A solve keeping every particle surface in range was built and
  refused: past the limit it finds no answer either, and the current's lithium leaves the
  cell's books (0.04–0.17 A·h in a minute) where the engine keeps it, at concentrations up
  to twice full. What shipped is `SURFACE_OUT_OF_RANGE`, raised when a step's answer has a
  surface past full or below empty and changing no value; it first fires on the 3 C
  scenario's 464 s cut-off step, which converged 0.22 % past full. Measured
  2026-10-05 (`porous-reversal.md`), identical on the engine before it: from 3 C up the
  electrolyte next to the positive current collector runs out mid-discharge and the Newton
  stops converging there — with 61.6 % still in the cell from full at 3 C (461 s, the 3 C
  golden's own 464 s cut-off), 34.3 % from half, 12.6 % from 20 %, and earlier at 4 and 5 C.
  At 1 C the solve never fails; at 2 C it fails only in the last 1–1.5 %, with the
  electrolyte intact and a particle surface nearly empty. Past the limit a constant current
  has nowhere to go in this model, the positive particle then overfills (1.47 of `c_max` at
  4 C), and the voltage runs off (−21 691 V at 455 s from half). A real cell's voltage
  collapses there and some other reaction carries a forced current; what that reaction is,
  and its parameters, nothing in this repo states — the `[reversal]` section describes the
  anode running out, a different reaction at the other electrode. The first `Dfn` spike
  read these failures as "a front node empty with up to 42 % of the bulk left"; that was its
  metric (an outer shell at 0 or 1) counting the overfilled positive particle, not the cause.
  **Spiked 2026-10-05 and recorded rather than built** (`porous-reversal.md`): a side
  reaction at the positive electrode is the energy-consistent channel, but it cannot switch
  on — the electrolyte's potential equation goes singular as its conductivity vanishes and
  the solve fails first. ~~The next step is solving the near-empty electrolyte (e.g. in its
  logarithm)~~ — struck 2026-10-06: aimed at an equation that was not failing. A channel for
  the current past the limit still needs a reaction, its parameters, and a reference that
  runs past the cut-off, and none of the three is in the repo.
* ~~**The pack's split under a current demand can cycle to its cap**~~ — **closed
  2026-10-05** (`porous-reversal.md`): with the pack current fixed every damped trial
  splits it the same way, so a split that cycles between passes — parallel cells either
  side of an edge, where one curve bends sharply — went to its cap and, on a scattered
  1S3P driven through empty at 3 C, to billions of amps. Where the loop gives up, each
  group's node voltage is now found by a bracketed search on the cells' own curves,
  committed only if every group settles. Not under an external short, and not on `Dfn`
  packs.
* ~~**`Demand::Current` leaving the window is unflagged** where `Power` is~~ — **closed
  2026-08-13** (`operating-point-window.md`): a current demand raises
  `OPERATING_POINT_OUT_OF_WINDOW` too, judged per group. `Rest` is still excluded by
  demand rather than by cause. Struck here on 2026-09-23, when the count above was found
  to include it.

**Approach.** The `Spm` range is the first *valid state window* declared by a cell model.
The unconverged holds and the absurd `Dfn` current want the same kind of window
(concentrations, voltages) with a flag on leaving it, which is the honest form of the
guard `voltage-target-blowup.md` declined: not a magnitude someone picked, but a bound the
model states about itself. **The `Dfn` has one now** (`dfn-long-step-holds.md`): the currents
that keep its bulk between the chemistry's empty and full over the step, declared through
`CellModel::current_window` and held at the pack for a voltage or power demand. The `Spm`'s
surface window is the next to declare the same way; the absurd current is a current demand,
which no such range may hold.

### H9. Performance is at the budget line and the instrument cannot see single digits

**Gap.** `Pack::step` at 100S10P is under the 50 µs budget on the step loop — ~36 µs features
off, 42–46 µs fully featured, since the `R0`-grid change of 2026-10-05 — but the fully featured
figure has no criterion reading yet (`ocv-segment-hint.md`). Since 2026-10-06 each cell
reuses the pack's RC decay instead of recomputing it, and the transcendentals go through
`libm` (H5): 30.1 / 35.8 µs at 5c8bc6a against 35.6 / 41.2 µs before both, best of twelve
alternating rounds on a saturated box (`cross-platform-math.md`).
The DFN and SPM benches for the cell-size change were never run. The box the measurements
were taken on has three performance states and reproducibility "is a property of the
minute" (`cell-size.md`). The DFN re-solves at a current it already probed (a priced 33 %).

**Approach.** In this order: ~~get a profiler before a fifth guessed item (the note's own
instruction)~~ — done 2026-10-05, the Windows Performance Toolkit with `samply`, symbolicated
by `llvm-symbolizer` (`ocv-segment-hint.md`); consume the DFN's converged probe (a slice, threaded through
`CellModel::advance`); write the SPM/DFN bench cases and run them only behind an
interleaved null. Do not touch the reciprocal-multiply item: it is not bit-identical and
was declined for that.

**Since 2026-09-23 the features-off figure was over the line, by more than recorded — and
since 2026-10-05 it is back under** (`ocv-segment-hint.md`). The end-of-step split's cost was
recorded as new/old 1.08, an ungated ratio on a loaded box; a long-loop instrument that
reproduces to ~1 % where criterion could not, run on every engine commit, put it at about
1.27, all of it in that one commit. About half was a second OCV-table search per cell. A
checked per-cell segment hint — bit-identical whatever it holds, so it carries no
invariant — removed both searches' cost. Criterion, registered and gated: **46.9 µs
`current`, 46.7 µs `power`, 53.3 µs `full`**, against 49.8 / 49.7 / 55.7 µs for the engine
before the split. `full` was still over; a profile (Windows Performance Toolkit installed the
same night) then put ~17 % of the step in the `R0` grid's three binary searches per cell, and
sharing its temperature bracket and hinting both axes — bit-identical — took the step loop to
0.73–0.86 of before: `full` 42–46 µs. Criterion's confirmation of that is owed (a loaded box
gave no verdict).

### H10. Smaller physics items, each one slice or less

| item | note | cost |
| --- | --- | --- |
| `[safety]` is one `Option` for two mechanisms; a lithium cell that plates but cannot run away is unrepresentable | `plating-absence.md` | schema change, snapshot bump, no file needs it yet |
| `runaway_power_w_at_onset = 0` means "reported, nothing burns" — the permissive convention plating now departs from | `plating-absence.md` | a validator rule and one doc |
| `[diffusion]`'s charge direction is unvalidated; NiMH has no Peukert fit | `diffusion-overpotential.md` | data (H3) |
| NiMH `[hysteresis]` is one width; lead-acid has no `[hysteresis]` | `phase-8-slice-c-hysteresis.md` | data (H3) — the table exists since v20 |
| Mixed ECM/SPM packs are unrepresentable though the solve is mixed-ready | `phase-6-porous-electrodes.md` | config surface + the `soc_true` question |
| BMS protection overshoot scales with `dt` because the sample rate is `dt` | `phase-2-thermal-bms.md` | accepted; document on the config |
| ~~Heat generation is held constant across a step, so a day-long step burns the day at the heat of its first instant — **measured at 6.6 K of a 20 K rise**~~ — **closed as a ledger 2026-09-23.** The thermal network integrates the exact step mean (`step-mean-heat.md`), and the *reported* pair — `q_gen_w` and `v_terminal` — is now the step's last instant on both sides, which closes the energy ledger step by step with no lag (`end-of-step-split.md`). A step-*mean* pair was the plan and was not built: the measurement for it found the parallel-group divergence instead, and once the split equalises end-of-step voltages the end is the one instant with a single node voltage. What is left is only that `q_gen_w · dt` is not the heat the pack absorbed at a coarse `dt` | `step-mean-heat.md`, `end-of-step-split.md` | a mean pair on `Telemetry` if a client ever needs it; it needs a current-weighted group mean, and at rest with circulating cells that has no voltage to divide by |
| Reading the `Spm`'s network heat at the end of the step instead of by its trapezoid is closer at an hour on three of four cases (0.81 against 0.70 K for a 0.85 K reference at C/5) and worse on a C/2 charge (5.02 against 4.11 K for 4.60 K) | `dfn-end-of-step-heat.md` | a one-line change and a sweep; mixed, so not taken |
| Snapshot body at 100S10P ≈ 600 KB is poor for a socket frame | `phase-4-server-wasm.md` | `Content-Encoding` on REST if it ever bites |

---

## 3. Proposed phases

In the form the earlier phases used: a framing sentence, slices, and an exit criterion that
is a test. None of these is scheduled; each is a proposal to be spiked first, on the
Phase 6/7/8 discipline of measuring before authoring.

### Phase 9 — the phase boundary (H1)

*Framing.* Give the teaching chemistry the porous physics the other one has, without
pretending Fickian diffusion is it.

*Slices.* (A) spike: a two-particle toy with a double-well OCP, does the plateau emerge and
is the solve stable across the spinodal; (B) `CellModel::SpmEnsemble` with `N` particles
per electrode and a seeded radius distribution, validated against the existing `Spm` at
`N = 1` to the bit; (C) the non-monotone OCP for LFP's positive electrode with a cited
source and a regularisation chosen by measurement; (D) an `[spm]` section for LFP via
`tools/reference/`, a scenario, and two guided-path steps.

*Exit.* A CC discharge of the LFP ensemble matches the Prada 2013 DFN reference within a
stated tolerance over the plateau; at rest after a partial charge and a partial discharge
to the same SOC the ensemble's OCVs differ by a measured, cited amount; `N = 1` is
bit-identical to `Spm`. Pinned by tests in `sim-data/tests/`.

### Phase 10 — degradation physics (H2)

*Framing.* Make at least one fade mechanism a consequence rather than a coefficient.

*Slices.* (A) `AgingModel` enum, semi-empirical law moved into it unchanged and bit-identical;
(B) reaction-limited SEI on `Spm`, consuming cyclable lithium and adding film resistance,
with PyBaMM goldens; (C) the same on `Dfn`; (D) a guided-path pair: the same cell aged by
the law and by the mechanism.

*Exit.* Calendar fade under the SEI model is `sqrt(t)`-shaped without a `sqrt` in the
code, matches the PyBaMM SEI reference within tolerance, and grows resistance without a
`r_growth_per_capacity_loss` coefficient. Pinned by `sim-data/tests/sei_golden.rs`.

### Phase 11 — the fitting pipeline (H3)

*Framing.* Retire placeholders by measurement, one file at a time, without touching Rust.

*Slices.* (A) `fit_ecm.py` against the LG M50 DFN goldens; the ECM half of that file
becomes a fit with a residual; (B) entropic coefficients where a set publishes them;
(C) a re-fit of lead-acid's `[diffusion]` against full discharge curves rather than a
capacity table; (D) the claims re-measurement each of those forces.

*Exit.* No constant in `nmc_21700_lgm50.toml` is labelled "placeholder"; the ECM-vs-SPM
pulse lessons quote numbers from a fitted circuit; every fit's residual is in its
provenance line. Pinned by a test that greps the shipped files for the label.

### Not phases: ~~H5 (determinism across platforms)~~ and ~~H6 (the thermal integrator)~~

Each is one slice with a perturbation table and belongs before Phase 9 rather than after
it, because both change what every later golden is allowed to promise. **H6 is done**
(2026-09-08) and in the event it changed no golden at all, because the new integrator is
gated above every `dt` any golden uses. **H5 is done** too (2026-10-06), and its
predicted re-pinning cost did not exist: no test moved.

---

## 4. Structure and process items

* **`CLAUDE.md` drifted from the code three times before this file existed** (the
  `ChemistryRegistry` that never was, the RC growth the spec had and the code did not, the
  OCV temperature correction the spec had and the code did not). The API sketch is now
  corrected; the rule going forward is that a spec-versus-code disagreement is a finding
  to be written up, and the code is presumed right until the note says otherwise.
* **No CI configuration exists**, by decision (`phase-4-server-wasm.md`): the gates are the
  two commands in the README. The first hosting-specific file in the repo should be a
  workflow that runs exactly those, plus a `wasm-pack build` check, and nothing that needs
  a Godot binary.
* **`crates/sim-data/tests/path_claims.rs` is 18 000 lines** and is the single largest file
  in the workspace. It works, its rules are documented at length inside it, and splitting
  it is worth doing only when a rule changes; it is named here so nobody is surprised.
* **The guided path has 34 steps and no argument about how many it should have**
  (`phase-8-chemistries.md`). Phases 9 and 10 each propose two more; Phase 11 proposes
  none and re-measures the existing ones instead. Decide the shape of the path
  before they land: the honest options are a longer single path or a set of short tracks
  per theme (chemistries, models, protection, aging), and the claims harness does not care
  which.
* **The out-of-tree trajectory instrument** (`ANCHORS.md`, not in this repo) is stale by at
  least one slice and has four documented blind spots. H5 was what would let it come in, and
  is closed (2026-10-06): it can come in now, as a native-versus-wasm parity check rather
  than a committed baseline.
* **`docs/plans/` has an index now** (`docs/plans/README.md`). Add a row per note.

---

## 5. Closed since the notes were written

Recorded so the inventory above is not re-derived from stale "Still open" sections:

| item | opened in | closed by |
| --- | --- | --- |
| protection chatters at the top of charge | `energy-hole.md` | `protection-chatter.md` (v12) |
| balancing has the same bandless comparator | `protection-chatter.md` | `balancing-chatter.md` (v13) |
| the low clamp fabricates energy | `energy-hole.md` | `low-clamp-reversal.md` (reversal branch) |
| over-discharge is free | `low-clamp-reversal.md` | `reversal-damage.md` |
| lead-acid rate behaviour is wrong (25.7 points) | `lead-acid-data-only.md` | `diffusion-overpotential.md` (3.3 points) |
| sodium-ion loop width understated below 35 % | `sodium-ion-chemistry.md` | `hysteresis-width-over-soc.md` (v20) |
| DFN aging-vs-resistance-growth unverified | `phase-7-dfn.md` | `dfn-aging-gap.md` |
| LTO plating sentinel / "a gate nobody prices" | `phase-8-slice-a-lto.md` | `plating-absence.md` (v19) |
| surface-vs-bulk stoichiometry not on the wire | `spm-scenario.md` | `surface-vs-bulk.md` |
| no DFN scenario file | `dfn-aging-gap.md` | `dfn-scenario.md` |
| the NiMH peak is a one-timestep corner | `phase-8-slice-c-spike.md` | `charge-acceptance.md` (v21) |
| the step-19 wedge | `surface-vs-bulk.md` | `path-wedge.md` (a renderer crash, not a lesson) |
| no per-cell current accessor | `phase-6-porous-electrodes.md` | `per-cell-current.md` (`CellView::current_a`, no snapshot bump) |
| thermal integration invalid above a 1.7-hour step (H6) | `phase-2-thermal-bms.md` | `thermal-implicit-integrator.md` (backward Euler above the gate, no snapshot bump) |
| the browser's wasm build and the native build part in the last bit on 33 of 54 runs (H5) | `phase-6-porous-electrodes.md` | `cross-platform-math.md` (every transcendental through `libm`; 108/108 by `tools/wasm-parity/`; no snapshot bump, no test moved) |
| ECM parallel groups and voltage holds diverge past a few hundred seconds of `dt` (11 000 K at an hour) | `end-of-step-split.md` (found measuring H10) | `end-of-step-split.md` (end-of-step sources, no snapshot bump) |
| `Spm` parallel groups and voltage holds diverge at long steps (10 000 K by the fourth hour) | `end-of-step-split.md` | `spm-end-of-step.md` (end-of-step curve, curve-read heat, no snapshot bump; past-empty physics open under H8) |
| a `Dfn` books the equilibrium voltage's fall across a long step as heat (3.7 K against 1.06 K at C/5, 55 K against 18 K at 1C) | `spm-end-of-step.md` | `dfn-end-of-step-heat.md` (end-of-step heat off the cell's own solve, no snapshot bump) |
| `Dfn` hour-long voltage and power holds unconverged and unbounded (269 of 405; 1e179 K; 3e146 A by the second hour) | `dfn-end-of-step-heat.md` | `dfn-long-step-holds.md` (Newton retries, a cell range, held pack current; no snapshot bump) |
| the pack search converging where the demand was not met, and landing an unmet power wherever the pass cap fell (120 silent `Spm` misses; 487 cap-dependent solves over the sweep) | `dfn-long-step-holds.md` | `spm-pack-window.md` (an `Spm` pack range, a trial scored on the next pass's step, a sign bracket; no snapshot bump) |
| an `Spm` driven past empty holds impossible lithium on a flat curve, fabricates energy, and blows up in parallel (NaN at 20 A on a scattered 1S3P) | `spm-end-of-step.md` | `porous-reversal.md` (surface-or-bulk reversal with a deficit, per-channel heat, a settle fallback for the split; v22) |
| over-discharge damage is ECM-only | `reversal-damage.md` | `porous-reversal.md` (the `Spm` at v22, the `Dfn` at v23) |
| a `Dfn` driven past empty runs to 1e74 A under a short, inf at 20 A, −1.6 of a stoichiometry at 1 C | `dfn-long-step-holds.md` | `porous-reversal.md` (bulk-edge reversal, kinetics-at-empty continuation; v23; the electrolyte limit stays open under H8) |
| the BMS can only coulomb-count (H7, engine half) | `ROADMAP.md` | `model-based-estimator.md` (`EstimatorConfig::Ekf`, four scenario twins; v24) |
