# A seventh chemistry — sodium-ion, fitted from published measurements

*(The heading said "a fourth" in its first draft, which was wrong: `chemistries/` holds
seven files with this one, and it is the seventh the guided path teaches. Counted rather
than remembered — the defect class this repo hunts, in the first line of a plan about it.)*

2026-08-31. The first chemistry added **after** Phase 8 closed, and therefore the first one
run against that phase's written recipe rather than against a phase plan:

> Adding a chemistry *after* the phase closes is not a reopening — it is one slice against
> this recipe:
>
> 1. Parameter file, provenance on every constant, `[reversal]` answered.
> 2. `chem` loading test, and a discharge that behaves.
> 3. Scenario file so a client can select it.
> 4. Guided-path steps, prose in digits, claims in `path-claims.toml`.
>
> If step 1 ever requires a code change, that is a finding about principle 10 and belongs in
> its own document — not silently in the chemistry slice.
>
> — `docs/plans/phase-8-chemistries.md`, "The stopping rule"

The owner chose the chemistry direction on 2026-08-31 and ruled two scoping questions the
same day; both are recorded below because both change the size of the slice.

---

## Everything above the "Results" heading was written BEFORE the engine ran

The sourcing and the fit below are measurements of *third-party data* and were made first —
they are what the file is built from. The **predictions** are about this engine and were
written before a single test was run against the new file. If the engine disagrees, the
engine is right and this section is a recorded miss, the shape
`docs/plans/phase-8-slice-a-lto.md` uses.

---

## The owner's two rulings

1. **Fit from the raw published measurements**, rather than hand-reading the source's
   figures (the LTO precedent) or shipping datasheet limits with placeholders. This departs
   from Phase 8's "**No fitting pipeline** … Sourced parameter sets only", which was a
   scoping decision *for that phase*; the recipe above places no such limit on a later
   slice. The cost is one new script; the gain is that most of this file is measured rather
   than eyeballed.
2. **Carry `[hysteresis]`.** The measurements show the effect plainly and the engine grew
   the state in Phase 8 slice C, so it costs no code. It also answers a question slice C
   could not: whether that state was a NiMH special case or a general facility.

## What was sourced, and how good each source is

| quantity | source | strength |
| --- | --- | --- |
| 1500 mAh nominal, 3.1 V nominal, 1.5–4.1 V window, 1C charge / 3C discharge, charge −10…45 °C, discharge −30…60 °C, 37.5 g, `< 20 mΩ` internal resistance | Hakadi 18650 sodium-ion cell datasheet, as reproduced in the source repository's README | **datasheet-class**, from a reseller sheet rather than a manufacturer engineering document |
| usable capacity, the OCV table, R0 against SOC, both RC pairs, the hysteresis loop width | Max Kraft-Schaefer, *Measurements with Hakadi 18650 Sodium Ion Cells*, <https://github.com/MaxMax-embedded/hakadi_soidum_ion_18650>, **CC0 1.0** — incremental-OCV and HPPC runs on cell 2, fitted here by `tools/reference/fit_na_ion_hakadi.py` | **fitted from raw published measurements** — the strongest tier in this repo other than the two files extracted from a PyBaMM parameter set, and the first that comes from a physical cell rather than a model |
| the hysteresis loop is ~20 mV wide above 35 % SOC and up to ~80 mV below it | the same repository's own analysis of its own data (`ocv_analysis.m`, quoted in its README) | **the data author's reading of their own rig**, independently reproduced here to 19.2 mV mean / 59.8 mV peak |
| aluminium current collectors on **both** electrodes, because sodium does not alloy with aluminium; hence tolerance of full discharge to 0 V, and easier transport | sodium-ion materials reviews and industry summaries | **mechanistic and uncontested** |
| but 0 V tolerance is **electrolyte- and chemistry-dependent**, not a property of "sodium-ion" as such | Desai *et al.*, *Zero volt storage of Na-ion batteries: performance dependence on cell chemistry!*, J. Power Sources 551 (2022) 232177 | **peer-reviewed**, and the reason this file does not bill over-discharge at zero |
| activation energies of 80–107 kJ/mol for electrode kinetics on a commercial Na-ion 18650, measured at 10/25/40 °C | *Towards accurate sodium-ion cell modelling*, J. Electrochem. Soc. (2025), doi:10.1149/1945-7111/adfd16 | **peer-reviewed**, used only as an order-of-magnitude anchor — it reports no ohmic-resistance-vs-temperature table, so the temperature axis of `[r0]` remains an extrapolation |

