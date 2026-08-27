# Phase 8 slice A — one chemistry, and whether principle 10 survives it

2026-08-27. `docs/plans/phase-8-chemistries.md` slice A: **add a cheap-tier chemistry with
zero lines of Rust changed, or name the code it needed and amend `CLAUDE.md`.** Either
outcome closes exit criterion 1; only leaving it untested does not.

The pick is ruled by the phase doc, not chosen here: *take the candidate that strains the
existing model shape hardest, subject to a citable public parameter set.* On that rule the
pick is **LTO** — lithium titanate — with sodium-ion and NCA as fallbacks for the one
condition the rule is subject to. The parameter set sourced below, so the fallbacks are not
needed.

---

## Everything above the "Results" heading was written BEFORE anything was run

The predictions here come from reading `chem.rs::validate`, `plating.rs`, `aging.rs` and
`bms.rs`, plus hand arithmetic. **If the engine disagrees, the engine is right and this
section is a recorded miss** — the shape `lead-acid-data-only.md` uses, and the reason that
document is still worth reading.

---

## What was sourced, and how good each source is

| quantity | source | strength |
| --- | --- | --- |
| 20 Ah, 2.3 V nominal, 1.5–2.7 V window, −30…55 °C, 10C charge and discharge, 20,000 cycles to >70 %, ~510 g, 116 × 22 × 106 mm, 0.53 mΩ at 1 kHz AC | Toshiba SCiB 20 Ah high-energy cell, manufacturer page and distributor spec sheets | **datasheet-class** — the strongest tier this repo has outside the PyBaMM-extracted LG M50 |
| 1200 W output / 1100 W input at 50 % SOC, 10 s, 25 °C | same | **datasheet-class**, and used below as an independent cross-check on the resistances |
| OCV shape: rest voltages descending from ~2.65 V to ~2.36 V across the SOC range, collapsing near empty | Parthasarathy, Laaksonen & Halagi, *Characterisation and Modelling Lithium Titanate Oxide Battery Cell by Equivalent Circuit Modelling Technique*, IEEE PES ISGT Asia 2021, Fig. 4 (HPPC rest staircase, 2.9 Ah LTO cell) | **hand-read from a published figure** — a shape, not a fit |
| two RC pairs, one fast (charge transfer) and one slow (diffusion) | same paper, §II | structural only |
| LTO anode plateau at ~1.55 V vs Li/Li⁺; no SEI, **no lithium plating**; charging demonstrated to −40 °C | materials literature (Li₄Ti₅O₁₂ reviews) | **mechanistic and uncontested** |
| LTO uses **aluminium** current collectors on both electrodes, because the anode sits above aluminium's lithiation potential; graphite/Cu comparative cells lost ~84 % capacity under repeated over-discharge where LTO/Al cells lost "little or no" capacity | over-discharge-tolerance patent literature and LTO reviews | **mechanistic**, with one quantitative comparative |

Everything not in that table is a labelled placeholder with a sizing argument, per the
`CLAUDE.md` provenance rule. **Unfitted is allowed; unlabelled is not.**

## Why LTO is the hard case and not a recoloured NMC

Four ways this file leaves the shape the shipped lithium files share:

1. **Its whole voltage window sits below every other lithium file's floor.** LTO tops out at
   2.70 V; the NMC 18650 file's *discharge cutoff* is 3.00 V. Only the 2 V lead-acid file is
   lower, and that one needed code.
2. **Its rate rating is 10C both ways**, against 0.3–3C for everything else shipped.
3. **Its `[reversal]` answer differs in kind, not in size.** The LFP and NMC files bill
   over-discharge against copper current-collector dissolution. This cell has no copper.
4. **It is the first shipped file where `[cell] t_charge_min_k` and `[safety]`'s
   `t_plating_min_k` do not coincide** — and the reason they cannot coincide is the finding
   below.

## The finding that is already certain, from reading rather than running

**`[safety]` has no way to say "this cell does not plate lithium."** Established by reading
`chem.rs::validate` and `plating.rs::plating_risk` before writing a line of TOML:

