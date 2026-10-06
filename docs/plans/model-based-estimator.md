# A model-based SOC estimator for the BMS (ROADMAP H7)

The BMS has one way to know how full the pack is: count the measured current
(`Bms::update_estimate`), and, after the pack has rested long enough, look the resting
voltage up on the chemistry's curve — but only where that curve is steep enough to trust.
That is enough to show drift, a wrong boot, and the LFP plateau refusing to answer, and the
guided path teaches all three. It cannot show what a production BMS does instead: run a
small model of the cell beside the pack and correct the count from the voltage **all the
time**, under load as well as at rest, by an amount that depends on how much it trusts each.

This slice adds that estimator — an extended Kalman filter (EKF) over an equivalent-circuit
cell the BMS owns — as an opt-in alternative to coulomb counting. Engine only. The lesson
that teaches it is a later slice: every guided-path step is its own slice under the claims
harness.

## What the filter is allowed to know

Principle 8 binds harder here than anywhere: a model-based estimator is exactly the place a
shortcut to ground truth would hide. The filter may use

* the chemistry's **tables** — `[ocv]` (with its temperature correction where the file has
  one), `[r0]`, `[[rc]]` — and its **nominal** capacity times the parallel count, the same
  capacity the coulomb counter already believes;
* the **sensor frame** — the mean group voltage, the mean probe temperature, the measured
  pack current — and the time it was sampled.

It may not read any cell's state of charge, RC voltages, hysteresis state, temperature,
`soh_*`, or scatter factors. So everything the engine models that the filter's copy does
not — hysteresis, the lead-acid diffusion term, charge acceptance, aging, scatter, a weak
cell, a porous model's physics, a sensor fault — reaches the filter as **model error**, and
the gap between its estimate and the truth is that error divided by the curve's slope.

## Design

* `BmsConfig::estimator: EstimatorConfig`, `#[serde(default)]` → `CoulombCount`, which is
  today's code path untouched. `Ekf(EkfConfig)` is the new arm; in TOML,
  `[pack.bms.estimator.Ekf]`, the externally tagged shape `[pack.cell_model.Dfn]` already
  uses.
* State `x = [soc, v_rc,1, v_rc,2]` per **average cell** (one or two RC pairs, from the
  chemistry), covariance `P` 3×3, both fixed-size arrays — `Pack::step` allocates nothing and
  must keep not allocating. Unused slots stay zero.
* **The filter's assumed noise is its own configuration**, not derived from
  `current_noise_sigma_a`. What the BMS assumes about its sensors and what they really do are
  different numbers in a real pack, and that difference is part of the lesson.
  `current_sigma_a` [A] sets the process noise (one draw per step, as the sensor model draws
  it, so the covariance grows by `σ²·B·Bᵀ` with `B` the current's input column);
  `voltage_sigma_v` [V] sets the measurement noise — and since the simulated voltage sensors
  are exact, it stands for **model error**, not sensor noise; `initial_soc_sigma` sets
  `P₀`. All three are placeholders in the provenance sense and will be labelled so.
* **Timing.** The frame holds the current measured over the previous sampled interval and
  the voltage at its end. The coulomb counter integrates that current over the *current*
  step's `dt` — a lag that the module docs accept. The filter does not copy it: it keeps the
  time of the last frame it consumed and predicts over `frame.sampled_at_s − last_t`, so the
  current and the voltage it pairs always cover the same interval at any `dt` pattern. Only a
  sampled frame is a measurement: the frame `Bms::new` synthesises at `t = 0` is not consumed,
  so a wrong boot is not erased by a read nobody took, and a zero-length step (which samples
  nothing) moves nothing.
* **Temperature.** `R0` and the OCV correction need one. The mean probe reading is the only
  legitimate source, so the filter is **refused at build** on a pack with no probes rather
  than given a fallback that would have to be a guess or a cheat.
* Update in Joseph form, `P` symmetrised, SOC clamped to [0, 1]; a non-finite reading or a
  non-positive innovation variance skips the update. Never panics; draws nothing from the
  RNG.
* `Bms` gains the estimator state → `SNAPSHOT_VERSION` 23 → 24 and a pair test. The
  reported estimate is still `Telemetry::soc_bms`; nothing on the wire changes shape.

## Predictions, registered before any run

Measured on the three gauge scenarios (`na_ion_gauge_corrects`, `lfp_gauge_declines`,
`na_ion_gauge_low`) at `dt` = 0.5 s, each with an `Ekf` twin differing by the estimator
alone, and on the BMS test file's synthetic cells. Filter settings for the scenario arms:
`current_sigma_a` = 0.05, `voltage_sigma_v` = 0.010, `initial_soc_sigma` = 0.05.

1. **Default unchanged.** With the field omitted, every test outside the snapshot-version
   pair passes unmodified and every scenario trajectory is bit-identical.
2. **Alignment.** On the steep linear test cell with exact sensors, no boot error and an
   exact model, the filter's estimate equals the true SOC to 1e-9 at every step under a
   pattern of alternating `dt` (0.3 / 1.7 s) and a load that changes sign. A filter that
   predicted over the wrong interval would see a non-zero innovation on every change.
3. **Wrong boot, steep cell.** Booted 3 points high on the same cell under a 1 C discharge:
   coulomb counting holds the 3 points; the filter is under 0.1 point within 60 s.
4. **Sodium-ion, mid-range.** The filter reads the midline continuously, so it carries the
   hysteresis bias under load as well as at rest. Prediction: it ends the rest within 0.15
   point of the coulomb counter's −0.494 point (both read the same midline at the same
   resting voltage), but it is already within 0.5 point of that figure **by the end of the
   300 s discharge**, where the counter is still +2.9 high.