Everything not in that table is a labelled placeholder with a sizing argument, per the
`CLAUDE.md` provenance rule. **Unfitted is allowed; unlabelled is not.**

The two source CSVs total about 30 MB and are **not committed**. The script carries their
URLs, matching the split `tools/reference/fit_ocv.py` already uses for its PyBaMM
dependency: script in tree, inputs not. PyBaMM is not installed on this machine and no
golden CSVs are generated for this cell — the same position the LTO file shipped in.

## What the fit produced

Run as `python tools/reference/fit_na_ion_hakadi.py <dir>`:

```
capacity_ah = 1.4558          # discharge leg 1.4558 A.h, charge leg 1.4128 A.h, ratio 1.0305

[ocv]  (strictly increasing in soc: True)
soc   = [0.00, 0.02, 0.05, 0.10, 0.15, 0.20, 0.25, 0.30, 0.35, 0.40, 0.45,
         0.50, 0.55, 0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90, 0.95, 1.00]
volts = [1.9886, 2.0921, 2.2284, 2.4343, 2.6047, 2.7245, 2.7898, 2.8259, 2.8608, 2.9196, 2.9993,
         3.0898, 3.1853, 3.2823, 3.3765, 3.4647, 3.5478, 3.6343, 3.7336, 3.8486, 3.9578, 4.0824]

R0 / R_10s against SOC [ohm], room temperature, mean of the 2 A discharge and charge pulses
  1.00  0.0558 / 0.0679      0.50  0.0736 / 0.0845
  0.90  0.0677 / 0.0790      0.40  0.0761 / 0.0878
  0.80  0.0702 / 0.0799      0.30  0.0804 / 0.0914
  0.70  0.0709 / 0.0807      0.20  0.0885 / 0.1068
  0.60  0.0719 / 0.0827      0.10  0.1165 / 0.1664

RC pairs   fast  tau  9.83 s  R 0.01693 ohm  C   580.9 F
           slow  tau 364.51 s  R 0.02165 ohm  C 16838.6 F
```

Three things in that output are worth stating plainly rather than leaving in a comment.

**The measured coulombic efficiency is 1.0305 — impossible.** The run reports 3 % more
charge out than in. The rig's own current sensor has a drifting offset (the rests read
+3 to +15 mA where the true current is zero), so this is an instrument artifact and not a
cell property. It matters because it is the entire reason the hysteresis width is uncertain,
below. Correcting for the offset by interpolating the rest readings made the ratio **worse**
(1.0556), so the correction is not applied and the artifact is left in the SOC axis, where
it is a 3 % scale error rather than a spurious voltage.

**`capacity_ah` is the measured 1.4558 A.h, not the rated 1.5 A.h.** The OCV table's SOC
axis is normalised to the same throughput, so the two are consistent by construction —
which is the property that matters, since every threshold in the engine is indexed on SOC.

**R0 is read from the first sample actually under load.** The logger emits one sample at
each step boundary that still carries the pre-pulse voltage; using it gives R0 ≈ 0, which is
how the bug announced itself. The correct sample is ~100 ms in, and with the fast pair's
time constant at 9.8 s that sample carries under 2 % of the RC pair — so it is an ohmic
reading, not a 100 ms one. The measured 56–117 mΩ is far above the datasheet's `< 20 mΩ`,
and the source explains why: that figure is an AC impedance at kilohertz, and the DC value
is several times larger. This file uses the DC value, because the DC value is what a load
sees.

## The hysteresis half-width, and why it is cited rather than fitted

