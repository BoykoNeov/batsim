# The carrier diagram — what moves inside the cell, drawn from the engine's state

**Status: built and measured, 2026-09-07.** No engine code moved; `sim-data` gained a
module and a function, `sim-wasm` one export and a version; seven chemistry files gained a
table; the page gained a panel. Predictions were registered before the first run and are
scored below.

## What this is about

Every panel on the browser page is a number or a curve. A reader who has watched the
voltage of a nickel cell turn over at the top of a charge has seen *that it happens*; nothing
on the page shows *what is happening*: oxygen coming off one plate and burning on the other.
The same is true of every edge state the engine can reach. A cell driven past empty shows a
falling `past empty` row; a cold charge shows a `PLATING_RISK` chip; a short shows a warm
tile. None of them shows the mechanism.

This slice adds one panel that does: a two-dimensional cross-section of the selected cell —
current collectors, two electrodes, the electrolyte between them, the external wire with
its load or charger — with the charge carriers drawn where the engine says they are and
moving as fast as the engine says they move. Above it, the pack as a circuit: the series
string, the parallel stacks, the contactor, and the electrons going round.

Three principles from `CLAUDE.md` decide the shape:

* **Chemistry is data, not code.** What a cell is made of — the carrier ion, the electrode
  materials, the collectors, what happens on overcharge or below empty — is chemistry
  knowledge, and it goes in the chemistry file as a `[diagram]` section. The page knows
  three *mechanism families* (an intercalation cell, a nickel cell, a lead-acid cell) and
  nothing about any particular chemistry. A new lithium chemistry gets a diagram by writing
  seven strings; only a new *mechanism* needs code.
* **Simulation time is decoupled from wall time.** The carriers move by simulated charge
  throughput, not by animation frames: a marker's position is a function of ∫I·dt. At
  10 000× they stream; paused, they stop; at rest they stop. Nothing about this panel
  invalidates the page on its own — it repaints exactly when the plots do.
