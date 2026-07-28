# Phase 6 — porous electrodes (`Spm`)

**Status: planned.** This file is written before the work so the decisions below are made
once; the "learned while building" material is appended as each slice lands, the way
`phase-2-thermal-bms.md` through `phase-5-godot.md` grew.

Unlike Phases 4 and 5, Phase 6 **is** a physics phase. It is the first phase since Phase 3
to touch what a pack *is*, and the first ever to add a second cell model. Two consequences
run through everything below: the `SNAPSHOT_VERSION` canary **inverts**, and the pack's
closed-form electrical solve stops being sufficient.

`CLAUDE.md` gives Phase 6 no `Exit:` line — as with Phase 5 it names deliverables and
stops:

> **Phase 6 (future) — porous electrodes.** Add `Spm`/`Dfn` variants to `CellModel`,
> evaluate `diffsol` for the stiff DAE solve, validate against PyBaMM directly. Nothing in
> earlier phases may assume ECM-only internals outside the `CellModel` enum.

So the exit criterion below is authored here, and argued rather than asserted.

| exit criterion (authored here) | met by |
| ------------------------------ | ------ |
| **1. The floor did not move.** Every ECM trajectory the repo already asserts is **bit-identical** before and after the phase — analytic goldens, the LFP PyBaMM goldens, the property tests, the snapshot-replay regression, and the `sim-godot` gate. | slice A carries it and every later slice re-checks it. This is `CLAUDE.md`'s "nothing may assume ECM-only internals" clause made *mechanical*: the accessors change shape, so if a trajectory moves, the refactor changed physics. |
| **2. The door opened.** A pack configured with `CellModel::Spm` runs, and its trajectory matches a committed **PyBaMM SPM golden** within a documented per-scenario tolerance — with the tolerance **built to fail** before it is trusted. | slice E. |
| **3. The new state is snapshotable.** Snapshot at t/2 → restore → continue is bit-identical for an SPM pack, exactly as it already is for an ECM pack. | slice C ships it; slice D re-checks it through the nonlinear solve. **This is the leg `diffsol` failed** — see the spike — and it is why the integrator is owned rather than imported. |

## Slices

| slice | scope | state | version |
| ----- | ----- | ----- | ------- |
| A | **model-agnostic cell interface.** `CellModel::state()`/`state_mut()` stop returning `&EcmState`; `pack.rs`'s eleven direct reaches into ECM internals go through the enum. **Zero physics change.** | planned | v9 — no bump, and a bump here means the refactor changed something |
| B | **`[spm]` chemistry section.** `sim-data` parses and validates half-cell OCPs, particle geometry, transport and kinetics; `BuildError` when a config asks for `Spm` and the chemistry has none. A new honest `nmc_21700_lgm50.toml`. **No engine physics.** | planned | v9 — the section is `Option`al and defaulted |
| C | **the SPM cell.** Backward-Euler finite-volume radial diffusion, Butler–Volmer kinetics, `CellModel::Spm`. Single cell only — the pack solve still sees a linear source. **Carries the one bump.** | planned | **v9 → v10** |
| D | **the nonlinear pack solve.** Iterate the existing linear group solve on tangent Thévenins; closed form remains exactly the closed form when every cell is linear. | planned | v10 — no further bump |
| E | **PyBaMM validation + wrap-up.** SPM goldens, tolerance built to fail, SPM's own perf budget measured, README. **Carries exit criteria 2.** | planned | v10 — no further bump |

Each slice keeps `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` clean.

---

## The spike, and what it settled

A throwaway crate under `M:\claud_projects\temp\phase6-spike` — outside the repo tree, as
Phase 5's was — was built and run **before this document was written**, because the two
decisions that shape the whole phase turn on facts that cannot be reasoned out.
Everything in this section is measured on this box, not inferred.