* The *cost* fields — `plating_fade_per_ah`, `plating_short_hazard_per_ah` — default to
  zero, but zero means **"the flag is raised and it costs nothing."** For LTO the flag
  itself is the lie; a cell that cannot plate should not report plating risk.
* Dropping `[safety]` entirely does turn plating off — and turns **thermal runaway off with
  it**, because the section is one `Option` covering two unrelated mechanisms. LTO is
  markedly *more* thermally stable than NMC, not immune, and a file that says nothing about
  its onset temperature is worse than one that says it late.
* That leaves the two gate parameters. `t_plating_min_k` is validated finite-and-positive
  with **no cross-check against anything**, so "never" has to be spelled as a temperature
  below every reachable one. `plating_c_threshold` has the same shape at the other end.

So the file ships a **sentinel**: `t_plating_min_k = 1.0`, one kelvin, labelled in the file
as a sentinel rather than a measurement. Absurd on its face is the point — no reader will
mistake it for a datasheet number.

**Whether that counts as "code the chemistry needed" is pre-registered here, before the
result is known**, because deciding afterwards is how a criterion gets closed by the
weakest reading available:

> **"Code the chemistry needed" means a change without which the chemistry would fail to
> load, fail to validate, or behave wrongly.** A sentinel that loads, validates and behaves
> correctly is not that. A stale doc comment is not that either — but it *is* a defect, and
> is fixed and declared rather than left.

On that rule the sentinel is **evidence for principle 10 with a caveat**, not against it,
and the caveat is worth writing into the phase's record: the schema expresses "mechanism
absent" by *section absence* for `[diffusion]`, `[spm]`, `[dfn]` and `[aging]`, but the two
mechanisms inside `[safety]` share one `Option` and cannot be switched independently.

## Predictions, registered before the first run

**P1 — zero functional Rust.** The file loads, validates and drives a pack with no change
to any `.rs` file. *Confidence: high; this is the hypothesis under test.*

**P2 — one doc comment goes stale.** `chem.rs`'s `t_plating_min_k` doc says "The two
coincide in both shipped chemistries." It is already stale (there are four files, and the
lead-acid one has no `[safety]` at all); this file makes it *false* rather than merely
out of date. Fixed and declared, not counted against P1.

**P3 — the rate claim, with a number.** Discharged at 10C to `v_min`, this cell delivers
**≥ 95 %** of the amp-hours it delivers at 1C. The shipped NMC 18650 file, driven at the
same 10C, delivers **≤ 50 %** of its own 1C figure. *This is the assertion the NMC file
fails, and it exists because "voltage falls under load" is an assertion all four shipped
files pass and is therefore evidence about nothing.*

**P4 — the plating contrast.** Charging at 4C at 243.15 K (−30 °C), the LTO cell raises
`PLATING_RISK` on **no** step. The NMC file under the identical demand raises it. Both from
parameters alone.

**P5 — a dormant code path wakes up on data.** `cyc_dod_stress_exp = 1.0` takes
`aging::cycle_increment`'s `exponent == 0.0` branch (pure throughput counting, weight
exactly 1.0). No shipped chemistry reaches it — LFP is 1.1, NMC 1.2, lead-acid 1.3. *If
this is right it is a small bonus finding: a branch that existed for a chemistry nobody had
written yet.*

**P6 — the floor does not move.** Exit criterion 3 is structural here, not measured: a file
no existing pack loads cannot move an existing trajectory. Every other test in the workspace
stays green and unchanged.

## The paired arithmetic, so a placeholder that is nonsense at scale shows up now

House style from the NMC file: numbers that only mean anything together get their product
written down.

**Resistances against the datasheet's power ratings.** The 1 kHz AC impedance (0.53 mΩ)
under-reads DC resistance because it misses charge transfer, so `R0` is set at 0.75 mΩ
(1.4× the AC figure) with RC pairs of 0.30 mΩ (τ = 6 s) and 0.25 mΩ (τ = 40 s). At the
datasheet's 10-second measurement point the settled resistance is

    0.75 + 0.30·(1 − e^(−10/6)) + 0.25·(1 − e^(−10/40)) = 1.05 mΩ