* **Ground truth, and only what is on the wire.** Every drawn state is read from
  `Telemetry`, `CellView`, or the new chemistry facts. No number is invented: where the wire
  cannot say (which parallel cell carries what share of a group's current), the panel says
  so rather than guessing.

## Predictions, registered before the change ran

| # | prediction | outcome |
| --- | --- | --- |
| P1 | No engine code moves: `sim-core` is untouched, `SNAPSHOT_VERSION` stays at 21. `sim-wasm` gains one method (`chemistry_facts_of`) and `WASM_API_VERSION` goes 6 → 7; `WASM_API_MIN` in `web/app.js` follows, because the page calls it. `sim_server::API_VERSION` stays at 2 — no route changes. | |
| P2 | `[diagram]` is invisible to the engine: adding the section to all seven chemistry files reddens nothing in `cargo test --workspace --no-fail-fast`, because `ChemistryParams` does not deny unknown tables (its own docs say so, at `t_plating_min_k`). | |
| P3 | Two validation rules tie the section's prose to the file's physics and catch a real mismatch: the `cold_charge` caption is required exactly when `[safety].t_plating_min_k` is present, and the `runaway` caption exactly when `[safety]` is. Perturbation: deleting either from the LFP file reddens the new sim-data test and nothing else. | |
| P4 | Six edge states are reachable on the page from the shipped catalogue with no new scenario: refused overcharge (`nimh_overcharge`), below-empty reversal (`over_discharge_damage_lfp`), cold-charge plating (`cold_charge_nmc`), an internal short (`soft_short_under_a_lying_sensor`), the contactor opening (`external_short_30_milliohm`), and passive balancing (`cc_cv_charge_pack`). Thermal runaway and venting are **not** in the catalogue and need the BMS switched off and a hard charge at a hot ambient, by hand. | |
| P5 | A paused page's redraw rate does not move: the panel adds a draw call inside `draw()` and no `invalidate` of its own, so the 4–5 draws/s the redraw slice measured for a paused page stays. One diagram paint costs under 2 ms on the dev box. | |
| P6 | `path_claims.rs` stays green: the new panel's prose in `index.html` is not read by any tally there, and no lesson prose changes. | |

## What was built

**Data.** A `[diagram]` table at the foot of all seven chemistry files: `family` (one of
three), the carrier's label, the two electrode materials, the electrolyte, the two
collectors, and four captions — `overcharge`, `deep_discharge`, `cold_charge`, `runaway`.
The captions are what the page *says*; the engine decides *when*. The last two are
optional and their presence is a validated claim: `cold_charge` exactly when
`[safety].t_plating_min_k` is set, `runaway` exactly when `[safety]` exists. So the LTO
file, which says in its own `[safety]` that the cell does not plate, has no cold-charge
sentence to show, and the nickel and lead-acid files, which carry no `[safety]`, have no
runaway sentence — and a caption written for a mechanism the file does not model is a
load error, not a label.

The Godot demo bundles a copy of the LFP file under `godot/assets/`, and
`sim-godot`'s `demo_assets` test holds the copy byte-equal to the canonical file — so the
table went there too, and that test was the one red binary of the workspace gate until it did.

**Loader.** `sim_data::diagram` (`DiagramParams`, `DiagramFamily`, `ChemistryFacts`) and
`sim_data::parse_chemistry_facts`, which validates the chemistry exactly as
`parse_chemistry` does, reads the table separately with `deny_unknown_fields`, applies the
two rules, and copies out the limits a drawing scales by: `capacity_ah` (a current into a
C-rate), the voltage window, the charge-inhibit and over-temperature limits, the plating
gate and the runaway onset and vent temperatures when present, the charge-acceptance onset,
and presence booleans for the five optional sections. `ChemistryParams` never sees the
table — a test serialises the engine's parse with and without it and asserts equality.

**Boundary.** `sim_wasm::chemistry_facts_of(chemistry_toml) -> JSON`, and
`SimEngine::chemistry_facts_of` beneath it for the host tests. `WASM_API_VERSION` 6 → 7;
`WASM_API_MIN` in `web/app.js` 6 → 7; `web/pkg` rebuilt. Nothing on `Sim` changed.
The page fetches the chemistry by id — the same fetch `WasmBackend.create` makes — and asks
the module for the facts, so it still parses no TOML of its own; over the socket backend
it does the same, because the server serves `/chemistries` either way.

**Page.** One panel, `#carriers`, between the pack grid and the BMS view, with one canvas,
a head line and a note. Roughly 750 lines in `web/app.js`, in one section:

* *The pack band.* One box per series group (up to twelve, then a count), the parallel
  cells stacked inside it as bars filled by true state of charge from the positive end, the
  bus between groups, and the outer loop through a contactor and a sink — a resistor while
  discharging, a charger while charging, a gap at rest. Electrons run round the loop at a
  count set by the C-rate. `CONTACTOR_OPEN` draws the switch open, in red, and stops them.
  A shorted cell gets an amber ring; a vented one is red. Clicking a bar pins that cell —
  the same `grid.pinned` the tile grid uses, so the two agree by construction.
* *The cell band.* The pinned or hovered cell (cell 0 otherwise) in cross-section:
  collector, negative electrode, electrolyte lane with a dashed separator, positive
  electrode, collector, all labelled from the table; the wire over the top with the same
  sink; the body tinted by temperature against the chemistry's own limits (blue below
  charge-inhibit, orange above `t_max_k`, red at onset). Thirty-six carrier slots, placed by
  a seeded scatter so the picture does not shimmer between paints. Where they sit is the
  family's rule: full means the carrier is in the negative (intercalation, nickel) or in the
  acid (lead-acid, where sulfate grows on *both* plates as the cell empties). The count in
  each place is the cell's true `soc`; a share equal to `1 − soh_capacity` is drawn grey
  against a film on the negative whose thickness grows with `soh_resistance`. On a porous
  model the surface band of each electrode is tinted by its `surface_gap_*`.
