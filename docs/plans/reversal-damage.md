# Over-discharge that leaves a mark

`docs/plans/low-clamp-reversal.md` closed the engine's last conservation hole: a cell
driven past empty stops sourcing at `OCV(0)` forever and instead falls through the
`[reversal]` ramp, carrying what it delivered as `soc_deficit` and repaying it on the way
back up. The books balance exactly.

**Too exactly.** A pack can be pumped below empty and charged back and it returns to the
state it started in — same capacity, same resistance, no record. That plan named the gap in
its own deferred list, and `docs/plans/reversal-ui.md` named it again after showing the
deficit in the client:

> **Over-discharge is still free.** Repayment is exact and the cell is unharmed; real
> reversal dissolves the anode current collector.

This slice makes it cost something.

## What actually happens in a reversed cell

Past empty the anode has no lithium left to give, so the cell finds the next thing it can
oxidise: the copper current collector the anode is coated on. Copper dissolves into the
electrolyte and re-plates elsewhere on the next charge — in the separator, on the cathode,
wherever the field takes it. Two consequences, and neither of them reverses:

* **Lost inventory and lost contact.** Active material that is no longer connected to a
  current collector is not part of the cell any more. Capacity is gone.
* **Higher impedance.** The re-plated copper is not where it was, and the contact it used
  to make is not remade.

So over-discharge belongs in the same family as lithium plating ([`crate::plating`]): a
mechanism that is not about how much charge moved, but about *the conditions it moved
under*. That is exactly the shape [`crate::aging`] already has a slot for.

## The design, in one sentence

A fourth additive fade term, `q_reversal`, accumulated per cell from the charge pulled past
empty and consumed on the aging sub-clock — structurally identical to the plating term, and
deliberately so.

### The accumulator is the deficit's *increase*, not the current

The charge a cell delivers past empty over one step is

```text
ah_reversed = max(0, deficit_after − deficit_before) · eff_capacity_ah · soh_capacity
```

and not `|i_cell| · dt / 3600` gated on "below empty". The two differ on exactly the step
that crosses zero — where only part of the step's charge came out past the boundary — and
the deficit difference is exact there while the gated current is not. It also excludes
repayment for free: charging a reversed cell *shrinks* the deficit, the difference goes
negative, and `max(0, ·)` charges nothing for it. Damage is done on the way down.

**Which capacity, and why it is load-bearing.** `eff_capacity_ah · soh_capacity` must be the
*same* product `coulomb_step` divided by on this step — the values read at `pack.rs` before
`advance` runs, not a post-tick state of health. `soc_deficit` is a fraction of that
capacity and nothing else; multiplying it by any other number produces amp-hours that do not
correspond to charge that crossed the terminals. Pinned by a test rather than argued
(`reversal_ah_matches_current_integral`), because nothing in the existing suite asserts on
reversal amp-hours and a leak here would be silent.

**Why the obvious feedback is not one.** Damage shrinks `soh_capacity`, which shrinks
`capacity_as`, which makes the same current produce a *larger* deficit fraction. That looks
like it should compound. It does not: the amp-hours are the deficit fraction times that same
shrinking capacity, so the product is unchanged. Damage is per amp-hour past empty and the
amp-hours are not inflated by the damage.

**The feedback that is real, and is being tested rather than argued.** A smaller cell
reaches empty *sooner* under the same demand, so a scenario holding a constant discharge
current gets into reversal earlier on each successive cycle and, if the demand runs to a
fixed clock, goes deeper. The bounding argument is that the `[reversal]` ramp hits
`floor_v` after a couple of percent of capacity and the delivered energy per excursion is
therefore bounded — but that is an argument, and the plating module earned the equivalent
claim with a committed test. So does this one:
`repeated_over_discharge_fades_without_running_away` fast-forwards repeated over-discharge
cycles and asserts the trajectory stays well clear of [`crate::aging::MIN_SOH_CAPACITY`].
**If it does not stay clear, that is the finding of this slice and it goes in this document
— not a tolerance to widen.**

### One new constant, not two

`reversal.fade_per_ah` — capacity fraction lost per amp-hour delivered past empty.
Resistance growth comes from the existing `aging.r_growth_per_capacity_loss`, the same
coupling calendar, cycle, and plating loss already share.

The rival design gave reversal its own resistance coefficient, on the physics that copper
dissolution is primarily a *contact* failure and the shared 1.5 under-reports it badly. The
physics is right and the design was still refused, for a reason that is worth recording
because it is not a physics reason:

> A second coefficient's only job would be to make the first coefficient's consequence more
> visible, and it would be picked by asking what looks legible rather than by fitting
> anything. This slice was chosen over *fitting* the two existing reversal placeholders
> precisely because fitting needs a source. Shipping two more sourceless numbers to avoid
> that is the wrong trade.

