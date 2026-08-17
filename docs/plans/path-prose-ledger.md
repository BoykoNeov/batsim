# The fourteen steps nothing was checking, measured

`crates/sim-data/tests/path_claims.rs` checks 67 claims across ten of the guided path's
twenty-four steps. The other fourteen have no claim at all, so they have no literal for
any of its seven checks to scan and nothing in the repo touches them. Two consecutive
slices said in writing that this was now the largest remaining gap —
`docs/plans/path-accounting.md` called it "the whole of the completeness gap, and the
larger half", and `docs/plans/operating-point-window.md` called it "the real gap, and
this slice is the second piece of evidence for it rather than an argument about it".

This slice is the measurement half. Every number in those fourteen steps' prose was
driven through the engine the way the page drives it and compared against what came
back. The ledger that would *keep* them true is designed at the end of this document and
deliberately not built: until the fourteen had been measured, nobody knew whether such a
check would be guarding a hundred and forty correct numbers or repairing thirty.

The answer turned out to be the first, which is itself the result. **Two defects in
roughly 145 measurement-shaped numbers**, and both of them are the same shape: a number
that was exactly right when it was written.

## What had to be measured, and how much of it there is

Counting numeric tokens over each step's `prose` and `expect` — the same slice of the
source `lesson_text` takes — the fourteen unclaimed steps print **350 numbers**. Most are
not measurements: they are currents the reader types into the demand box, ambient
temperatures, marks, C-rates, ordinals naming other steps, and constants the chemistry or
scenario file declares. Classifying each token against the lesson block's own fields, the
scenario file, the chemistry file, and the "step N" shape leaves **145** that no source
accounts for. Those are the ones only a run can settle, and those are what this slice
measured.

Three steps — `pack-disagrees`, `belief-drifts`, `lying-sensor` — have **no** such token
at all. Every number in them is a scenario constant. They are ledger-ready before a
single claim is written, which is worth knowing before the next slice budgets anything.

## The instrument, and the two things it did not know

The measurement harness is a temporary `#[test]` inside `path_claims.rs` itself, reusing
its `lessons()` scraper, its `build`, and its demand mirrors, and writing one CSV per
trajectory. Twenty-six trajectories: the fourteen steps as configured, plus the variants
their own prose instructs a reader to produce — the BMS unchecked, the ambient dragged to
−5 °C, a 6 A demand, a 1 C rerun of both porous models, the two step lengths step 18 asks
for, the two buttons step 18 contrasts, and the continuations several steps tell the
reader to press Run for.

Before trusting a number from it, it was validated against numbers the prose already
carries: `protection-off` reproduces 335.0 s, 1.9731 V, 3.7 %, 345.0 s and −2.6355 V
exactly, and the per-cell crossing spread of 345.0 → 356.5 s with it. That step was
repaired one slice ago, so agreeing with it is evidence about the harness rather than
about the prose.

It then got two things wrong, and both are worth more than the defects they hid.

### The CC-CV mirror ignored the sub-clock it pinned

`MIRRORED` has pinned `const CCCV_PERIOD_S = 10` since this file was written, and nothing
in the file read it. `demand_now` mirrors the page's `ccCvDemand` — the inner comparison —
but not the loop around it: `advance` chops each frame's steps at multiples of
`CCCV_PERIOD_S / dt` and holds **one** demand across the whole window, so which step the
legs change on is a property of the simulation rather than of how the browser scheduled a
frame. The mirror decided every step.

A pinned constant no code consults is exactly the "looks like coverage" shape this file
rejects everywhere else, so it is fixed here rather than recorded: `drive` now holds a
CC-CV demand across each window. Measured cost of the gap, on `two-legs` where the
constant-voltage leg actually engages: the switch lands one step later, 5420.5 s rather
than 5420.0 s.

