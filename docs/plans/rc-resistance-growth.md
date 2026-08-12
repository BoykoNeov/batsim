# Aging grows the RC resistances too — the spec was right and the code was not

**Status:** landed. Owner decision taken: **change the code**, not the spec sentence.

`CLAUDE.md`'s physics spec has said, since Phase 0:

> Health applies as multipliers: effective capacity = nominal × `soh_capacity`;
> effective R0 **and RC resistances** = nominal × `soh_resistance`.

The code has only ever grown `R0`. `docs/plans/reversal-damage-ui.md` found this while
working out a 3.2 mV sag ("Noticed, not fixed"), recorded it as an owner's call, and
changed nothing. This is that call, resolved the other way: the spec sentence stays, and
the equivalent circuit's RC pairs start carrying aging's resistance growth.

## What was actually wrong, and how far it reached

The gap is one expression. `chem.rc[k]` is read in **exactly one place** in the whole
workspace — `ecm::advance_cell`, the exact-exponential RC update — and it read
`pair.r_ohms` unscaled:

```rust
*v_rc = rc_update(*v_rc, i, pair.r_ohms, pair.c_farad, dt);
```

Everything downstream follows from the `v_rc` that line produces, so there is no second
site to fix and no risk of the two halves disagreeing:

* **Voltage.** `cell_source` builds `e = OCV − Σ v_rc`, so terminal voltage inherits it.
* **Heat.** `cell_heat_w` is `i·(i·r0 + Σ v_rc)`. This is the half `reversal-damage-ui.md`
  did *not* check, and it was checked before this slice was scoped: it reads the same
  state, so an aged cell was under-reporting its sag **and** its heat, by the same factor,
  for the same single reason. The code was at least self-consistent — the spec sentence
  was the outlier, not one of two disagreeing code paths.

Measured on the previous slice's fixture: RC overpotential `0.020000 V` = `2 A × 0.010 Ω`
exactly, in both an aged and an unaged arm whose terminal voltages differed. That
measurement is what named the defect, and it is what the new test inverts.

## The decision, and the two options refused

**Scale by `soh_resistance` alone.** Not by `eff_r0_factor` (= `r0_factor ×
soh_resistance`), which is the multiplier already threaded into `CellModel::advance` and
therefore the tempting free option.

* **Refused: fold in the static factor too.** `r0_factor` is manufacturing scatter and the
  `WeakCell` fault, and both are named for `R0` in the *public* config —
  `Scatter::r0_sigma`, `Fault::WeakCell { r0_factor, .. }`. Making them move the RC pairs
  is a second modelling change nobody asked for, and it is not what the spec sentence says:
  that sentence sits under *"Health applies as multipliers"*. It also has real blast
  radius — `calendar_fade_hot.toml`, `cc_cv_charge_pack.toml`, both external-short
  scenarios and `soft_short_under_a_lying_sensor.toml` all set `r0_sigma = 0.03`, so this
  option moves **shipped fresh-pack scenarios** while the chosen one cannot.
* **Refused: hold τ fixed by shrinking `c_farad`.** The spec says *resistances*, and says
  nothing about capacitance. Scaling `R` and leaving `C` lets `τ = R·C` grow with age,
  which is a consequence of the sentence rather than a second physical claim. An aged cell
  therefore relaxes *slower* as well as further, and the new test pins that rather than
  leaving it to prose.

The multiply happens at the call site, not inside `rc_update`: that is a `pub` pure
function with its own direct tests, and its signature is worth leaving alone.

**Plumbing: `soh_resistance` becomes its own parameter of `CellModel::advance`, beside the
`eff_r0_factor` that stays exactly as it is.** This looks like a new asymmetry and is the
removal of an old one — `advance` already takes `eff_capacity_ah` (static factors only)
*plus* `soh_capacity` separately, for the reason its doc gives: SOC has to keep meaning
"fraction of the capacity this cell has today". Resistance was the field that deviated from
that shape by arriving pre-multiplied. Now both arrive split, and each arm takes the
combination it wants. The DFN arm keeps consuming the product and is untouched.

## The prediction, written before the suite ran

`soh_resistance` is **exactly** `1.0` until fade accumulates — `CellAging::default` seeds
it at `1.0` and `1.0 + k·0.0` is `1.0` — and `x * 1.0` is bit-identical for every finite
`x`. So:

> **Every test that does not enable aging must be bit-identical**, including all PyBaMM
> goldens, the analytic golden, `scatter.rs`, `thevenin_cache.rs`, and every SPM/DFN test.
> Only ECM tests that enable aging *and* accumulate fade may move.

This retires a stale claim rather than merely passing: `reversal-damage-ui.md` said
changing the code "would move every golden". That was never true, and the reason it is not
true is checkable ahead of the run. **If anything outside the aged-ECM set moves, the
change is wrong** — most likely by reaching `eff_r0_factor` or by touching `cell_source` —
rather than the suite reporting an expected trajectory shift.

