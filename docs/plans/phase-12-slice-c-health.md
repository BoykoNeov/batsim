# Phase 12 slice C — what plating does to a reaction cell's health and its flags

**Status: BUILT 2026-10-18.** Predictions registered before any engine code (3deefba), scored in
"Results" below: the design holds, plating off is bit-identical, and reading (a) is chosen on
measurement. **Recorded misses:** the stripping step after a cold constant-current charge is 4.3 mV,
not under 2 mV (prediction 11); three estimates were off (4, 7, and 11's premise); and predictions 5
and 11 could not be read on the protocol registered, because **at 0 °C and below a 4.2 V hold leaves
the `Dfn` in a state whose next discharge does not converge** — a defect older than this slice,
found by it, recorded under "Found on the way" for the owner. Plan: `phase-12-plating.md` §C; the
reaction it builds on: `phase-12-slice-b-reaction.md`. Snapshot 29 → 30. Tests:
`crates/sim-data/tests/plating_health.rs` (new) and `plating_dfn.rs` (its exit test extended).

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

## Results (run 2026-10-18)

Release builds, the instruments `plating_health.rs`'s tests (two `#[ignore]`d diagnostics hold the
defect's numbers) and `plating_dfn.rs`'s exit test. Shipped LG M50, drawn fades and both hazards
zeroed unless said.

* **1, held.** `cargo test --workspace --release --no-fail-fast`: 108 test binaries, none failing.
  Every golden is unchanged, and so is every test outside the two plating files except three
  repinned for the new key and the bump — a count or a version each, nothing that moves a
  trajectory: `dfn_chemistry.rs` (the `[dfn]` value count, the plating section's 3 → 4, and the new
  key pinned by name), `lgm50_provenance.rs` (the placeholder list 21 → 22) and `snapshot_version.rs`
  (the v29 pair, and the v29-shaped section). Slice B's hash harness (its source kept at
  `W:\temp\claude\phase12b\hash\src\main.rs`; this slice built it twice, against an export of the
  parent and against this tree, and deleted both builds) hashes the six packs alike, and a 1e-15
  nudge of the initial SOC moves all six. The browser build, rebuilt after, reads 172/172
  bit-identical against native (`tools/wasm-parity/parity.mjs`).
* **2, held.** `the_version_field_is_what_rejects_a_v29_snapshot` and
  `a_v29_shaped_plating_section_reads_the_safety_tag_into_its_hazard`: a v29 `[dfn.plating]` reads
  the chemistry's `[safety]` tag and seven bytes of its onset as the hazard, and fails at the
  onset's top byte.
* **3, held.** After 0 °C 1C CC–CV the dead lithium is 0.0584 % of the window; the multiplier handed
  to the model is exactly 1.0, the reported `soh_capacity` 0.999416208 = 1 − δx_dead/Δx to 1e-12,
  and the stoichiometry conversion agrees with the dead store over the electrodes' capacity to 1e-9.
* **4, held on its bars; its magnitude missed, and it was rerun where it can be read.** The 0 °C
  hold leaves almost no metal — 0.0007 % of capacity, 1.2 % of the dead store — so on the registered
  charge reading (a) drifted 5.0e-13 over the hour while reading (b) rose only 7.2e-6, not the
  "few × 1e-4" registered. On the charge's constant-current leg alone (to 4.2 V, metal 0.0258 %):
  (a) drifted 1.6e-11, (b) rose 2.58e-4, all of the metal stripped within the hour. **Reading (a)
  is chosen.**
* **5, not readable as registered; held on the constant-current leg (changed after
  registration).** A C/2 discharge straight after the 0 °C hold runs on unconverged solves (below),
  so it is not run. After the leg alone the discharge converges on every step, and the reading's
  change times the capacity it is weighted by is the charge passed to 10 digits (2.425581670 A·h
  both). *An instrument error first:* the test weighted by the electrodes' window, which the LG M50
  file's `capacity_ah` rounds — `window/capacity_ah − 1 = 6.3287e-8`, exactly the 6.3e-8 miss it
  printed. The engine weights by `capacity_ah`; so does the test now.
* **6, held.** On all nine of the sheet's charges (`plating_dfn.rs`'s exit test, now tracking the flag)
  `PLATING_RISK` rises on exactly the steps on which the gross plated rose — 0 mismatches — and on at
  least one step exactly where a charge plates: 1441 steps on 0 °C 0.3C and 91 on 25 °C 0.7C (both
  allowed), none on 25 °C 0.3C or 45 °C 0.7C, 3766–5964 on the forbidden five. A threshold `Dfn`
  cell on 0 °C 0.3C and 25 °C 0.7C raises it on no step. Down on every zero-length step mid-charge
  (where the 1 s step raises it), every rest and every discharge step after a cold charge.