And the legibility it was supposed to buy is not there. At 1C on the shipped LFP cell
(`R0 ≈ 0.020 Ω`, sag `≈ 46 mV`), one deep reversal costing ~1 % capacity raises resistance
1.5 % under the shared coupling — **~0.7 mV of extra sag**. A coupling seven times larger
reaches ~4.6 mV. Neither is something a reader sees. The visible consequence of
over-discharge in this engine is the capacity that did not come back, and that needs one
coefficient, not two.

**The cost is stated, not hidden**, exactly as `plating.rs` states its own: this model
under-reports the resistance cost of over-discharge, the honest fix is a second coefficient
with a fit behind it, and until there is a fit the shared coupling is what ships.

### The coefficient is required, not defaulted

`plating_fade_per_ah` carries `#[serde(default)]` and zero means "plating is reported but
costs nothing". That default is defensible there because `[safety]` is an *optional*
section: a chemistry without one is declaring it cannot say what plating costs.

`[reversal]` is required. A chemistry file that reaches the loader has already declared how
its cell behaves below empty, so a missing damage coefficient is an omission rather than a
declaration — and the value silently supplied would be "over-discharge is free", which is
the exact defect this slice exists to remove. So `fade_per_ah` is required: all three
shipped TOMLs gain it, and an external file gets a load error naming the field.

### Sizing the placeholder

Every number in the shipped chemistries is a labelled placeholder and this one is no
different, but placeholders still have to be plausible *together* — the trap
`docs/plans/phase-3-aging-faults.md` slice A named. The scale is set against the two fade
coefficients already in the file:

| mechanism | per Ah | relative |
| --- | --- | --- |
| ordinary cycle throughput | `2.0e-5` | 1× |
| charge carried while plating | `1.0e-3` | 50× |
| **charge delivered past empty** | **`2.0e-1`** | **10 000×** |

The anchor: the `[reversal]` ramp is sized so the cell collapses from its empty-endpoint OCV
to `floor_v` over 2 % of capacity, so "2 % past empty" is a full, unambiguous reversal
rather than a graze. Each chemistry's coefficient is then set so that **one full reversal
costs ~1 % of that cell's capacity** — normalising per cell, not per amp-hour, the same
choice `v_per_soc` already makes against each cell's own `OCV(0)`:

| chemistry | capacity | 2 % past empty | `fade_per_ah` | cost of one full reversal |
| --- | --- | --- | --- | --- |
| LFP 26650 | 2.303 Ah | 0.0461 Ah | `2.2e-1` | 1.01 % |
| NMC 18650 | 3.000 Ah | 0.0600 Ah | `1.7e-1` | 1.02 % |
| LG M50 21700 | 5.153 Ah | 0.1031 Ah | `1.0e-1` | 1.03 % |

The consequence of normalising per cell is that the *per-amp-hour* ratios against the other
two fade coefficients differ between files — ~11 000× ordinary cycle throughput on the LFP
cell, ~5 700× on the two NMC ones. That is stated in each file rather than smoothed over:
keeping the **lesson** identical across chemistries is worth more than keeping the ratio
identical, because the lesson is what a reader carries between them.

One deep over-discharge therefore costs roughly what 200 cold charges cost, and roughly
what a year on the shelf costs. Bad enough to matter on the first event, not so bad that the
cell is scrap — a curve, not a cliff, which is the same shape the plating coefficients were
sized to.

### Scope: equivalent-circuit cells only, and that is physics

`CellModel::soc_deficit()` returns `0.0` for `Spm` and `Dfn`, and its own doc already says
why: those models never clamp, so there is no truncation for a deficit to record. Their
lithium simply keeps moving and `soc` reports the readout leaving its window. A
porous-electrode cell driven past empty is a different mechanism needing a different model,
and nothing here stubs it — the accumulator reads a quantity that is structurally zero for
those variants, so they pay nothing and no branch is needed to arrange it.

## Versions

| constant | before | after | why |
| --- | --- | --- | --- |
| `sim_core::SNAPSHOT_VERSION` | 14 | **15** | `CellAging` gains two stored fields; `ChemistryParams::reversal` gains a required one |
| `sim_server::API_VERSION` | 2 | **2** | no wire shape changes; nothing is added to telemetry |
| `sim_wasm::WASM_API_VERSION` | 6 | **6** | same |

**This bump's argument is structural, and saying so is the point.** v14's was semantic and
unrecoverable: `soc_deficit` was state a v13 blob had no record of, and defaulting it to zero
would have continued a trajectory with the fabricated energy back in it. Nothing here is like
that. `q_reversal` and `ah_reversed_since_tick` both default to the *correct* v14 reading —
a v14 pack accrued no reversal damage, because there was none to accrue — so the state half
of this bump would restore honestly.

What forces it is the layout rule in `CLAUDE.md`, plus the same deserialization argument v14
made: `reversal.fade_per_ah` is required and carries no `#[serde(default)]`, and the
chemistry is serialized inside every snapshot, so a v14 blob fails at *deserialization*
first in every configuration and the version check is belt to that braces. No semantic story
is manufactured for it.