| question | answer |
| -------- | ------ |
| Can `diffsol`'s solver state be extracted, serialized and restored **bit-identically** through its public API? | **No.** See below. This is decisive. |
| Is `diffsol` deterministic in place (same problem, same build, twice)? | **Yes** — the uninterrupted run is bit-identical to itself. The failure is specific to restore. |
| What does `diffsol` cost as a dependency? | **137 crates, 40.8 s cold build.** `sim-core`'s dependency list is `serde`, `rand_chacha`, `bitflags`, `thiserror`. |
| Does a fixed-step backward-Euler SPM converge at coarse `dt`? | **Yes, unconditionally.** 1 A for 1 h at `dt` = 1 s / 60 s / 3600 s gives `x_neg` = 0.738403 in all three; terminal voltage differs by 8e-4 V between the extremes. |
| What does one SPM cell-step cost? | **0.0966 / 0.2151 / 0.4739 µs** at N = 5 / 10 / 20 shells (two particles, two tridiagonal solves). |
| Does the nonlinear current solve converge, and in how many iterations? | **3, mean and worst**, over a 600-step CV hold at a 1e-9 V tolerance, safeguarded Newton with bisection fallback. |

### `diffsol` fails principle 5, and this is the measurement

```text
uninterrupted, twice          : identical            <- diffsol is deterministic in place
straight through  y0 = 1.35470759667195281e-4  (0x3f21c1a4f6f498eb)
split at t = 1.0  y0 = 1.35470753691242259e-4  (0x3f21c1a4e9d06f53)
relative difference 4.4e-8                            <- restored run is NOT bit-identical
```

The cause is structural, not a usage error. `BdfState`'s `order` and `diff` — the
multistep difference matrix, which *is* the integrator's history — are `pub(crate)`, and
the `Bdf` solver privately holds `n_equal_steps`, `jacobian_update` and
`prev_error_norm`, all of which decide the next step. The public `StateRef` exposes only
`y`, `dy`, `t`, `h`, so the most faithful restore the API permits still discards the
history. `OdeSolverMethod::checkpoint()` returns an opaque, non-serializable `Self::State`
and its own doc comment says it "will force a reinitialisation of the internal Jacobian".

`CLAUDE.md` asks Phase 6 to **evaluate** `diffsol`, and this is the evaluation: it is a
good library that cannot satisfy principle 5 through its public surface, because an
adaptive multistep integrator's state is larger than its solution vector and this one does
not expose the remainder. A fixed-step method whose entire state *is* the concentration
vector passes trivially — which is the second half of the spike, and it works.

Two secondary findings would have argued the same way even had the first passed. The
dependency: 137 crates into `sim-core`, whose dependency list `CLAUDE.md` constrains
explicitly and whose purity rule is the repo's first principle. And the fact that the
stiffness `diffsol` exists to handle is a **DFN** problem — SPM's radial diffusion is a
linear tridiagonal system with a known sparsity pattern, and calling a general-purpose
implicit ODE suite to solve it would be paying for machinery the model does not use.

**The answer is therefore "declined for SPM, and here is what it would cost for DFN":** if
Phase 7 ever ships `Dfn`, `diffsol` becomes a genuine candidate for the electrolyte DAE —
and the same bit-identity question has to be answered first, most plausibly by upstreaming
serde support for `BdfState` or by pinning the solver to fixed steps. `Dfn` is out of
scope here (see below), so that question is deferred rather than answered.

### The fixed-step SPM works, and these are its numbers

Radial diffusion by **backward Euler on a finite-volume grid** — one tridiagonal Thomas
solve per particle per step. Unconditionally stable, so a coarse aging fast-forward needs
no sub-stepping, unlike the thermal network. Explicit Euler would be stable only to
`dt ≈ dr²/(2·D)`, which for LG M50 parameters is ~12 s at N = 10 and ~3 s at N = 20 —
adequate for real time and useless for fast-forward, which is exactly the trap
`CLAUDE.md`'s "same code path serves real-time stepping and months-long aging" line warns
about.

```text
SPM step, N =  5 shells      0.0966 us/step
SPM step, N = 10 shells      0.2151 us/step
SPM step, N = 20 shells      0.4739 us/step

CV hold at 3.9 V, 600 steps of dt = 1 s:
  newton iterations: mean 3.00, worst 3

1 A for 1 h at dt =    1 s  ->  x_neg = 0.738403, V = 3.720798
1 A for 1 h at dt =   60 s  ->  x_neg = 0.738403, V = 3.720799
1 A for 1 h at dt = 3600 s  ->  x_neg = 0.738403, V = 3.721571
```

The spike's own ECM stand-in is **not** quoted as a ratio here, because it is a stripped
arithmetic sketch rather than this repo's `Pack::step`, and a ratio against it would
flatter the SPM. The honest comparison is in "SPM gets its own budget" below.

---

## Decisions already made (do not re-derive)