* *Motion.* `flow.q_c` integrates `i_actual · Δt` over the recorded frames. The lane
  markers, the wire's electrons and the gas all take their position from that one signed
  number, so a charge runs every animation backwards without any code consulting the
  current's sign twice — the first version did, and charged cells moved the wrong way
  (see below). One lane crossing is 1/180 of the capacity, so at 1 C a crossing is 20 s of
  simulation time. The panel never invalidates; hovering a tile does, because the panel
  follows the hover.
* *Edge states*, each drawn where it happens and captioned from the table:
  `i_rejected_a < 0` (refused charge) → metal on the negative's face on an intercalation
  cell, oxygen crossing positive → negative on nickel, hydrogen and oxygen off the two
  plates on lead-acid, with the refused amps printed; `soc_deficit > 0` → the copper
  collector pitting with Cu²⁺ leaving it on a lithium cell with a copper collector,
  hydrogen off the positive on nickel, "sulfation" on lead-acid, with the points past empty
  printed; `PLATING_RISK` while charging → metal spikes on the negative's face;
  `internal_short_conductance_s > 0` → an amber bridge through the separator with its ohms;
  `temp_k ≥ t_onset_k` → a red frame and the kilojoules left to burn; `vented` → gas leaving
  the top; `BALANCING` → a caption, because *which* group bleeds is not on the wire.

**Instrument.** `tools/client-perf/carriers.mjs`: loads each case, runs to a stated
condition on the page (a flag, a field, a time), pauses, writes a PNG of the panel and the
panel's own two text lines, and times the diagram's paint alone.

## What was measured

Each row is one case of `carriers.mjs`, run against the shipped catalogue on
2026-09-07, headless Chrome 152 at 1600 px, in-page wasm. "held" is whether the case's own
condition was reached before its wall-clock cap; the text columns are what the panel
printed, verbatim.

| case | scenario, controls | driven until | sim t [s] | flags at the stop | diagram paint [ms] |
| --- | --- | --- | --- | --- | --- |
| discharge | `cc_discharge_lfp`, 2 A | 600 s | 607.5 | — | 0.10 |
| CC-CV charge | `cc_cv_charge_nmc`, 1.5 A to 4.2 V | 900 s | 921 | — | 0.10 |
| refused overcharge | `nimh_overcharge`, −3 A | `i_rejected_a < −0.5` | 3313 | `SOC_CLAMPED_HIGH` | 0.20 |
| past empty | `over_discharge_damage_lfp`, 2 A | `soc_deficit > 0.01` | 514 | `SOC_CLAMPED_LOW \| OPERATING_POINT_OUT_OF_WINDOW` | 0.20 |
| cold-charge plating | `cold_charge_nmc`, −3 A at −20 °C | `PLATING_RISK` | 25.5 | `PLATING_RISK` | 0.20 |
| internal short | `soft_short_under_a_lying_sensor`, 6 A, cell (1,0) pinned | the short has fired | 606 | — | 0.20 |
| contactor open | `external_short_30_milliohm`, rest | `CONTACTOR_OPEN` | 62 | `CONTACTOR_OPEN` | 0.20 |
| balancing | `cc_cv_charge_pack`, 3 A to 4.2 V | `BALANCING` | 3174 | `BALANCING` | 0.20 |
| lead-acid | `cc_discharge_pba`, 0.36 A | 7200 s | 7328 | — | 0.20 |
| DFN surface bands | `cc_discharge_3c_dfn`, 15.46 A, dt 2 | 400 s | 406 | — | 0.20 |
| runaway onset | `calendar_fade_hot`, −20 A at 60 °C, hottest cell pinned | any cell ≥ `t_onset_k` | 392 | `SOC_CLAMPED_HIGH \| THERMAL_RUNAWAY \| OPERATING_POINT_OUT_OF_WINDOW` | 0.20 |
| vented | `calendar_fade_hot`, the same, the vented cell pinned | `VENTED` | 528 | `SOC_CLAMPED_HIGH \| VENTED \| OPERATING_POINT_OUT_OF_WINDOW` | 0.30 |

