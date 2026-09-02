# Roadmap — the scientific hurdles, and the phases after 8

Phases 0–8 are complete and each is pinned by a committed test (see the README's status
table). Ninety-seven design notes under `docs/plans/` record what each slice measured,
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
growth, a fault queue, emergent plating and runaway, snapshots at `SNAPSHOT_VERSION` 21,
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

### H5. Cross-platform determinism is not promised, and the browser and the server may already disagree

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

### H6. Thermal integration stops being valid above a 1.7-hour step, which the aging fast-forward exceeds

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

### H7. The BMS can only coulomb-count, so it cannot teach what a real one does

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

### H8. The pack solve has three named soft spots

* **11 of 810 in-window solves stay unconverged** on a scattered 1S3P SPM holding a
  voltage on its own knee (`voltage-target-blowup.md`); bracketing was declined because the
  residual is not a scalar monotone one.
* **A `Dfn` driven by an absurd `Demand::Current` is unrecoverable** (−1105 V forever);
  the only guard is a magnitude, i.e. an invented constant.
* **`Demand::Current` leaving the window is unflagged** where `Power` is
  (`power-operating-point.md`); the window flag is one bit for the whole pack; `Rest` is
  excluded by demand rather than by cause (`operating-point-window.md`).

**Approach.** Take the third first — it is a predicate change with a measured blast
radius. The first two want a per-model *valid state window* declared by the cell model
(concentrations, voltages) with a flag on leaving it, which is the honest form of the
guard `voltage-target-blowup.md` declined: not a magnitude someone picked, but a bound the
model states about itself.

### H9. Performance is at the budget line and the instrument cannot see single digits

**Gap.** `Pack::step` at 100S10P measured 47.2 µs against a 50 µs budget, features off; the
fully-featured figure is unmeasured and estimated at or over the line (`pack-step-perf.md`).
The DFN and SPM benches for the cell-size change were never run. The box the measurements
were taken on has three performance states and reproducibility "is a property of the
minute" (`cell-size.md`). The DFN re-solves at a current it already probed (a priced 33 %).

**Approach.** In this order: get a profiler before a fifth guessed item (the note's own
instruction); consume the DFN's converged probe (a slice, threaded through
`CellModel::advance`); write the SPM/DFN bench cases and run them only behind an
interleaved null. Do not touch the reciprocal-multiply item: it is not bit-identical and
was declined for that.

### H10. Smaller physics items, each one slice or less

| item | note | cost |
| --- | --- | --- |
| `[safety]` is one `Option` for two mechanisms; a lithium cell that plates but cannot run away is unrepresentable | `plating-absence.md` | schema change, snapshot bump, no file needs it yet |
| `runaway_power_w_at_onset = 0` means "reported, nothing burns" — the permissive convention plating now departs from | `plating-absence.md` | a validator rule and one doc |
| Over-discharge damage is ECM-only; porous models carry no deficit | `reversal-damage.md` | needs a porous-model reversal path, which is H1/H2 territory |
| `[diffusion]`'s charge direction is unvalidated; NiMH has no Peukert fit | `diffusion-overpotential.md` | data (H3) |
| NiMH `[hysteresis]` is one width; lead-acid has no `[hysteresis]` | `phase-8-slice-c-hysteresis.md` | data (H3) — the table exists since v20 |
| Mixed ECM/SPM packs are unrepresentable though the solve is mixed-ready | `phase-6-porous-electrodes.md` | config surface + the `soc_true` question |
| BMS protection overshoot scales with `dt` because the sample rate is `dt` | `phase-2-thermal-bms.md` | accepted; document on the config |
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

### Not phases: H5 (determinism across platforms) and H6 (the thermal integrator)

Each is one slice with a perturbation table and belongs before Phase 9 rather than after
it, because both change what every later golden is allowed to promise.

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
* **The guided path has 32 steps and no argument about how many it should have**
  (`phase-8-chemistries.md`). Phases 9–11 each propose two. Decide the shape of the path
  before they land: the honest options are a longer single path or a set of short tracks
  per theme (chemistries, models, protection, aging), and the claims harness does not care
  which.
* **The out-of-tree trajectory instrument** (`ANCHORS.md`, not in this repo) is stale by at
  least one slice and has four documented blind spots. H5 is what would let it come in.
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
