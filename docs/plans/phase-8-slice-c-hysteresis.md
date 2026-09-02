# Phase 8, slice C — the hysteresis state, the OCV temperature correction, and NiMH

**Status: LANDED 2026-08-28. `SNAPSHOT_VERSION` 17 → 18** — the phase's one budgeted bump,
carrying two mechanisms. `WASM_API_VERSION` stays at 6 and `sim_server::API_VERSION` at 2;
no call signature, telemetry field or lesson prose changed, and all three of those belong to
slice D.

This slice was authored against `docs/plans/phase-8-slice-c-spike.md`, which ran first on the
Phase 6 and Phase 7 discipline of spiking before designing. The spike's recommendation is
taken whole, including its cut.

## What landed

| | what | where |
| - | ---- | ----- |
| 1 | **`[hysteresis]`** — a per-cell memory of drive direction that does **not** decay at rest | `chem.rs::HysteresisParams`, `ecm.rs::hysteresis_update`, one field on `EcmState` |
| 2 | **`ocv.t_ref_k`** — the OCV table's reference temperature, which switches on `OCV(soc) + ∂U/∂T·(T − T_ref)` | `chem.rs::OcvTable`, `ecm.rs::open_circuit_v` |
| 3 | **`chemistries/nimh_subc_3ah_generic.toml`** — a 3.0 Ah sub-C cell, the first shipped file to use either | — |
| 4 | The refused-charge heat is read **per cell** rather than hoisted | `CellModel::rejection_ocv_v` |
| — | **Cut, per the spike: the charge-acceptance taper.** | — |

## The two terms land in different functions, and that is the design

This is the thing most likely to read as an inconsistency later, so it is argued here as
well as in both doc comments.

* **The temperature correction goes in `open_circuit_v`.** It *moves the equilibrium
  potential* — that is what an entropy coefficient is — and its energy already has a
  matching channel on the thermal side, the reversible term in `cell_heat_w`, which reads
  the same `∂U/∂T`. Adding it to the source without touching the overpotential is what keeps
  that pair matched.
* **Hysteresis goes in `ecm_overpotential_v`.** It is *dissipative*: the area enclosed by
  the loop is energy the cell does not give back, and it has no other channel. That function
  is the single expression feeding the solve, `CellModel::heat_w` and
  `CellModel::overpotential_v` — its own doc says that is so the three "cannot disagree about
  what the cell is losing" — so routing the term through it makes the heat *follow* from the
  voltage rather than being a second site that has to remember. The diffusion term is in
  there for exactly this reason.

The sign table, which was checked before the code was written and is pinned by
`the_loop_costs_heat_in_both_directions`:

| | `h` settles at | overpotential term `−M·h` | source `OCV − η` | `q_irrev = I·(…)` |
| - | - | - | - | - |
| discharge (`I > 0`) | `−1` | `+M` | `OCV − M` | `+M·I` > 0 |
| charge (`I < 0`) | `+1` | `−M` | `OCV + M` | `−M·I` > 0 |
| rest (`I = 0`) | held | held | displaced | `0` |

Had hysteresis been added to `open_circuit_v` instead, the voltage would have moved and the
heat would not, and the pack energy balance would have quietly stopped closing on the one
chemistry that uses the section — with every existing energy test still green, because none
of them loads a chemistry that has it.

## Exit criterion 3 is structural on both halves

`phase-8-chemistries.md` names slice C as "the place this criterion is actually at risk".
It is not, and the reason is the same one `[diffusion]` used:

* `ecm_overpotential_v` and `advance_cell` **match on the `Option`** and never execute a
  line of the hysteresis term when it is `None`.
* `open_circuit_v` **matches on `t_ref_k`** and returns the bare table lookup when it is
  `None`, rather than adding `0.0 · (T − T)`. That would in fact be bit-identical for finite
  values, but only by an argument about signed zeroes that a future edit could invalidate
  silently.

So no chemistry without the sections can move by a ULP, and that is by construction rather
than by measurement. The claim is nonetheless written down as a test —
`no_chemistry_but_nimh_carries_the_v18_sections` — because the construction only protects
files that *don't* declare the sections, and nothing but that test stops a future edit
adding one to a shipped file.

The gate for the temperature correction is **`t_ref_k`, not the coefficient column**, and
that choice is load-bearing. `docv_dt_v_per_k` has existed since Phase 2 and is supplied for
*heat*, where no reference temperature is needed because the entropic term reads the cell's
absolute temperature. Gating on the coefficient alone would hand a voltage shift measured
from an unstated origin to any file that had added the column for the other reason — a
fabricated constant arriving by omission, which is the shape the provenance rule exists to
refuse. `a_coefficient_without_a_reference_temperature_is_accepted` pins that the pre-v18
configuration stays exactly pre-v18.