* **7, held; its estimate missed.** With `[safety]`'s plating fade and hazard at their shipped values
  the reaction cell's −10 °C 1C CC–CV run is bit-identical to the zeroed run (snapshot, chemistry
  aside). The threshold cell bills 0.30 %, not the ~0.45 % estimated: it bills only while the
  current is above its 0.4C, and the hold's taper falls below it. The reaction cell loses 0.164 %.
* **8, held.** `soh_resistance` 1.002463925 = 1 + 1.5 × 0.001643 to 1e-12; on a filmed reaction cell
  the multiplier is the same `1 + r_growth·δx_dead/Δx`.
* **9, held.** Slice B's 13 tests pass unchanged; its printed figures move in the fourth digit or
  later: the step-length bar ×1.00402 / ×1.00376 / ×1.00373 on the charge (×1.0040 recorded), engine ÷
  PyBaMM 0.9771–0.9925 on the forbidden charges (0.9765–0.9907), mildest forbidden ÷ worst allowed
  24.41 (24.4).
* **10, held — after an instrument fix.** (i) With no tick inside the charge, the accumulator holds
  the dead lithium to 13 digits at 1 s and 60 s steps; (ii) −10 °C 1C CC–CV dies 8.46e-3 A·h at the
  shipped 0.7, a 0.591 % chance of a short (0.616 % sized on slice A's 8.66e-3); (iii) a warm charge
  that plates nothing leaves the RNG untouched; (iv) at a hazard of 1e6 the first tick after lithium
  dies lands one `[safety]` short. *The test first asserted the shunt stayed one short*; at that
  hazard every later tick that sees more lithium die lands another, as the threshold's shorts add.
* **11, missed as registered — on the constant-current leg; not readable on its own protocol.** The
  registered charge (−10 °C 1C CC–CV) is followed by a discharge on unconverged solves, so the
  measurement was taken after the leg alone (to 4.2 V, metal 2.39e-3 A·h, the store at its largest):
  **the reaction arm starts 4.3 mV above the control** — stripping carries up to 9.0 % of a C/2
  current beside the intercalation — and the gap fades as the store empties: 3.9 mV at 10 s, 1.3 mV
  at 60 s, the store empty at 76 s, 0.2 mV at 120 s. Against "under 2 mV over the control's first
  90 %", a miss: a short stripping step, about a minute long, is there. The extra charge (2.86e-3
  A·h) matches the metal stripped (2.39e-3) within one step's charge — the registered 1e-6 of
  capacity is below one 1 s step's own charge, so the bar was widened to two steps' after
  registration. Pinned as measured (`a_short_stripping_step_after_the_constant_current_leg`). C/10
  was not run. **Its premise was also wrong**: it assumed the metal left after a CC–CV is about the
  dead store. At −10 °C it is 47 % of it (`what_a_cold_hold_leaves_as_metal`), at 0 °C 1.2 % — the
  colder the hold, the less it strips back.
* **12, held.** A reaction pack holding dead amp-hours seven seconds past a tick, at a hazard that
  rolls, restores through bincode and JSON and continues bit for bit.
* **13, held.** In a debug build, this slice's four fast tests (the mid-tick snapshot, the flag on probe, rest and discharge steps, the filmed multiplier, the short's draws and landing) and slice B's five (refusals, store, Jacobian, ledger, snapshot replay) pass.
* **14, not measured.** The change adds two reads of the dead store per step on reaction cells with
  aging, and one call per cell per tick; no timing was taken.

## Found on the way

* **A cold 4.2 V hold leaves the `Dfn` unable to converge a discharge** (`report_discharge_after_a_hold_*`,
  older than this slice: a threshold cell, no reaction, does the same). After 0 °C 1C to 4.2 V and the
  hold to C/20, 1413 of the hold's steps flag `SURFACE_OUT_OF_RANGE` — a particle surface past full,
  the first 6883 s into the charge, every hold step converged — and the C/2 discharge that follows is
  out of range from its first step and unconverged on 1146 of its first 1200, costing 30 ms a step on the recommended
  grid and 190–250 ms on the fine one (0.2 and 1.1 ms after the leg alone). At 25 °C: no step out of
  range, none unconverged. The same family as ROADMAP H8's "no physics past full", reached now by
  an ordinary cold charge at 1 s steps rather than a charge driven past the ceiling. Slice D's lesson
  runs a cold charge; whether it may end with a hold is the owner's.
* **The dead lithium is in a feedback loop the threshold never had**: it grows the resistance, which
  deepens the overpotential that plates more. `plating.rs`'s "cannot run away" argument is the
  threshold's and does not cover a reaction cell; at the shipped numbers one forbidden charge raises
  the resistance 0.25 %, but nothing here bounds the loop over many cold cycles. Open.
* **A per-pack switch would have disagreed with a per-cell one**: `tick` first chose its branch from
  the pack's selector while the step's accumulation keyed off whether the cell holds a store, so a
  store-less cell on a reaction pack (what a hand-edited snapshot, or this slice's control arms,
  build) would have rolled threshold amp-hours at the dead-lithium hazard. Both now key off the cell.