Discharging from a 2.3 V nominal to the 1.5 V floor through 1.05 mΩ draws 763 A for
**1145 W**, against the datasheet's 1200 W. Charging from 2.3 V to the 2.7 V ceiling draws
382 A for **1031 W**, against the datasheet's 1100 W. Two independent checks, both within
5 %, on numbers that were not fitted to them.

**Cycle fade against the cycle-life claim, which is a derivation and not a placeholder.**
20,000 full cycles to 70 % retention. `aging::cycle_increment` counts throughput as
`|I|·dt` in both directions, so one full-depth cycle of a 20 Ah cell moves 40 Ah:

    cyc_fade_per_ah = 0.30 / (20000 × 40 Ah) = 3.75e-7

Eighty times gentler per amp-hour than the LFP file's placeholder and a hundred times
gentler than the NMC file's, which is the ordering LTO exists for. **The one assumption is
that the rated cycle is full-depth**; if the rating were quoted at a shallower depth the
per-amp-hour figure would be larger, so this is the optimistic end of the claim.

**Calendar fade.** `cal_pre_exp = 3.4e3` with `Ea = 5.0e4` gives
`k = 3.4e3 · e^(−50000/(8.314·298.15)) = 5.90e-6`, and at full charge (`soc_stress` 1.2)
over a year (`√3.156e7 s` = 5618) that is **4.0 % in a year at 25 °C** — against 13.7 % for
the LFP set and 15.8 % for the NMC set, computed the same way from their own files.

**Runaway.** `runaway_energy_j / heat_capacity_j_per_k` = 102 kJ / 510 J/K = a **200 K**
adiabatic rise, so a fully reacted cell peaks near 693 K (420 °C) — the mildest of the three
shipped runaway sets (LFP 253 K, NMC 818 K), which is the ordering LTO's reputation implies.
Onset to vent is 40 K × 510 J/K = 20.4 kJ, released at 10 W rising to about 62 W across that
span, so an adiabatic cell at onset vents in roughly **12 minutes** — against 3 minutes for
LFP and 1.5 for NMC.

**Thermal.** 510 g at ~1000 J/(kg·K) gives 510 J/K. The prismatic case is
2(0.116·0.106) + 2(0.116·0.022) + 2(0.106·0.022) = 0.0344 m²; at a natural-convection
h ≈ 8 W/(m²·K) that is **0.28 W/K**.

---

## Results

Every prediction scored, and every miss reported as a miss. Measured 2026-08-27.

**P1 — CONFIRMED. Zero functional Rust.** `chemistries/lto_20ah_generic.toml` loads,
validates, builds a pack and runs a full charge and discharge with **no change to any engine
source file**. The only `.rs` edits in the slice are the doc-comment fix P2 predicted and the
tests, neither of which is code the chemistry needed by the rule registered above. **Exit
criterion 1 closes on the zero-code branch**, with the `[safety]` caveat recorded — and with
a cost that was not free and is declared below.

**P2 — CONFIRMED.** `chem.rs`'s `t_plating_min_k` doc claimed the physics threshold and the
BMS charge-inhibit limit "coincide in both shipped chemistries". Amended, and extended with a
section naming why this cell is the one where they cannot.

**P3 — CONFIRMED**, by `ten_c_costs_lto_almost_nothing_and_nmc_most_of_its_capacity`:

| file | 1C delivered | 10C delivered | retained at 10C | predicted |
| --- | --- | --- | --- | --- |
| LTO 20 Ah | 19.974 Ah | 19.722 Ah | **98.7 %** | ≥ 95 % |
| NMC 18650 | 2.964 Ah | 1.093 Ah | **36.9 %** | ≤ 50 % |