Result: **the prediction held exactly.** The full workspace suite is green with no
tolerance changed, no expectation edited, and no golden regenerated. Not one aged-ECM
assertion moved either — `aging.rs`, `scenario_aging.rs`, `reversal_damage.rs` and the
aging property tests all pass unmodified, because each of them asserts on capacity, on
health factors, or on conservation identities that a larger RC resistance does not break.
So the observable blast radius of this change is exactly the new test, and the previous
slice's "would move every golden" over-priced it by the whole suite.

## The test

`aging.rs::aging_grows_the_rc_resistance_of_an_ecm_cell`, built at the **configuration**
level — a pack with `aging: Some(..)` and `cell_model: Ecm` — rather than around the
internal parameter name. That is the lesson from `dfn-aging-gap.md`, where grepping the
internal name mis-scoped the slice, because a test that exercises this never mentions it.

It asserts the RC overpotential, not the terminal voltage: terminal voltage moves for `R0`
reasons anyway, so an assertion on `V` alone would pass even if the change had not landed.

The fixture is arranged so that the growth factor the step used is *provable* rather than
assumed:

1. **Age at rest** for 60 days of sim time (60 steps of 86 400 s) at 318.15 K and full
   charge. Rest means no cycle fade and — the part that matters — an RC overpotential still
   exactly `0.0` when the measurement starts.
2. **Read `s = cell.soh_resistance`: 1.422175.** That is a big number and it is not a claim
   about a cell. `aging.rs`'s fixture uses coefficients its own header calls "deliberately
   faster than anything shipped", so 60 days costs it ≈ 28 % of capacity; at
   `r_growth_per_capacity_loss = 1.5` that is a 42 % resistance rise. The shipped LFP
   scenario's 7.26 % is a different fixture and a different exposure — nothing here should
   be read against it.
3. **One 60-second step at 2 A.** The sub-clock period is 3 600 s and the ageing phase left
   the accumulator at exactly zero (86 400 is 24 × 3 600), so this step **cannot tick** and
   `s` is provably what it ran with — asserted by comparing `soh_resistance`'s *bits* across
   the step, not assumed. 60 s is three time constants of the unworn pair (`0.010 Ω ×
   2000 F = 20 s`): long enough that the resistance dominates the answer, short enough that
   the lengthened time constant is still visible in it.
4. **Compare against `r·i·(1 − exp(−dt/(r·c)))` with `r = 0.01·s`**, evaluated in the order
   `rc_update` evaluates it, to a relative `1e-12`. Not bit-identical — that was the
   intention, and the 4 ULP it missed by are a fact about the pack solve rather than about
   this change; see "Learned while building".

Two guards bracket the comparison, and the second is the one that does unusual work. The
measured `0.024993154 V` against the unworn closed form's `0.019004259 V` is a ratio of
**1.3151**, which must be **greater than 1** (the change landed) and **strictly less than
`s` = 1.4222**. That second bound is how `τ`'s growth is pinned by an assertion instead of
by prose: the capacitance was left alone, so the time constant lengthened with the
resistance and the aged pair is *further from its own steady state* after the same 60 s.
Had `τ` been held fixed, the ratio would be exactly `s`.

## The client half: one lesson step was quoting a number this change moved

The suite was green and the guided path was **wrong**, which is this repo's most familiar
failure and the reason the tutorial-numbers gap is still the top open item. Step 21 of the
browser path asks the reader to check a piece of arithmetic at the terminals, and that
arithmetic was `I·R0·(s − 1)` — true before this slice, and short by the RC half after it.

Re-measured on the engine, driving `scenarios/over_discharge_damage_lfp.toml` at the
step's own `dt = 0.5` and 2 A to its own 600 s mark:

| quantity | before | after |
| -------- | ------ | ----- |
| `terminal` at the mark | −0.067 V | **−0.069 V** |
| extra sag vs an unworn cell | 3.2 mV (`R0` only) | **4.6 mV** = 3.2 (`R0`) + 1.5 (RC) |
| `terminal` at 300 s | −0.065 V | −0.065 V (unchanged) |

**Everything else in that step held**, which is worth stating as precisely as the changes:
empty at 207.5 s and 1.9306 V, through zero at 287.5 s, `soh cap` 99.45 / 98.84 / 95.16 %
at 250 / 300 / 600 s, `soh res` 1.0726 ×, 9.704 points past empty, 383.0 s to repay,
0.2128 against 0.2182 A·h. None of those is a voltage under load, which is exactly why —
this change moves terminal voltages on aged cells and nothing else.

The rewritten prose is a better lesson than the one it replaces, and that is the argument
for spending the edit rather than just correcting a digit: the old text attributed the
whole drift to `R0` because that was all there was, and the new one has to name both
resistances and their sum, `2 A × 0.032 Ω × 0.0726`. It also gets to say why the slow half
arrives late — the time constant grew too — which is the second half of the decision above,
stated where a reader meets it. The lag is about a tenth of a millivolt, so the step says
it is below what the row can print rather than quoting an unreadable number: the recurring
defect in this path is a number that is *true and unreachable*, not a number that is wrong.

**Four other scenarios enable aging and none of them needed a word changed.**
`calendar_fade_hot` runs at rest, so its RC pairs are empty and its "everything else stays
still" claim survives; the two external-short files and `soft_short_under_a_lying_sensor`
accrue about a millionth of a point of fade before their fault fires, which cannot reach
the fourth digit of a 183.84 A spike.