5. **Sodium-ion, near empty.** The same, against the counter's −0.975: within 0.25 point at
   the end of the rest. The wider loop costs the filter what it costs the rest read.
6. **LFP, mid-plateau.** The model is exact here (no hysteresis in the file, no scatter, no
   aging, isothermal), so the only model error is the 20 mA offset times `R0`. Estimated
   from the information a 10 mV-trusted sample carries on a 0.022–0.057 V-per-unit segment:
   the filter **does** converge — it is not refused the way the rest read is — but slowly:
   still more than 1 point off at the end of the 300 s discharge, under 0.5 point by the end
   of the hour's rest. The figure the roadmap predicted ("the observer's gain is small
   mid-range") is right about the gain and wrong about the outcome when the model is
   exact.
7. **LFP with a model error.** The real failure on a flat curve is model error, not gain.
   Give the same LFP run a cell whose `R0` is 20 % above the table (a `WeakCell` with
   `r0_factor` 1.2): under 1 C the voltage the filter expects is ~9 mV too high, which on a
   0.022–0.057 V-per-unit segment is worth several points. Prediction: the filter's error
   **under load** exceeds 3 points (and has the sign that reads the cell emptier than it
   is), and relaxes back toward the truth at rest when the current, and with it the error,
   goes away. The same weak cell on sodium-ion (slope ~1.9) costs under 0.5 point.
8. **Offset, no bias state.** With a 50 mA offset and no boot error over an hour of 1 C
   cycling on the steep cell, the counter drifts linearly; the filter's error stays bounded
   under 0.2 point.
9. **Snapshot.** A snapshot taken mid-run with the filter live, restored and continued,
   reproduces the telemetry stream bit for bit. `Pack::step` with the filter on allocates
   nothing.

## Results

Measured 2026-10-06 on the in-tree engine (the spike *was* the implementation, run through
a scratch harness at `W:\temp\claude\ekf\spike` before any test was written). Every
number below is now re-measured in tree, to the fourth decimal the scenario headers print,
by `crates/sim-data/tests/gauge_filter.rs`.

Estimate minus truth in points of charge; booted 3 points high, 1 C for 300 s, rest
3600 s, `dt` 0.5 s; `sigma` is the filter's own standard deviation, in points.

| run | estimator | at 30 s | discharge ends | rest ends | sigma at end |
| --- | --- | --- | --- | --- | --- |
| sodium-ion, 60 % | counter | +3.0025 | +2.9001 | −0.4941 | — |
| sodium-ion, 60 % | filter | +0.0465 | −0.2445 | −0.5630 | 0.0147 |
| sodium-ion, 25 % | counter | +3.0025 | +2.9001 | −0.9753 | — |
| sodium-ion, 25 % | filter | +0.0050 | −0.6012 | −1.0015 | 0.0131 |
| LFP, 60 % | counter | +3.0067 | +2.9420 | +2.0574 | — |
| LFP, 60 % | filter | +2.9864 | +1.9534 | +0.6286 | 0.2055 |
| LFP, 60 %, `R0` × 1.2 | counter | +3.0067 | +2.9420 | +2.0574 | — |
| LFP, 60 %, `R0` × 1.2 | filter | −14.7430 | −11.4428 | −6.6661 | 0.0646 |