**P4 — CONFIRMED.** `cold_fast_charge_plates_the_nmc_cell_and_not_the_lto_cell` charges both
files at 4C from 20 % SOC at 243.15 K for 300 s. The NMC pack raises `PLATING_RISK`; the LTO
pack raises it on no step. Parameters alone — no engine code distinguishes the two cells.

**P5 — CONFIRMED.** `cyc_dod_stress_exp = 1.0` makes this the first shipped chemistry to take
`cycle_increment`'s `exponent == 0.0` branch.
`lto_is_the_first_chemistry_to_count_pure_throughput` pins it at the one point where the two
weightings disagree completely — a zero-depth micro-cycle, which LTO bills in full and every
depth-weighted chemistry bills at nothing.

**P6 — CONFIRMED.** `cargo test --workspace --no-fail-fast` exits 0, every test binary
green, with no golden and no pre-existing assertion touched. Criterion 3 held structurally:
no existing pack loads this file. **Scored on the exit code, not on a count of green lines:**
without `--no-fail-fast` the run stops at the first failing binary, so a tally of "ok"
lines says nothing on its own — the same trap the perturbation table below walked into.

### The strongest objection to P1, and the answer

**"The chemistry needed the guard loosened."** It is the one place a skeptic has a real
target: the cycle-fade band's ceiling moved from 10,000 to 50,000 full cycles, and it moved
because LTO would not fit under the old one. Stated plainly rather than folded into a list,
because burying it is how a criterion gets closed by the weakest available reading.

The answer is that the old band was **not** a plausibility statement. "500 full cycles cost
1–50 % of capacity" is a claim about how long a cell lives, and it is only a wide, harmless
claim if every cell lives about as long as a graphite cell. The moment a chemistry rated at
20,000 cycles arrives, that band is asserting the chemistry assumption, not guarding against
an unevaluated coefficient. The replacement — full cycles to 20 % capacity loss — is the
quantity a datasheet actually quotes, so a reader can check it against a source rather than
against a convention, and it *tightens* the floor (200 → 300 cycles) while loosening the
ceiling.

**The slack is real and is deliberate.** The new ceiling is 50,000 where this cell needs
13,333, so a future file could sit anywhere up to 3.75× beyond LTO unchallenged. That is the
intended headroom: published cycle ratings above 20,000 exist, and a band that stops exactly
at the best cell currently in the tree would have to move again on the next one. A band that
has to move for every new chemistry is not a band.

What the objection does **not** reach: no engine source file changed. The guard is a test, and
under the rule registered before the result was known, a test that encoded a chemistry
assumption is a defect in the test.

### The measured coefficients, for the record

| quantity | LFP | NMC | **LTO** |
| --- | --- | --- | --- |
| calendar fade, 1 year at 25 °C, full charge | 13.67 % | 15.86 % | **3.99 %** |
| full cycles to 20 % capacity loss | 2,171 | 1,111 | **13,333** |
| adiabatic runaway ceiling | 253 K | 818 K | **200 K** |
| adiabatic onset → vent | — | 98 s | **988 s** |
| capacity lost to 2 % of capacity past empty | — | 0.9926 % | **0.0003 %** |

The 13,333 figure is the derivation closing on itself: 20 % loss at 13,333 cycles is the same
line as the datasheet's 30 % at 20,000, which is where `cyc_fade_per_ah` came from.

### Two guards in the repo assumed every cell is a graphite cell

Not predicted, and the most useful thing the slice found. Both band tests in
`crates/sim-data/tests/load.rs` said "every shipped chemistry" while enumerating two files by
hand, and **both rejected LTO for being what LTO is**:

* **The cycle-fade band** asked that 500 full cycles cost between 1 % and 50 % of capacity.
  LTO's 500 cycles cost **0.75 %**, and it fell out of the bottom of the band while being
  exactly right. Rewritten as full cycles to 20 % capacity loss, banded 300–50,000 — see the
  objection above, which this is the target of.
