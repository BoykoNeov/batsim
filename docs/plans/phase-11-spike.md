# Phase 11, the spike — what a fit can replace in the LG M50 file, and what it costs

**Status: RUN 2026-10-15. No Rust.** PyBaMM 26.6.2.0, DFN, isothermal, `Chen2020` and
`ORegan2022` (both the LG M50 cell). **Scouting precision**: 50 radial points and `rtol = atol
= 1e-8`, not the converged 200 / 1e-10 the committed goldens use (`common.Reference`), so every
millivolt below is good to about a millivolt and no more. Harness and raw output:
`W:/temp/claude/phase11-spike/` (`hppc_gen2.py`, `fit_spike.py`, `rdc.py`, `imax10.py`, `entropic.py`, `apply_fit.py`, the
`*.npz` runs, `fit*.txt`); kept until slice A rebuilds it as `tools/reference/fit_ecm.py`. The
plan this feeds is not written yet: it waits on the owner's decisions at the foot.

## The question

`docs/ROADMAP.md` §Phase 11 proposed four slices — (A) fit the LG M50 circuit to the DFN,
(B) entropic coefficients where a set publishes them, (C) a lead-acid `[diffusion]` refit, (D)
the lesson re-measurement — and the exit "no constant in `nmc_21700_lgm50.toml` is labelled
placeholder", pinned by a test that greps for the label. Each part was measured rather than
assumed.

## 1. The circuit fit (slice A)

An HPPC-like train on the Chen2020 DFN at 263.15, 298.15 and 318.15 K: from full, at each 10 %
of charge, a 30 s discharge pulse at 1C, 300 s rest, a 30 s charge pulse at 0.75C, 300 s rest,
then a C/2 move of 10 % and an 1800 s rest. The rests are long enough: the voltage still moves
at most 0.07 mV over the last 60 s of any of them. The circuit was simulated the engine's way
(exact exponential RC update under piecewise-constant current, voltage at the end of the step)
against the **reference's own OCV** along the coulomb count, so the `[ocv]` table's 13.91 mV
interpolation error does not land in the resistances. Fitted: `R0` at 11 state-of-charge nodes
per temperature, and **two RC pairs shared across temperatures**, because `[[rc]]` holds plain
numbers — the only thing the schema lets vary with state of charge and temperature is `R0`.

| | 263 K RMS / max | 298 K RMS / max | 318 K RMS / max |
| --- | --- | --- | --- |
| shipped placeholders | 13.84 / 110.4 mV | 10.28 / 51.5 mV | 8.68 / 50.0 mV |
| fitted, RC shared (the schema) | 8.31 / 92.8 mV | **4.01** / 44.0 mV | 4.12 / 41.3 mV |
| fitted, RC per temperature (the schema forbids) | 7.66 / 78.5 mV | 3.81 / 48.9 mV | 3.61 / 36.6 mV |

The shared pairs come out at **17.2 mΩ / 40 s and 9.0 mΩ / 333 s**, against the placeholders'
10 mΩ / 9 s and 8 mΩ / 72 s. The schema's restriction costs little (4.01 against 3.81 mV at
25 °C). Where the error sits, at 298 K: rests 3.2 mV RMS, C/2 moves 5.0, **pulses 12.4** (charge
11.4) — the pulses are what the lessons read, and they are 1 % of the samples. Weighting them
×8 moved the pulse fit little and the whole fit slightly worse, so weighting is a slice-A
choice, not a fix. The fitted `R0` zig-zags between neighbouring nodes (20.5 mΩ at 90 %, 29.8 at
80 %): eleven free nodes absorb what a linear circuit cannot represent, so slice A needs fewer
nodes or a smoothness penalty, and a residual stated per segment, not one number.

**Against the real cell.** LG's specification sheet (below) gives the DC resistance at 50 %
charge, 0.5C for 30 s, 25 °C: **30 ± 6 mΩ**. The fitted circuit's 30 s resistance there is
`R0 + Σ R(1 − e^(−30/τ))` ≈ 27 + 9 + 1 = 37 mΩ, just outside; the DFN itself reads 35 mΩ at 1C
(§3). An independent check slice A can carry as a test.

## 2. One circuit cannot be right at both lesson rates

The same train at 3C (298 K). A linear circuit answers 3× the current with exactly 3× the drop;
the DFN does not, because its charge-transfer overpotential goes as the arcsinh of current.

