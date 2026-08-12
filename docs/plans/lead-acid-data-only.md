# Lead-acid as data alone — how far does it get?

2026-08-12. `CLAUDE.md` promises that **adding a chemistry never requires a code change**, and
names lead-acid (with Peukert) as a chemistry to ship. Lead-acid is the first real test of
that promise, because unlike LFP and NMC it is not a lithium cell wearing different numbers.

This slice does **not** build lead-acid. It asks the cheaper question first: **write the
parameter file using only sections the engine already reads, and measure where it breaks.**
No new per-cell state, no `SNAPSHOT_VERSION` bump, no migration. The answer prices the phase
that `path-wedge.md:386` currently sizes by guesswork.

> **Closed by `docs/plans/diffusion-overpotential.md` (2026-08-12, `SNAPSHOT_VERSION` 17).**
> The answer to this file's question turned out to be "far, and then it stops at a wall no
> parameter reaches" — so the phase was priced, then built. One extra state per cell and a
> `[diffusion]` section took the worst error against Peukert from **25.7 points to 3.3**.
>
> Everything measured here is still live rather than superseded:
> `crates/sim-data/tests/lead_acid_rate.rs` now runs **both** arms, and the control arm is
> this file's model — the shipped chemistry with `[diffusion]` stripped. Every assertion
> below still passes against it, and that is also how the engine proves a chemistry without
> the section is untouched by the term.

---

## Everything below the line was written BEFORE the parameter file existed

The predictions here come from a hand model of the ECM (`docs/plans/` has no other), not from
the engine. That is the point: the engine run is the test, and a test whose expected answer is
written afterwards is not a test. If the engine disagrees with these numbers, **the engine is
right and this section is a recorded miss** — that is the outcome this file is shaped to make
reportable.

---

## Prediction 1 — the pedagogical claim, corrected before it was written down anywhere

The tempting sentence is "lead-acid's resting voltage tracks charge almost linearly, where LFP
is famously flat, so on lead-acid you can read charge off a voltmeter." **The second half is
right and the first half, as usually stated, is backwards.** Measured off the shipped LFP table
and a lead-acid curve built from specific gravity:

| SOC window | LFP | lead-acid |
| --- | --- | --- |
| 0.45–0.65 (LFP's plateau) | 0.39 mV per 1 % | **1.79 mV per 1 %** |
| 0.25–0.85 | 2.12 | 1.76 |
| 0.00–1.00 | **16.0** | 1.80 |

Lead-acid spans ~180 mV end to end; LFP spans 1600 mV. **In absolute terms LFP moves 8.9× more
voltage than lead-acid, not less.** What is actually true is *uniformity*: lead-acid's slope is
1.76–1.80 mV/% at every charge level, flat to within 2 %, while LFP's varies **40×** between
its dead zone and its end knees. So a voltmeter reading means the same thing everywhere on
lead-acid, and inside LFP's plateau lead-acid is 4.5× more informative. That — not steepness —
is the contrast worth teaching, and the guided path must not inherit the wrong version.

> **Do not quote the numbers in this paragraph.** The qualitative claim survived; the
> quantitative half did not. "Flat to within 2 %", "40×" and "4.5×" are all artifacts of the
> windows chosen above — 0.45–0.65 straddles LFP table breakpoints at 0.55 and 0.65, so it
> measures the nodes as much as the curve. Measured decile by decile the figures are **1.7×**,
> **248×** and **3.2×**. See "Prediction 1 was WRONG in its quantitative half" below; this
> paragraph is left unedited because it is the registered prediction, not the finding.

## Prediction 2 — ohmic sag is not where lead-acid's rate penalty comes from

Anchored on real hardware rather than assumed, because the whole rate prediction rests on it.
Internal resistance is quoted per *battery*; the engine wants it per 2 V *cell*:

| | capacity | battery | per cell | sag at 1C |
| --- | --- | --- | --- | --- |
| 12 V 7.2 Ah AGM (UPS class) | 7.2 Ah | 23 mΩ | 3.83 mΩ | 27.6 mV |
| 12 V 60 Ah flooded (starting) | 60 Ah | 5 mΩ | 0.83 mΩ | 50 mV |
| 12 V 100 Ah AGM (deep cycle) | 100 Ah | 4 mΩ | 0.67 mΩ | 67 mV |

Cross-checked independently: at 0.83 mΩ/cell a 60 Ah starting battery cranking at 400 A reads
**10.7 V**, and real cranking voltage is 9.5–10.5 V. The anchor is sound.

**The consequence is the interesting part.** Sag at 1C as a fraction of nominal cell voltage:

* lead-acid — 27.4 mV on 2.0 V = **1.37 %**
* LFP — 46.1 mV on 3.3 V = **1.40 %**

**Ohmically the two chemistries are in the same class.** So ohmic resistance cannot be why
lead-acid is rated at the 20-hour rate and lithium at 1C. Real Peukert behaviour is acid
depletion inside the porous plates — a diffusion limit — and the ECM has no diffusion term.
That is the structural claim this slice exists to test.

## Prediction 3 — the headline number, and it is a *shape* failure not a magnitude one

Hand-model sweep, 1S1P, fixed ambient, discharge to a 1.75 V cutoff, delivered capacity
expressed against the C/20 reference. Reference curve is Peukert with **n = 1.1** (AGM class;
flooded runs 1.2–1.3, quoted as a second column in the results but not as the target).

| rate | predicted engine | Peukert n=1.1 | error |
| --- | --- | --- | --- |
| 0.05C | 100.0 % | 100.0 % | +0.0 |
| 0.10C | 100.0 % | 93.3 % | +6.7 |
| 0.20C | 100.0 % | 87.1 % | +12.9 |
| 0.50C | 100.0 % | 79.4 % | +20.6 |
| 1.00C | 100.0 % | 74.1 % | **+25.9** |
| 2.00C | 92.9 % | 69.2 % | +23.8 |
| 3.00C | 84.1 % | 66.4 % | +17.7 |

**Predicted headline: over 0.05C → 3C a real AGM loses 33.6 % of its capacity and the engine
loses 15.9 %, so the engine reproduces ≈ 47 % of the real rate-dependent loss — and every bit
of what it does reproduce sits above 1C.** Below 1C it reproduces *none* of it.

## Prediction 4 — tuning `R0` cannot fix this, and that is the finding

The obvious objection is that `R0`'s rise near empty is a free parameter, so the gap above just
reflects the number chosen. **Tested before writing the file, and the objection does not hold.**
Scaling every resistance by **1.5×** makes 3C land almost exactly on Peukert (+0.6 points) —
and leaves the mid-range no better than before:

| rate | ×1.0 error | ×1.5 error |
| --- | --- | --- |
| 0.20C | +12.9 | +12.9 |
| 0.50C | +20.6 | +20.6 |
| 1.00C | **+25.9** | **+24.9** |
| 2.00C | +23.8 | +15.0 |
| 3.00C | +17.7 | **+0.6** |

**Worst error moves 25.9 → 24.9 points. The knee moves; the flat does not go away.** Ohmic sag
produces a *flat-then-knee* curve because at low rate `I·R` is negligible against the OCV
headroom; Peukert is a smooth power law that starts losing capacity at the very first rate
increase. These are different shapes, and no choice of `R0` turns one into the other. Fitting
one rate is easy and meaningless.

**A falsifier I expected to fire, and it does not — recorded because it was planned as an
arm.** I expected inflating `R0` to fake Peukert would betray itself as excess heat, since heat
and sag share the same `I²R`. At 1.5× the heat rises by **1.0×** (the shorter discharge cancels
the higher resistance), and the cranking cross-check still reads 9.7 V, inside the real 9.5–10.5
band. **So heat does not discriminate and that arm is not worth building.** The shape argument
above is the discriminator, and it is the only one.

---

## What gets measured

1. **The parameter file loads and validates** against the shipped validator, using only
   `[meta] [cell] [ocv] [r0] [[rc]] [reversal] [thermal] [aging]`.
2. **The rate sweep, through the engine**, 0.05C → 3C, checked against predictions 3 and 4.
3. **A timestep check.** The hand model overshoots to 100.1–100.3 % at coarse `dt` purely
   because the last step crosses the cutoff — delivered capacity quantizes at `I·dt`. Two `dt`
   values at one rate settle whether the engine's numbers are resolution-limited before any of
   them are quoted.
4. **Thermal held fixed first**, so a rate effect is not read off a curve that is partly
   self-heating. Whether self-heating adds to the effect is a separate, later question.

## What is deliberately not in the file

* **`[safety]` is omitted.** It is `Option`, and its live half is *lithium plating* — a
  mechanism lead-acid does not have. Inventing plating constants for a lead-acid cell would be
  a fabricated number in a repo whose rule is that there are none. Lead-acid does have a real
  charging thermal runaway, but it is a different mechanism from the one the section models.
* **No Peukert term.** See the refusal below.

## Decision: Peukert must never scale the coulomb count

`CLAUDE.md` names Peukert for lead-acid. The textbook implementation multiplies the charge
count by `(I/I_ref)^(n−1)`. **Refused, for two independent reasons.**

*It is wrong physics.* A lead-acid battery discharged hard and then rested **recovers** the
missing capacity — the acid diffuses back out of the plate pores. Capacity that comes back is
not capacity that was consumed. A scaled coulomb count destroys it permanently and cannot give
it back, so the model would get the rest-recovery experiment exactly backwards, and that
experiment is one of the most teachable things lead-acid does.

*It puts a fudge factor on the ledger this repo has spent three slices making exact.*
`low-clamp-reversal`, `energy-hole-closed`, and `reversal-damage` all exist to make charge and
energy account for themselves. `coulomb_step` is the single place charge enters or leaves, and
a rate-dependent multiplier there means `∫I·dt` no longer equals `ΔSOC × capacity` — breaking
the property test that has guarded that identity since Phase 1.

**If a rate term is ever added, it belongs on the voltage side** — a diffusion overpotential
that grows with sustained current and relaxes at rest. That is what the physics actually is,
it recovers on rest for free, and it leaves the charge ledger untouched. It is also per-cell
state, which is what makes it phase-sized.

---

# Results

`chemistries/pba_agm_2v_generic.toml` and `crates/sim-data/tests/lead_acid_rate.rs`.
Five tests, all green. Run the tables with
`cargo test -p sim-data --test lead_acid_rate -- --nocapture`.

## It loads, with no code change at all

**The parameter file parsed and validated first try, and nothing in `sim-core` or `sim-data`
was touched to make lead-acid exist** — so `CLAUDE.md`'s "chemistry is data, not code" holds
for this chemistry, at least as far as construction goes.

The *test around it* took three iterations, and they are worth separating from that result,
because two of them were not typos. It first failed to compile, because it was written against
`CLAUDE.md`'s API sketch rather than the real signatures (see "Noted, not fixed" below). Then
two assertions failed: the timestep tolerance, which had been written as a round number instead
of a derived one, and the OCV uniformity claim — **which failed because the prediction was
wrong, not because the test was.** That one is recorded as a miss immediately below.

Two things the existing knobs turned out to express without modification:

* **`[safety]` omitted.** Optional, and its live half is lithium plating, which lead-acid
  does not have. Asserted absent, so a later slice cannot quietly fill it in.
* **Calendar fade inverted.** Lead-acid degrades worst left *flat* (sulfation); lithium
  degrades worst left *full*. `cal_soc_stress` is a table, so `[1.8, 1.0, 1.1]` says that
  with no code change. Asserted, because it is the single clearest demonstration of the
  data-not-code claim in the repo.

## Prediction 1 was WRONG in its quantitative half — recorded as a miss

I registered that lead-acid's OCV slope is "1.76–1.80 mV/%, flat to within 2 %". **It is not.**
That number was an artifact of the wide averaging windows I measured over, which spanned
several table segments and cancelled the variation. Measured decile by decile, as the test now
does:

| | flattest decile | steepest decile | variation |
| --- | --- | --- | --- |
| lead-acid | 0.128 V/soc | 0.220 V/soc | **1.7×** |
| LFP | 0.040 V/soc | 9.781 V/soc | **248×** |

Lead-acid's curve is *concave* — noticeably steeper when empty than when full, which follows
from the electrolyte being a reactant. The qualitative claim survives and is now stated at the
strength the data supports: **lead-acid's slope varies 1.7× where LFP's varies 248×, and
lead-acid's flattest decile is still 3.2× more informative than LFP's flattest.** It never goes
flat. That, not "steeper", is the reason a resting voltmeter reads charge on lead-acid.

The miss is the pre-registration working as intended. The wrong number was written down before
the test could contradict it, so the contradiction was visible instead of absorbed.

## Predictions 2–4 held

The engine's sweep against the hand model that predicted it:

| rate | predicted | **engine** | Peukert n=1.1 | error |
| --- | --- | --- | --- | --- |
| 0.05C | 100.0 % | **100.0 %** | 100.0 % | — |
| 0.10C | 100.0 % | **100.0 %** | 93.3 % | +6.7 |
| 0.20C | 100.0 % | **100.0 %** | 87.1 % | +12.9 |
| 0.50C | 100.0 % | **99.9 %** | 79.4 % | +20.5 |
| 1.00C | 100.0 % | **99.9 %** | 74.1 % | **+25.7** |
| 2.00C | 92.9 % | **92.7 %** | 69.2 % | +23.5 |
| 3.00C | 84.1 % | **83.8 %** | 66.4 % | +17.4 |

**Headline, as registered: over 0.05C → 3C a real AGM loses 33.6 % of its capacity and the
engine loses 16.2 %, so the engine reproduces 48 % of the real rate-dependent loss (predicted
47 %) — and none of it below 1C.**

Prediction 4 held too, and it is the conclusive part. The test searches for the resistance
scale that best fits 3C and finds **×1.46** (predicted ×1.5). At that scale 3C lands at 66.5 %
against Peukert's 66.4 % — an essentially exact fit at the rate it was fitted on — and **the
worst error across the sweep moves 25.7 → 25.4 points, still at 1C.** Fitting one rate buys
0.3 points everywhere else.

**So the answer to "how far does lead-acid get as data alone" is: all the way to a working,
loading, validating cell, and about half the rate behaviour — with the missing half structural
rather than a matter of fitting.** Ohmic sag is flat-then-knee; Peukert is a power law from the
first rate increase. No resistance table turns one into the other.

## Timestep — and a systematic bias the convergence check could not see

**Every amp-hour figure in this document is a lower bound, short of the true delivered
capacity by at most one step's charge (`I·dt`).** The step that carries the terminal below
`v_min` has already moved charge, but a real cutoff happens partway through it, so excluding
that step undercounts and including it overcounts. The tables quote the conservative end.

This nearly went unstated, and the way it nearly did is the lesson. The first version of the
timestep test compared `dt` against `dt/4` and passed — **but that check is structurally
incapable of detecting this bias, because the omission scales with `dt`, so both runs are
wrong in the same direction and agree with each other.** It measured convergence and would
have been read as accuracy. The test now brackets instead, and asserts on the width:

| rate | dt = 1 s | dt = 0.25 s | lower bound moved | one-step quantum |
| --- | --- | --- | --- | --- |
| 0.05C | [7.2142, 7.2143] | [7.2142, 7.2143] | 0.0000 | 0.0001 |
| 1.00C | [7.2040, 7.2060] | [7.2045, 7.2050] | 0.0005 | 0.0020 |
| 3.00C | [6.0480, 6.0540] | [6.0510, 6.0525] | 0.0030 | 0.0060 |

The widest bracket is **0.099 % of its own value**, at 3C. Quartering `dt` quarters the
bracket, which is the direct evidence that its width is the cutoff-crossing quantum and not
some other error.

Recomputing the headline from the bracket's **upper** ends — 6.0540 Ah at 3C against 7.2143 Ah
at C/20, the two rows the headline is built from — gives a 16.1 % loss against the real 33.6 %,
i.e. **48 %** again. (Derived from those two endpoint rows, not from re-running the whole sweep
on upper bounds.) So the conclusion is insensitive to which end of the bracket is quoted, and
now demonstrably rather than presumably.

## What this prices

A lead-acid **phase**, if one is wanted, is now specified by measurement rather than guess:

* The missing mechanism is a **diffusion overpotential** — a voltage term that grows under
  sustained current and relaxes at rest. It is per-cell state, hence a `SNAPSHOT_VERSION`
  bump and a migration. That is the phase-sized part, and it is the *same* piece of state OCV
  hysteresis needs, so the two should be scoped together rather than as separate phases.
* It must reproduce a smooth power-law falloff from the very first rate increase, and must
  **recover on rest** — the acceptance test is a hard discharge to cutoff, a rest, and a
  second discharge that delivers materially more.
* Everything else — parameters, aging signs, protection limits, the missing `[safety]` — is
  already done and needs no code.

## Deliberately not done here

* **Not wired into the guided path or any client.** The parameter file is reachable by id
  through the existing scenario mechanism (`sim-server` resolves an id against the chemistry
  directory), so nothing blocks it, but a teaching page that presented this cell's rate
  behaviour as accurate would be presenting the half the engine gets wrong.
  **Both halves of that changed, in order.** `docs/plans/diffusion-overpotential.md` closed
  the gap in the engine (25.7 → 3.3 points), which removed the objection above; then
  `docs/plans/lead-acid-client.md` wired it in — one scenario file and guided-path steps 22
  to 24. The acceptance test this document named for the mechanism and never measured — a
  hard discharge, a rest, and a second discharge that delivers materially more — is now the
  subject of step 24, driven through the page's own pulse mode.
* **Thermal held fixed.** The sweep is isothermal on purpose, so a rate effect is not read off
  a curve that is partly self-heating. At 3C a real lead-acid cell does warm measurably and
  that would *add* to the effect — a separate question, and one worth doing only after the
  diffusion term exists, since otherwise it would paper over the gap rather than measure it.

## Noted, not fixed here

`CLAUDE.md`'s API sketch has `Pack::new(config: &PackConfig, chems: &ChemistryRegistry)`. The
real signature is `Pack::new(config: &PackConfig, chem: ChemistryParams)` — **there is no
`ChemistryRegistry` type in the workspace at all** (`grep` returns zero hits outside the doc),
and the chemistry is taken *by value*, not by reference. Found by writing against the sketch
and having it fail to compile. Same class of spec-vs-code divergence as the RC-resistance-growth
one resolved on the code side in `rc-resistance-growth.md`, and the resolution is probably the
same — the code is the one that works, so the sketch should be amended to match. Recorded so it
is not rediscovered a fourth time; out of scope here.

Also corrected while writing the test: the OCV lookup is the free function
`sim_core::ecm::ocv_lookup(&chem.ocv, soc)`, not a method on `ChemistryParams`.