## What the shipped NiMH file measures

Numbers from `crates/sim-data/tests/nimh_chemistry.rs`, 1S1P at 1 C, from `soc = 0.10`,
ambient 298.15 K, no BMS, no aging.

### The falling end-of-charge voltage

| | |
| - | - |
| clamp | t = 3240.1 s |
| peak | 1.517831 V at 299.02 K |
| **fall at +10 K** | **8.780 mV**, 136 s after the peak |
| fall at +20 K | 17.351 mV, 284 s after |
| fall at end of run | 42.435 mV at 366.6 K |
| **isothermal control** | **0.000000000 mV**, and nothing after the peak moves at all |

**8.78 mV at the +10 K mark, inside the 5–10 mV window a charger terminates on.** The
instant is the assertion, not an incidental: the spike recorded scoring a pre-registered
prediction green over a twenty-minute run where the honest figure at the instant a charger
fires was under half of it. That test now pins the +10 K number and requires it to arrive
within ten minutes of the peak.

Split between the two channels, at that instant:

| channel | |
| ------- | - |
| OCV temperature correction | 3.501 mV (40 %) |
| `R0(T)` | 5.279 mV (60 %) |

Roughly half each, which is the split the spike predicted the term would buy and the reason
it was built. A test refuses either channel carrying more than three quarters.

### The measurement that had to be redesigned, and why it is worth recording

The first version of the test charged from `soc = 0.90` and measured **4.395 mV** at +10 K
against an expected ~5.4 mV from the ohmic channel alone. The missing millivolts were not a
parameter error and no amount of refitting would have found them:

* at 1 C a charge from 0.90 lasts 360 s, which is **barely one time constant** of this
  cell's slow RC pair, so that pair was still filling through the overcharge and **lifting**
  the terminal voltage by ≈ 3.3 mV while temperature pulled it down;
* the hysteresis state was still crossing for the same reason and added ≈ 1.3 mV more of
  lift.

Both are real physics correctly modelled; the *experiment* was wrong. A charge from 0.10
lasts 3240 s — eleven time constants — so both have settled before the cell fills, and what
happens after the peak is temperature and nothing else. That is also the realistic
experiment, since a charger starts from an empty pack.

**The general lesson is one this repo keeps re-learning in new clothes: a measurement taken
before the transients it is not about have settled reports their sum.** The isothermal
control arm did *not* catch this — it was green in both versions, because pinning
temperature removes the fall entirely and says nothing about what else is moving. What
caught it was a **pre-computed expectation** for the ohmic channel that the measurement
missed by a factor of six.

### The resting-voltage memory

Two arms arrive at `soc = 0.600000000` from opposite directions and rest four hours:

| | |
| - | - |
| arrived by charging | 1.294986173 V |
| arrived by discharging | 1.245013827 V |
| gap | **49.9723 mV**, against a declared loop width of 50.0000 mV |
| four hours later | **the same bits**, both arms |
| NMC control, same experiment | gap below 1 nV |

The gap is asserted against `2·scale_v·(1 − e^(−γ·0.30))` — derived from the two declared
parameters rather than pinned as a measured number, so editing either moves the expectation
with it.

The bit-identity across the second four hours is the load-bearing assertion, and the rest
length is chosen for it: both RC pairs have time constants of 300 s or less, so by the first
reading `exp(−14400/300)` has taken their residue to about 1e-21 V — below the last bit of a
1.3 V number. What is left moving between hour four and hour eight is nothing at all, and
**that is the property that made this a new state rather than a third `[[rc]]` entry.**

## The perturbation table, and the test it produced

The structural claims above are by construction. The claim that needs an experiment is that
the new code is **reached**, and reached where the design says it is. Five perturbations,
each breaking one thing and each reverted; what is recorded is *which assertion* reddened,
because an exit code alone has been the wrong signal in this repo before.

| # | broken | reddened |
| - | ------ | -------- |
| 1 | the hysteresis state is never advanced | 3 engine tests + the shipped-file memory test |
| 2 | **hysteresis moved into `open_circuit_v`** | **only `the_loop_costs_heat_in_both_directions`** |
| 3 | the correction gated on the column instead of `t_ref_k` | the gate test + the refused-charge test |
| 4 | the hysteresis sign reversed | 4 engine tests + 2 shipped-file tests |
| 5 | the refused-charge OCV hoisted back to the table | only the refused-charge test |