| applied to the 3C run | RMS (state of charge > 5 %) |
| --- | --- |
| shipped placeholders | 29.95 mV (all) |
| the 1C fit | 18.09 mV |
| a fit to the 3C run itself | 10.65 mV |

The best mid-range `R0` at 3C is ~16 mΩ against ~24 mΩ at 1C. So a fitted circuit has a
**design rate**, and its provenance line has to say which and carry the other rate's residual.
The lessons' point about the circuit — "×3.00, because that is what linear means" — survives any
fit unchanged; the millivolts under it do not (§6). (The 3C run's ~1 V maximum is its last pulse
at 4 % charge, where the DFN collapses; the circuit cannot follow that at any fit.)

## 3. Cold: the parameter set the file uses does not get cold enough

Chen2020 publishes an activation energy for its kinetics only; its solid diffusion and its
electrolyte do not depend on temperature (`[spm]` says so, with a TODO). ORegan2022 — the same
cell, a later parameterisation — has Arrhenius transport throughout. The 30 s pulse resistance
at about 50 % charge, 1C (`rdc.py`):

| source | 25 °C | −10 °C | ratio |
| --- | --- | --- | --- |
| Chen2020 DFN | 35 mΩ | 52 mΩ | **1.5×** |
| ORegan2022 DFN | 29 mΩ | 72 mΩ | **2.5×** |
| LG specification sheet | 30 ± 6 mΩ (0.5C) | — | — |

That compares the models with each other at one current; LG's sheet does not give the cold
resistance directly. What it does give is a table of the largest current that reaches 2.50 V in
10 s from each state of charge — so the like-for-like check is to find that current in each
model (`imax10.py`: bisection on a 10 s constant-current DFN run from 50 %, isothermal):

| largest 10 s current from 50 % to 2.50 V | 24–25 °C | −10 °C |
| --- | --- | --- |
| LG specification sheet | 37 A | 14 A |
| ORegan2022 DFN | 40.9 A | 22.1 A |
| Chen2020 DFN | 55.2 A | **54.1 A** |

