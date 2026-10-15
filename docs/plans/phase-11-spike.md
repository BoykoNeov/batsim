# Phase 11, the spike — what a fit can replace in the LG M50 file, and what it costs

**Status: RUN 2026-10-15. No Rust.** PyBaMM 26.6.2.0, DFN, isothermal, `Chen2020` and
`ORegan2022` (both the LG M50 cell). **Scouting precision**: 50 radial points and `rtol = atol
= 1e-8`, not the converged 200 / 1e-10 the committed goldens use (`common.Reference`), so every
millivolt below is good to about a millivolt and no more. Harness and raw output:
`W:/temp/claude/phase11-spike/` (`hppc_gen2.py`, `fit_spike.py`, `rdc.py`, `entropic.py`, the
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
| LG specification sheet | 30 ± 6 mΩ (0.5C) | ≈ 89 mΩ implied | **≈ 2.6×** |

The sheet's last row is read off its table of the largest current that reaches 2.50 V in 10 s
at 50 % charge — 37 A at 24 °C, 14 A at −10 °C — as `(OCV(0.5) − 2.50 V) / I` with OCV(0.5) ≈
3.75 V; it is an estimate at a far higher current, so the ratio is the usable part. A circuit
fitted to Chen2020 at −10 °C is **milder than the real cell** by about 1.7×; the shipped
placeholder (2.2×) happens to be closer. ORegan2022 agrees with the sheet. Fitting the circuit
to ORegan2022 outright was tried and is messy (its own stoichiometry window, 5.048 Ah against
Chen2020's 5.153, puts `R0` nodes at zero); taking only its *temperature ratio* is clean.
ORegan2022 and OKane2022 also give the activation energy `[spm]`'s TODO asks for, which would
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
heat in one model and none in the other two.

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
| `[aging]` | `r_growth_per_capacity_loss` | the same WMG set's pulse-resistance rise (Phase 10 slice A read it) | new |
| `[aging]` cycle | `cyc_fade_per_ah`, `cyc_dod_stress_exp` | LG sheet bounds it only (≥ 80 % energy after 500 cycles at 0.33C); no data to fit an exponent | none |
| `[reversal]` | 3 | none in hand | none |
| `[safety]` runaway | `t_onset_k`, `t_vent_k`, `runaway_energy_j`, `runaway_power_w_at_onset`, `runaway_ea_j_per_mol` | none in hand (ARC studies of this cell exist in the literature; not read) | none |
| `[safety]` plating | `plating_c_threshold`, `plating_fade_per_ah`, `plating_short_hazard_per_ah`, `plating_short_ohms` | none in hand (OKane2022 has a plating *model*, not these constants) | none |
| `[spm.sei]` | `initial_thickness_m` (Chen2020's own placeholder) | none | none |

**The roadmap's exit is not reachable by this phase.** About 13 constants have no source the
project holds, and the fit pipeline cannot make one. Two further points for the exit's test:
a grep for the word passes if someone deletes the word, so the test has to check what replaced
it (each fitted value's provenance names its fit and its residual); and "placeholder" also sits
in prose a grep must not count.

**The LG specification sheet**: LG Chem, *Product Specification, Rechargeable Lithium Ion
Battery, Model INR21700 M50 18.2Wh*, document LRB-PS-CY18.2Wh-M50, 2017-09-22, revision 0
("Tentative"), fetched from a distributor's copy
(`asset.conrad.com/media10/add/160267/c1/-/en/002114679DS01/…`). A draft revision, so each value
cited from it says so.

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
be rewritten, not re-measured. A 10 % nudge is the floor: the fit moves the pairs 2–4× and every
one of these numbers with them. If the `[aging]` calendar is fitted too, steps 38–39 (the shelf)
lose their contrast — "15.86 points … the unfitted placeholder" against the film's 1.40 — and
need rewriting rather than re-measuring. Slice C (lead-acid) was not measured here.

## Decisions for the owner

1. **The exit.** Narrow it to what has a source (the circuit, `[cell]`, `[thermal]`, entropic,
   and — if 4 says so — `[aging]`'s calendar and resistance growth), with the remaining
   placeholders on a named list the test holds fixed; or keep it whole and add a literature
   search for runaway, plating and over-discharge data.
2. **Cold.** Fit the circuit's temperature behaviour to Chen2020 (one parameter set, too mild by
   ~1.7× at −10 °C), or take the temperature ratio from ORegan2022 (matches LG's sheet; two sets
   named in one file). Separately: give the porous models ORegan2022's diffusion activation
   energy (colder porous cells; every cold porous test moves).
3. **Entropic heat.** On the circuit only (no Rust; the models disagree about heat), on all three
   (a Rust change: `[spm]`'s coefficient becomes a table), or not this phase.
4. **The drawn aging curve.** Fit it to the WMG storage data (the shelf lesson is rewritten: two
   fitted descriptions instead of a placeholder against physics), or keep it as the shelf
   lesson's deliberate placeholder.

Not asked, a default unless overruled: the circuit is fitted at **1C**, the lessons' first rate,
with the 3C residual in its provenance; and the `[ocv]` grid is re-shaped for this file in the
same slice, since the fit makes its own TODO's condition true.