`sim_core` puts a discharging cell at `OCV − scale_v` and a charging one at `OCV + scale_v`,
so `[ocv]` is the **centre** of the loop and `scale_v` is its half-width. The centre is
robust. The half-width is not:

| how the two legs are lined up | mean full width | range |
| --- | --- | --- |
| each leg normalised to its own throughput | **19.2 mV** | −12.3 … 59.8 mV |
| absolute A·h above the empty endpoint | 45.6 mV | 24.9 … 85.0 mV |
| offset-corrected absolute A·h | ~70 mV | 48 … 119 mV |

An eight-fold spread, driven entirely by the 3 % coulombic inconsistency above. **A number
from that family is not a measurement.** So the file cites the source's own figures —
~20 mV above 35 % SOC, up to ~80 mV below — which come from the data author's analysis of
their own rig, and uses the first row as the independent reproduction of them: 19.2 mV mean
is a close match to their 20 mV, which is also the evidence that the per-leg normalisation
is the right alignment. `scale_v = 0.010` (a 20 mV loop) follows, and the table above is
reproduced in the file's provenance note so the uncertainty travels with the number.

The negative minimum in the first row is real and is the alignment error showing itself: at
90 % SOC the two legs cross by 12 mV, which cannot happen physically.

## The finding: a constant half-width cannot say what this cell does

**This is a principle-10 finding and it is recorded here rather than fixed.** The measured
loop is **SOC-dependent by a factor of three or four** — around 20 mV over the upper
two-thirds of the range and up to 80 mV in the bottom third, where the OCV curve flattens.
`HysteresisParams::scale_v` is a single scalar, so no value of it can express that shape.
The file ships the upper-range figure and therefore **understates the loop below 35 % SOC by
roughly three times**.

Expressing it would need either a table (an `[ocv]`-shaped `soc`/`volts` pair for the
half-width) or a second coefficient, and either is a schema change to `HysteresisParams` —
i.e. a snapshot version bump, since the chemistry is serialized inside every snapshot and
`bincode` is positional. **It is not made here.** Nothing shipped needs it, one chemistry
wanting it is not a design case, and this is the same shape as slice A's `[safety]` gap:
found by reading and measuring, recorded, and fixed later if a second file wants it.

What this does *not* undermine: the lesson below reads the hysteresis at mid-to-high SOC,
where the shipped figure is the measured one.

## Why the lesson is not the zero-volt story

The obvious lesson from a sodium-ion cell is that draining it flat costs it nothing, where
the same abuse permanently costs a lithium cell — the aluminium-collector argument. **That
lesson is already shipped.** `chemistries/lto_20ah_generic.toml` bills over-discharge at
`fade_per_ah = 0.0` for exactly that reason and says so at length. A second cell telling the
same story with the same number teaches nothing new, and the guided path's step 26 already
sits on the LTO file.

What this cell has that nothing shipped has is **the shape of its open-circuit curve**.

| span | LFP 26650 | Na-ion 18650 | ratio |
| --- | --- | --- | --- |
| the working middle, 20–80 % | 2.60 mV per point of charge | 15.16 | **5.8×** |
| LFP's own flat plateau, 45–75 % | 1.02 | 18.28 | **18×** |

**Both rows are reported and the first is the one quoted**, because choosing the span that
maximises a ratio is how a true number becomes a misleading one.

**The first draft of this section said "thirteen times" and it was wrong.** That figure came
from the illustrative `[ocv]` block in `CLAUDE.md` rather than from
`chemistries/lfp_26650_generic.toml`, and the sketch is a *shape*, not the shipped table:
the real LFP curve climbs 130 mV through a knee between 15 % and 25 % that the sketch does
not have. The test written for the claim failed at 5.83×, which is the test working. It is
also the second time this repo has been caught reading that block as a source.