**At pulse-power currents Chen2020 has almost no cold penalty** (55 → 54 A): what limits a
7–10C pulse in it is transport it does not make temperature-dependent, so the colder kinetics
barely show. It is also 1.5× too generous at room temperature. ORegan2022 is close at room
temperature (41 against 37 A) and still too generous cold (22 against 14 A). The models run
isothermal and the real cell warms during such a pulse, so this is a comparison of the sheet's
own measure, not of resistances. Summary for decision 2: **Chen2020 is the wrong source for
the circuit's temperature behaviour; ORegan2022 is much closer but not a match.** Fitting the
circuit to ORegan2022 outright was tried and is messy (its own stoichiometry window, 5.048 Ah
against Chen2020's 5.153, puts `R0` nodes at zero); taking its *temperature ratio* onto the
Chen2020 fit is clean. ORegan2022 and OKane2022 also give the activation energy `[spm]`'s TODO asks for, which would
make the porous halves colder too — no golden moves (they run at the reference temperature),
but every cold porous test would.

## 4. Entropic heat (slice B)

Chen2020's entropic keys are literally 0. ORegan2022's, as a cell coefficient `dU_p/dT(y) −
dU_n/dT(x)`:

| state of charge | 0 | 0.05 | 0.1 | 0.2 | 0.5 | 0.8 | 0.95 | 1.0 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| on ORegan2022's window [mV/K] | −0.240 | −0.409 | −0.300 | −0.075 | +0.017 | +0.060 | +0.116 | +0.098 |
| on Chen2020's window [mV/K] | −0.263 | −0.431 | −0.369 | −0.076 | +0.019 | +0.052 | +0.103 | +0.100 |

Mapping it onto Chen2020's stoichiometry window moves it by at most 0.07 mV/K, so mixing the two
sets costs little numerically. It is not small physically: at 1C near empty, `I·T·dU/dT` ≈ 0.6
W against `I²R` ≈ 0.9 W. **Only the circuit can take it without Rust**: `[ocv]
docv_dt_v_per_k` is a column on the state-of-charge grid, while `[spm] docp_dt_v_per_k` is one
number per electrode. Shipping it on the circuit alone makes "the same cell" generate reversible
heat in one model and none in the other two. **Its cost was not measured**: it moves every LG
M50 circuit run that is not isothermal, and it has a sub-choice — heat only, or heat plus the
voltage correction `OCV + dU/dT·(T − t_ref_k)` that a stated `[ocv] t_ref_k` switches on, which
moves voltages away from 25 °C too.

## 5. What each labelled constant could be replaced by

Every line of `nmc_21700_lgm50.toml` the file itself labels placeholder or TODO (the header and
`[meta]` prose aside), with the source this spike found for it:

| section | constants | source found | slice |
| --- | --- | --- | --- |
| `[cell]` | `max_charge_c`, `max_discharge_c`, `t_max_k` | **LG specification sheet**: 0.7C (25–45 °C), 1.5C (25–55 °C), charge 0–45 °C / discharge −20–55 °C. One `t_max_k` for both directions — the schema cannot say "45 to charge, 55 to discharge" | data |
| `[r0]`, `[[rc]]` | 9 + 4 | the DFN fit, §1–3 | A |
| `[thermal]` | heat capacity, a stated lower bound (55.8 g of a cell it says weighs 68–70 g) | **LG sheet: 68.5 ± 1.0 g**; ORegan2022 gives measured component heat capacities | data |
| `[ocv]` | the grid TODO (13.91 mV error; "if this file ever carries ECM goldens") | a per-chemistry grid in `fit_ocv.py`; slice A makes the condition true | A |
| `[ocv]` entropic | absent (Chen2020's 0) | ORegan2022, §4 | B |
| `[spm]` | diffusion activation energy (TODO) | ORegan2022 / OKane2022 | B or none |
| `[aging]` calendar | `cal_pre_exp`, `cal_ea_j_per_mol`, `cal_soc_stress` | **the WMG storage data already in the repo** (`tests/golden/nmc_21700_lgm50/wmg_storage_growth.csv`) | new |
| `[aging]` | `r_growth_per_capacity_loss` | the same WMG set's pulse-resistance rise — a **weak** source: Phase 10 slice A's fit to it left a 1.13 mΩ residual against a 1.29 mΩ signal | new |
| `[aging]` cycle | `cyc_fade_per_ah`, `cyc_dod_stress_exp` | LG sheet bounds it only (≥ 80 % energy after 500 cycles at 0.33C); no data to fit an exponent | none |
| `[reversal]` | 3 | none in hand | none |
| `[safety]` runaway | `t_onset_k`, `t_vent_k`, `runaway_energy_j`, `runaway_power_w_at_onset`, `runaway_ea_j_per_mol` | none in hand (ARC studies of this cell exist in the literature; not read) | none |
| `[safety]` plating | `plating_c_threshold`, `plating_fade_per_ah`, `plating_short_hazard_per_ah`, `plating_short_ohms` | none in hand (OKane2022 has a plating *model*, not these constants) | none |
| `[spm.sei]` | `initial_thickness_m` (Chen2020's own placeholder) | none | none |

**The roadmap's exit is not reachable by this phase.** Fifteen constants have no source the
project holds (cycle 2, reversal 3, runaway 5, plating 4, the film's starting thickness 1;
sixteen if the porous activation energy is declined), and the fit pipeline cannot make one. Two further points for the exit's test:
a grep for the word passes if someone deletes the word, so the test has to check what replaced
it (each fitted value's provenance names its fit and its residual); and "placeholder" also sits
in prose a grep must not count.

**The LG specification sheet**: LG Chem, *Product Specification, Rechargeable Lithium Ion
Battery, Model INR21700 M50 18.2Wh*, document LRB-PS-CY18.2Wh-M50, 2017-09-22, revision 0
("Tentative"), fetched from a distributor's copy
(`asset.conrad.com/media10/add/160267/c1/-/en/002114679DS01/…`). It is stamped **"CONFIDENTIAL"**
and **"Tentative"** (revision 0, a draft) on every page; whether a document marked so, obtained
from a retailer, is a citable source under the provenance rule is the owner's call (decision 1).
Each value cited from it would say all of this.

## 6. What a fit costs the guided path

Measured, not estimated: a scratch worktree with the LG M50 `[r0]` and both `[[rc]]` resistances
scaled by 1.1, the whole workspace suite run (`--no-fail-fast`, below normal priority). **One
test binary went red, `path_claims`; nothing else in the workspace reads these values.** With
its claim check switched (in the worktree only) from first-failure to report-all:

| step | lesson | claims moved |
| --- | --- | --- |
| 2 | `same-discharge-other-chemistry` | 1 (the "620 mV") |
| 12 | `circuit-repeats-itself` | 15 (every millivolt of the tooth, its 99.5 %) |
| 14 | `three-times-the-current` | 7 (397.3, 224.3 mV, the circuit's 1.84, −0.442, −0.657 V) |

Plus one sentence whose **words** go false, caught by the numeral check: step 13 says "the RC
pairs are 9 s and 72 s because someone picked round numbers". A fitted circuit has 40 s and
333 s, picked by nobody, and the step's comparison ("a comparison against placeholders") has to
be rewritten, not re-measured.

**Then with the spike's own fit in the file** (its 11 × 3 `R0` grid and the two shared pairs,
`c_farad = τ/R`), the same run: again only `path_claims` red, now **25 claims** (step 12: 16,
step 2: 2, step 14: 7). A nudge understated what kind of cost it is — **two lessons need
redesigning, not re-measuring**:

* **Step 12's teeth stop being identical.** Its rest is 600 s; the fitted slow pair's 333 s
  leaves `e^(−600/333)` ≈ 16 % of it standing when the next pulse starts (the placeholders' 72 s
  left 2 × 10⁻⁴). The five rebounds read 75.35, 76.23, 76.35, 76.37, 76.37 mV, so "identical to
  four decimal places" is false in words; "99.5 % within 300 s" becomes 97.5 %; and the sag's
  "two levels" (212.8 … 221.3 mV, the `[ocv]` node passing under the train) become a staircase,
  195.6 → 236.5 mV, mostly the fitted `R0`'s zig-zag between 80 and 90 % (§1).
* **Step 14's "×3, because that is what linear means" breaks.** The instant drop goes 113.75 →
  389.15 mV between the 1C and 3C runs, **×3.42**, not ×2.99 (the rebound stays ×3.00). A 3C
  pulse travels three times as far along the state of charge, and a fitted `R0` that varies with
  state of charge reads a different resistance at the end of it. The scenario file's "no choice
  of R0 and RC can escape it" is true only of an `R0` that is flat over the pulse — which the
  placeholders were, between 50 and 100 %, and a fit is not. Smoothing `R0` would shrink the
  ×3.42, not remove it.
* Step 2's two numbers (620 → 640 mV, 618 → 626 mV) are a re-measure. If the `[aging]` calendar is fitted too, steps 38–39 (the shelf)
lose their contrast — "15.86 points … the unfitted placeholder" against the film's 1.40 — and
need rewriting rather than re-measuring. Slice C (lead-acid) was not measured here.

## Decisions for the owner

**Taken 2026-10-15** — recorded, with three follow-ups the measurements forced, in
`phase-11-fitting.md` §"The owner's decisions". The list below is as it was put.

1. **The exit.** Narrow it to what has a source (the circuit, `[cell]`, `[thermal]`, entropic,
   and — if 4 says so — `[aging]`'s calendar and resistance growth), with the remaining
   placeholders on a named list the test holds fixed; or keep it whole and add a literature
   search for runaway, plating and over-discharge data.
2. **Cold.** Fit the circuit's temperature behaviour to Chen2020 (one parameter set; almost no
   cold penalty at pulse currents, 54 A at −10 °C where the real cell gives 14), or take the
   temperature ratio from ORegan2022 (22 A — much closer, not a match; two sets named in one
   file). Separately: give the porous models ORegan2022's diffusion activation
   energy (colder porous cells; every cold porous test moves).
3. **Entropic heat.** On the circuit only (no Rust; the models disagree about heat), on all three
   (a Rust change: `[spm]`'s coefficient becomes a table), or not this phase.
4. **The drawn aging curve.** Fit it to the WMG storage data (the shelf lesson is rewritten: two
   fitted descriptions instead of a placeholder against physics), or keep it as the shelf
   lesson's deliberate placeholder.

Not asked, a default unless overruled: the circuit is fitted at **1C**, the lessons' first rate,
with the 3C residual in its provenance; and the `[ocv]` grid is re-shaped for this file in the
same slice, since the fit makes its own TODO's condition true. The re-grid's own cost was not
measured: the 4.0 mV above is the circuit's error with the table taken out, and an engine run
adds the table's error back unless the grid changes — step 2's 620 mV / 5708.5 s sentence reads
that table directly.
