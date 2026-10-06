# The `Dfn` past its rate limit: a corrected diagnosis, a solve that was refused, and a flag

ROADMAP H8's last open bullet said the `Dfn` "has no solution above its electrolyte's
limiting current", and `porous-reversal.md` recorded the next step as "solving the
near-empty electrolyte, e.g. in its logarithm", because "the electrolyte's potential
equation goes singular as its conductivity vanishes and the solve fails first". This slice
measured that before building it. The diagnosis was wrong, the solve that would keep the
cell physical past the limit was built and refused on measurement, and what shipped is a
flag: `EventFlags::SURFACE_OUT_OF_RANGE`, raised when a step's answer has a particle surface
past full or below empty. **No number the engine produces changed.**

All measurements: the shipped LG M50, 1S1P, isothermal, no BMS, 10/5/10 nodes, 10 shells
and 1 s steps unless stated (the 3 C scenario is 20 shells at 2 s). Harnesses:
`W:\temp\claude\dfn-elyte\h\` (the spike, against a worktree) and `h2\` (the flag map).

## How much it matters

Two facts, measured first because the owner asked whether the payoff was worth it.

**It happens only past the cell's own cut-off.** The first unconverged step against the
first step below the chemistry's 2.5 V `v_min`:

| from | 2 C | 3 C | 4 C | 5 C |
| --- | --- | --- | --- | --- |
| 100 % | cut 1658 s, fails 1773 s | cut 460, fails 461 | cut 107, fails 108 | cut 49, fails 60 |
| 50 % | cut 767, fails 884 | cut 187, fails 188 | cut 60, fails 61 | cut 35, fails 36 |
| 20 % | cut 240, fails 344 | cut 82, fails 89 | cut 39, fails 40 | cut 25, fails 27 |

At 3 C and above the failure is the step after the cut-off, or within a few; at 2 C it is
past empty, in the last percent. The side note that prompted this check generalised from
the 3 C row; it holds on every row.

**A protected pack cannot reach it.** The BMS derates a discharge to `max_discharge_c`,
1.5 C on this cell (a placeholder). So this is a defect of the unprotected mode only — a
supported mode, but one where the cell is already past what its datasheet allows.

## What was actually failing

**Not the electrolyte's charge equation.** Raising `KAPPA_FLOOR_S_PER_M` from 1e-12 to
1e-6 left the failure map above bit-identical; 1e-3 and 1e-1 changed the physics (the
cut-off moved) before they moved the failure. A conductivity going singular would have
answered to that floor. The logarithm plan aimed at the wrong equation, and is struck.

**The positive electrode is pinched.** At the first failing step of the 3 C discharge from
full (t = 461 s), dumped node by node from the last Newton iterate: the electrolyte rows
are within 2e-6 of the 1e-8 tolerance (mass rows within 3e-9), and the rows that are far
from it are **Butler–Volmer** rows at the positive nodes next to the separator (scaled
residuals 3.7e-2, 7.8e-1, 9.8e-2), whose
particle surfaces sit at or past full (raw `c_s/c_max` 1.0054, 1.0000, 0.99999, 1.0001).
There the electrolyte is still 17–219 mol/m³. Deeper in, the particles have room (0.31–0.60
of `c_max`) and the electrolyte is gone (0.67 down to −0.23 mol/m³). Salt where there is no
room, room where there is no salt: the reaction cannot go anywhere, and the kinetics clamp
at `SURFACE_EDGE_FRACTION` is what the Newton runs into.

**And it was already converging on impossible states.** Over the 60 s after each cut-off
the engine's committed states have an outermost shell past full on 15–51 of the 60 steps,
reaching 1.04 of `c_max` at 3 C and 2.09 at 5 C from a fifth, with the voltage down to −204 V.
Lithium is conserved through all of it, to 1e-13 A·h — by putting it where it cannot be.

## The solve that was refused

**Predictions, registered before the run.** P1: a Newton that keeps every particle surface
inside its range leaves every step that never touches the range bit-identical; the 3 C
voltage moves under 1 mV before 455 s and its cut-off by at most 2 s. P2: the solve still
fails at or just after the cut-off, because no answer exists past the limit, but the state
stays physical — no surface past full, no negative electrolyte.

What was built, in a worktree: the damping line-search rejects a trial that moves a surface
from inside its range to outside it, and a start already outside is pulled to just inside.
Three variants, in the order tried:

* **Always on.** Not bit-identical: the 3 C discharge from full differs from its first step,
  by 9e-9 V (the line-search shortens a step the old Newton overshot and came back from).
  Physically nothing, but 138 committed `Dfn` fingerprints pin bits. **P1 failed** as
  written.
* **On the electrolyte as well** (`c_e > 0`). The 3 C discharge from full stops converging at
  303 s, a minute and a half *before* its cut-off. The model needs slightly negative
  electrolyte to reach its own in-window answers: `c_e` first goes below zero at 303–308 s
  at 3 C from full, at 71 s at 4 C; `C_E_FLOOR_MOL_PER_M3`'s doc records the reference
  reaching −0.0007 mol/m³ the same way. Refused.
* **A fallback only** — the plain solve first, unchanged; the in-range solve only where that
  failed or converged with a surface out of range. Bit-identical up to the cut-off on all
  nine cases compared (1, 2, 3, 4, 5 C and C/5, from full, half and a fifth): the first
  differing step is at or after the cut-off on every one, and the cut-off step itself does
  not move. **P1 holds for this variant.**

What the fallback did past the cut-off, over the same 60 s:

| from, rate | steps past full: before → after | worst shell (× `c_max`) | lowest V | lithium off the books |
| --- | --- | --- | --- | --- |
| 100 %, 3 C | 15 → 0 | 1.036 → 0.936 | 2.37 → −4.09 V | 1e-13 → 0.043 A·h |
| 50 %, 4 C | 40 → 0 | 1.577 → 0.905 | 1.40 → −8.43 V | 1e-13 → 0.119 A·h |
| 20 %, 5 C | 51 → 0 | 2.088 → 0.937 | −204 → −2.77 V | 1e-13 → 0.174 A·h |

(The lithium column is from the network-thermal arm of the same harness; the rest
isothermal.)

**P2 held in its letter and failed in what it was for.** No surface past full, and every
step past the limit still fails. But the in-range Newton cannot satisfy the solid's charge
balance either: the current flows at the terminals and nothing inside the cell carries it,
so the cell loses up to 3 % of its capacity from its books in a minute. Past this limit the
model can keep its concentrations possible or its lithium conserved, **not both**, and the
old engine's choice — conserved, at impossible concentrations — is the one every other
invariant in this repository is built on. The fallback also reached the charge side: the
`Dfn` arm of `surface_gap.rs`'s overcharge fixture stopped raising `SOC_CLAMPED_HIGH`,
because the in-range solve refused the over-full negative surface and the charge went
nowhere instead. And the lesson's 2.422 V at 464 s moved to 2.436.

The fallback's patch is kept at `W:\temp\claude\dfn-elyte\spike_domain_fallback.patch`.

## What shipped: a flag

Offered three ways to close (record only; record and flag; book the excess as
over-discharge, which would bill reversal damage to a cell holding 60–90 % of its charge),
the owner chose the flag. `EventFlags::SURFACE_OUT_OF_RANGE` (bit 14) is raised by
`dfn::advance` when the answer it commits has any particle surface outside `[0, c_max]` —
and, since the follow-up below, by `spm::advance` on the same terms. It is read off the
solve the step already ran; nothing is re-solved and no value changes. It is not raised
for negative electrolyte (in-window, above). As first committed (707f939) the bound was the
band the kinetics clamp to, `(SURFACE_EDGE_FRACTION, 1 − SURFACE_EDGE_FRACTION)` of
`c_max`; the follow-up moved it to the physical bounds, and every row of the table below
re-measured identical under the new rule.

**Where it fires** (`h2`; "cut" is the first step below 2.5 V, or above 4.2 V on a charge):

| case | cut | first flagged | flagged steps after |
| --- | --- | --- | --- |
| 3 C from full, 20 shells, 2 s (the scenario) | 464 s | **464 s**, converged | 21 of the next 21 |
| 3 C from full, 20 shells, 1 s | 464 | 464 | 41 of 41 |
| 3 C / 4 C / 5 C from full | 460 / 107 / 49 | 461 / 107 / 49 | 60–61 of 61 |
| 3 C / 4 C / 5 C from half | 187 / 60 / 35 | 188 / 60 / 35 | 60–61 of 61 |
| 3 C / 4 C / 5 C from a fifth | 82 / 39 / 25 | 89 / 41 / 26 | 54–60 of 61 |
| 1 C and 2 C from full, half, a fifth; C/5 from half | — | **never**, to cut + 60 s | 0 |
| 1 C charge from half, 1 s / 10 s steps | 661 / 670 | 2088 / 2090 s | — |
| 0.5 C charge from half, 10 s steps, to 4000 s | 2520 | **never** | 0 |
| 3 C charge from half, 1 s / 10 s steps | 24 / 30 | 240 / 250 s | 88 of the 279 steps to 3030 s (10 s) |

Never before a cut-off. The scenario's cut-off step is the case that matters: at its 2 s
step it **converged** — `SOLVE_UNCONVERGED` first arrives at 466 s — with a surface 0.22 %
past full (2.2e-3 of `c_max`, eight orders of magnitude outside the band; at 1 s the same
step fails to converge, 0.38 % over). Before this flag nothing reported that the guided
path's "2.422 V at t = 464 s" was read off a state no particle can hold.

## The guided path

Step `the-electrolyte-starves` already told the reader that the shelf after 466 s "is the
solver, not the cell". It now also names the new flag arriving on the cut-off step itself:
the step's last reading is the model past its own edge too, so the verdict (empty, a volt
under the twin) stands and the last decimals do not. No numeral was added to the prose,
so the step's claims ledger needs no new claim.

## Tests

`crates/sim-data/tests/dfn_electrolyte_limit.rs`:

* `the_3c_cut_off_step_is_the_first_one_flagged` — the scenario's pack at 2 s: nothing
  flagged before 464 s, 464 s flagged and converged at 2.421753 V (the flag changes no
  number), and the flag stays up for the next ten steps.
* `a_1c_discharge_through_its_cut_off_never_raises_it` — to a minute past the cut-off.
* `a_charge_driven_on_past_full_raises_it_sooner_the_faster_it_is` — from half charge at
  10 s steps, 3 C flagged within 300 s and 1 C not before 2000 s but within 2200 s. Its
  first draft asserted that 1 C was never flagged, from a run stopped twenty minutes past
  the ceiling; the flag arrives four minutes after that.
* `an_spm_drained_past_empty_is_not_flagged_for_sitting_on_its_margin` and
  `an_spm_charged_on_past_full_is_flagged` — the follow-up below.

Perturbed in the worktree, `sim-core` and `sim-data` run whole in debug (`--no-fail-fast`,
below-normal priority), red read off the failing tests by name, the file restored after
each (driver `W:\temp\claude\dfn-elyte\perturb.py`; its first run was void — a `\t` in a
Windows path became a tab and cargo never started, which an exit code alone did not show):

| # | change | exit | turned red |
| --- | --- | --- | --- |
| D1 | the flag check deleted | 101 | `the_3c_cut_off_step_is_the_first_one_flagged`, `a_charge_driven_on_past_full_raises_it_sooner_the_faster_it_is` |
| D2 | the flag raised on every step | 101 | those two and `a_1c_discharge_through_its_cut_off_never_raises_it` |

Nothing else in the suite reddens on either, and that is worth saying plainly: before this
slice no test looked at a `Dfn`'s surface past full at all, and the guided-path harness does
not check that a flag a sentence names is up, so the step's new sentence naming the flag is
held by `the_3c_cut_off_step_is_the_first_one_flagged` and by nothing in `path_claims.rs`.
What that harness does notice is a flag *arriving* at an instant a sentence prints and a
claim is read at. D2's flag arrived on every `Dfn` lesson's first step, an instant no
sentence prints; the follow-up's S2 arrived at one a sentence does, and reddened it.

## Follow-up: the `Spm` raises it too

A side note after the first commit pointed out that the `Spm` over-fills the same way on a
fast charge (`surface_gap.rs`'s fixture records a surface at 1.16 on both models) and stayed
silent: one impossible state, flagged on one model only. The owner chose to add it, measured.

**The first rule was wrong for the `Spm`, and only a measurement showed it.** Written as the
`Dfn`'s — flag a surface outside the clamp band, `SURFACE_EDGE` = 1e-6 of `c_max` here —
it fired on 494 steps of 1S1P discharges at 2–5 C driven past empty, from the first step
past empty on. Every one was rounding: past empty this model's split parks a surface **on**
the band's edge by construction (`porous-reversal.md`), and the closed form lands it a hair
outside — the negative surface below the empty edge by 2.3e-19 to 2.4e-18 of `c_max` (158
steps), the positive above the full edge by about 1e-16 (336 steps), dumped per step. A flag
reading "a particle past full or empty" on a model doing exactly what it was designed to do
would have been the false report this flag exists to prevent. So the bound is now the
physical one, `[0, c_max]`, on **both** models: the `Dfn` map above re-measured identical
row for row, including the charges.

**Where the `Spm` raises it** (1S1P, isothermal, `W:\temp\claude\dfn-elyte\h3\`):

| case | first flagged | steps flagged |
| --- | --- | --- |
| 1–5 C from full, half and a fifth, 1 s, to ten minutes past empty | never | 0 of 744–4200 |
| 1 C from a tenth for an hour at 10 s; 5 A for two hours at 60 s | never | 0 |
| 1S3P, 5 % scatter, 20 A through empty, at 1 s and 10 s | never | 0 |
| charge 0.5 C from half, 10 s, to 4000 s (ceiling 2840 s) | never | 0 of 400 |
| charge 1 C from half, 10 s (ceiling 1040 s) | 2100 s | 191 of 400 |
| charge 3 C from half, 10 s (ceiling 120 s) | 660 s | 335 of 400 |

So on the `Spm` it is a charge-side flag only: its discharge side already has a surface edge
and a reversal, and they keep the surface in range.

**Perturbed** on the same terms as D1–D2 (driver `W:\temp\claude\dfn-elyte\perturb_spm.py`):

| # | change | exit | turned red |
| --- | --- | --- | --- |
| S1 | the `Spm`'s check deleted | 101 | `an_spm_charged_on_past_full_is_flagged` |
| S2 | the `Spm` back on the clamp-margin rule | 101 | `an_spm_drained_past_empty_is_not_flagged_for_sitting_on_its_margin`, **and** `path_claims.rs::every_claim_matches_the_engine` |

S2's second red was not predicted and is the stronger evidence. Guided-path step
`three-times-the-current` drives an `Spm` past empty and quotes "the tooth that finishes at
11 280 s"; under the margin rule the run raises `SURFACE_OUT_OF_RANGE` at that very instant,
and the harness refuses to account a printed time by a reading taken where a flag arrives.
So the false flag would have been on the page, at the moment a sentence names.

## Still open

* **The `Dfn` past its rate limit has no physics** — this flag names the state, it does not
  give the cell somewhere to put the current. A channel for it (a side reaction at the
  positive electrode, which the owner declined here) needs parameters nothing in the repo
  states and a reference that runs past the cut-off, which PyBaMM's does not.
* ~~**The `Spm` has the same over-full surface on a fast charge** and does not raise this
  flag~~ — **closed by the follow-up below**: it raises it now. Its surface past full still
  has no physics (ROADMAP H8), as the `Dfn`'s does not.
* **The `Dfn` past full on a charge** is the same gap as the `Spm`'s and was not on the
  ROADMAP: driven on past the ceiling, a surface goes past full 220 s later at 3 C and
  twenty-four minutes later at 1 C.
* **At 2 C the `Dfn` fails past empty, in its last percent**, with the electrolyte intact —
  the "no surface edge" bullet in `porous-reversal.md`. This flag does not fire there in
  the minute after the cut-off.