* **The plating band** asked that one full cold charge cost between 0.05 % and 5 % capacity
  and carry a 0.01–5 % short hazard. For a cell that cannot plate, all three figures are
  zero. The test now has two arms: **a chemistry either prices plating or cannot reach it**,
  and the second arm asserts the gate is genuinely shut (`t_plating_min_k` strictly below the
  cell's own charge floor) so that a *declared* absence is not confused with a half-written
  file.

This is the same assumption as the `[safety]` schema one, in a second place: the repo encoded
"every cell plates" once in a struct and once in a test. **Neither is engine code and neither
counts against P1 under the registered rule — but neither was free.**

### Three of my own numbers were wrong, all caught by measuring

* **The runaway timing.** The paired arithmetic above predicted onset-to-vent in "roughly 12
  minutes", from averaging the exothermic power at the two ends of the span (10 W and 62 W →
  28 W). Integrated through the engine's own `reaction_power` it is **988 s, about 16.5
  minutes**. The Arrhenius rise is convex, so the cell spends most of the span near the cold
  end: the true mean power is 20.6 W, the endpoint average runs **36 % fast**, and the
  prediction was **26 % short**. Corrected in the file. *Averaging the endpoints of an
  exponential is not averaging the exponential* — worth naming because the same arithmetic
  appears in the LFP and NMC files' comments.
* **The fade ratio.** The file first said LTO's per-amp-hour cycle fade was "80× gentler than
  the LFP placeholder and 80× gentler than the NMC one". It is **53×** against LFP and 80×
  against NMC; the two were never the same number. Corrected.
* **"Over-discharge costs this cell nothing" — as first written, false.** The over-discharge
  test originally ran one arm and asserted the LTO cell lost essentially no capacity. It
  lost **0.038 %**, and every bit of that was ordinary calendar fade over the hour the
  discharge takes. Calendar loss goes as `√t`, so a cell's first hour is worth **1/93rd** of
  its first year rather than 1/8766th — the curve is steepest where nothing has happened yet.
  The fix is a control arm: run to empty, run 72 s past empty, subtract. The shelf time
  cancels and what is left is the damage. **A claim about a rare event measured without a
  control is a claim about whatever else was happening at the time.**

### And one that was caught before anything ran

`[reversal] v_per_soc` was first sized at 30.0 — a collapse spread over 5 % of capacity rather
than the 2 % the LFP and NMC files use — reasoning that LTO's gentler over-discharge behaviour
should show up in the ramp. **Wrong, and changed before the first run.** The ramp is the
*voltage* collapse below empty; what the aluminium collector changes is the *damage*, which is
`fade_per_ah`. Sizing both differently would have smeared this chemistry's one real
distinction across two fields and made neither legible. The file keeps the shipped sizing rule
exactly (`1.50 / 0.02 = 75.0`) and spends the whole difference in the one field the source
speaks to.

### `fade_per_ah = 0.0`, and why that is a statement rather than a gap

The LFP and NMC files bill over-discharge at ~1e-1 capacity fraction per amp-hour past empty,
sized against dissolution of the anode's copper current collector. **This cell has no
copper.** The LTO anode sits at ~1.55 V vs Li/Li⁺, above aluminium's lithiation potential, so
both electrodes use aluminium foil and the mechanism the other files bill for does not exist
here. The comparative in the over-discharge literature is stark: graphite-on-copper cells
averaged ~84 % capacity loss under repeated over-discharge cycling where LTO-on-aluminium
cells lost little or none.