**Measured engine-side rather than through the browser**, and the conditions are why that
is enough here: the step pins `dt = 0.5`, `Current(2)` and `until_s = 600`, the harness fed
exactly those, and the readout row is a direct three-decimal print of the telemetry frame.
The changed quantity moved by 3.5 mV — two orders above that row's last digit — so there is
no throttling or decimation question to answer. `web/pkg` was rebuilt, which is the part
no version constant could have caught.

## Learned while building

### A 1S1P pack does not hand its cell the current you demanded

The test was written to assert **bit-identical** equality against the closed form, and it
failed by 4 ULP. The formula was right; the assumption underneath it was not. Even on a
one-cell pack the solve does not pass the demand through — it aggregates a Thévenin
source, computes a node voltage, and hands the cell `(E − V)/R` back. That round trip is
`2.0 A` to within a few ULP, and the RC update inherits the difference.

Worth recording because the *tempting* diagnosis was the one that would have been wrong:
"the step must have aged the cell mid-measurement." It had not, and the test proves so
independently — it compares `soh_resistance`'s bits across the measurement step. The
tolerance in the test is therefore about the **current**, not about aging or about the
exponential, and it is annotated as such at ~1e4 ULP: four orders of magnitude tighter
than the ~24 % error the defect itself produces.

Transferable: on this engine, "1S1P" buys a simple *topology*, not an exact *current*. Any
future test that wants a bit-exact per-cell current has to read it back rather than assume
the demand reached the cell — and no accessor exposes one today.

### The test was run against the unfixed code, not merely observed to pass

The multiply was reverted, the test re-run, and it failed with `rel = 2.396e-1` — the aged
cell reporting exactly the unworn `0.019004 V`. Then restored. A green assertion proves
the code and the test agree; only the red one proves the test can *see* the defect, and
this repo has shipped a perturbation harness that reported five greens that were all lies
(`docs/plans/surface-vs-bulk.md`). One deliberate red is cheap.

### The eighth argument, and why it is not a struct

`CellModel::advance` now takes eight arguments and trips `clippy::too_many_arguments`. It
carries a targeted `#[allow]` with the reason rather than bundling the multipliers into a
struct: which factors arrive *combined* and which arrive *split* is precisely what this
signature exists to say, each arm takes a different combination on purpose, and a struct
would move that decision out of the compiler's reach at every call site.

## Versions

**Nothing moves, and the argument is precedent rather than preference.**

* **`SNAPSHOT_VERSION` stays at 15.** No new state, no layout change: `v_rc` is the same
  `Vec<f64>` carrying the same volts. The temptation to bump comes from the constant's own
  doc — *"the version field's job is to guard future semantic changes to an unchanged
  layout"* — and an aged v15 blob written by the old binary does continue on a different
  trajectory here. But `energy-hole.md` settled exactly this case: it changed overcharge
  heat by a factor of 41, added no state, and did not bump. The practised rule is "bump on
  layout or on state that would restore dishonestly", not "bump on any trajectory change",
  and one slice does not get to redefine it.
* **`sim_server::API_VERSION` and `sim_wasm::WASM_API_VERSION` stay.** No wire field is
  added, removed, renamed or re-meant. Every telemetry field means what it meant.
* **`web/pkg` was rebuilt, and that is the check that matters.** The bundle embeds the
  engine, so an aged pack in the browser keeps the old dynamics until it is rebuilt — and
  because no version constant moves, the page's own compatibility check *cannot see* a
  stale bundle. This is the second time in three slices that the rebuild, not a constant,
  is the thing to verify. It is `.gitignore`d, so nothing about it lands in the commit and
  every other working copy has to rebuild it too; the prose in step 21 will read as wrong
  on any machine that does not.

## Deferred, with a price

* **The porous-electrode models are untouched, and their coupling is different by
  construction.** The SPM applies `soh_resistance` to its contact resistance and — by an
  explicit Phase 6 decision recorded in `spm.rs` — deliberately *not* to its
  charge-transfer resistance, so that the reported factor stays exactly the factor the
  telemetry says it is. The DFN inherits that. Nothing here re-opens it; the spec sentence
  being repaired is about `R0` and RC pairs, which is equivalent-circuit vocabulary.
* **The growth coefficient is still shared.** One `r_growth_per_capacity_loss` per
  chemistry now drives `R0` and the RC pairs together, so this slice makes the *slow*
  resistance grow at exactly the rate the *instant* one does. Real cells do not oblige;
  splitting the coefficient is a chemistry-file change and a fit, and the placeholder
  provenance note is unchanged by this slice.
* **`Telemetry::soh_resistance` is still an `R0` ratio** — "the pack's present series
  resistance over what unworn cells in the same topology would present" — and stays one.
  It needs no change and does not move: it is a ratio of the instantaneous Thévenin
  resistances the solve uses, and the RC pairs were never in it. On a pack with mixed
  aging it is now, strictly, a summary of one of two coupled quantities rather than of
  the only one. Recorded, not fixed.