### Predictions, scored

1. **Default unchanged — held.** The four counter rows reproduce the partner files' own
   headers to four decimals, and the full suite passes with only the snapshot-version pair
   rewritten for v24 (as every bump does) and `estimator: Default::default()` added to
   each hand-built `BmsConfig`.
2. **Alignment — held.** Worst miss below 1e-9 under alternating 0.3 / 1.7 s steps with a
   sign-changing load and a rest; also on a 2S3P pack, and on a cold pack with an `R0`
   that triples and an OCV temperature correction (two arms added during the work, after
   the perturbation table below showed nothing else would catch their defects).
3. **Wrong boot, steep cell — held.** Under 0.1 point by 60 s; the counter holds 3.
4. **Sodium-ion mid-range — held.** Ends −0.563 against the counter's −0.494 (within 0.15)
   and is −0.24 at the end of the discharge, within 0.5 of the counter's final figure.
   That the loop is the cause has a control arm (added after review): the same twin on the
   chemistry with `[hysteresis]` removed ends at −0.105, the remainder being the offset
   (−0.002 with the offset removed too, scratch harness only).
5. **Sodium-ion near empty — held.** −1.0015 against −0.9753; −0.064 without the loop.
6. **LFP with an exact model — half right.** More than a point off at the end of the
   discharge (1.95), as predicted; **not** under 0.5 by the end of the rest (0.63, against
   the counter's 2.06 on the same run — the comparison, not the distance moved). The
   information estimate counted samples and forgot that the filter's own sigma shrinks as
   it learns, which slows the tail.
7. **LFP with a model error — held, and understated.** −14.7 points under load (predicted
   "more than 3", sign right). The prediction said the error would "relax back toward the
   truth at rest"; it relaxes from −11.4 to −6.7 and **stops there**, because by then the
   filter is sure of itself — the finding this note is about. **Where** it stops is the
   `[ocv]` table's 0.45 node, held there by the offset (advisor's catch, then measured): from
   1800 s to the end the estimate sits on 45.00 %, because above the node the plateau's
   0.057 V per unit pulls less than the 20 mA offset pushes and below it 0.297 V per unit
   pulls more. With the offset removed it crosses the node and is at 45.45 %, −6.22 points
   and still moving, at the hour's end. Both are asserted in `gauge_filter.rs`. The sodium-ion half ("the
   same weak cell costs under 0.5 point") was **wrong**: the spike measured −1.33 at the
   end of the discharge. Sodium-ion's `R0` is 0.074 Ω, more than three times LFP's, so the
   same 20 % is 21 mV, not 9; the prediction priced LFP's resistance onto the other cell.
8. **Offset — wrong as written.** "Bounded under 0.2 point" at 0.05 A of assumed current
   noise: the error is bounded but settles at −0.737 point, by the third hour. It shrinks
   to −0.117 at 0.2 A assumed and +0.035 at 1 A — telling the filter to expect more current
   noise makes it lean on the voltage harder. The counter is 8 points out at four hours.
   The test (`an_offset_drifts_the_counter_and_settles_the_filter_on_a_steep_cell`)
   asserts the settling, the bound under a point, and the shrinking, not the 0.2.
9. **Snapshot and allocations — held.** Bit-identical through bincode mid-run; zero
   allocations with the filter on (a new arm in `step_allocations.rs`).

Gates: `cargo test --workspace` 742 passed, 0 failed, across 88 test binaries; clippy with
`-D warnings` and `cargo fmt --check` clean. `node tools/wasm-parity/parity.mjs` after a
rebuilt `web/pkg`: **124 / 124** bit-identical (62 runs and their snapshot handoffs, against
108 / 54 before — the eight new runs are the four filter scenarios, so the filter's `exp`
and its matrix arithmetic agree between the browser and the native build too).

### What the lesson actually is