**Nothing in `path-claims.toml` moved**, and the reason is worth stating rather than
taking as reassurance: the only claimed CC-CV step is `leg-that-is-not-there`, whose LFP
cell never reaches the band at all, so it is on a constant current under either rule. That
is why the gap was invisible for six slices — not why it was harmless.

> **Both paragraphs are dated, and one clause was wrong when it was written.**
> `cccv_window_steps` did *not* read the pinned 10 — it carried its own copy of the number,
> and the pin still had no reader at all; `Tie::Page` gave it one when step 9 was ledgered
> (`docs/plans/path-ledger-two-legs-step.md`).
>
> "Nothing in `path-claims.toml` moved" was true *this morning* and is not true now. The
> two-legs claims landed nine hours after this fix, and measured today — with `windowed`
> forced back to `false` — the harness's own suite goes red on `i_at:5420`, which reads
> −1.5552 A against the −1.5 the constant-current leg has. The sentence's point survives
> intact and is sharpened: a gap is invisible exactly as long as nothing claims the thing
> it moves, and the window of invisibility here closed within a day of it being recorded.

One thing the mirror still does not model is `ccCvDone`, the page's completion test. It is
evaluated at the end of each *chopped chunk* rather than each window, so how long a
finished charge keeps running depends on the browser's frame schedule. It is the one place
the page's CC-CV behaviour is not a function of the simulation alone, and it is why
`what-protection-costs` can honestly say an unprotected charge finishes at 4820 s when the
current crosses the taper at 4817.5: the reader is told at the next decision boundary.

### Five numbers looked wrong, and the harness was the thing that was wrong

Step 13's five rebound figures — 17.3, 24.3, 31.7, 35.4, 37.2 mV — did not reproduce.
Measured as the voltage climb from the first *stepped* rest sample to the end of the rest,
they come out 16.85, 23.71, 31.09, 34.83 and 36.66: every one low, by 0.45 to 0.61 mV, on
numbers printed to a tenth. Step 12's twin figure for the circuit, 74.8 mV, was 71.83 by
the same reading.

A worktree at the commit that wrote them (`3784c86`, 2026-07-30, before all of Phase 7)
produced the same values as today, so the model had not moved under them. The conclusion
on offer was that six numbers had never been true.

They are all exact. The measurement is taken from the **zero-length probe** — `readNow`'s
`dt = 0` read, the engine's own contract for "what is this pack doing now" — at the instant
the current stops, not from the first step after it. A stepped sample is already `dt` into
the relaxation, which on the circuit is 2.9 mV of the RC pairs letting go and on the
particle is the same order. Probing at rest with the clock stopped gives 74.77 mV on the
circuit and 17.27 / 24.27 / 31.65 / 35.40 / 37.24 on the particle, and the whole
decomposition closes with it: 132.8 mV of instantaneous drop, 74.8 of slow climb, 5.3 that
never returns, summing to the 212.8 mV of sag the step quotes.

Two lessons, and the second is the one that generalises:

* **A harness that only samples stepped rows cannot reproduce a number the page's probe
  produced**, and the failure is silent and one-sided — every affected number reads
  slightly *low*, which is exactly what a plausible drift looks like.
* **The engine's zero-length step is not an optimisation, it is an instrument.** Three
  steps quote a reading taken before the reader presses Run (3.927 V, 2.808 V, 3.798 V,
  and `0.00 / 0.00` surface gaps); four more decompose a pulse with it. A ledger that
  cannot take that probe cannot check about a fifth of what these steps say.

## What was found

### `nothing-to-clamp`: six numbers, stale since 2026-08-12

The unprotected arm's post-clamp current sequence and its peak temperature have drifted:

| the step says | it now measures |
| --- | --- |
| 58.02 A at the flag | 58.04 A |
| 40.19 A four and a half seconds later | 40.33 A |
| 17.04 A at 250 s | 17.03 A |
| 1.95 A at 300 s | 1.94 A |
| 0.098 A at 400 s | 0.103 A |
| peaks at 376.3 K | 376.37 K, which the panel prints `376.4` |