### The `SNAPSHOT_VERSION` canary inverts — and the version check finally does its job

Phases 4 and 5 ran on the rule *a bump means an adapter has leaked into the engine*, and
it held across ten consecutive slices. **That rule does not apply to Phase 6.** This phase
adds serialized per-cell state, so **exactly one bump is expected**: v9 → v10, in slice C,
the slice that adds the variant. The canary is not deleted, it is re-pointed:

- Slices A, B, D and E must **not** bump. A bump in A means the "pure refactor" changed
  the layout. A bump in B means an optional chemistry section was not optional. A bump in
  D means the solver put state on the pack that belongs in a scratch buffer.
- Slice C must bump **exactly once**, with a note in `SNAPSHOT_VERSION`'s doc comment in
  the established house style.

There is a wrinkle here worth pinning with a test, because it is a **first for this repo**.
Every bump from v2 to v9 carries the same caveat, stated at v3 and repeated five times:

> Note what actually rejects a v2 snapshot here: the layout change, at deserialization,
> exactly as the v2 note above describes. **The version check never sees those bytes.**

That stops being true at v10. Adding a variant to an externally-tagged enum does not
change how the *existing* variants serialize, and `[spm]` is an `Option`al chemistry
section, so a v9 blob remains structurally deserializable — and `Pack::restore`'s
`snapshot.version != SNAPSHOT_VERSION` check is then the *only* thing that rejects it.
This is the first bump whose version field does the job it was written for, and therefore
the first one that **can** be pinned by a test. Slice C ships that test.

### `CellModel::state() -> &EcmState` is a live violation, and it is slice A

`CLAUDE.md` states, in the Phase 6 line itself:

> Nothing in earlier phases may assume ECM-only internals outside the `CellModel` enum.

That is false today, and it is false in `sim-core` rather than in an adapter. Both
accessors are `pub` and both are ECM-typed:

```rust
pub fn state(&self) -> &EcmState
pub fn state_mut(&mut self) -> &mut EcmState
```

`pack.rs` reaches straight through them: `cell_source(cell.model.state(), …)` at 993, 1000
and 1378; `state.v_rc` / `state.temp_k` / `state.soc` fed to `cell_heat_w` and
`docv_dt_lookup` at 1112–1118; `soc_before` / `temp_before` at 1134–1135;
`advance_cell(cell.model.state_mut(), …)` at 1149; `cell.model.state().temp_k` at 1187;
and `EcmState { … }` constructed directly at 532–541. No non-ECM variant can satisfy that
signature, so the enum is a slot in name only.

The good news, verified rather than assumed: **no adapter crate reaches in.**
`grep` over `sim-server`, `sim-wasm`, `sim-godot` and `sim-data` for `.state()`,
`EcmState` and `v_rc` returns nothing. `CellView` — the public per-cell ground-truth
view — is the boundary, and it holds a `v_rc_sum` scalar rather than ECM state. So the
violation is confined to one file plus two `pub` signatures, which is what makes slice A
tractable.

Slice A's gate is **not** "the tests pass". It is Phase 5 slice A's stronger claim, about
the diff and about the trajectories: no existing test is *modified*, and every golden is
bit-identical. A behaviour-preserving refactor that quietly moved a trajectory would be
green under a tolerance and caught by a bit comparison.

### `cell_heat_w`'s signature is ECM-shaped; the physics underneath is not

```rust
pub fn cell_heat_w(i: f64, r0: f64, v_rc_sum: f64, temp_k: f64, docv_dt_v_per_k: f64) -> f64
```

The doc comment already derives the irreversible term as `I·(OCV − V_terminal)` and notes
that the `I²·(R0 + Σ R_rc)` form in `CLAUDE.md`'s sketch is only the steady-state special
case. `I·(OCV − V)` generalizes to any model; `r0` and `v_rc_sum` do not. Slice A moves
the signature to `(i, ocv, v_terminal, temp_k, docv_dt)` or folds heat into the per-model
step — and the choice is constrained by exactly one thing: the pack energy balance must
keep closing **exactly**, which is what the existing property test asserts and what the
`SourceCache` memo depends on.

### `soc_true` means one thing, and for SPM that thing is lithium inventory

`Telemetry::soc_true` cannot fork. Its meaning stays the one v6 already fixed — *fraction
of the capacity this cell has today* — but the two models compute it differently:

- **ECM:** coulomb-counted. `soc` is a state variable, and `∫I·dt = ΔSOC·capacity` is
  **definitional** — it is arithmetic, not physics.
- **SPM:** derived from the mean negative-particle stoichiometry, mapped onto the
  chemistry's stoichiometry limits. It is a *readout* of the lithium inventory, not a
  counter.

The consequence must be written into the tests rather than discovered by them. Of the four
property-test families in `CLAUDE.md`'s testing strategy:

| property | ECM | SPM |
| -------- | --- | --- |
| SOC stays in [0, 1] | holds | holds — by the stoichiometry limits, not by a clamp |
| charge conservation `∫I·dt = ΔSOC·capacity` | **exact** (definitional) | **approximate** — surface vs. bulk concentration diverge under load; needs a stated tolerance and a rest-to-equilibrium in the assertion |
| `V ≤ OCV` discharging, `≥ OCV` charging | holds | holds — but "OCV" must mean the *equilibrium* voltage at mean stoichiometry, not at surface |
| parallel-group currents sum to group current | holds | holds — it is Kirchhoff, and slice D must not break it |
| pack energy balance | holds | holds, and is the sharpest SPM check available: it catches a wrong overpotential that a voltage-RMS tolerance would absorb |
| snapshot round-trip equality | holds | **must** hold — exit criterion 3 |

### The pack solve stops being closed-form, and the repo already wrote the check

`ecm::solve_current`'s doc comment says it, unprompted, in Phase 1:

> so Phase 1 deliberately does **not** use the Newton/bisection loop that `CLAUDE.md`
> prescribes (that is forward-cover for models that are nonlinear within a step, e.g.
> SPM/DFN, which Phase 1 does not have).

Phase 6 cashes that. The design, and the reason it is this design rather than a rewrite:

**Iterate the existing linear solve on tangent Thévenins.** Each cell reports, for the
current iterate, a linear source `(E_k, R_k)` tangent to its own `V_k(i)` at that
operating point. The group and series aggregation, the external-short transform, the
bleed conductances and the protection clamp are then **the code that already exists**,
unchanged, running on tangent sources. The outer loop is a Newton iteration over that
linearization.

Two properties make this the right shape rather than merely a convenient one:

- **When every cell is linear the tangent is exact**, so the first iteration *is* today's
  closed form and the loop exits on its first residual check. Structured that way, the ECM
  path is bit-identical by construction rather than by tolerance — which is what makes
  exit criterion 1 survivable through slice D, and it is the reason not to reach for a
  general nonlinear system solve over all cell currents at once.
- **It does not nest.** The naive formulation (outer solve on pack current, inner solve
  per cell for `V_k(i) = V_node`) is a nested iteration whose cost is the product of two
  loop counts times the cell count. Tangent linearization keeps one loop.

Guards, per `CLAUDE.md`: iteration cap, bisection fallback, and the convergence tolerance
lives in **config** so it is snapshotted and cannot silently differ between two runs of
the same scenario. The spike measured 3 Newton iterations mean *and* worst at 1e-9 V on a
CV hold, which is the encouraging half; the discouraging half is that a spike solves one
cell and slice D solves a pack, so the iteration count is a **slice D measurement**, not
an inherited fact.

### The chemistry TOML cannot describe an SPM, and this is the item most likely to be found late

`[ocv]` is a *full-cell* curve. An SPM needs, at minimum: half-cell OCPs `U_n(x)` and
`U_p(y)` over stoichiometry, particle radii, solid diffusivities, maximum concentrations,
reaction rate constants, interfacial areas (or the electrode geometry and volume fractions
that give them), electrolyte concentration, stoichiometry limits at both ends of charge,
and a lumped contact resistance. None of it exists in the schema.

So slice B adds an **optional `[spm]` section**, validated in `sim-data` (monotone OCPs,
positive geometry and transport, stoichiometry limits ordered and within [0, 1]), and
`Pack::new` returns a `BuildError` when a config selects `CellModel::Spm` and the
chemistry has no `[spm]`. Optional is what keeps slice B off the version counter.

The provenance rule applies with full force and is *easier* to satisfy here than for the
ECM tables: SPM parameters are **extracted** from a PyBaMM parameter set rather than
fitted to its output, so every number has a literal citation.

### The SPM chemistry is NMC, and that resolves the NMC identity debt honestly