The roadmap's sentence — "converging where coulomb counting drifts, and diverging on the
sodium-ion cell's hysteresis where the rest-OCV gate refused to correct" — had the wrong
cell. On the shipped scenarios the rest read *corrects* on sodium-ion and *declines* on
LFP. What the filter adds is speed on sodium-ion (half a minute against ten), no escape from
the hysteresis there (it lands where the counter lands), slowness rather than refusal on
LFP, and one new failure the counter cannot have: on a flat curve a model error of a few
millivolts is ten or more points of charge, and while the filter is making that error under
load its uncertainty collapses, so at rest it no longer listens to the voltage that would
put it right. The counter, which never reads the voltage under load, is the better
estimator on that cell. (The exact figure it stops at, −6.67, is the table's 0.45 node with
the offset holding it there — quote the collapse, not the number, when teaching it.)

### Perturbation table

Each row breaks `bms.rs` one way, runs `bms_ekf`, `step_allocations`, `snapshot_version`,
`gauge_filter` and `scenario` with `--no-fail-fast`, and restores the file (hash checked).
Harness: `W:\temp\claude\ekf\perturb.py`.

| break | tests that reddened |
| --- | --- |
| A — predict over this step's `dt`, the counter's convention | `..._at_any_dt_pattern`, `..._at_the_probe_temperature`, and the three filter-scenario tests (the first step then consumes the boot frame too) |
| B — drop `R0`'s slope from the measurement Jacobian | the three filter-scenario tests (the synthetic cells have a flat `R0`) |
| C — short-form covariance update instead of Joseph | **none** — the difference is below the scenarios' 5e-5-point tolerance on every run here; Joseph is a robustness choice these runs cannot see |
| D — consume the synthesised boot frame | `..._at_the_probe_temperature` and the three filter-scenario tests |
| E — no process noise | the offset test and the three filter-scenario tests |
| F — tables read at 298.15 K, not the probe | `the_filter_reads_its_tables_at_the_probe_temperature` only (every scenario is isothermal at 298.15 K) |
| G — pack current through one cell | `the_filter_models_the_average_cell_of_a_series_parallel_pack` only (every scenario is 1P) |
| H — OCV temperature correction dropped | `the_filter_reads_its_tables_at_the_probe_temperature` only |

F, G and H were uncaught until the two arms named under prediction 2 were written; they
were written because the table was about to be.

## Deliberately not done

* **The guided-path lesson.** The four scenarios are loadable from the picker (the server
  lists the directory), but no step teaches them; that is its own slice under the claims
  harness, and it should be the LFP pair — the filter that corrects slowly beside the same
  filter on a cell 20 % off its table. **Done since**, as steps 33 and 34
  (`path-gauge-filter-steps.md`).
* **A bias state for the current offset.** The textbook fix for prediction 8 is a state the
  filter estimates; leaving it out is what makes "assume more noise" visible as a tuning
  trade-off. Add it as a separate estimator arm if a lesson wants the contrast.
* **Per-group filters.** One filter on the mean group voltage, reporting one number as the
  counter does. A real pack runs one per series group and reports the weakest; that needs a
  second telemetry field.
* **The filter's sigma on the wire.** `Bms::soc_sigma` exists for tests and for the lesson
  that will want it; `Telemetry` and both client payloads are unchanged, so neither
  `sim_server::API_VERSION` nor the wasm constant moves.
* **The counter's one-step lag** is left as it was. The filter does not copy it, and
  changing the counter would move every gauge lesson's numbers for no teaching gain.

## Still open

* ~~The lesson (above).~~ **Landed 2026-10-06** as guided-path steps 33 and 34, on the LFP
  pair at LFP's own 1 C: `docs/plans/path-gauge-filter-steps.md`. The filter's sigma is
  still not on the wire; the steps state the collapse in words and name the test.
* Whether the Joseph form matters anywhere in this engine (row C): not on these runs.
* A filter whose `voltage_sigma_v` is too small for its model error is how "confidently
  wrong" happens. **Swept since** (5–100 mV, `docs/plans/path-gauge-filter-steps.md`, "The
  headline rides on a hand-picked setting"): up to about 30 mV the weak-cell filter only goes
  wrong later and ends the rest about as far out; at 100 mV it ends +0.31 points, inside its
  own error bar. Loosening costs the exact-model LFP file almost nothing by the end of the
  rest (about a point earlier, under load) and the sodium-ion filter pair about half a point
  at rest. What is still open is a scenario the *reader* can sweep it
  in — the page has no control for it — and a lesson on choosing it.