The guided path already teaches the LFP consequence — a voltage reading barely constrains
the charge state, so the fuel gauge drifts and cannot correct itself. This cell is the
mirror image: **voltage reads charge directly.** And the second half of the lesson is the
limit on that, which the same measurements supply. Measured on the shipped parameters, two
runs meeting at 45 % charge from opposite directions rest **18.36 mV** apart — **1.2 points
of charge that no voltage reading can resolve**, however good the sensor.

That is a two-sided lesson — a real advantage and its real floor — built entirely from
measured numbers and shipped machinery.

The zero-volt behaviour still ships, in `[reversal]`, and it is deliberately **not** zero.
The LTO argument is structural (there is no copper, so the mechanism does not exist). Here
the literature is explicit that 0 V tolerance depends on the electrolyte, so a zero would
claim more than the sources support. The file bills one hundredth of the LFP figure: a full
reversal costs 0.0064 % of capacity where LFP's costs 1.0 %. Three files, three different
answers to the same required question, which is what `[reversal]` was built to force.

## Predictions, pre-registered

| | prediction |
| --- | --- |
| **P1** | **The floor does not move.** No existing trajectory changes by a ULP — a new file that no existing pack loads cannot move one, and no Rust is touched. |
| **P2** | **Zero lines of engine code.** The file loads and validates against `chem.rs` unchanged. This is the third independent test of principle 10 and the first with a `[hysteresis]` section written by someone other than the slice that built it. |
| **P3** | **The OCV monotonicity rule passes on the first try**, and the fit's own `strictly increasing: True` is why. The wider risk was never monotonicity but the *voltage window*: 1.5–4.1 V overlaps no shipped lithium file's, and the validator carries no lithium assumption (the 2 V lead-acid file proved that). |
| **P4** | **`v_max = 4.15` sits 68 mV above `OCV(1.0)` = 4.0824, and 58 mV above a rested full cell once hysteresis is counted.** The shipped relation is a protection limit ~50 mV above the charge-termination voltage; here termination is the datasheet's 4.1 V, so 4.15 V. That gap must stay **inside** `ProtectionConfig::v_release_band_v` (80 mV by default), and it does. **This entry was amended twice before anything ran, and the round trip is worth recording.** The first amendment moved `v_max` to 4.25 V on the argument that a rung which trips must be able to release — and that argument is **backwards**. `docs/plans/protection-chatter.md` measured the opposite: the over-voltage rung releases on a *rested* reading, a saturated cell rests at its own `OCV(1.0)`, so a band **narrower** than `v_max − OCV(1.0)` releases on every load removal and the pack chatters at the step rate. The 80 mV default ships precisely because it is *wider* than the shipped LFP file's 50 mV gap. 4.25 V would have put this cell's gap at 158 mV and handed it the chatter the band exists to prevent. **The lesson is the one the repo keeps relearning: read the plan document for a mechanism before reasoning about it from its field names.** |
| **P4b** | **A shipped-file rule that `v_max − OCV(1.0)` is always inside the band would be FALSE, and must not be built.** The NiMH file sits at 175 mV, deliberately: a NiMH cell under 1C charge legitimately reaches ~1.50 V at the terminals, so a tighter limit would trip on ordinary charging. The two constraints — do not trip during a normal charge, do not chatter once tripped — pull opposite ways, and each file resolves them for its own cell. This one takes the same side as LFP, at the same cost: a 1C constant-current charge carried all the way to full trips the limit near the top. |
| **P5** | **The BMS charge-state estimate tracks truth far better on this cell than on LFP**, from the OCV slope alone and with no estimator change. This is the lesson's engine-side claim and the one most likely to need its size adjusted after measurement. |
| **P6** | **The hysteresis state is reachable and visible.** A discharge-then-rest and a charge-then-rest to the same true charge state settle about 20 mV apart, and the gap survives into the telemetry rather than being lost in the RC pairs — which is exactly what slice C got wrong the first time, by measuring before the 364 s pair had settled. **The slow pair here is 6 minutes long**, so any measurement of the loop must rest for at least half an hour of simulated time. |
| **P7** | **`[safety]` keeps a plating gate.** Unlike LTO, this cell has no structural argument that it cannot plate — sodium plating on hard carbon at low temperature and high rate is reported — so omitting the gate would assert something the sources do not support. The gate is anchored on the datasheet's −10 °C charge floor and labelled a placeholder. |