**Case 2 is the one worth the table.** Moving the term into `open_circuit_v` leaves *every
voltage claim in the slice green* — the cell still rests where it should, the loop is still
the right width, the memory still survives a rest — and reddens exactly one test, the heat
one. That is the invariant the routing decision was made for, and it is now the only thing
standing between this design and the quieter one.

### Case 5 was green on the first run, and the fix was a test

Nothing in the tree noticed the refused-charge endpoint being read from the chemistry's
table instead of from the cell. `the_refused_charge_is_booked_at_the_corrected_open_circuit_voltage`
was written to close that, and **its first version asserted the wrong quantity** — which is
the more useful half of the story:

* the draft expected the difference to be `2·M·|I|`, counting hysteresis through both the
  overpotential and the rejection heat;
* it measured `M·|I|`, and the measurement was right. The displacement lives in
  `ecm_overpotential_v`, so it *already* reaches the heat through `q_irrev = I·(OCV − V)`,
  and `open_circuit_v` at `soc = 1.0` with no `t_ref_k` **is** the bare table lookup, bit for
  bit. Booking the rejection at the hysteretic source would have counted the loop twice.

So the per-cell read matters for **the temperature correction alone**, and the test is
written on that: two arms both carrying the entropy column, differing only in whether the
reference temperature is stated, held away from it. The whole difference in generated heat
is the rejection term, `∂U/∂T·(T − T_ref)·|I|` — measured **−0.040000000 W** against a
predicted −0.040000000 W, and zero with the hoisted lookup restored.

A useful check fell out of writing it: at full clamp nothing is stored, so **all electrical
power must arrive as heat**. Both arms satisfy `q_gen = V_terminal·|I|` exactly, which is
what says the rejection term and the overpotential term are not double-counting each other.

## What the BMS sees, which is deliberately nothing

`ocv_invert` — the estimator's rested-OCV correction — inverts the raw `volts` column and
knows nothing about `h`. It was left alone on purpose.

Design principle 8 says the BMS consumes sensor readings and maintains its own estimate, and
that the gap between truth and estimate is a feature to expose. A real BMS inverting a curve
it only ever measured one direction of is exactly how a hysteretic chemistry fools one: on
this cell the correction is biased by up to 25 mV, in whichever direction the pack was last
driven, which on the flat part of the NiMH plateau is worth a large slice of state of
charge. That is a free contrast for slice D beside the LFP estimator-drift lesson, and it
costs nothing to have.

## Two decisions recorded rather than left in a transcript

### The shipped lead-acid cell does not get a `[hysteresis]` section

`lead-acid-data-only.md` and `diffusion-overpotential.md` both scoped this mechanism with
lead-acid resting-voltage memory in mind, and the section would serve it. It is still not
added, for two reasons that point the same way:

1. **No fitted lead-acid hysteresis constant exists.** Adding one would ship the unlabelled
   number the provenance rule forbids outright.
2. It would move every shipped lead-acid trajectory, putting exit criterion 3 in play for a
   number nobody measured.

The *capability* lands here. Adding the data later is a change to one file and no Rust,
which is the whole point of the `Option` being chemistry data.

### The charge-acceptance taper stays cut

*Superseded 2026-09-02: built as `docs/plans/charge-acceptance.md`, after the phase closed.
The paragraph below is what this slice decided and is kept as written.*

The spike measured that this cell's voltage peak turns a **one-timestep corner** — the slope
reverses 29-fold in one 0.1 s step — because a hard SOC clamp takes charge acceptance from
100 % to 0 % instantly. A real NiMH peak is rounded over tens of seconds. No parameter fixes
that; it needs a third mechanism.

It is cut, and slice D's lesson must therefore be written about the **number** (a −ΔV in
millivolts, on a telemetry row, against an isothermal control arm) rather than the **shape**
(a peak, on a plot). That is the same choice `phase-8-slice-b-lto-client.md` recorded: *"a
shape on a plot is weaker than a number on a row."* If a future session wants the rounded
peak, it is a slice against the recipe in the phase plan's stopping rule, not a reopening.

## What this cost

