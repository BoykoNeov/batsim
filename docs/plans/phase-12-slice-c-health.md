# Phase 12 slice C — what plating does to a reaction cell's health and its flags

**Status: PREDICTIONS REGISTERED 2026-10-18, before any engine code.** Plan: `phase-12-plating.md`
§C; the reaction it builds on: `phase-12-slice-b-reaction.md`. Snapshot 29 → 30. Tests:
`crates/sim-data/tests/plating_dfn.rs` (slice B's file, extended).

Until this slice a `Dfn` cell running the plating reaction still ran the threshold beside it: its
`PLATING_RISK` was the threshold's, `plating_fade_per_ah` billed the threshold's amp-hours, and the
dendrite short rolled on them. Plated lithium itself moved no health number. This slice replaces all
of that on a reaction cell, and touches no other cell.

## The owner's decision, 2026-10-18: a new hazard constant

Asked with the numbers (below); **answered: a new labelled placeholder.** The threshold's
`plating_short_hazard_per_ah` (1.2e-3 per A·h) is a hazard per amp-hour *charged under the
threshold's conditions*, sized for ~0.6 % per full cold charge of this cell. Dead lithium is a far
smaller quantity: on the fine grid, CC–CV at slice A's sizing, 1.24e-3 A·h on −10 °C 0.3C,
3.0e-3 A·h on 0 °C 1C, 8.7e-3 A·h on −10 °C 1C, 2.65e-2 A·h on −10 °C 2C. Reused per dead
amp-hour, that number gives a −10 °C 1C charge a 0.001 % chance and −10 °C 2C 0.003 %, so a short
would never be seen. So `[dfn.plating]` gains **`short_hazard_per_dead_ah`**, sized so a −10 °C 1C
CC–CV charge keeps the threshold cell's 1 − exp(−1.2e-3 · 5.153) = 0.616 %:
`λ = −ln(1 − 0.00616) / 8.66e-3 = 0.714`, **rounded to 0.7 per A·h** (placeholder, no source). At
0.7 the sheet's other charges carry 0.21 % (0 °C 1C), 0.68 % (0 °C 2C), 1.8 % (−10 °C 2C), 0.087 %
(−10 °C 0.3C), and the coldest allowed one (0 °C 0.3C) 3.5e-5. The short's resistance stays
`[safety].plating_short_ohms`; validation refuses a positive `short_hazard_per_dead_ah` on a
chemistry whose `[safety]` has no positive `plating_short_ohms`. The LG M50 placeholder list in
`lgm50_provenance.rs` grows by this key, naming this note and the decision, as the amended rule
allows.

## The design

1. **Capacity is read off the electrodes, once.** Dead lithium is gone from the negative's inventory
   as the film's lithium is, so it moves the negative's full down by the same conversion,
   `δx_dead = 3·mean_i(dead_i)/(R_p·c_max)` (mol per m² of interface to a stoichiometry; the grid's
   nodes are equal in width, which is why the mean is the volume weight, as `bulk_stoich`'s is). The
   film's `film_shift` becomes a **lithium-loss shift**, film plus dead, and `film_capacity_fraction`
   the electrodes' capacity fraction, `1 − (δx_film + δx_dead)/Δx`. The pack's reported
   `soh_capacity` and `soc_true`'s weighting pick it up through the door Phase 10 slice B built. The
   multiplier the model is handed does **not** carry it, or it is billed twice. A film-only cell adds
   `0.0` and is bit-identical; a no-film, no-plating cell keeps its `None` path.
2. **Reversible metal is stored charge — reading (a).** Lithium sitting as metal never entered the
   graphite, so today's readout counts it as missing. It is not lost: it strips back on a rest, and on
   a discharge it delivers its charge to the terminals. So `soc_true` reads
   `(x̄ + δx_rev − x_min) / (x_max − δx_film − δx_dead − x_min)`, `δx_rev` by the same conversion.
   Chosen by the two checks registered below, against reading (b), metal left out of the numerator.
   The unclamped value feeds `SOC_CLAMPED_HIGH`/`LOW` (`soc_flags`) and nothing else: the held
   ranges (`current_window`) and `SURFACE_OUT_OF_RANGE` read stoichiometries directly, and
   `surface_gap` differences two readings that take the denominator only, never `δx_rev`.
3. **`PLATING_RISK` on a reaction cell is "this step plated lithium"**: the gross plated on any of its
   sub-steps above zero, reported out of the march through `Advanced`. Not "the dead store grew",
   which never rises at `dead_share = 0`. The threshold (`plating_risk`) is not evaluated on such a
   cell, and `accumulate` gets `false`, so none of its amp-hours collect. **A zero-length step plates
   nothing and does not raise it** — unlike the threshold's flag, which judges conditions and so
   reports on a probe step; the reaction's flag reports an event, and none happened. A voltage hold
   cut into pieces (`plating-hold-pieces.md`) raises it if any piece did.
4. **No threshold fade.** On a reaction cell `tick` bills no `plating_fade_increment`; its `q_plating`
   stays `0.0`.
5. **Resistance grows with the dead lithium.** `CLAUDE.md`: never capacity fade without resistance
   growth. The film carries its own physical resistance; dead lithium has none in the law. So the
   dead capacity fraction `δx_dead/Δx` enters `soh_resistance`'s loss at
   `r_growth_per_capacity_loss` — the threshold's coupling, which keeps the lesson's two cells
   comparable — and **not** `soh_capacity`'s. Read off the electrodes at each tick, not stored.
6. **The dendrite short rolls per amp-hour of dead lithium.** The dead store is read before and after
   `advance` (gated on aging, as the reversal accumulator is) and the difference, in amp-hours, goes
   into the accumulator the threshold used (`ah_plating_since_tick`), whose meaning on a reaction cell
   is therefore *dead* amp-hours since the tick. `tick` rolls `1 − exp(−λ·ah)` with `λ` the new key.
   Same RNG, same order, same "no draw at zero probability" rule, so every other pack draws exactly
   as before.
7. **The bump, 29 → 30.** `PlatingParams` gains a field (`ChemistryParams` is in the snapshot), and
   the accumulator's meaning changes on a reaction cell: a v29 snapshot of one restored mid-tick would
   mix the two meanings. Pair test as for v29.

## Predictions

Shipped LG M50, fine grid, isothermal, 1 s steps, CC–CV = CC to 4.2 V then 4.2 V held to C/20, test
chemistry with the drawn fade and both hazards zeroed unless said (slice B's `chem()`, plus the new
key).

1. **Plating off is bit-identical.** Every golden and every test outside `plating_dfn.rs` unchanged;
   slice B's hash harness (`W:\temp\claude\phase12b\hash`, keys `plating` and `version` stripped) hashes
   alike on the six packs, a 1e-15 nudge moving all six.
2. **The 29 → 30 pair test**: a v29-tagged blob is refused by the version field; a v29-shaped
   chemistry with plating fails at the new key.
3. **Capacity counted once.** After a 0 °C 1C CC–CV charge: the multiplier handed to the model is
   exactly `1.0`; the reported `soh_capacity` equals `1 − δx_dead/Δx` to 1e-12; and `δx_dead/Δx`
   equals the dead lithium in amp-hours over the fresh electrodes' capacity (`probe::spm_capacity`)
   to 1e-9. The figure is about 0.059 % (slice A's table).
4. **Reading (a) holds through a rest.** The same charge, then 1 h at rest: the reversible store falls
   to under half of its end-of-charge value (the spike: it strips back within an hour at 0 °C), and
   `soc_true` stays within 1e-9 of its end-of-charge value. Reading (b), computed in the test from the
   same states, rises by the stripped share, a few times 1e-4.
5. **Reading (a) conserves charge on a discharge.** After the same charge, straight into C/2 to
   2.5 V: `Δ(soc_true · Q)` equals `−∫I·dt` to 1e-9 of the charge passed, `Q` the reported capacity
   (constant: nothing plates on a discharge). Reading (b) misses by the metal stripped on the way.
6. **The flag is the reaction's.** On a reaction cell `PLATING_RISK` rises on a step iff the gross
   plated on it is above zero. It **rises** on the two allowed charges that plate, 0 °C 0.3C and
   25 °C 0.7C, where the same charge on a threshold `Dfn` cell never raises it (0 °C is not below
   `t_plating_min_k`; 25 °C is warm). It **never rises** on 25 °C 0.3C or 45 °C 0.7C (which plate
   exactly nothing), on any rest or discharge step, or on a zero-length step at any state, including
   mid-charge at 0 °C 1C. It rises on every forbidden charge.
7. **No threshold fade.** With `plating_fade_per_ah` at its shipped 8.75e-4 instead of zeroed, a
   reaction cell's run of −10 °C 1C CC–CV is bit-identical to the zeroed run. The same charge on a
   threshold `Dfn` cell bills ~0.45 % (8.75e-4 × the ~5.15 A·h it charges); the reaction cell loses
   ~0.168 % — recorded, not judged.
8. **Resistance follows the dead lithium.** After the −10 °C 1C charge the reported `soh_resistance`
   equals `1 + 1.5·δx_dead/Δx` at the last tick to 1e-12 (about 1.0025); on a cell with the film too,
   `soh_resistance_reported` is the film's formula with that multiplier.
9. **Slice B's results move only through the resistance growth.** With both hazards zeroed, every
   `plating_dfn.rs` assertion passes unchanged; the figures it prints move by under 1 % relative
   (resistance up by at most ~0.8 %, at −10 °C 2C). Exits 1 and 2 keep their margins.
10. **The short.** (i) The dead amp-hours the ticks consume over the −10 °C 1C charge equal the dead
    store's lithium in amp-hours to 1e-12 relative (no leak, no double count, ticks of 10 s across
    steps of 1 s and 60 s); (ii) at the shipped 0.7 the summed hazard over that charge is
    `0.7 × 8.7e-3 ≈ 0.61 %`; (iii) on a warm charge that plates nothing the RNG state after the run
    equals a fresh pack's — no draw; (iv) with `λ` forced to 1e6 a short lands on the first tick after
    lithium dies and the cell's shunt is `1/plating_short_ohms`; (v) the threshold's
    `plating_short_hazard_per_ah` at its shipped value draws nothing on a reaction cell.
11. **The stripping plateau: absent at this `k`.** After the −10 °C 1C CC–CV charge (reversible
    ≈ 0.17 % of capacity), straight into a discharge to 2.5 V at C/10 and at C/2, −10 °C: against a
    control arm — the same state with the reaction taken out of the snapshot — the terminal voltage
    differs by **under 2 mV** over the first 90 % of the control's discharge, so no plateau a lab
    would see. Reason: the plating exchange current `F·k·c_e ≈ 4.6e-3 A/m²` is so small that at the
    graphite's ~0.09 V near full, stripping carries ~0.02 A/m², about 12 % of a C/10 current and a
    few % of C/2 (estimated, not run). Registered: stripping's share of the cell current peaks under
    20 % at C/10 and under 10 % at C/2; the reaction arm delivers more charge than the control by the
    metal it stripped, to 1e-6 of capacity. If the voltage difference exceeds 2 mV, the plateau is
    present and the prediction failed.
12. **Snapshot mid-tick**: a reaction pack holding dead amp-hours not yet consumed by a tick (1 s
    steps, snapshot at 7 s past a tick) restores through bincode and JSON and continues bit for bit,
    with `λ` large enough that the next tick rolls.
13. **Debug build**: the fast plating tests pass in a debug build.
14. **Cost**: none measurable — one read of the dead store before and after `advance`, on reaction
    cells with aging only.