## The lesson, and the second story that turned out to be already shipped

The zero-volt story was already taught (LTO). **So was the second candidate**, and finding
that out was worth the hour it took: guided-path step 29, on the NiMH cell, already shows a
gauge fooled by a cell's direction memory — perfect sensors, a rested pack, and an estimate
**29.8 points** wrong because the cell rests on a branch the chemistry's curve does not
have.

Two stories already told is not two dead ends, because both are the *same* story told at
different curve slopes, and this cell is the third point on that axis:

| cell | curve through the working middle | what the gauge does after a rest |
| --- | --- | --- |
| LFP | flat (1.02 mV per point on its plateau) | **refuses to correct** — below `min_ocv_slope_v_per_soc`, a reading there is noise amplification. Taught at step 4. |
| NiMH | flat, with two branches 50 mV apart | corrects, onto the wrong branch, and lands **29.8 points** out. Taught at step 29. |
| **sodium-ion** | **steep (18.28 mV per point)** | **corrects, and lands about 1.2 points out** — the same memory, on a curve steep enough that it costs almost nothing. |

**The slope is the explanatory variable**, and no step in the path says so, because until
this file there was no cell on the steep end to say it with. That is the lesson: not "this
chemistry is good", but *what the shape of the curve decides* — whether the gauge may read
the cell at all, and what the cell's memory costs it when it does.

The two scenario files are built for exactly that: `scenarios/na_ion_gauge_corrects.toml`
and `scenarios/lfp_gauge_declines.toml` differ in **one field**, so the correction is
attributable to the cell rather than to anything about the BMS. The existing LFP step
cannot serve as that control — it runs on a scenario carrying a soft short and a lying
voltage sensor as well.

## Slice steps

1. **DONE** — `tools/reference/fit_na_ion_hakadi.py`, output above.
2. **DONE** — `chemistries/na_ion_18650_generic.toml`.
3. **DONE** — loading and behaviour tests in `sim-data`:
   * `tests/load.rs::na_ion_chemistry_loads_and_validates`, plus the file added to all
     **three** shipped-parameter guards (`shipped_aging_…`, `shipped_plating_…`,
     `shipped_runaway_…`). Those guards enumerate files by hand, so a new chemistry that is
     not added to them ships unguarded — the tripwire lesson from
     `docs/plans/plating-absence.md`, applied rather than rediscovered.
   * `tests/load.rs::no_chemistry_predating_v18_carries_the_v18_sections`, **renamed** from
     `no_chemistry_but_nimh_…`: that name became false the moment a second file declared
     `[hysteresis]` legitimately, and a stale self-description is the defect class this repo
     hunts hardest.
   * `tests/na_ion_chemistry.rs` — the curve's slope against LFP's, the hysteresis loop with
     an LFP control arm, which floor a discharge hits at 1 C and at 3 C, and the
     over-voltage limit against the release band.
   * `tests/na_ion_gauge.rs` — the estimator correcting here and declining on the matched
     LFP control, and the residual the loop explains.
4. **DONE** — `scenarios/cc_discharge_na_ion.toml`, `na_ion_gauge_corrects.toml`,
   `lfp_gauge_declines.toml`. The last two differ in one field.
5. **DONE** — two guided-path steps, `a-curve-worth-reading` and
   `still-wrong-and-it-has-stopped`, taking the path from 29 steps to 31. Both fully
   ledgered on the slice that added them: 12 new claims across 2 new arms, 4 new ledger
   vocabulary rules, and both steps in `spelled` at 0. Three scenario files added to the
   picker in `web/index.html`.
6. **DONE** — `cargo test --workspace` and `cargo clippy --workspace --all-targets --
   -D warnings` clean, run below normal priority.

---

## Results

Written after the engine ran. Everything above this heading is the pre-registration.

### The predictions