Every one of them was exact when written. A worktree at their own commit (`6e8006f`,
2026-08-11) returns 58.022955, 40.186378, 17.044867, 1.948597, 0.098005 and a peak of
376.3408 K at 245.5 s — the prose's numbers to the digit it prints them to.

Bisected, the mover is **`2e306b2`, "Aging grows the slow resistances too"** (2026-08-12).
At `001934e`, the reversal-damage slice immediately before it, the 240 s current is still
40.186; at `2e306b2` it is 40.327. `external_short_100_milliohm.toml` carries
`[pack.aging]`, the unprotected run is pulled deep past empty, and over-discharge damage
grows `soh_resistance` — which now scales the RC pairs as well as `R0`, which is precisely
what that slice was for.

That slice registered a bit-exact prediction of its own blast radius and checked it against
the suite. This step was not in it, and could not have been: it has no claims, so there was
nothing to redden. It is the third documented instance of the same failure —
`protection-off` twice, and now this.

The peak instant is left at 245.5 s. The temperature is flat to four decimals across
245.0–245.5 s, so which of the two steps is the true maximum is not resolvable at the
precision the panel prints, and 245.5 is what makes the sentence's "ten seconds after the
flag" true.

### `three-times-the-current`: a 600-second contrast that does not exist

> It lasts 600 s longer before it clamps, at 11 880 s against the particle's 11 280

Both models raise `SOC_CLAMPED_LOW` at **11 880.5 s**. They must: the clamp is the coulomb
counter reaching zero, the two runs are the same cell at the same current from the same
charge, and coulomb counting is the same arithmetic whichever cell model is underneath it.
No reading of the particle's trajectory gives 11 280 — it falls under 1 V at 11 260.5 s and
pins at 0.3095 V at 12 586 s.

Unlike the step 19 numbers this one was **false when written**: the same worktree at
`6e8006f` clamps both models at 11 880.5 s too. It is the only claim in the fourteen steps
that was never true, and it is a claim about a *contrast* rather than a value — the kind a
golden table of numbers would not catch even if one existed, because both of its numbers
would have to be measured against the right event before the sentence could be wrong.

Repaired by saying what actually separates them, which is the better lesson anyway: they
clamp together because neither model is asked, and ten minutes earlier — at the end of the
tooth finishing at 11 280 s — the particle is already down to 0.61 V while the circuit is
still at 1.84 V.

### Everything else

The remaining ~140 numbers reproduce. Two are worth naming because they were the ones most
likely to have rotted:

* `the-gradient-itself` is exact on all sixteen of its measured figures, including the six
  surface-gap readings, the 5.80 → 5.81 tick at 518 s (about halfway through the
  discharge), the 0.0031-point drift across the plateau, 96 % of the rebound arriving
  before the negative gap first prints `0.00`, and the final 33 mV taking twenty-four
  simulated minutes.
* `looks-fine-from-outside` and `the-electrolyte-starves` are exact on all of theirs,
  including the 12-seconds-in-3484 agreement at 1 C (0.34 %) and the 2.28× disagreement at
  3 C, which is a ratio of two amp-hour integrals a hundred steps apart.

One thing that is true and worth recording without being a defect: `looks-fine-from-outside`
says "Not one flag is raised on any step of the run", which is true of the run to its
500 s mark — and then instructs the reader to press Run again and quotes readings at 1058
and 1060 s. `OPERATING_POINT_OUT_OF_WINDOW` arrives at **1062 s**, two seconds past the
last number the step quotes. The sentence is not falsified, but the step whose whole
subject is "an answer that gives you no sign it is wrong" now has a sign two steps past
where it stops looking. The previous slice's enumeration could not see this: it ran each
step to its own mark, and this arrival is on an instructed continuation.

## The ledger, designed and not built