Zero is what the field's own documentation sanctions for that case (`0 = over-discharge is
free`), and it is a *structural* claim, not a missing fit. What zero still hides is named in
the file: cathode over-lithiation and electrolyte reduction are real, far smaller, and
unfitted. It does **not** disable the voltage collapse — `v_per_soc` and `floor_v` are
independent of `fade_per_ah`, so the cell still falls through its reversal ramp and the
external circuit still pays for the energy. It is the *damage* that is free, which is the
physics.

### Perturbations

Ten cases across two rounds. Each edits one number in the chemistry file and names the check
it must redden; the file is restored from an in-memory copy rather than by `git checkout`,
which would revert the whole slice and flip the line endings.

Round one covered the three band tests in `load.rs`:

| perturbation | reddened |
| --- | --- |
| `cyc_fade_per_ah` 100× gentler (1.3 M cycles to 20 %) | the aging band |
| `cal_pre_exp` 100× larger | the aging band |
| `runaway_energy_j` 100× too small (2 K ceiling) | the runaway band |
| `t_plating_min_k` sentinel raised to 273.15 K | the plating band's no-plating arm |

Round two covered the five behavioural tests, one perturbation per claim:

| perturbation | reddened |
| --- | --- |
| `t_plating_min_k` sentinel raised to 273.15 K | the cold-charge contrast, **and** the plating band |
| `cyc_dod_stress_exp` 1.0 → 1.1 | the pure-throughput test, **alone** |
| the whole `[r0]` grid 10× | the 10C rate claim |
| `v_max` 2.75 → 3.10 | the window guard, **and** the loading test |
| `[reversal] fade_per_ah` 0.0 → the NMC value | the over-discharge contrast |
| CONTROL: a comment reworded, no number touched | nothing |

**Round one was wrong about the first case and reported it green on the wrong check.** With
`t_plating_min_k` raised, it named only the band test — because `cargo test` stops at the
first failing *binary*, and `load` fails before `lto_chemistry` ever runs. Round two adds
`--no-fail-fast` and the cold-charge contrast appears where it should have all along. The trap
is already recorded in this repo's notes and it was still walked into; a perturbation table
built without `--no-fail-fast` **understates coverage and cannot tell you it is doing so**.

**The depth-exponent case is the informative one.** Moving `cyc_dod_stress_exp` off 1.0
reddens the pure-throughput test and *nothing else* — not the aging band, which reads the
weight at full depth where `dod^(exp−1)` is 1.0 for every exponent. The band is structurally
blind to that parameter, which is exactly why the test asserting it had to be written
separately rather than assumed covered.

## What this slice did not do

* **No scenario file, no guided-path step, no claim in `path-claims.toml`.** Those are slice
  B, and the digits rule from slice 0 binds prose that is not written yet. Keeping slice A to
  one TOML plus tests is what makes exit criterion 3 structural instead of measured.
* **No entropic term.** `docv_dt_v_per_k` is omitted, as in the LFP and NMC files. LTO's
  entropy profile is unusual and worth having, but it is a fit this slice did not do.
* **The `[safety]` coupling is recorded, not fixed.** Splitting plating and runaway into two
  independently-absent sections is a schema change with a `SNAPSHOT_VERSION` question
  attached, and the phase spends its one bump on slice C. Written down so the next session
  finds it rather than rediscovering it.
* **Two shipped files are still outside the band tests.** `nmc_21700_lgm50.toml` and
  `pba_agm_2v_generic.toml` are not in the hand-written lists those tests enumerate, and
  making the lists a directory scan is a separate job. Declared rather than quietly left: the
  tests say "every shipped chemistry" and cover three of five.
* **`plating_c_threshold = 0.0` is deliberately the most permissive value**, not a second
  sentinel. If the temperature sentinel were ever "corrected" upward by someone who had not
  read the comment, this cell would flag plating on the first cold charge, immediately and
  loudly, rather than drifting into a plausible-looking wrong answer. One sentinel, and a
  tripwire behind it — and the round-two perturbation above is that tripwire being pulled on
  purpose.

## What this prices for slice B

The chemistry is reachable by id through the existing scenario mechanism, so nothing blocks
wiring it up. Two lessons it can carry, both contrasts against steps the guided path already
has:

1. **The same cold fast charge, a different outcome.** The path already teaches cold-charge
   plating on a graphite cell. Re-running that lesson's demand on this cell and getting no
   flag at all is a one-scenario lesson with its control arm already built.
2. **10C.** Nothing else in the path goes above 3C, and the rate contrast here is 98.7 %
   against 36.9 % — large enough to read off a plot without a number beside it.