| | outcome |
| --- | --- |
| **P1** | **GREEN.** The full workspace suite passed with 612 tests and zero failures before any client work started, on a tree that had added a chemistry file, four scenario files and five tests. No existing trajectory moved. |
| **P2** | **GREEN. Zero lines of engine code**, and the third independent pass for principle 10. What the slice *did* have to touch outside `sim-core` is worth naming, because none of it is engine: three hand-enumerated shipped-parameter guards in `tests/load.rs`, and the scenario picker in `web/index.html`, which is a curated `<option>` list and not a directory listing. A chemistry that is not added to those is loadable and invisible. |
| **P3** | **GREEN**, first try, no validator change. |
| **P4** | **GREEN.** Measured: `v_max` 4.1500 V, rested full cell 4.0924 V, gap **57.6 mV** inside the 80.0 mV band. `the_over_voltage_limit_sits_inside_the_release_band` asserts it. |
| **P4b** | Stands, and was not built. |
| **P5** | **GREEN, and the size needed no adjustment — the *comparison* did.** The estimate lands 0.494 points from the truth on this cell and 2.057 points from it on the matched LFP control. But the first draft of the control's assertion was wrong in a way worth recording: it asserted that the LFP estimate *does not move* across the rest, and it moves 0.885 points. That movement is not a correction — it is the 0.02 A sensor offset still integrating with the pack open, worth 0.868 points over an hour on a 2.303451 A.h cell, plus one step of frame lag. **"Did it correct" is not answerable from how far an estimate moved.** The check now measures where each estimate *ends* relative to what its own table says at its own resting voltage: 0.036 points away here, 2.057 points away on LFP. |
| **P6** | **GREEN on the effect, and the rest length was the thing to get right.** Measured 18.36 mV, against a prediction of "about 20 mV" and a declared half-width of 20 mV; the shortfall is `gamma`, which crosses the loop asymptotically and gets 8.3 points of throughput about seven eighths of the way over. The warning in the prediction was not paranoid enough. A 900 s rest leaves 2.6 mV of the 365 s pair unrelaxed **in the same direction the loop pushes**, which is a fifth of the residual wearing the loop's name; the tests and both lessons rest for 3600 s. |
| **P7** | **GREEN.** The gate ships, anchored on the datasheet's −10 °C floor and labelled. |

### What the two lessons ended up being about

The plan said the lesson was the curve's slope, and it is, but the *second* step is not the
one that was planned. The plan had step two as "and it lands short, by what the loop is
worth". Measurement offered something better, and it came out of the P5 correction above:

* At the first mark (3900 s) the sodium-ion estimate has landed and the LFP one has not.
* At the second mark (7500 s), an hour later, **the sodium-ion panel has not changed at all
  and the LFP one has moved a whole tenth of a percent** — 56.8 % to 55.9 %. One estimate
  has something to stop against and the other does not, and a reader can *see* the
  difference between them without being told a number.

So the second step is "one estimate stops, the other keeps walking", and the hysteresis
residual is the explanation for *where* the first one stopped rather than the subject. That
is a better shape: the visible thing is the motion, and the number is the cause.

The honest half is in the prose of both steps and in `lfp_gauge_declines.toml`'s header: the
reading the LFP estimator refused was a **good** one. Inverting its table at its resting
voltage returns its truth to four figures. The gate is a claim about what a reading on that
curve is worth in general — 5 mV of sensor error buys 8.8 points of charge there against
0.3 points here — and not a claim about the case in front of it.

### Five defects found in the checking machinery, none of them in the engine

1. **The scenario picker is a hardcoded list.** `sim-server` serves `scenarios/` and
   `/scenarios` enumerates the directory, so the *server* needs nothing; the browser page's
   picker is eleven `<option>` elements in `web/index.html`. An instruction to "load X from
   the picker" for a file that is not one of them is a click nobody can make, and
   `every_arm_is_instructed_by_its_own_step` says so in exactly those words. Both new
   scenario files and the bare discharge went in.