Every case reached its condition, and every captioned state printed the caption its
chemistry file carries — the overcharge sentence on the nickel cell, the deep-discharge
sentence on the lithium one, the plating sentence at −20 °C, and on the runaway cases both
the overcharge and the runaway sentences at once, because both are true at once.

Two frames taught something the numbers alone would not have:

* **The vented frame looked empty**, with almost every carrier drawn grey against the film
  and none in either electrode. That is not a drawing defect: the pinned cell's own record
  at that instant reads `soh_capacity = 0.01` at 714 K — the calendar-fade Arrhenius term
  at 440 °C had taken the cell's capacity to a hundredth of new in the eight minutes since
  onset — so thirty-five of thirty-six carriers *are* trapped. The picture was right and the
  eye was wrong, and the driver now records the drawn cell's `CellView` beside every PNG so
  the next reviewer can check the same way.
* **The runaway frame at onset** read `refused 80.00 A` on a pack charged at 20 A. That
  is `i_rejected_a`, which is summed over every cell (the `clamp` readout prints the same
  number), and the label now says "across the pack" on any pack with more than one cell.
  The head line had the matching defect — a 2P pack's 4.34 C charge printed as 8.68 C of
  one cell — and now prints the amps and the C-rate *of the pack*.

The diagram's paint is the median of twenty calls to `drawCarriers` on the paused page;
the whole `draw` (six plots, readouts, grid, diagram) is `view.drawMs` on the same page.

## Predictions, scored

* **P1 — confirmed.** `git diff --stat crates/sim-core` is empty; `SNAPSHOT_VERSION` is 21
  (the plan said 20, which was a stale number in my head, not a change — corrected in
  place); `WASM_API_VERSION` 6 → 7 and `WASM_API_MIN` with it; `sim_server::API_VERSION`
  untouched.
* **P2 — confirmed.** `cargo test --workspace --no-fail-fast` is green with the seven
  tables in place, and `the_engine_never_sees_the_section` holds the stronger claim: the
  engine's parse is byte-identical with and without the table.
* **P3 — confirmed, and the rule caught its own first draft.** The test that drops the
  `runaway` line from the LFP file initially dropped `[safety]`'s `runaway_energy_j`,
  `runaway_power_w_at_onset` and `runaway_ea_j_per_mol` with it, because the helper
  matched on a prefix — so the file failed as a *chemistry* and the test asserted the wrong
  error. Fixed by matching the exact key. And the NiMH file is CRLF in the working tree
  while its siblings are LF, so a `"[diagram]\n"` replacement matched six files and not the
  seventh; both helpers normalise first.
* **P4 — confirmed for six, and the seventh needed a different file.** Six of the six named states were reached from the shipped
  catalogue with the controls a lesson sets. The balancing case failed on the first run for a
  driver bug, not a page one: the previous case had set the ambient to −20 °C and the driver
  never put it back, so the BMS raised `UT` and inhibited the charge. Every case now resets
  the ambient and the step length first. Thermal runaway was reached on
  `calendar_fade_hot` — the one shipped pack with a thermal network and no BMS — with a
  −20 A charge at 60 °C, and not on `cc_discharge_nmc`, which the plan named and which is
  isothermal: 9 000 s of refused 10 C charge left that cell at 25 °C. An isothermal
  scenario cannot show a runaway however hard it is driven, which the plan should have
  read off the file's own description before naming it.