## Exit criterion

A cell driven past empty and charged back reports `soh_capacity < 1` and
`soh_resistance > 1` where it previously reported exactly `1.0` for both — pinned by
`sim-core/tests/reversal_damage.rs`, seven tests, plus a clean whole-workspace gate.

---

# What the measurement said, and the three places it moved the plan

Everything above is the plan as drafted. This section is what happened when it was run.

## 1. "Stays far from the capacity floor" was the wrong claim, and it failed

The plan promised the mirror of `plating.rs::repeated_cold_charging_fades_without_running_away`:
repeated over-discharge cycles stay well clear of `MIN_SOH_CAPACITY`. Written that way, it
**failed** — 60 excursions to 2 % past empty left the test cell at 4.3 % of nominal, only
four times the floor.

Not a bug. The series is a clean geometric decay: each cycle costs a fixed *fraction* of
what is left, because the charge a fixed-depth excursion delivers past empty is proportional
to the capacity still there. Measured ratio ~0.951 per cycle at the test's deliberately
exaggerated coefficient, so `0.951⁶⁰ ≈ 0.043` — exactly where it landed. Nothing overshoots,
oscillates, or accelerates.

So the claim was wrong rather than the code, and it was **split** rather than loosened:

* `repeated_over_discharge_fades_without_running_away` now asserts the *shape* — monotone,
  decelerating over every consecutive triple, finite, never below the floor — at a
  coefficient chosen to make the shape legible, and says outright that a large enough
  coefficient does reach the floor.
* `the_shipped_coefficient_is_the_right_order_of_magnitude` makes the *magnitude* claim
  where it is actually true, at the LFP file's `2.2e-1`: sixty full reversals cost about
  half the capacity and leave the cell ~50× above the floor.

Widening a tolerance would have kept one test and lost the distinction. The distinction is
the finding.

## 2. The two readings of "repeat the abuse" disagree, and only one was planned

The plan's no-runaway argument assumed a fixed-*depth* excursion. Under a fixed-*absolute*
draw — a load taking the same amp-hours every cycle from a shrinking cell — the damage per
cycle **grows**, because the charge needed to reach empty falls with capacity so more of
that unchanged draw lands past empty. That is real physics and the engine should not flatter
it, so `a_fixed_absolute_draw_accelerates_instead` pins it, and defends only the tail: the
acceleration stays finite and lands on `MIN_SOH_CAPACITY` rather than producing a negative
capacity the coulomb count would then divide by.

**The first attempt at that test asserted nothing at all.** Its discharge leg was 120 s at
2.5 A = 0.083 Ah against the 0.125 Ah needed to reach empty from `soc = 0.05`, so the cell
never reversed, the health series was forty exact `1.0`s, and the test failed on the
acceleration assertion for the one reason that had nothing to do with acceleration. The leg
is now 300 s with the arithmetic in a doc comment beside it.

## 3. Clippy's argument limit, and the fix that was not a suppression

A fourth mechanism put `CellAging::tick` at 8 arguments against clippy's limit of 7. The
`#[allow]` was available and refused: the three chemistry-side inputs come from three
different sections of one file and are conceptually one thing — what this chemistry says
wear costs — so they are now a `FadeParams` bundle assembled once per aging tick rather than
once per cell. Six arguments, and the call site reads better than it did before the slice.

## Versions, confirmed

`SNAPSHOT_VERSION` 14 → **15**. `API_VERSION` 2 and `WASM_API_VERSION` 6 do not move: no
wire shape changes and nothing is added to telemetry. `snapshot_version.rs`'s pair moved
v13→v14 to v14→v15 and its argument was **re-derived rather than renamed**, per the
assertion that file carries for exactly this purpose — the mechanism is the same as v14's (a
required field with no `#[serde(default)]` inside a struct every snapshot carries) but the
*reason the field is required* is different, and the module now says which part it inherits.

## Deferred, with a price

* **No shipped scenario reaches the damage.** This is sharper than the plan's draft admitted:
  it is not that nothing *names* over-discharge as the cause, it is that the only shipped
  scenario with aging enabled is a hot-storage run that never approaches empty, so every
  scenario that goes past empty has aging off and pays nothing. The mechanism is complete,
  tested, and **currently unreachable from any client** — a distinct kind of gap from an
  invisible one, and its own slice.
* **The resistance coupling is shared**, and under-reports what over-discharge does to
  impedance. The second coefficient was refused with a measurement (under 5 mV of extra sag
  at 1C, either way), not a preference — but the measurement says it would be *invisible*,
  not that it would be *wrong*. A fit would change that.
* **`fade_per_ah` is a placeholder** in all three chemistries, sized against the other
  placeholders rather than fitted — the third labelled number in `[reversal]`, joining the
  two the previous slice shipped.
* **Porous-electrode over-discharge** is not modelled at all. See the scope note.
