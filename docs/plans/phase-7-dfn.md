# Phase 7 — the electrolyte (`Dfn`)

**Status: slices A, B and C have landed; D remains. The repo is at `SNAPSHOT_VERSION`
11.** Everything above the slice notes was written *before* the work, after a spike, so
the decisions that shape the phase are made once and made from measurements; the
"learned while building" material is appended as each slice lands, the way
`phase-2-thermal-bms.md` through `phase-6-porous-electrodes.md` grew. Where a slice note
contradicts the pre-work text, **the slice note is the measurement and wins** — the
projected `~30×` cost below is the live example, and it is labelled where it appears.

## Framing: this phase completes a bullet rather than opening one

`CLAUDE.md` has no Phase 7. `Dfn` sits inside the Phase 6 bullet:

> **Phase 6 (future) — porous electrodes.** Add `Spm`/`Dfn` variants to `CellModel`,
> evaluate `diffsol` for the stiff DAE solve, validate against PyBaMM directly. Nothing
> in earlier phases may assume ECM-only internals outside the `CellModel` enum.

Phase 6 shipped the `Spm` half, declined `diffsol` on measurement, and left `Dfn` — the
variant the bullet's "stiff DAE" clause was actually about. So Phase 7 is the second
half of one bullet, and its exit criteria are **authored here and argued**, exactly as
Phase 6's were.

One sentence on what the phase buys, because it is the thing a reader will want first.
An `Spm` holds the electrolyte at a constant concentration; that *is* the
single-particle approximation. A `Dfn` solves for it. Everything below follows from how
badly that assumption fails, and the spike measured it.

| exit criterion (authored here) | carried by |
| ------------------------------ | ---------- |
| **1. The floor did not move ~~bit-identically~~ — amended by slice A, see below.** Every ECM trajectory is bit-identical before and after the phase. Every SPM trajectory is bit-identical **except** where the old OCP tables were clamping, which slice A measured and corrected: the last ~1.4 % of the shipped 1C golden. The criterion as first authored said "bit-identical" on the premise that the extension was provably inert for the SPM; **that premise was false**, and the amendment is a measured, argued exception rather than a widened tolerance. | slice A, re-checked by every later slice |
| **2. The door opened.** A pack configured with `CellModel::Dfn` runs, and its trajectory matches a committed **PyBaMM DFN golden** within a documented per-scenario tolerance, with the tolerance **built to fail** before it is trusted. At least one scenario must reach electrolyte depletion — the regime an SPM cannot represent at all, and therefore the only one that proves the phase did something. | slice D |
| **3. The new state is snapshotable.** Snapshot at t/2 → restore → continue is bit-identical for a DFN pack. **This includes the Newton warm-start vector**, which is state and not a cache — see slice B. | slice B, re-checked by slice C |

## Slices

| slice | scope | version |
| ----- | ----- | ------- |
| A | **`[dfn]` chemistry section** — schema, validation, extraction script, LG M50 gains one. **Plus the OCP table extension the spike found is required**, which is the part that can move an SPM trajectory and so carries exit criterion 1. No engine physics. **Landed; it did move one, and criterion 1 is amended — see the slice A note.** | v10 (no bump) |
| B | **the DFN cell** — grid, state, the coupled Newton with an *analytic* banded Jacobian, `CellModel::Dfn`, `CellModelConfig::Dfn`, both `BuildError`s, the version-check test. Single cell; the pack still sees a linear source. **Carries the one bump. Landed; see the slice B note.** | **v10 → v11** |
| C | **pack integration** — the tangent, which for a DFN cannot be the central difference `spm.rs` uses. Sensitivity solve off the already-factorised Jacobian. **Landed; see the slice C note.** | v11 (no bump) |
| D | **PyBaMM DFN goldens, tolerance built to fail, DFN's own perf budget, README.** Carries exit criterion 2. | v11 (no bump) |

Each slice keeps `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` clean.

**One `SNAPSHOT_VERSION` bump for the phase, on slice B**, the slice that adds the
variant — the same budget Phase 6 set and held (C2 spent it; D and E did not).

Adapter API versions: **check each constant's own doc before paying a bump.** Phase 6
planned to move `API_VERSION` and `WASM_API_VERSION` together and they had already
parted; only one moved. That mistake is recorded once and should not be paid twice.

---

## The spike, and what it settled

A throwaway crate at `M:\claud_projects\temp\phase7-spike`, outside the repo tree as
Phase 5's and Phase 6's were, path-depending on `sim-core`/`sim-data` so the half it
shares with the shipped model *is* the shipped code — it calls `sim_core::spm::diffuse`
and `ocp_lookup` and reads `chemistries/nmc_21700_lgm50.toml` rather than copying either.
Everything in this section is measured on this box. Full working notes, including the
probe scripts, live beside it in `FINDINGS.md`.

Baseline first, per `CLAUDE.md`'s "do not start a phase before the previous one's tests
pass": at `e486ca4`, `cargo test --workspace` is green and clippy is clean.

| question | answer |
| -------- | ------ |
| What does Chen2020 publish for the electrolyte, and in what **form**? | Callables — but closed-form published polynomials underneath, so they can be stored **exactly**. |
| Does a fixed-step scheme converge on the coupled DAE, and at what `dt`? | **Yes, to `dt = 60 s` at every rate up to 3C**, including through full electrolyte depletion. |
| What does one DFN cell-step cost? | **~30× an SPM cell-step** with an analytic banded Jacobian; ~850× with the dense numerical one the spike used. |
| `diffsol`, re-evaluated against the DFN system? | **Declined again**, on a checked version. |
| Is the phase worth building at all? | **Yes, and the answer is a cliff rather than a slope.** |

### The transport-property schema is not a table, and that is a better answer

`ParameterValues("Chen2020")` returns **callables** for `Electrolyte diffusivity` and
`Electrolyte conductivity`. That is the same shape as Phase 6's trap, where Chen2020's
kinetic activation energies existed only as literals inside function bodies. The obvious
resolution — sample onto a grid and document the interpolation error, as the OCP tables
do — turns out to be unnecessary. The functions' source is a published closed-form fit
(Nyman 2008, LiPF6 in EC:EMC 3:7) with no temperature dependence at all:

```python
D_c_e   = 8.794e-11*(c/1000)**2 - 3.972e-10*(c/1000) + 4.862e-10        # m2/s
sigma_e = 0.1297*(c/1000)**3 - 2.51*(c/1000)**1.5 + 3.329*(c/1000)      # S/m
```

So `[dfn]` stores **coefficient/exponent pairs in `x = c_e/1000`**, which is exactly the
form the fit takes. This is data under `CLAUDE.md`'s principle 10, and it carries **no
interpolation error** — which matters, because Phase 6 found the OCP tables' own
1.90/1.88 mV interpolation error *was* the accuracy floor. Introducing a second sampled
table would have raised that floor for nothing.

Two consequences to carry into slice A:

- The conductivity has an `x^1.5` term, so this is a sum of power terms, not a
  polynomial. `(coefficient, exponent)` pairs cover both; a coefficient *array* would not.
- **`powf` is not in the set that may be bit-pinned.** Phase 6's rule is that only pure
  IEEE-754 arithmetic and decimal→`f64` parsing may be committed as an exact-bit
  assertion, because those are identical on every conforming platform while `exp`,
  `asinh` and `powf` are not. `x^1.5` evaluated as `x * sqrt(x)` *would* be pinnable
  (`sqrt` is IEEE-exact); evaluated as `powf(1.5)` it is not. Slice A should pin the
  parsed coefficients, not any value computed through them.