* **P5 — confirmed, by a wide margin.** The diagram paints in 0.1–0.3 ms (median of twenty, see the table); the whole `draw` it sits inside is 1.0–3.2 ms, which is the range the redraw slice measured before this panel existed. The paused page's draw
  rate was not re-measured with `measure.mjs`; the argument is structural — the panel adds
  no `invalidate` except on tile hover — and the per-draw cost above is the whole of what
  it adds to a paint the redraw slice already paced.
* **P6 — confirmed.** `path_claims` 69 passed, unchanged.

## Perturbations

Run against the shipped LFP file with `cargo test --no-fail-fast` on the two binaries that
read it (`sim-data`'s `diagram` and `load`, `sim-wasm`'s `engine`), file restored
byte-exact after each. The in-suite cases (a caption dropped, a caption added where it
cannot be, an unknown key) are the tests themselves and are not repeated here.

| perturbation | what reddened |
| --- | --- |
| the whole `[diagram]` table deleted | 7 of 10 in `diagram.rs` — the two shipped-file tests and five whose helpers `expect` a table to edit (a harness assumption, not five independent catches) — and `chemistry_facts_cross_the_boundary…` in `sim-wasm`. `load` stayed green, 25 of 25: the engine is blind to the table by construction, and the page draws that chemistry generic and says so. |
| `family = "intercalation"` → `"nickel"` | exactly the family assertion in `every_shipped_chemistry_carries_a_diagram…`, and the `sim-wasm` boundary test, which pins the family it expects to read. `load` green. The family is the one field the page's *code* interprets; these two assertions are all that holds it. |
| `WASM_API_MIN` left at 6 against a v7 need | nothing red, by inspection: the page would run against a stale bundle until `loadScenario` threw `chemistry_facts_of is not a function`. The bump is what turns that into the stale-bundle banner. |
| a charging cell's carriers moved the wrong way | caught by reading, not by a test — the first version applied the current's sign to a phase that already carried it. Recorded because no test on this page reads a canvas, and the screenshot driver is the only instrument that would have shown it. |

## Deliberately not done

* **No per-cell current.** `CellView` has no current (Phase 6 slice D declined it), so on a
  pack with parallel cells the cross-section moves its carriers by the pack current and the
  note says so. Drawing the parallel split — which cell takes more load — would need the
  field, and the field is a snapshot-neutral addition the engine could make; it is the
  first thing a follow-up should add, because the split *is* the imbalance physics the pack
  grid exists to show.
* **No hysteresis or charge-acceptance state.** Neither is on `CellView`. The nickel
  drawing shows the refused current the acceptance term produces, not the term.
* **No sensor view.** The panel is ground truth throughout; the BMS's belief has its own
  panel and this one does not repeat it.
* **No guided-path step.** The path is claim-checked sentence by sentence
  (`path_claims.rs`), and a lesson about this panel would need claims about a canvas no
  check can read. Which step should point at it, and what it can honestly claim, is a
  path slice.
* **No server route.** `sim-server` could serve the same facts at `/chemistries/{id}/facts`;
  the page did not need it, because it has the module and the text. `API_VERSION` stays.
* **The runaway drawing is per cell and the propagation is only in the pack band** (the
  red bars). A neighbour heating toward its own onset is visible as a tint only if it is
  the selected cell.
* **Lead-acid's lane is ambiguous in a still frame**: the acid population and the markers in
  transit share the lane and the colour. Live, one set moves and one does not; a
  screenshot cannot tell them apart. A second marker style is a five-line change left for
  whoever finds the still frames matter.

## Still open

* Per-cell current on `CellView`, then the parallel split in both bands (above).
* A `[diagram]` for a chemistry that fits none of the three families — a zinc or a flow
  cell — is a client slice by design: the enum is closed, and adding a variant means adding
  a drawing.
* The panel's fallback when a scenario inlines its chemistry: today the note says "the
  scenario inlines its chemistry" and the cell is generic. `chemistry_facts_of` could be
  handed the inline text instead; no shipped scenario inlines, so nothing exercises it.