The instrument the next slice needs is a scan over each step's **whole** prose rather than
over the sentences a claim already quotes, requiring every number to be tied to something.
Check 6 (`every_number_in_a_claimed_literal_is_accounted_for`) is the same idea confined to
claimed literals, and it could refuse a waiver variant because all 42 of its sentences
happened to need none. A whole-prose ledger cannot: most of the 350 tokens are not
measurements.

The design constraint carried over from `Accounted` is the one that makes it worth
building: **every arm is a derived numeric fact, never a declaration.** A declared
`accounts = "setting"` beside a token that is really something else is a fresh instance of
the defect `tol_from` exists to catch. The arms, each checkable against a file already in
the tree:

| arm | tied to | derivation |
| --- | --- | --- |
| `Claimed` | `path-claims.toml` | the existing six checks, unchanged |
| `Setting` | the lesson's own block | the number is a field the page acts on — `demand.value`, `ambient_c`, `until_s`, `dt`, `speed_x`, `on_s`, `off_s`, `v_cell`, `taper` |
| `Scenario` | `scenarios/*.toml` | the number is the value of a **named field** the sentence is about, not any number in the file |
| `Chemistry` | `chemistries/*.toml` | same, against the chemistry the step's scenario names |
| `Ordinal` | `web/app.js` | "step N" where N indexes a lesson that exists |
| `Derived` | the sentence itself | an arithmetic identity over other numbers in the same sentence — `×2.99`, `88 %`, `4.6 points`, `720 s past the mark` |

The `Scenario` and `Chemistry` arms are where the care goes. A generous match — "this
number appears somewhere in the scenario file" — accounts for 130 of the 350 tokens and
means nothing: a scenario file has enough numbers in it that a `2` finds `series = 2` by
accident. The arm has to name the field, which means the ledger carries a small mapping
from prose vocabulary to file keys, and that mapping is the honest cost of the check.

Two capabilities the harness needs first, both established by this slice:

1. **A zero-length probe row**, or a fifth of these steps cannot be checked at all.
2. **Instructed continuations and control changes.** `Leg` covers one demand change after
   the mark. These steps also ask for a BMS toggle, two step lengths, an ambient change,
   `Clear queued`, `Clear latched BMS fault`, and "press Run again" — all reproduced by
   the temporary harness here, none of them by `run()`.

Sizing, from the classification: about 145 claims' worth of measurement, against 67 built
over six slices. Three steps need none. It is three or four slices, not one, and an opt-in
list of ledgered steps is the right shape provided the list lives in the file and names the
steps it does not cover — the same contract `every_covered_step_exists` already keeps in
one direction.

## Deferred, with a price

* **Nothing here stops the next drift.** This slice is a measurement, and a measurement
  ages the moment it is taken. The two defects it found were both introduced by slices
  that ran a green suite; the fourteen steps are exactly as unguarded now as they were
  before, minus two wrong numbers. That is the whole argument for the ledger and it is
  unchanged by having done this.
* **The peak-temperature instant in `nothing-to-clamp` is unresolved, not verified.** The
  trajectory is flat to four decimal places across two adjacent steps, and the dump prints
  four. Reading it needs either more digits or a statement that the panel cannot tell them
  apart; the prose keeps 245.5 s because the sentence's own arithmetic depends on it.
* **`what-protection-costs`'s two perf claims and `the-electrolyte-starves`'s three are not
  measured here** — 140× and 500× per-step costs, and "about ten times the circuit's
  arithmetic". They are deliberately ratios rather than durations, for the reason step 14
  states, and this pass ran no benchmark. They are the only numbers in the fourteen steps
  that no trajectory can settle.
* **The `Derived` arm is the one with no precedent.** Every other arm checks a token
  against a file; this one checks a token against other tokens in the same sentence, which
  needs the sentence parsed into an expression. The cheap version — a list of declared
  identities — is a declaration, which is the thing the design refuses. Getting it wrong
  re-opens the hole rather than narrowing it.
* **The CC-CV completion test is still frame-dependent** and therefore still outside any
  mirror. Named above; no claim reads past a taper today.