2. **A pulse train repeats, and the second lesson's mark was past the second tooth.** The
   off-leg was 5400 s on a 300 s pulse, so a second pulse fired at 5700 s — inside a run to
   7500 s. `every_claim_matches_the_engine` caught it as a charge state 8.3 points below the
   prose. Both steps now use a 9000 s off-leg, which outlasts both marks. The measurement
   the prose was written from had been taken with a single pulse, so the numbers were right
   and the *lesson* was wrong, which is the direction that would have shipped.
3. **Nine self-counts in the checking files were stale, and every one of them in English
   words.** Three were about the ledger's own lists ("the twenty-four entries above", "all
   twenty-four steps", "these are the seven steps") and had been wrong since phase 8's
   earlier slices added five lessons; the rest were counts of claims and numerals that this
   slice moved. The three about the lists are now `TALLIES` entries, derived from
   `[ledger].steps` and `[ledger].spelled` — the second of those needed a new derivation
   (`n_word_scanned`) that had simply never been written, while its twin for the digit
   ledger had existed since the self-count sweep. Three more past-tense counts beside them
   are now `NOT_DERIVED` waivers with the reason written down, which is what that table is
   for: a frozen measurement must not be "fixed" into a false present-tense one.

4. **Six more self-descriptions in `path_claims.rs` that two new lessons made false**, found
   by following the digits rule's own failure message — which was one of them. It said the
   ledger *"scans every one of the twenty-four steps"*, inside a panic string, where no
   tally reaches. Three were present-tense coverage claims and now say "every step in the
   path", which does not rot. One was a count of steps carrying no claims: fourteen when it
   was written, **two** now, because the two lessons this slice added both carry claims. One
   was made explicitly past. And one was simply **false**: *"The twenty-fourth,
   `the-gradient-itself`, is the last step in the path"* — it is neither the last step nor
   the last ledgered any more, and it was written as a present-tense fact about a file that
   was always going to grow. It now says what was true: the last one *left to ledger*.

5. **Two sentences in the SHIPPED lesson prose went false when the path grew, and both are
   superlatives spelled in English.** Step 28 opened *"A sixth chemistry, and the last this
   path adds"*, which a seventh step-30 chemistry contradicts two steps later. Step 29 said
   the NiMH file's `[ocv]` being a midline is *"a thing no other parameter file in this repo
   has to say about itself"* — false the moment `na_ion_18650_generic.toml` declared
   `[hysteresis]`. Neither is a quantity, so the digits ban does not read them; neither is a
   digit, so the ledger does not see them; and step 28 sits in `spelled` at **0**, which
   means "no quantity of a shape the reader reads" and not "nothing here is false". **A step
   with a green zero can still contradict the step after it.** A third, in a code comment,
   was contestable rather than false and was made exact.

The wider point about (3), (4) and (5) is the one the repo keeps paying for:
`every_count_above_an_english_block_is_derived` and its siblings read digits. A number
spelled in letters is invisible to every check in this file, and the places these files
describe themselves — doc comments, ledger notes, and the assertion messages an author reads
when a check fires — are all prose. **The message a check prints when it fails is prose
about the file too, and it is the least likely place anyone looks for a stale number.**

And the sharper version, from (5): the checks in this repo are all about whether a number is
*right*. Nothing in them asks whether a **claim about the path's own shape** is still true,
and "the last chemistry this path adds" is exactly the kind of sentence a lesson wants to
write. The cheap discipline is a grep for superlatives — `the last`, `the only`, `no other`
— over `const LESSONS` on any slice that adds a step. That found both of these in one pass.

### What is NOT closed

`HysteresisParams::scale_v` is one scalar, and the measured loop on this cell is three to
four times wider below 35 % charge than above it. The shipped file therefore **understates
the loop in the bottom third of the range**, and says so in its provenance. Expressing it
needs a table or a second coefficient, which is a schema change and a snapshot bump; nothing
else in the tree wants one, and one chemistry is not a design case. Recorded, not fixed —
the same shape as slice A's `[safety]` gap.


*(Written after the fact. Nothing above this line was edited once the engine ran.)*