* **8 bytes per ECM cell.** `EcmState` 48 → 56, `CellModel` 56 → 64, `Cell` 184 → 192 —
  paid by every ECM cell including those whose chemistry has no `[hysteresis]` section,
  which is the standing cost of `EcmState` being one struct rather than a variant per
  feature. It buys the `Option` being a *path* instead of a multiplier, which is what makes
  the bit-identity claim structural; the alternative trades the 8 bytes for a per-cell branch
  on the solve's hot path and a second serde shape. `cell_footprint` is updated with the new
  arithmetic, which is what that test exists for.
* **One branch per cell per step** in `advance_cell`, and one in each of
  `ecm_overpotential_v` and `open_circuit_v`. Not benchmarked: `Pack::step` sits at
  ≈ 42–54 µs against a < 50 µs budget and the suite runs below normal priority, which makes
  the bench unfair in the first place. Recorded rather than measured, deliberately.
* **The refused-charge OCV stops being hoistable.** `Pack::step` used to read
  `ocv_lookup(chem.ocv, 1.0)` once per step for every cell, because the endpoint a refused
  charge is pushed against was a property of the chemistry alone. It is now a property of the
  *cell* — hysteresis displaces the source by up to `scale_v`, and a cell being force-fed at
  the top of its window is displaced the most — so it moved to `CellModel::rejection_ocv_v`,
  called only from the clamp arm. **The cost moves rather than growing**: a pack that never
  clamps no longer pays a lookup per step, and one that does pays a lookup per *rejecting
  cell* instead of amortising one across the pack. Bit-identical for every chemistry with
  neither section, because the clamp puts `soc` at exactly 1.0 with a zero deficit, which is
  `open_circuit_v`'s early return.

## What the NiMH file deliberately does not have

Three absences, each a decision rather than an omission, and each asserted in `load.rs` so a
future edit has to argue with a test rather than with a comment:

* **No `[aging]`.** `ChemistryParams::aging` documents `None` as "this parameter set cannot
  say how this cell ages", which is the truthful statement — and configuring `AgingConfig`
  against it is a build error rather than a silently ageless pack, which is the behaviour
  wanted.
* **No `[safety]`.** NiMH has no lithium-style self-sustaining decomposition exotherm to
  parameterise, and the heat that gets an abused cell to venting is the *overcharge* heat
  this file's whole lesson is about, which is emergent from `i_rejected_a` and needs no
  `[safety]` at all.
* **No `[diffusion]`.** NiMH does lose capacity at rate, but nobody here has fitted a
  Peukert exponent for it, and the lead-acid section exists because that fit *was* done.

**A finding against slice A, from the middle one.** `phase-8-slice-a-lto.md` recorded that
`[safety]` "has no way to say a cell does not plate", because zeroing the cost fields still
raises the flag and dropping the section switches off runaway too — so the LTO file ships a
labelled sentinel `t_plating_min_k = 1.0`. A **non-lithium** chemistry can say it cleanly, by
omission, because it does not want the other half either. So the schema gap slice A found is
**narrower than it looked**: it binds on lithium chemistries that do not plate, not on every
chemistry that does not plate. That did not close it — LTO was still wearing the sentinel;
`docs/plans/plating-absence.md` closed it afterwards by making the gate an optional pair —
but it bounds it, and it is the kind of thing that is invisible until a second file walks
into the same section from the other side.

## What is not settled

* **The parameters are order-of-magnitude, and one of them is not sourced at all.**
  `hysteresis.gamma` is a labelled placeholder: it describes how *fast* the loop is crossed,
  which needs a partial-cycle measurement that datasheets do not carry. `scale_v` is half a
  reported charge/discharge separation. `docv_dt_v_per_k` is **derived** rather than quoted —
  sized by a thermal-balance argument against the observable that a NiMH cell on a 1 C charge
  stays roughly flat in temperature through the plateau — and it therefore depends on this
  file's own `R0` and RC values. Refit the overpotentials and it must move with them. All
  three say so at the field.
* **The RC split is the weakest thing in the file.** The *sum* of the resistances is
  constrained by the charge-plateau voltage; the division between the two pairs, and hence
  the two time constants, is constrained by nothing.
* **The hysteresis loop is one number across the whole SOC range.** Real loops narrow at the
  ends. Making `scale_v` a table is a schema change and was not needed for anything measured
  here.
* **The charge direction of `[diffusion]` is still unvalidated**, unchanged by this slice and
  noted only because the two sections now sit beside each other.

## Next

Slice D: teach NiMH. It carries exit criterion 4, and the spike plus this slice have already
decided its shape — a −ΔV in millivolts on a telemetry row with an isothermal control arm
beside it, and the estimator bias above as a second contrast if it is wanted. That closes the
phase.