Two facts decide this together.

**SPM is a solid-solution model, and LFP is a two-phase material.** Lithium iron phosphate
intercalates through a moving phase boundary, which is precisely what produces the flat
plateau this repo already exploits pedagogically (the LFP estimator-drift scenario). A
single-particle model with Fickian diffusion is physically the wrong model there — it can
be made to *fit*, but shipping it as "porous-electrode physics for LFP" would be the kind
of unlabeled claim `CLAUDE.md`'s provenance rule exists to prevent. NMC/graphite is the
chemistry SPM was built for and the one PyBaMM's canonical SPM examples use.

**Extraction forces the identity question that fitting let us defer.** The blocked call
recorded in memory is that `chemistries/nmc_18650_generic.toml` claims an 18650 / 3.0 Ah
identity while `tools/reference/common.py` maps it to `Chen2020`, which is the LG M50
**21700 / 5 Ah** cell. For an ECM fit that mismatch is a capacity rescale one could argue
about. For SPM it is not arguable: particle radii, electrode thicknesses and interfacial
areas *are* the cell's physical identity, and there is no honest way to put Chen2020's
geometry in a file labelled 18650.

So slice B ships a **new** `chemistries/nmc_21700_lgm50.toml`, honest from the first line,
carrying both an ECM section and an `[spm]` section extracted from Chen2020. The existing
`nmc_18650_generic.toml` is **not** touched: its provenance line already says
`"OCV/R0 shape hand-fit to typical NMC 18650 datasheets; … order-of-magnitude
placeholders (TODO: fit against PyBaMM Chen2020/OKane2022)"`, which is honest, and it is
referenced only by `sim-data`'s own load/scenario tests, not by any shipped scenario. The
long-standing debt is discharged by *adding a truthful file* rather than by renaming one
and breaking two tests. **Open question below** asks the owner whether the TODO on the old
file should now be struck or redirected.

LFP keeps its ECM path and its existing goldens, untouched, and gets no `[spm]` section.
That is a scope decision with a physics reason, and it goes in the README rather than
being silently absent.

### SPM gets its own budget, and the ECM budget is not it

`CLAUDE.md`'s `< 50 µs per step at 100S10P` is an **ECM** budget. The honest arithmetic:

- The whole ECM step at 100S10P sits at ≈ 42–54 µs for 1000 cells, i.e. **≈ 0.045 µs per
  cell per step**, against a `< 50 µs` budget that `docs/plans/pack-step-perf.md` already
  calls *marginal*.
- One SPM cell-step measured **0.215 µs** at N = 10 shells, before the nonlinear solve's
  repeated voltage evaluations.

So an SPM cell costs roughly **5×** the engine's entire current per-cell step, and a
100S10P SPM pack would land in the hundreds of µs. That is not a regression to fix; it is
what a physics-based model costs, and pretending otherwise by quietly widening the ECM
budget would be the dishonest move. Slice E therefore states a **separate SPM budget at a
stated topology and shell count**, and the ECM budget is re-verified unchanged.

Two hazards to check rather than assume, both peculiar to this phase:

- **`SourceCache`'s debug assertion.** `pack.rs` re-evaluates `cell_source` for **every
  cell, every step, in debug builds** to prove the memo is not stale. Behind an SPM source
  that is an expensive recompute on every test in the suite. Slice C measures whether the
  debug test run stays usable and, if not, decides deliberately (sample the check, or gate
  it) rather than discovering it as a mysteriously slow `cargo test`.