Everything else the model needs is a plain float in the set: `t+` = 0.2594,
thermodynamic factor **1.0** (so the `1 + dlnf/dlnc` term drops out entirely), porosities
0.25 / 0.47 / 0.335, Bruggeman 1.5 on the electrolyte and **0** on the electrode,
separator 12 µm, and solid conductivities 215 and **0.18** S/m.

Note `ε_e` is a *different number* from the `active_volume_fraction` (`ε_s`) that `[spm]`
already carries, and `ε_s + ε_e ≠ 1` — binder, additive and carbon make up the rest. A
schema that derived one from the other would be wrong.

### Why the phase is worth building: a cliff, not a slope

PyBaMM DFN against PyBaMM SPM on the shipped LG M50 set, same solver, same grid,
`t_interp` throughout (never `t_eval` resampling — Phase 6's 346 mV chord trap):

| C-rate | mean \|DFN − SPM\| | max | DFN to cut-off | SPM to cut-off | DFN Ah | SPM Ah |
| ------ | ------------------ | --- | -------------- | -------------- | ------ | ------ |
| 0.2C | 11.0 mV | 13.3 mV | 18226.5 s | 18234.6 s | 5.063 | 5.065 |
| 1C | 58.1 mV | 67.8 mV | 3555.3 s | 3567.8 s | 4.938 | 4.955 |
| **3C** | **366.4 mV** | **899.3 mV** | **557.4 s** | 1084.7 s | **2.322** | 4.520 |

**Every cell in those two cut-off columns is a real termination** — `sol.termination` is
`'event: Minimum voltage [V]'` for all six. The first cut of this table was not: it
passed a fixed integration window per C-rate and printed the last sample, so three of six
cells reported the *window* rather than a cut-off, including the SPM's at 3C, which was
the denominator of the headline. This is the same failure as the "range over (x,t)"
metric two paragraphs down — a column measuring one thing under another thing's label —
and it is recorded rather than quietly corrected because it took a second probe to catch
and will be available to catch the next one.

Corrected: at 3C the DFN reaches the cut-off in **51.4 %** of the time the SPM does, and
delivers **2.32 Ah against 4.52**. At 1C and C/5 the two models reach the cut-off within
0.3 %, so the whole effect is a cliff between 1C and 3C rather than a slope.

That is a statement about the two *models* as PyBaMM implements them; it is **not** yet a
statement about batsim's shipped SPM, which is validated against PyBaMM SPM at C/5, 1C
and GITT only — 3C is outside that set. Slice D should either add a 3C row to that
comparison or keep the claim model-shaped. Getting this backwards in a lesson would be
the "written from reasoning, not from the screen" mistake the UI slices already paid for
three times.

**A basis note, because this document quotes two different "3C".** The table above is
PyBaMM's own Chen2020 basis, where nominal capacity is **5.0 Ah**, so 3C is 15.0 A. The
prototype runs against `nmc_21700_lgm50.toml`, whose `capacity_ah` is **5.153198**, so
its 3C is 15.459594 A — and the reference figures quoted in the OCP finding below were
re-run at *those* amps, which is why PyBaMM ends at 488.5 s there and 557.4 s here. Both
are right for their own experiment; they are not the same experiment.

Where the volts go, measured as the spread **across x at fixed t** (the first cut of this
probe measured the range over (x,t), which a discharge dominates with the OCP moving and
which says nothing about any ohmic drop):

| C-rate | φ_s positive | φ_s negative | φ_e | c_e spread |
| ------ | ------------ | ------------ | --- | ---------- |
| 0.2C | 2.04 mV | 0.0020 mV | 16.3 mV | 270 mol/m³ |
| 1C | 10.7 mV | 0.0105 mV | 97.2 mV | 1881 mol/m³ |
| 3C | 42.1 mV | 0.0364 mV | 668 mV | 3495 mol/m³ |

**The negative electrode's solid phase is equipotential to within 36 µV even at 3C** —
215 S/m is simply a lot of conductivity. The positive electrode's 0.18 S/m buys 2–42 mV.
**The electrolyte is the whole story**: φ_e alone is 668 mV of the 899 mV gap.

This is worth stating plainly because it is a temptation the phase should refuse. It
would be possible to ship "SPM plus an electrolyte" and drop the solid-phase potential
equations, and at these parameters it would be nearly indistinguishable. It is refused
anyway: `σ_s` is chemistry data, a set with a worse positive conductivity would expose
it, and a model named `Dfn` that silently omits one of the DFN's four equations is the
kind of quiet lie the `MIN_SHELLS = 2` note already refused once.

### The fixed-step scheme converges, and `diffsol` is declined again

The prototype eliminates the particles from the Newton system, which is the one
structural idea worth testing and the one that carries into slice B:

> Each particle's backward-Euler radial solve is **linear in its own surface flux**, so
> the surface concentration is an exact affine function of the local reaction current,
> `c_surf = c0 + β·j`. Two tridiagonal solves per particle per **step** — not per Newton
> iteration — give `c0` and `β`, after which the particles leave the Newton system.

That holds the unknown count at **4 per x-node** (`c_e`, `φ_e`, `φ_s`, `j`) instead of
`4 + N_r`, and it makes the Jacobian block-tridiagonal with a 4-wide block. In the
separator the `φ_s` and `j` rows are the trivial equations `φ_s = 0`, `j = 0` and
decouple, which keeps the block uniform.

Convergence, at 10/5/10 nodes and `N_r = 10`, damped Newton, tolerance 1e-8 on a
row-scaled infinity norm (the rows carry A/m² and mol/(m²·s) and differ by ~1e5, so an
unscaled norm would call the mass equation converged long before it is):

| C | dt = 1 s | 10 s | 60 s | 600 s | 3600 s |
| - | -------- | ---- | ---- | ----- | ------ |
| 0.2 | 2.59 / 0 | 3.06 / 0 | 3.21 / 0 | 4.60 / 0 | 10.40 / 0 |
| 1.0 | 3.03 / 0 | 3.25 / 0 | 4.41 / 0 | 12.67 / **1** | 39.00 / 0 |
| 2.0 | 3.10 / 0 | 4.25 / 0 | 5.43 / 0 | 24.67 / **1** | — |
| 3.0 | 5.56 / 0 | 7.50 / 0 | 12.93 / 0 | 34.00 / **1** | — |

(mean Newton iterations / steps that failed to meet tolerance.)

**Up to `dt = 60 s` nothing fails, at any rate up to 3C, including through complete
electrolyte depletion.** Accuracy degrades well before convergence does — at 1C,
`dt = 60 s` overshoots the cut-off by 50 s — which is the useful shape: the scheme
complains by being wrong slowly rather than by falling over.

So the "same code path serves real-time stepping and months-long aging fast-forward"
promise **holds, with a stated envelope**: a fast-forward at `dt = 3600 s` converges at
0.2C (10.4 mean iterations) and is the wrong tool at 1C and above. That is a
documentation obligation for slice B, not a redesign.

`diffsol` is at **0.16.1**; `BdfState` still publishes no fields and implements neither
`Serialize` nor `Deserialize`, so principle 5 still cannot be met through its public API.
The decline is re-issued on a checked version rather than inherited. The DFN-specific
argument is *stronger* than Phase 6's, not weaker: the whole point of an adaptive
implicit suite is the step-size control, and the spike shows a fixed step is adequate
here — so the dependency would buy machinery the model does not need, at the cost of the
one property the project will not trade.

### The `c_e` floor is load-bearing, and that is the trap

`κ_e(c_e) → 0` as `c_e → 0` degenerates the electrolyte potential equation, and the
reference genuinely goes there: at 3C, PyBaMM's own `c_e` reaches −0.0007 mol/m³ and
**90.6 % of the run has `c_e < 100 mol/m³` somewhere**. The guard is a floor applied
**inside the transport and kinetics lookups only** — never on the state, which is
`spm.rs`'s `clamp_surface` precedent. Sweeping it at 3C:

| floor \[mol/m³\] | t to cut-off | Ah | min c_e | worst iterations |
| ---------------- | ------------ | -- | ------- | ---------------- |
| 100 | 1044 s | 4.483 | −343.6 | 8 |
| 10 | 940 s | 4.037 | −180.0 | 16 |
| 1 | 885 s | 3.801 | −33.6 | 18 |
| 0.1 | 877 s | 3.766 | −3.58 | 19 |
| 0.01 | 876 s | 3.762 | −0.29 | 21 |
| 0.001 | 876 s | 3.762 | −0.016 | 19 |

The answer **converges in the floor** below ~0.1 mol/m³, and the scheme still solves
there (0 unconverged, worst 19–21 iterations). At 1C the floor is **completely inert** —
1 and 100 give identical runs, because `c_e` never falls below 464.

But read the top of that table. A floor of 100 buys four Newton iterations and pays 0.72
amp-hours for them, **monotonically and silently**. Nothing raises a flag; the run simply
reports a healthier cell. So the floor is a constant that owes its own justification and
its own test — a `dfn_floor_is_inert_at_one_c`-shaped assertion that pins the *inertness*
rather than the value, plus a comment saying what raising it would buy and cost. This is
the "tidiest-looking answer is the one to distrust" case, and the distrust was warranted.

### The finding slice A has to act on: the OCP tables' margin is a Phase 6 artefact

The prototype agrees with PyBaMM to **3–8 mV through the bulk at 1C** — but over the
**whole trajectory, with no window**, the worst disagreement is **250.9 mV**, all of it
at the knee (the prototype reaches the cut-off at 3490 s against the reference's 3555 s,
and the last few points are comparing two curves falling at different moments). Phase 6's
exit criterion 2 is explicitly whole-trajectory with no SOC window, and this plan cites
that precedent, so the bulk figure alone would set slice D up to look like a regression
against a standard it never claimed. Mean over the whole 1C overlap is 20.3 mV.
Amp-hours agree to **0.6 % at 2C**. At 3C the prototype ran 876 s against the reference's
488.5 s at the same amps.
Both terminate on minimum voltage — `sol.termination` was checked at every rate and is
`'event: Minimum voltage [V]'`, not some zero-electrolyte event, which was the first
hypothesis and was wrong.

**Grid refinement does not close the gap.** 5/3/5 → 10/5/10 → 20/10/20 → 40/20/40 gives
718 → 838 → 876 → 884 s, converging *away* from 488, and `N_r` at 10 / 20 / 40 changes
nothing at all. Phase 6's transferable diagnostic applies: an error that does not move
with the discretisation is not a discretisation error.

**The electrolyte is not it either.** `c_e(x)` matches the reference to under 1 % at
every node at t = 100 / 300 / 450 s — the peak 3159.5 against 3165.4, and the depleted
tail 18.90 against 19.63. The transport is right, which is what makes the next step
conclusive rather than a guess.

**It is the OCP tables.** The positive particle's **surface** stoichiometry reaches
**0.9998** and spends **27.6 % of the 3C run above 0.9040**, which is the top of the
table `nmc_21700_lgm50.toml` ships. Above it `ocp_lookup` clamps flat — by design, and
correctly so — exactly where the real OCP plunges. A modelled cell whose positive OCP
stops falling keeps delivering voltage, which is precisely the symptom.

The detail that makes this a *phase* finding rather than a bug report: the
**x-averaged** positive surface stoichiometry peaks at only **0.7065** in this 3C DFN
run, comfortably inside the table. It is the *local* value at the separator-facing edge
that leaves it, and only a model that resolves the electrode thickness has one. So the
3C severity of this failure is a DFN phenomenon, and Phase 6 sized its 0.05 margin for
the model it was making it for.

> **Slice A measured this and found the inference that followed it to be wrong.** The
> paragraph below originally read that 0.7065 *bounds* the SPM, so the extension was
> "provably inert" there. It does not bound it, and it is not. See the **slice A** note
> at the end of this document — the short version is that a
> quantity measured on a model that **terminates early** does not bound the same
> quantity in a model that runs to completion. The DFN delivers 2.32 of 5.15 A·h at 3C,
> so its positive electrode never fills; a 1C SPM discharge that reaches the cut-off
> fills it to `stoich_max` and its surface leads to **0.9115**, past the old table top.

The **negative** table is not implicated: its surface stoichiometry stayed within
\[0.2560, 0.9014\] against a table covering \[0, 0.960618\]. Only the positive was
measured to leave its table, and only at the separator-facing edge.

Slice A therefore extends the OCP tables to cover the full stoichiometry range — the
positive because it is measured to need it, the negative because a margin sized for one
model and inherited by another is exactly what this finding is about. Same extraction
script, same provenance. Two things to verify rather than assume:

- Extending a table **beyond** its current range does not change `interp1` inside that
  range, so SPM trajectories that stay inside should be bit-identical. That is the claim,
  and exit criterion 1 is where it gets checked rather than believed. ~~The measurement
  above is a stronger safety argument than the check~~ — it is not, and slice A found out
  which way round it goes. **The claim as stated is true; the assumption that SPM
  trajectories stay inside is false.** Checked, not believed, is what saved it.
- `crates/sim-data/tests/spm_exact_bits.rs` pins specific parsed literals. Appending
  points adds literals without moving existing ones — again, checked, not assumed.

### Cost, and the design decision it forces

Measured in one run so the ratios hold even where this box's absolutes do not. `SPM N=20`
through the real `Pack::step` at 1S1P came out at 1.30–1.36 µs, consistent with Phase 6's
own figure, so the box is in a comparable state to the one that set the SPM budget.

| model | µs/step | × SPM N=20 |
| ----- | ------- | ---------- |
| ECM (`Pack::step` 1S1P) | 0.182 | 0.1 |
| SPM N=10 | 1.087 | 0.8 |
| **SPM N=20** | **1.360** | **1.0** |
| DFN 6/3/6, N_r=8 (60 unknowns) | 482 | 355 |
| DFN 10/5/10, N_r=10 (100 unknowns) | 1154 | 849 |
| DFN 20/10/20, N_r=20 (200 unknowns) | 4587 | 3373 |

Those DFN rows are a **dense numerical Jacobian and a dense LU**, and they are an upper
bound rather than a budget. The component every implementation keeps, measured
separately:

| grid | unknowns | one residual evaluation |
| ---- | -------- | ----------------------- |
| 6/3/6 | 60 | 3.28 µs |
| 10/5/10 | 100 | 5.53 µs |
| 20/10/20 | 200 | 11.02 µs |
| 30/15/30 | 300 | 14.95 µs |

Linear in the unknown count, as a finite-volume residual should be. Three cost tiers at
10/5/10 and ~3 Newton iterations:

| Jacobian | linear solve | µs/cell-step | × SPM N=20 |
| -------- | ------------ | ------------ | ---------- |
| numerical, dense (`m` residual evals) | dense LU | **1154, measured** | 849 |
| numerical, banded (15 residual evals) | banded LU | ~265, projected | ~200 |
| **analytic, banded** (≈1 residual eval) | banded LU | **~40, projected** | **~30** |

**The numerical Jacobian, not the linear solve, is what makes the dense number look
catastrophic** — 15 colours × 5.5 µs is still 83 µs per assembly. So an **analytic
Jacobian is a slice-B requirement, not an optimisation to be deferred**, and that is the
single most load-bearing thing the cost measurement says. Slice D re-measures and states
the real budget; the ~40 µs above is a projection and is labelled as one everywhere it
appears.

Even at the analytic tier a DFN cell-step is ~30× an SPM one and ~200× an ECM one, so the
topology arithmetic is stark and the docs must say so rather than let someone discover it:
1S1P ≈ 40 µs, 10S10P ≈ 4 ms, 100S10P ≈ 40 ms. **A DFN pack of more than a few cells is
not a real-time configuration**, and unlike the SPM budget there is no shell count to
trade down — the cost is in the x-grid, which is what the model is *for*.

---

## Design decisions settled here, so a slice does not re-open them

**`[dfn]` extends `[spm]`, it does not duplicate it.** Every electrode geometry, kinetic
and OCP number a DFN needs is already in `[spm]`, including two 45- and 20-point OCP
tables. `[dfn]` carries only what is new: the electrolyte fits, `t+`, the thermodynamic
factor, the three porosities and Bruggeman exponents, the separator thickness, and the
two solid conductivities. `Pack::new` with `CellModelConfig::Dfn` therefore requires
**both** sections, and gets a `BuildError` naming whichever is missing — the same
"name both halves" rule `MissingSpmParams` already follows, because a build error that
says only "invalid config" costs its reader an hour.

**The Newton warm-start vector is state, not a cache**, and it goes in the snapshot. This
is the exact sibling of `SpmState::i_last`, whose doc comment already argues the case: a
Newton that stops at a tolerance lands somewhere that depends on where it started, so the
starting point decides the trajectory at the 1e-8 level, and exit criterion 3 is a
bit-identity claim. A restored cell that re-seeds its guess from a cold default is a cell
that continues a *different* trajectory.

**The snapshot grows by roughly an order of magnitude per cell**, and that is expected
rather than a problem to engineer around: at 10/5/10 with `N_r = 10` a DFN cell is
25 + 200 + 100 ≈ 325 `f64` against an SPM cell's ~41. Slice B should state the number in
a doc comment so nobody is surprised by a snapshot file, and should *not* reach for a
compact encoding — `CLAUDE.md` asks for one serde value with a version field, and
readability of the snapshot has been worth more than its size at every previous phase.

**The pack's tangent cannot be a central difference for this model.** `spm::source_at`
takes `−dV/di` by evaluating `voltage()` at `i ± h`, and the pack's nonlinear solve
re-takes every cell's tangent on every pass. For an SPM a `voltage()` is a handful of
table lookups. **For a DFN it is a full nonlinear solve**, so the existing contract would
cost three DFN solves per cell per pass — the structural risk the SPM's shape hides.

The answer, and it is cheaper than the SPM's: the converged Newton has already factorised
the Jacobian, so `dV/di` comes from **one extra back-substitution** with the applied
current as the right-hand side — a sensitivity solve, exact to the discretisation rather
than a difference quotient, and a small fraction of one residual evaluation. Slice C's
job is to give `CellModel` a source contract that lets a model return a tangent it
computed during its own solve, without breaking the ECM arm's exactness or the
`SourceCache` invariant that `source` is a pure function of state while `source_at` is
not. Phase 6's `SourceCache` note is the thing to read first.

**Mixed ECM/SPM/DFN packs stay unrepresentable**, for the reason Phase 6 recorded:
`cell_model` is one value for the whole pack, so there is nothing for a `BuildError` to
reject. The solve is already per-cell and mixed-ready. Nothing in this phase changes that
and nothing in this phase should add the config surface.

**Aging on DFN stays a multiplier**, as it is on SPM, with the same honest cost stated
rather than implied: it is LAM's inventory effect plus charge-transfer growth, and there
is no electrode slippage. A DFN could in principle age its porosity — SEI growth fills
pores, which is a real and visible mechanism — and that is a genuinely attractive future
slice, but it is not this phase and adding it would put a second unvalidated model inside
the one being validated.

---

## Gates, and where they are blind

**The out-of-tree trajectory instrument** at `M:\claud_projects\temp\phase6-baseline`
is what exit criterion 1 is measured with. The anchor is
**`after-sliceA-p7.txt`** (9 cases, 1254 lines), *not* Phase 6's `after-sliceE.txt`:
slice A moved 82 lines and re-anchored, and `ANCHORS.md` beside it records which three
regions moved and why. Diffing a slice-B run against the Phase 6 anchor would carry
slice A's delta forward and teach its reader to ignore it. **Three** known blind spots,
all of which have to be checked against this phase's actual diff rather than assumed
away:

- It **enumerates its 17 telemetry fields by name**, so an 18th would be invisible to it.
  Phase 7 is not expected to add one — `solve_iterations` already reports what a nonlinear
  model costs, and it was added for exactly this class of model — but if a slice adds a
  field, it also extends the instrument.
- It builds cases from scenario TOML through `parse_scenario`/`build_pack`, so a
  `#[serde(default)]` `PackConfig` field is invisible too. `CellModelConfig::Dfn` is an
  enum *variant* on an existing field, which is reachable from scenario TOML, so new DFN
  cases are additive and valid from slice B forward — the same status slice E's SPM cases
  have.
- **The `CV 3.5 V` leg of both LG M50 cases is garbage in the anchor itself**, found by
  slice A. `Demand::Voltage(3.5)` is 1.75 V/cell on a 2.5 V-cut-off cell, and the SPM's
  voltage-demand path answers with NaN, Inf, negative kelvin and megavolt terminals — in
  `after-sliceE.txt` as well as in the new anchor. A baseline whose tail is garbage
  cannot detect a change in that region, so the instrument is blind from that leg to the
  end of those two cases. Recorded rather than fixed: it is a real hole in the SPM's
  `Demand::Voltage` path and deserves its own slice, not a correction folded into this
  phase.

**Slice A is the slice where criterion 1 is genuinely at risk**, and this is the
difference from Phase 6, where slice A was a pure refactor with a mechanical gate. Here
slice A edits `nmc_21700_lgm50.toml`, which the SPM reads. The claim that appending OCP
points cannot move an interpolation inside the old range is almost certainly true and is
exactly the kind of "almost certainly" that Phase 6 caught twice. Run the anchor.

> **It was run, and it was the assumption around the claim that broke.** The claim held
> exactly as stated; what failed was the belief that no SPM trajectory leaves the old
> range. See the slice A note below.

**Built-to-fail discipline for slices C and D**, inherited and non-negotiable: the
perturbation harness restores from a copy it made itself and **never** `git checkout` —
that mistake destroyed uncommitted work once already. `cargo test --no-fail-fast`, or the
first failing binary hides the second's verdict. Slice a text file in Python with `'rb'`,
because a `'w'` round trip on Windows turns `\r\n` into `\r\r\n` and makes an identical
file diff as wholly different. And **tabulate which tests actually catch the
perturbation** rather than assuming the suite does; Phase 6 found two of four new tests
passed with the feature removed.

A DFN-specific perturbation worth building deliberately: **set the Bruggeman exponent to
0**. It changes every effective transport property by a factor of `ε^1.5` ≈ 0.12–0.32,
which is large, physical, and invisible to any test that does not run at a rate where the
electrolyte matters. If a 1C golden passes with it, the goldens are testing the wrong
rate — which is the whole reason criterion 2 requires a depletion scenario.

---

## Slice A — the `[dfn]` chemistry section

**Landed. `SNAPSHOT_VERSION` unmoved at v10.** `cargo test --workspace` green,
`cargo clippy --workspace --all-targets -- -D warnings` clean.

What shipped: `[dfn]` schema (`DfnParams`, `PowerTerm`, `DfnElectrode`, `DfnSeparator`)
with validation, `tools/reference/extract_dfn.py`, the block itself in
`nmc_21700_lgm50.toml`, both OCP tables extended to the full stoichiometry range, and
`crates/sim-data/tests/dfn_chemistry.rs` (20 tests). No engine physics.

### The finding: "provably inert for the SPM" was false, and the gate caught it

This plan argued — and commit `b5ef486` deliberately *strengthened* the argument — that
extending the OCP tables could not move an SPM trajectory, because the x-averaged
positive surface stoichiometry peaks at 0.7065 even at 3C and an SPM has only the
x-averaged quantity. **Measured, the shipped 1C SPM golden reaches 0.9115**, past the
0.9040 the positive table used to stop at, at every shell count.

The inference was invalid, and the reason generalises:

> **A quantity measured on a model that terminates early does not bound the same
> quantity in a model that runs to completion.** The 0.7065 came from a 3C DFN run that
> hits the cut-off at 557 s having delivered 2.32 of 5.15 A·h — its positive electrode
> never fills. An SPM discharge that *reaches* the cut-off fills the positive bulk to
> `stoich_max` = 0.8540, and the surface leads the bulk from there.

Same family as the two errors this document already records fixing: a column measuring
one thing under another thing's label, and a metric taken over the wrong domain. The
tell here was different, though, and worth having: the claim was checkable and the plan
said to check it, so the only thing that saved it was **running the gate instead of
believing the argument**.

Measured across all three shipped SPM goldens (`ocp-probe`, replaying through the real
`Pack::step`):

| golden | pos surface max | rows whose voltage moves | worst move |
| ------ | --------------- | ------------------------ | ---------- |
| `spm_cc_c5_25c` | 0.8649 | **0** of 595 | — |
| `spm_pulse_relax_25c` | 0.5924 | **0** of 1560 | — |
| **`spm_cc_1c_25c`** | **0.9115** | **9–15** of 699 | **5.9–9.9 mV** |

Every differing row is at the end of discharge. The stoichiometry column is an estimate
— the probe reads the profile *after* the step and applies that step's flux, the
"read the step BEFORE" offset the CC-CV slice already paid for — but the trajectory
column is direct and carries the conclusion on its own.

**It is a correction, not a regression.** Against the PyBaMM SPM reference, N=40's worst
error over the 1C golden halves, 5.04 → 2.63 mV. The old table clamped the positive OCP
flat from 0.9040 up, exactly at the end-of-discharge knee where the real OCP plunges, so
the modelled cell held its voltage too high. Exit criterion 1 is therefore **amended
rather than met**: bit-identity holds everywhere except where the old table was wrong.

### The second-order consequence: an assertion that was measuring the artefact

`shell_count_convergence_puts_the_documented_default_at_the_floor` asserted
`fine > 0.5 * default` — "refining past the default buys nothing, because the residual is
the OCP tables rather than the grid" — and it failed. Its premise was true *for the wrong
reason*: the floor was the **clamp**, not the tables' 1.88 mV interpolation error.

It was rebuilt from all three goldens rather than repaired by relaxing its constant,
because the discriminating data points away from moving `DEFAULT_SHELLS`:

| scenario | N=5 | N=20 | N=40 |
| -------- | --- | ---- | ---- |
| `cc_c5` | 8.75 | **2.58** | 3.36 |
| `cc_1c` | 53.06 | 6.65 | **2.63** |
| `pulse_relax` | 30.59 | **1.87** | 2.98 |

N=40 improves only at 1C and is *worse* at the other two rates, so 20 is still where the
curves cross and refining is a trade. A single-rate bound dressed as a default-shells
decision is precisely what the old assertion turned out to be; shipping a second one
would have been the same mistake with a different constant.

### How the extension was made safe, and the check that discriminates

`extract_spm.py` refines its grid greedily from `linspace(lo, hi, 9)` against a dense
reference on the same interval, so **regenerating over [0, 1] in one pass would move
every breakpoint** and shift the interpolation inside the old window — the failure mode
that would have been indistinguishable from the real finding. The script now generates
the **core over the old margin window alone** and concatenates two separately-tabulated
tails, so the extension is append-only *by construction* rather than by inspection.

The check that discriminates is stronger than "the old points are all still there":

1. every old breakpoint **and its potential** identical bit-for-bit, and
2. **no new breakpoint strictly inside the old range** — because a shifted dense
   reference can insert one without moving any existing point, and that alone changes
   `interp1` there.

Both verified mechanically on both electrodes before anything else ran. Both fits turned
out to be finite and monotone non-increasing over the whole extension, so no endpoint had
to be truncated; graphite's is *exactly* constant at 0.092020 V above 0.96, which is why
two points describe that tail.

Free discriminator worth reusing: `the_shipped_spm_geometry_derives_to_exact_bits` reads
**scalars only**, so it stays green through a table edit and goes red on a scalar one.
When the sibling hash moved and it did not, that was positive evidence the change was
confined to the tables.

### Smaller things

- **The snapshot grew 11 bytes in every case, including ECM-only ones** — `,"dfn":null`,
  because the chemistry is serialized inside the snapshot. Bytes, not layout: the field is
  `#[serde(default)]` and serde ignores the unknown key in the other direction, so both
  directions round-trip at v10. The bump stays budgeted for slice B.
- **`extract_spm.py`'s regex cannot reach the Nyman coefficients**: they are
  *expressions*, not `name = value` assignments. `extract_dfn.py` parses the function
  source with `ast` and then evaluates the parsed terms against PyBaMM's own callable at
  seven concentrations, refusing to emit on disagreement. `inspect.cleandoc` is the wrong
  dedent for this — it strips the body's indentation relative to the first line and turns
  a module-level `def` into an `IndentationError`; `textwrap.dedent` is right.
- **`[dfn]` needs no bit-pin file.** `[spm]` hashes because it carries 148 numbers;
  `[dfn]` has 22 named ones, so asserting each by name is feasible and strictly stronger
  (a failure says *which* moved). What a per-value pin loses is the new-field tripwire a
  hash gets for free, so that is a separate test counting the numbers serde can see —
  which trips on a field added inside `DfnElectrode` or `DfnSeparator` too, where a reader
  looking only at `DfnParams` would miss one.
- **Validation checks the transport fits at one point and says so.** Both are
  non-monotone over the range a 3C discharge visits, so no cheap sampling would prove
  positivity; at `x = 1` (`c_e = 1000`, the fit's own reference) the sum is
  `Σ coefficient` — plain arithmetic, no `powf`, the one value derived from this section
  that could ever be bit-pinned.
- **A pre-existing hole, found and recorded rather than fixed.** The `CV 3.5 V` leg of
  both LG M50 instrument cases is NaN, Inf, negative kelvin and megavolt terminals — in
  Phase 6's anchor as well as the new one. `Demand::Voltage(3.5)` is 1.75 V/cell on a
  2.5 V-cut-off cell, and the SPM's voltage-demand path does not survive it. The
  instrument is blind from that leg to the end of those two cases.

---

## Slice B — the DFN cell

**Landed. `SNAPSHOT_VERSION` v10 → v11**, the phase's one bump, spent where it was
budgeted. `cargo test --workspace` green (55 binaries), `cargo clippy --workspace
--all-targets -- -D warnings` clean.

What shipped: `crates/sim-core/src/dfn.rs` (the grid, the state, the residual, the
analytic banded Jacobian, a damped Newton, a band LU with partial pivoting, and the
sensitivity solve that produces the cell's tangent), `CellModel::Dfn` arms throughout
`ecm.rs`, `CellModelConfig::Dfn` with `BuildError::MissingDfnParams` and
`BuildError::BadNodeCount`, `spm::ocp_slope` / `ecm::interp1_slope`, and
`crates/sim-data/tests/dfn_cell.rs` (13 tests). The pack layer's own code is untouched
apart from the new config arm and one added argument.

### Exit criterion 1 held with **no exception**, and that was tested rather than argued

The instrument (now anchored at `after-sliceB-p7.txt`) moved **18 lines, all of them
`## final snapshot` hashes** — nine cases, one line each — with `len=` unchanged on all
nine, because "10" and "11" are the same number of bytes. Not one telemetry line moved.

"The version bump explains the whole diff" is exactly the shape of claim slice A caught
being false, so it was **run**: with `SNAPSHOT_VERSION` alone reverted to 10 and nothing
else touched, the instrument's output is byte-identical to `after-sliceA-p7.txt`. Reverted
by restoring `pack.rs` from a copy made beforehand, never `git checkout`.

### The design decision the slice turned on: what a DFN shows the pack

The plan says slice B leaves "the pack still seeing a linear source" and slice C brings the
tangent. There is a tempting alternative that had to be refused: give `terminal_v_at` a
**quasi-static** solve — freeze `c_e`, keep the mass row as `c = c_old`, and the particle
affine map degenerates to exactly `spm::c_surface`'s half-shell extrapolation, so one
residual serves both regimes and the pack gets a real nonlinear `V(i)` from slice B
onwards. It is elegant, and it is wrong for three compounding reasons:

- **It is slice C's job.** A `terminal_v_at` that runs a Newton solve *is* the pack
  integration.
- **It builds the wrong artifact either way.** Taking the tangent off it by central
  difference is the thing this plan explicitly forbids; taking it by sensitivity builds
  slice C's mechanism against the *quasi-static* Jacobian, when the one the plan argues
  for comes off the **transient** step's already-factorised matrix.
- **Cost.** A per-pass `terminal_v_at` + `source_at` is ~6 Newton solves per step on top
  of `advance`'s one.

So the discriminator held to was: **slice B contains no Newton solve outside `advance`.**
`DfnState::tangent` carries the line the last solve produced, `source_at` ignores its
operating point exactly as the ECM arm does, and `terminal_v_at` evaluates that line.

The consequence is worth naming because a reader will meet the number before the reason: a
DFN pack reports **`solve_iterations == 1`**, the same as an all-equivalent-circuit pack,
even though `is_linear` answers `false` and the iteration really runs. It exits on its
first pass because the curve it measures the aggregate against *is* the line it
aggregated. That number is a statement about this slice, not about the physics, and
`the_pack_solve_has_nothing_to_chase_yet` pins it so slice C's change is visible rather
than silent.

### The tangent is a sensitivity solve, and it landed here rather than in C

`dR/di_app` is nonzero only in the solid-charge row at each current collector (−1 and +1),
so `dV/di` is **one back-substitution** against a factorisation the step has already paid
for. Delivering it through the *existing* `source` contract needed no contract change,
which is why slice B could have it: what slice C still owes is letting the pack ask for a
tangent at a current the cell has not solved at.

One deliberate cost inside it: the Jacobian is **re-assembled and re-factorised at the
converged point** rather than reusing the last iteration's, which was taken one Newton
step short of the answer. That buys a tangent actually tangent to the curve at the point
reported.

### Cost: the analytic Jacobian delivered, and the projection was still optimistic

Measured through `Pack::step` at 1S1P, 10/5/10, `N_r = 10`, priced in the same process as
the models compared against: **50–65× an `Spm` N=20 step**, ~350–500× an ECM one. Against
the spike's dense-numerical 849× that is a 13–23× improvement, and the analytic assembly
is what bought it — but the plan projected ~30×, and the gap is two things the projection
did not cost: the damping line-search evaluates the residual at least once more per
iteration, and the sensitivity solve re-assembles at the converged point. Both are
deliberate. Slice D re-measures and states the budget; **the ~40 µs figure elsewhere in
this document remains a projection and should not be quoted as a result.**

### The measurement that produced a constant: the damping search's depth

A cheap-looking loop restructure (carrying the residual across the iteration boundary
instead of recomputing it, worth ~25 % of the step) introduced a real bug and then exposed
a real one. The bug: the `!accepted` fallback recomputed the step at a lambda the residual
had never been evaluated at, so the carried residual described a different point — 39 steps
of a 3C discharge falsely flagged `SOLVE_UNCONVERGED`. Fixing it left **27** still flagged,
where the original loop had zero, which is what surfaced the real finding.

The original had been halving lambda once *past* its last evaluation, and that accident was
load-bearing. Counting unconverged steps of a 3C discharge against the attempt cap:

| attempts | 11 | 12 | **13** | 14 | 16 | 20 |
| -------- | -- | -- | ------ | -- | -- | -- |
| unconverged steps | 53 | 27 | **0** | 0 | 0 | 0 |

The knee is sharp and sits at 13: the step that gets a depleted cell moving is around
`2^-12` of the Newton step, and a search stopping one halving early reads that as a local
minimum and gives up. `DAMPING_ATTEMPTS = 16`, past the knee, costing nothing on any step
that converges.

**The trajectory those 27 steps produced was the same to every printed digit.** What moved
was the *flag* — which is the honest thing to have moved, and the reason a numerical event
is a flag rather than a silent approximation.

### What the model reproduces

Against the spike's own prototype and PyBaMM, at 10/5/10 with `N_r = 10`:

| | this engine | spike prototype | PyBaMM DFN |
| - | ----------- | --------------- | ---------- |
| 1C to cut-off | 3484 s | 3490 s | 3555 s |
| 1C minimum `c_e` | 461 mol/m³ | 464 | — |
| 3C minimum `c_e` (floor 0.01) | −0.20 mol/m³ | −0.29 | ~−0.0007 |

And the cliff, which is the phase's reason to exist: at 3C the DFN delivers **3.12 A·h in
726 s** against the *same cell as an SPM* managing **4.55 A·h in 1060 s** — 69 % and 68 %.
PyBaMM's own pair is 51 %, so this engine **under-states** the cliff at 3C; quantifying
that is slice D's job, and it is stated here so slice D does not meet it as a surprise.
Both models read the same `[spm]` block, the same OCP tables and the same particle solver,
so the difference between them is the electrolyte and nothing else.

### Smaller things settled here

- **`c_e0` is not a `[dfn]` field.** It is `SpmParams::c_e_mol_per_m3` — the number the
  single-particle model holds constant *is* the DFN's initial uniform field. Adding a
  field would have been slice A's schema changing under slice B, and would have tripped
  `dfn_chemistry.rs`'s serde-visible-number count.
- **`x^1.5` is `x·sqrt(x)`, decided before any golden exists.** `PowerTerm`'s doc records
  that the first is bit-pinnable and `powf(1.5)` is not; changing the form after slice D
  commits goldens would move every one of them.
- **`0.0 · x^-1` is `NaN`, and the constant term is special-cased to exactly zero.** With
  the floor at 0.01 mol/m³ the argument never reaches zero, so this guard is **not
  currently reachable** — stated rather than dressed up as a fixed bug. It is written this
  way because the floor is the only thing standing between the derivative and a `NaN`, and
  a future floor is not this file's to assume.
- **The node counts are recovered from the state, not stored.** `n_s = c_e.len() −
  c_neg.len() − c_pos.len()`, saturating, so a hand-edited snapshot yields a degenerate
  grid rather than a panicking underflow, and `advance` refuses a grid that does not
  describe the state it came from.
- **Neither adapter version moved**, checked against each constant's own doc rather than
  as a pair: `API_VERSION` bumps on *renames* and this is additive; `WASM_API_VERSION`
  bumps on the method surface and no method changed.
- **`EventFlags::SOLVE_UNCONVERGED` now means "the pack solve **or** a cell's own solve"**,
  and its doc says so in the same commit. No 18th telemetry field, per this document's own
  gate note.
- **`dfn::probe` is a `#[doc(hidden)]` module in the shipped crate**, and that is a real
  cost accepted for a reason. The analytic Jacobian is the one piece of this file that can
  be *silently* wrong — a bad entry costs Newton iterations and, past the damping, nothing
  else — so it needs a check reading the same two private functions the solve does. The
  alternative was a hand-built chemistry in a `sim-core` unit test, which would have
  checked the Jacobian against a parameter set nobody ships.
- **The scenario surface is tested, not asserted.** This plan claimed `CellModelConfig::Dfn`
  is reachable from scenario TOML — the claim the instrument's coverage of this phase rests
  on. `a_dfn_pack_is_selectable_from_a_scenario_file` runs it, and pins the four field
  names, which are a file-format contract the moment a scenario names them.

### Two things slice B leaves for slice D, named rather than left to be found

- **No DFN counterpart to `aging_grows_the_dc_resistance_of_the_shipped_spm_cell`.** The
  `eff_r0_factor` path is wired (it divides `m_ref` on both electrodes and multiplies
  `contact_resistance_ohm`, the same two places and the same argument as the SPM's, which
  matters because this chemistry's contact resistance is Chen2020's own **0**) but no DFN
  test configures aging, and `probe::jacobian_pair` hardcodes the factor to 1.0. So
  `CLAUDE.md`'s "never model capacity fade without resistance growth" is *implemented* for
  the DFN and **unverified** for it.
- **The 3C cliff margin is thin and points the wrong way.** The test asserts the DFN
  delivers under 0.8 of the SPM's amp-hours; the measured ratio is 0.69 on 10/5/10, and
  refining moves it *up* — about 0.72 at 30/15/30. A default-grid change in slice D can
  therefore fail that test without any physics having regressed.

### What the Jacobian test can and cannot say

Three kinks make the residual non-differentiable — the surface clamp, the `c_e` lookup
floor, and every OCP breakpoint — and at each one the analytic derivative takes a branch
while a central difference straddles it. So the comparison runs at five states chosen away
from the floor, with that distance **asserted rather than assumed** (a first attempt failed
on exactly this: a 40-step 3C state sat at `c_e = 0.05`, five times the floor). Worst
row-relative disagreement across them is ~1e-9 against a 1e-5 tolerance. A depleted state
is then checked for the weaker property that actually matters there: every Jacobian entry
finite.

## Slice C — pack integration

**Landed. `SNAPSHOT_VERSION` unmoved at v11.** `cargo test --workspace` green (407 tests),
`cargo clippy --workspace --all-targets -- -D warnings` clean.

What shipped: `CellModel::probe_at` replacing `source_at` **and** `terminal_v_at`,
`dfn::probe_at` (a real solve at the trial current) and `dfn::setup_for`, `spm::probe_at`
(with `spm::terminal_v` deleted and `source_at` reduced to a projection of it), the pack
loop's second tangent buffer and swap, a `debug_assert` pinning the precondition of the
deferred optimisation, and six tests in `crates/sim-data/tests/dfn_cell.rs` (13 → 19).

### What the slice actually buys, which is not what this plan predicted

The plan and the slice B note both frame the payoff as the parallel group's current split.
**It is not.** A stale line is stale mostly in its *intercept*, by about one step of
`dV/dt`, and inside a parallel group that staleness is common-mode: it moves every cell's
`E` by nearly the same amount and cancels out of the split. On a 5 %-scatter 1S2P pack at
1C it moves the split by ~30 µA on 2.6 A — **12 ppm**, rising with rate.

Where it is first-order is the two demands whose *current* is solved off the line:

| | before slice C | after |
| - | -------------- | ----- |
| `Power(50 W)`, first step | 23.3 A drawn, **87.0 W delivered** | 50.000000 W |
| `Power(50 W)`, settled | 75 mW off | ~1e-8 W |
| `Power(20 W)`, settled | 2.4–3.9 mW off | ~2e-9 W |
| `Voltage(3.60)` after a long 1C, first step | −4.64 A, holding **3.651 V** (51 mV off) | ≤1e-9 V |
| `Voltage(3.90)`, settled | 120–190 µV off | ≤1e-9 V |

The first rows are the seed resistance being asked to answer a demand; the settled rows are
the one-step staleness. Both now land on the pack solve's own `SOLVE_TOL_V`. This reaches
past the DFN: the browser client's CC-CV policy sizes its switching band on how far the
voltage solve lands from its target, and for this model that distance was three to four
orders larger than the equivalent circuit's 1.4e-4 V.

A 1S1P constant-current discharge is the degenerate case and is **unchanged to every
printed digit** (1C: 3484.0 s, 4.9872 A·h, min `c_e` 461.173; 3C: 726.0 s, 3.1177 A·h,
−0.197), because `solve_current` hands back the demand whatever the aggregate says. Only
the last bits of `i_k` move. `solve_iterations` goes 1 → **2** there, which is what
`the_pack_solve_re_linearizes_where_it_put_the_cell` now pins.

### The measurement that had to come first, and did

`SOLVE_TOL_V` is 1e-9 V while the cell's own Newton stops at `NEWTON_TOL = 1e-8` on a
row-scaled residual. If that termination error were *noise* in `i` above 1e-9 V, the pack
residual would floor above its tolerance, every step would cost 32 solves, and every step
would raise a false `SOLVE_UNCONVERGED` — a cost blowup wearing a physics problem's label.

Measured before writing the contract, by sweeping trial currents around the operating point
and subtracting the tangent the solve itself reports. The deviation falls as **exactly `h²`**
— 7.9e-8, 7.9e-10, 7.9e-12, 7.9e-14 at `h` = 1e-2 … 1e-5 — and then sits at ~4e-16 V, one
ULP of `V`. There is no noise floor: the inner solve's termination error is smooth in `i`,
and 1e-9 V is reachable. (The same sweep cross-checks the sensitivity tangent: `R_fd − R`
is ~1e-11 on `R = 0.0235 Ω` where curvature and cancellation are both small.)

### The dt = 0 arm, which was a landmine and was unguarded by the suite

`advance` refuses `dt <= 0`, so `solve` had never had to survive it — and this repo reads
instantaneous voltage with zero-length probe steps. `probe_at` therefore answers `dt <= 0`
with the stored line. **Removing that guard broke no test in the workspace**, which is why
`a_zero_length_probe_step_does_not_reach_the_solver` now exists.

What it prevents, measured: at `dt = 0` the mass rows' `(c − c_old)/dt` vanishes, the solve
returns the same voltage at every current, and a curve with `dV/di = 0` has its tangent
resistance clamped to the `1e-9 Ω` floor that exists to stop the pack dividing by zero. A
`Voltage(3.90)` probe then asks for **1.03e9 A** and runs the pack solve to its cap. With
the guard: 6.86 A, one pass, no flags. A *current* demand cannot catch this, because
`solve_current` returns the demand whatever the resistance is — the test needs a voltage
demand specifically.

### Cost: three solves where slice B had one, and an SPM regression it does not repay

Warm, alternating runs on the same box (the first pair was discarded — a build immediately
before a measurement inflated everything, exactly as the perf note warns):

| | slice B | slice C | |
| - | ------- | ------- | - |
| ECM 100S1P | 4.27–4.39 µs | 4.21–4.40 µs | unchanged |
| SPM 1S1P | 1.03–1.05 | 1.22–1.23 | **+17 %** |
| SPM 10S1P | 10.15–10.89 | 11.29–11.76 | +8–11 % |
| DFN 1S1P | 58.1–61.9 | 165–184 | **2.8–3.2×** |

The DFN factor is exactly the structure: two probes plus `advance`, where slice B ran one
solve. The equivalent circuit is untouched because the loop breaks on `is_linear` before any
probe — which is the check that caught the contaminated first run, since a 2× there could
only have been the box.

**The SPM regression is real and buys the SPM nothing.** `spm::source_at` already re-took
its tangent at the trial current, so the SPM's iteration was already correct; what it now
pays for is the merged call computing a tangent on the pass that turns out to converge,
where the old shape skipped it. That waste is not removable without reintroducing the
two-evaluation shape the DFN cannot afford: whether the tangent is needed is known only
after every cell's gap is in, and by then a second call is a second solve. Stated rather
than engineered around.

**The one duplicate that is removable is priced and deferred.** `advance` re-solves at
exactly the current the converged pass probed at — bit-for-bit, which `pack::step` now pins
with a `debug_assert` on `to_bits()`, so every test in the suite checks it. Consuming that
probe would take a DFN step from three solves to two (~33 %), and it needs a solved artifact
plus `StepSetup`'s particle affine maps threaded through `CellModel::advance`. That is a
slice, not a tidy-up. The assert is what makes the deferral *verified* rather than plausible,
and it is also why `dfn::probe_at` carries no `EventFlags` channel: the probe the step's
answer rests on is the identical solve `advance` raises `SOLVE_UNCONVERGED` for. Probes on
intermediate iterates are deliberately unreported, on the same reasoning that keeps the
pack's protection flags a per-pass binding rather than an accumulator.

### Protection now runs once per pass, and the clamp does not chatter

This slice created an exposure it did not set out to: `apply_protection` is called **inside**
the solve loop, so a DFN pack went from one call per step to two-to-five. A derate is a
*clamp*, which is not smooth, and a demand sitting on the limit could in principle land under
it on the stale-line pass and over it on the corrected one, cycling to `SOLVE_ITER_CAP` and
raising `SOLVE_UNCONVERGED` on a physically fine pack.

**Nothing in the repo covered it.** Every DFN test used `bms: None`, and no case in the
out-of-tree instrument pairs a nonlinear cell model with a BMS at all — the BMS cases are
equivalent-circuit, which breaks on `is_linear` before the loop body ever runs.

Swept at 0.90/0.98/0.999/1.0/1.001/1.02/1.10× the 1.5C discharge limit under both a current
and a power demand: **never unconverged, at most 4 passes**, worst exactly on the limit under
a power demand. The reason is structural rather than lucky, and is why it is worth writing
down: `apply_protection` builds its allowed window from the **sensor frame and the
chemistry**, never from `i_req`, so every pass applies the *same* clamp to the same interval,
and a projection onto a fixed interval cannot introduce an oscillation the unclamped map does
not have. The only `i_req`-dependent part is the `OC` flag, which the pack already treats as a
per-pass binding rather than an accumulator — the argument that comment makes now covers a
loop that really iterates. `a_derate_inside_the_iteration_does_not_chatter` pins it.

The contactor latch is the other per-pass mutation and it is idempotent: it is set from the
frame, not from the iterate, so N calls decide what one call would.

### Exit criterion 1: an exception, and it is 431 sign bits

76 lines moved against `after-sliceB-p7.txt`, and **every one of the 431 changed fields is
`7ff8000000000000` → `fff8000000000000`** — the sign bit of a quiet NaN. No finite value
moved in 1254 lines. All of them are in `lgm50_2s2p_spm_scatter_thermal_aging` from its
`CV 3.5 V` leg onward: the region `ANCHORS.md` already records as garbage in the anchor
itself.

The cause was **bisected, not argued**. With the SPM arm alone reverted to the old two-call
shape, the instrument is byte-identical to the slice B anchor. So it is exactly
`spm::probe_at` evaluating `voltage(&w, s, i)` once where two functions each evaluated it —
the same value by purity, and no guarantee about which NaN bit pattern equivalent code
produces. The merge was kept: un-merging to preserve the sign of a NaN would be writing
deliberately redundant code, and the merge is the slice's contract.

**And the snapshot hashes agreeing is not corroboration here.** They did not move on any
case — but `serde_json` spells every non-finite float `null`, so an FNV over the snapshot
JSON cannot tell `+NaN` from `−NaN`. That is a *fourth* blind spot of the instrument, and it
is recorded in `ANCHORS.md` beside the other three. Anchor is now `after-sliceC-p7.txt`.

### Built to fail: 5 of 5, and one test that exists only because of it

Removing the mechanism (making `probe_at` answer with the stored line at every `dt`) fails
all four of the demand/topology tests and nothing else in the workspace — no pre-existing
test covered the mechanism, which is expected, and better than Phase 6's two-of-four.
Removing only the `dt <= 0` guard failed **nothing**, and that is where the fifth test came
from. A perturbation harness that had only run the first perturbation would have reported a
clean sweep over a hole. The sixth test came from a question the perturbations could not ask
at all — what the slice changed *for other components* — which is the derate note above.

### What slice D inherits

- The 3C cliff assertion still passes and its margin is **unmoved**: the constant-current
  trajectory did not change, so the 0.69-against-0.8 thinness slice B recorded is exactly as
  it was, and refining the grid still moves it the wrong way.
- The DFN perf budget slice D must state is **≈160 µs per cell per step at 10/5/10 with
  `N_r = 10`, on a 2-pass 1S1P pack** — and it must be quoted per cell, because the pass count
  is not a constant: a scattered parallel group needs 3 where a single cell needs 2. Measured
  1S1P 160.5, 2S1P 305.2, 1S2P 314.1, 2S2P 687.2 µs, i.e. 155–172 µs/cell and roughly linear,
  so a 10S10P DFN is ~16 ms/step. That is ~135× an `Spm` N=20 step in the same process, not
  slice B's 50–65×; both that factor and the ~40 µs projection earlier in this document are
  superseded.
- The aging-vs-resistance-growth gap slice B named is untouched: still implemented and still
  unverified for this model.

## Environment

PyBaMM is installed at `M:\claud_projects\temp\pybamm-venv` **and**
`M:\claud_projects\temp\phase6-pybamm\.venv`; both were checked this time and both run
**26.6.2.0**, the pinned version. Do not create a third.

`t_interp`, never `t_eval` resampling. `sim.solve(t_eval=[t0, t1])` asks for an interval
and returns the solver's own adaptive steps; resampling those onto a uniform grid draws a
chord across each step and cost Phase 6 a 346 mV artefact that read as a converged physics
disagreement.

See [`phase-6-porous-electrodes.md`](phase-6-porous-electrodes.md) for the phase this one
completes.