- **`N` is a cost knob with a physics meaning.** Shell count is config, it is part of the
  snapshot layout (the concentration vector's length), and it trades accuracy against
  cost linearly. Slice E should report the accuracy-vs-cost curve rather than picking a
  default silently.

### Scope: `Spm` in, `Dfn` out but demonstrably addable

`CLAUDE.md` names both variants. Shipping both in one phase is not the right call, and the
reason is specific rather than schedule-shaped: **`Dfn` is the variant that actually needs
the stiff DAE solve.** It adds electrolyte diffusion and migration across three domains
with an algebraic potential constraint — a genuine DAE, the thing `diffsol` exists for,
and the thing the spike just showed cannot satisfy principle 5 without upstream work.

`Spm` plus slice A's accessor work satisfies "keep the door open" **demonstrably** rather
than aspirationally: after this phase, adding `Dfn` is adding a variant, because slice A
removed the ECM-typed accessors that would have blocked it and slice D removed the
closed-form assumption that would have broken it. That claim is the phase's real
deliverable, and it is checkable by reading the diff of slices A and D.

---

## Slice detail

### A — the model-agnostic cell interface

Touches `sim-core` only. **Zero physics change**, and the gate is built around that claim.

- Replace `CellModel::state()/state_mut()` with a model-agnostic surface. The shape to
  aim for, refined during the slice: the quantities `pack.rs` actually needs are
  `soc`, `temp_k` (read *and* write — thermal owns it), the Thévenin/tangent source, the
  terminal-voltage-at-current, the heat term, and the state advance. Everything else is
  ECM-private.
- `cell_heat_w`'s signature generalizes (see above).
- `EcmState` construction at `pack.rs:532–541` moves behind a constructor on the enum.
- Both accessors stop being `pub`, or stop being ECM-typed. A `#[doc(hidden)]` escape
  hatch is not acceptable — that is the violation with a fig leaf.
- **Gate, in this order:**
  1. `git diff --name-only | grep -i test` → nothing modified (added is fine).
  2. Every golden and every scenario trajectory **bit-identical**, compared on
     `f64::to_bits`, not on a tolerance. A tolerance-based comparison cannot distinguish
     "refactor preserved behaviour" from "refactor moved it slightly".
  3. `SNAPSHOT_VERSION` still 9.
  4. `cargo test --workspace` and clippy clean, including the `sim-godot` gate under
     `--ignored` (it asserts bit-identity against a Godot process and is the one test that
     would catch a change the in-process goldens somehow shared).

### B — the `[spm]` chemistry section

Touches `sim-data` and `sim-core`'s `ChemistryParams`; **no engine physics**.

- `[spm]` schema and validation: half-cell OCP tables monotone in stoichiometry, particle
  radii / diffusivities / rate constants / concentrations positive, stoichiometry limits
  ordered and inside [0, 1], shell count within a sane range.
- `BuildError` when a `PackConfig` selects `Spm` and the chemistry has no `[spm]`. Tested,
  and tested for the *message* naming both the chemistry and the missing section — a
  `BuildError` that says only "invalid config" costs whoever hits it an hour.
- `chemistries/nmc_21700_lgm50.toml`, extracted from Chen2020, every number with a literal
  provenance citation.
- `tools/reference/` gains an extraction script (`extract_spm.py`, alongside the existing
  `fit_ocv.py`) so the provenance line is literally true and the file is regenerable.
  `PARAM_SETS` gains the new id.
- The section is `Option`al and defaulted, so **v9 does not move**. If it does, it was not
  optional.

### C — the SPM cell (carries the one bump)

Touches `sim-core`. Single cell only; the pack still solves a linear source, because
turning the pack solve nonlinear at the same time as introducing the model would leave two
suspects for any discrepancy.

- `spm.rs`: `Particle` (shell concentrations), backward-Euler finite-volume diffusion by
  Thomas algorithm, surface-concentration extrapolation, Butler–Volmer overpotential,
  half-cell OCP lookup, terminal voltage, mean-stoichiometry SOC readout.
- `CellModel::Spm(SpmState)`, satisfying slice A's interface. During slice C the pack
  drives it through the **tangent at the previous current**, which is exact enough for a
  single cell and is what slice D generalizes.
- **`SNAPSHOT_VERSION` 9 → 10**, with the doc-comment note in house style.
- **The version-check test** described above: construct a v9-shaped snapshot that is
  structurally valid under v10 and assert `restore` rejects it with
  `RestoreError::VersionMismatch`. Every prior bump's doc note says no test could pin
  this; at v10 one can, and the note should say so.
- Analytic goldens for the cell in isolation, the way Phase 0 did for the ECM: at a
  constant flux the shell system has a known long-time behaviour, and the spike's
  `dt`-independence result (identical `x_neg` at `dt` = 1 s and 3600 s) is a test, not an
  observation.
- Snapshot round-trip on an SPM cell — exit criterion 3, first half.
- The `SourceCache` debug-assert cost check.

### D — the nonlinear pack solve

Touches `sim-core`'s `pack.rs`.

- Tangent-Thévenin iteration as designed above: cap, bisection fallback, tolerance in
  config (and therefore in the snapshot).
- **The fast path must be bit-identical, by construction and by test.** Every ECM golden
  re-run and compared on bits; the residual check must exit on the first iteration for an
  all-linear pack, and there should be a test asserting the *iteration count* is 1 there,
  not merely that the answer matches — an answer that matches after three iterations means
  the fast path is gone and only the tolerance is hiding it.
- Mixed packs: an SPM cell in parallel with an ECM cell is a legal configuration and a
  genuinely interesting pedagogical one. Decide explicitly whether it is supported; if it
  is, it is the sharpest test of the tangent formulation, and if it is not, it is a
  `BuildError` rather than silent nonsense.
- Kirchhoff property test re-run against SPM groups: currents still sum.
- Snapshot round-trip through the nonlinear solve — exit criterion 3, second half.
- Iteration count **measured**, on a pack, across a schedule that includes CV legs and
  current reversals.

### E — PyBaMM validation and wrap-up (carries exit criterion 2)

- `tools/reference/generate.py` gains SPM scenarios for the new chemistry. Note the
  comparison is now **SPM vs SPM** — batsim's SPM against PyBaMM's SPM on the same
  parameter set — which is a far tighter test than the existing ECM-vs-DFN goldens and
  should carry a correspondingly tighter tolerance. A **second** comparison against
  PyBaMM's DFN on the same parameters is worth generating too, because the SPM-vs-DFN gap
  is a *physics* result worth showing pedagogically rather than a test failure.
- Committed CSVs under `tests/golden/nmc_21700_lgm50/`, Rust integration tests with
  per-scenario documented tolerances.
- **The tolerance is built to fail before it is trusted.** Perturb the model deliberately
  — a wrong diffusivity, a dropped overpotential term — and confirm the tolerance rejects
  it. Phase 4 and Phase 5 both did this and both found something; a tolerance nobody has
  seen reject anything is a number, not a test.
- SPM's own perf budget, measured with `docs/plans/pack-step-perf.md`'s discipline
  (ratios against a same-session baseline, alternating arm order, warm template, never
  after a build storm), plus the accuracy-vs-`N` curve.
- The ECM budget re-verified unchanged, which is the perf half of exit criterion 1.
- README: status row, the new chemistry, the SPM/LFP scope note, and the `diffsol`
  evaluation result in one sentence with a pointer here.
- Python environment: `uv` is on PATH, PyBaMM is **not** currently installed and there is
  no `.venv` in the tree. `tools/reference/README.md`'s documented
  `uv venv --python 3.11 .venv` route is the path; the CSVs are committed, so this cost is
  paid once by whoever regenerates them and never by the Rust test run.

---

## Open questions

- **Does `nmc_18650_generic.toml`'s TODO get struck or redirected?** Its provenance line
  promises a future fit "against PyBaMM Chen2020/OKane2022". Slice B's argument is that
  Chen2020 *cannot* honestly fit an 18650, which makes half that TODO unachievable as
  written. Options: redirect it to an 18650-class NMC parameter set (e.g. an NMC532/622
  parameterization), strike it and label the file permanently a hand-fit teaching
  placeholder, or retire the file once a real NMC exists. **Owner's call**; nothing in
  Phase 6 is blocked either way, because slice B adds a file rather than editing this one.
- **Is a mixed ECM/SPM pack supported?** Slice D decides. It is the sharpest available
  test of the tangent formulation and a genuinely interesting teaching scenario ("what
  does the cheap model get wrong?"), but it also doubles the configuration surface and
  raises a real question about what `Telemetry::soc_true` means when averaged across two
  models that compute it differently.
- **What is the default shell count `N`?** Slice E's accuracy-vs-cost curve should decide
  it, not taste. The spike ran N = 10 at 0.215 µs; N = 5 is 2.2× cheaper and N = 20 is
  2.2× dearer, so the curve is close to linear and the decision is about accuracy alone.
- **Does the aging model apply unchanged to SPM?** Calendar and cycle fade are written
  against `soh_capacity` / `soh_resistance` multipliers on an ECM. On an SPM, capacity
  fade physically *is* lithium inventory loss, which the model represents directly. Slice
  C should decide whether aging keeps acting as a multiplier (simple, consistent, slightly
  dishonest) or reduces the inventory (physical, and a bigger change than this phase
  wants). Recommend the former for Phase 6, documented as a deliberate simplification.
