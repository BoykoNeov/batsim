# The current each cell actually took — the parallel split, put on the wire

**Status: built, 2026-09-07.** Predictions were registered before anything ran and are scored below.

## What this is about

`docs/plans/carrier-diagram.md` shipped a cross-section of one cell with its charge
carriers drawn where the engine says they are, and it ends by naming its own first
follow-up:

> **No per-cell current.** `CellView` has no current (Phase 6 slice D declined it), so on a
> pack with parallel cells the cross-section moves its carriers by the pack current and the
> note says so. Drawing the parallel split — which cell takes more load — would need the
> field, and the field is a snapshot-neutral addition the engine could make; it is the
> first thing a follow-up should add, because the split *is* the imbalance physics the pack
> grid exists to show.

That is the gap this slice closes, and the sentence above is worth reading twice. The
closed-form parallel solve is the one piece of physics `CLAUDE.md` singles out with an
instruction not to shortcut it —

> Currents naturally split by state — a low-resistance or high-SOC cell takes more load.
> This is where imbalance physics emerges; do not shortcut it by averaging cells.

— and no client has ever been able to see it. Every panel that touches the split shows it
*integrated*: the pack grid's SOC spread is where the currents have already been, the
weak-cell scenario is the same story after an hour. The instantaneous quantity — which
cell is taking the load **now** — has been computed inside `Pack::step` since Phase 1 and
thrown away at the end of every step.

Three places in the tree state its absence in prose, and all three become false:

* `web/app.js:1400` — the pack grid's metric list: "**per-cell current** — Phase 6 slice D
  declined the accessor on purpose. So 'which cell is taking the most load right now' is
  not on this menu."
* `web/app.js:1985` — the carrier diagram's header: "Per-cell current is *not* on the wire
  … the split between parallel cells is a thing this page cannot draw, and it does not
  guess."
* `web/app.js:2718` — the panel's own live note: "the carriers move by the pack current:
  which parallel cell carries what share is not on the wire".

## Three questions the design has to answer first

### 1. Which current is "this cell's current"?

Three candidates exist in the step, and they are different numbers:

* **The internal branch current** `i_k = (E_k − V_node)/R_k`, computed at `pack.rs:2361`.
  Its own comment says what it is: "this is what moves charge through the electrodes, so
  it is what drains SOC, drives the RC pairs, and is charged for throughput."
* **The terminal current**, smaller by `V_node · shunt_g` on a shorted cell — the part of
  the branch current that never leaves the cell.
* **An end-of-step re-derivation** from the reporting pass at `pack.rs:2694`, which
  recomputes each group's node voltage from end-of-step state.

**The branch current wins, and not by a close margin.** It is the number the cell was
actually advanced with — the one that moved its SOC, drove its RC pairs and generated its
heat over the step that just happened. The other two are a hypothetical. It is also the
right number for the drawing: the carrier diagram moves markers by charge that crossed the
electrodes, which is exactly what this integrates. And it costs no arithmetic at all,
because the split loop already computed it and then dropped it on the floor.

The consequence to document rather than hide: on a shorted or balancing pack the reported
currents **do not sum to the pack current**. They sum to `i_actual + Σ V_node·G_shunt +
Σ V_node·G_bleed`. `crates/sim-core/tests/properties.rs:283` already knows this and states
it in its own doc comment; `CLAUDE.md`'s one-line summary of the same property does not,
and anyone reading only that line would write a test that goes red the moment a bleed
switch closes.

### 2. Where does it live, given the field must not cost a snapshot bump?

Three options, and the note quoted above already asserts the answer is snapshot-neutral:

| option | cost |
| --- | --- |
| an `i_last_a` field on `Cell` | `SNAPSHOT_VERSION` 21 → 22, a migration story, and `Cell` grows 176 → 184 bytes, handing back a slice of what `cell-size.md` measured |
| the pack current on `Pack`, split re-derived inside `Pack::cell()` | same bump; and the re-derivation is a *second* expression of the split that has to agree with the first, including on the probe-step path `report_from_solve` exists for |
| a `#[serde(skip)]` per-cell buffer on `Pack`, written where the split is computed | no bytes, no bump, no second derivation, `Cell` untouched |

The third. The file already has two `#[serde(skip)]` buffers on `Pack` and documents the
rule that separates them:

* `SourceCache` is **carried across the step boundary** and is correct because a cold
  recompute reproduces it exactly.
* `StepScratch` is **written before it is read within one step**, and its doc says
  explicitly that a buffer carrying a value across the boundary "would be state".

The new buffer is neither: it is carried across the boundary (`Pack::cell()` reads it after
`step` returns) and it is *not* recomputable from stored state, because re-deriving it
needs the pack current and no pack stores one. So it is a third kind — **a report about the
step that just happened** — and it gets its own type with that contract written on it. It
is never read by the physics, which is what keeps it out of the determinism argument
entirely.

### 3. What does a pack that has not stepped report?

`Option<f64>`, and `None` means "no time-advancing step has run since this pack was built
or restored". `0.0` is not available for that job: a resting cell genuinely carries 0.0 A,
and `CellView::surface_gap_neg`'s doc already argues this exact case ("deliberately not
`0.0`" — indistinguishable, to a plotting client, from a real measurement).

The gate is `dt > 0.0`, which joins the family the step already names — the BMS sensor
clock, the aging sub-clock, the fault queue and the vent latch. A zero-length probe
therefore leaves the previous reading standing, exactly as `overpotential_v` does on a
porous cell (`spm.rs:638`: "leaves this reading whatever the last real step left it").

*(Overtaken before it shipped: there is no gate, and a probe reports the split it was
probed at. The reasons are under P4 in the scoring below, and on the `CellCurrents` doc.)*

**The honest cost of `#[serde(skip)]`, stated here rather than discovered later:** a
restored pack reports `None` until its next step, where a live pack at the same instant
reports a number. Nothing in the trajectory moves — the field is not state and no physics
reads it — but a client that restores and immediately reads the cells sees one empty
sample. The browser page samples the cells on a 250 ms clock and falls back to exactly its
present behaviour when the field is absent, so the visible consequence is a quarter second
of the old drawing after a restore. That is the price of not bumping, and it is worth it.

## What will be built

**Engine.** `CellView` gains `current_a: Option<f64>`, discharge-positive, documented as
the internal branch current of the last time-advancing step. A `CellCurrents(Vec<f64>)`
newtype on `Pack`, `#[serde(skip)]`, series-major / parallel-minor like every other
per-cell buffer, with `PartialEq` always true and `Debug` printing its length — the two
deliberate impls `SourceCache` and `StepScratch` both carry, for the same reason. Filled in
the split loop at `pack.rs:2308`; taken and put back like `src_cache`, so `Pack::step`
still allocates nothing.

**Adapters.** `sim_wasm::WASM_API_VERSION` 7 → 8, which is what `CellView` field additions
cost on that boundary (v5 was the surface gaps, v6 was `soc_deficit`).
`sim_server::API_VERSION` stays at 2: its own doc says "Adding a field or an error code
does not bump it."

**Client.** The carrier diagram moves the selected cell's carriers by *that cell's* current
rather than the pack's; the pack circuit band draws the split across a group's parallel
cells; the pack grid gains a `current_a` metric so the question "which cell is taking the
most load right now" is answerable by looking. The three false sentences above are
rewritten to say what is now true.

## Predictions, registered before the change ran

| # | prediction | outcome |
| --- | --- | --- |
| P1 | No snapshot bump: `SNAPSHOT_VERSION` stays 21, and `crates/sim-core/tests/snapshot.rs` passes untouched. `WASM_API_VERSION` goes 7 → 8 and `WASM_API_MIN` in `web/app.js` follows; `sim_server::API_VERSION` stays 2. | |
| P2 | The reported number is the one that moved the charge: over one step from rest on a scattered 1S6P equivalent-circuit pack, every cell's `current_a` equals the current reconstructed from its own ΔSOC to within 1e-9 relative. This is `properties.rs`'s existing `parallel_currents_sum_to_group_current` reading the field directly instead of reconstructing it — the two derivations are independent, which is what gives the check teeth. | |
| P3 | The sum is `i_actual` **plus the leakage**, not `i_actual`: on a fault-free pack with no bleed the per-cell currents sum to `i_actual` to rounding; on a pack carrying a soft internal short they sum to `i_actual + V_node·G_shunt`, and the excess is the short's current to within 1e-9. | |
| P4 | `None` means what it says: a freshly built pack reports `None` for every cell; a pack restored from a snapshot reports `None` until its next step; one `dt > 0` step fills every entry; a `dt == 0` probe afterwards leaves the reading unchanged. Perturbation: removing the `dt > 0.0` gate reddens exactly the probe case and nothing else. | |
| P5 | Nothing is paid for it: `Pack::step` still allocates zero times (the buffer is taken and put back, `pack-step-allocations.md`'s harness re-run), and `size_of::<Cell>()` is still 176 bytes. | |
| P6 | Pack equality survives the new field. There is **no test in the tree today** that compares two `Pack` values across a serde round trip — `sim-wasm/tests/engine.rs:438` compares JSON strings, which a `#[serde(skip)]` field cannot move — so `SourceCache`'s claim that a derived `PartialEq` "would make `snapshot != roundtrip(snapshot)`" is currently unguarded. This slice adds that test. Perturbation: deriving `PartialEq` on the new newtype reddens it and only it. | |
| P7 | The page draws the split. On the weak-cell scenario the pack grid under the new metric shows the parallel cells of a group carrying visibly different currents at the same instant, and the carrier diagram's note about not knowing the share is gone. | |
| P8 | `path_claims.rs` stays green: no lesson prose changes, and `web/path-claims.toml` contains no claim about the metric list (checked — three matches for "metric", none of them about the selector). | |

## What was measured

**The engine side, on the test packs in `crates/sim-core/tests/cell_current.rs`** — a
scattered 1S3P equivalent-circuit pack (2.5 Ah cells, 5 % capacity and R0 scatter, seed
fixed), stepped 1 s at 3 A, which is about 1.2 C:

| reading | cell 0 | cell 1 | cell 2 | sum |
| --- | --- | --- | --- | --- |
| after the 1 s step at 3 A (the split the cells were advanced with) | 0.95388 A | 0.97504 A | 1.07108 A | 3.000 A |
| a `dt = 0` probe at 3 A afterwards (the split at the moved state) | 0.95266 A | 0.97429 A | 1.07305 A | 3.000 A |
| relative gap | 1.3e-3 | 7.7e-4 | 1.8e-3 | — |

The first version of the probe test asserted those two rows equal bit for bit, and they
are not, for a reason the plan did not see: a time-advancing step reports the split at the
state it **started** from, because that is the current each cell was actually advanced
with, and then moves the cells. A probe afterwards re-solves at the moved state. The two
are the same physics one second apart, and one second at 1.2 C moves the split by about
a part in a thousand. Both rows are now pinned — equal to the step's own sum, unequal to
each other, within 1e-2 of each other — and the doc on `CellView::current_a` says which
instant the number is about.

The rest of the engine-side measurements are the tests' own assertions, each of which
compares the field against a derivation that does not read it:

* every one of the 128 proptest cases of `parallel_currents_sum_to_group_current`
  (scattered 1S{2..6}P, random current and seed) — the reported current equals the one
  reconstructed from the cell's own ΔSOC to 1e-9 relative, and the reported currents
  sum to `i_actual` to 1e-6;
* `rest_circulates_current_between_mismatched_parallel_cells` — at zero pack current the
  reported currents match the ΔSOC reconstruction to 1e-6 relative, one positive and one
  negative;
* `a_shorted_cell_carries_more_than_the_terminals_take` — with a 2 Ω soft short on one
  cell of a 1S3P group, `Σ I_k = i_actual + i_internal_short_a` to 1e-9, and on the same
  pack shape without the short `Σ I_k = i_actual` to 1e-9;
* `a_deserialized_pack_reports_none_for_exactly_one_step` — `None` on a pack read back
  from bincode bytes, `Some` one step later; and an in-process `Pack::restore(&snapshot())`
  keeps the reading, because it is a clone;
* `a_pack_equals_its_own_serde_round_trip` — a pack that has stepped five times compares
  equal to its own snapshot read back through bytes.

The first sim-core run was 36 binaries, 281 passed, 1 failed — the probe test as first
written, above. The commit gate afterwards — `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast` — ran clean: 76 binaries, 659 passed, 0 failed.

**The page**, driven by `tools/client-perf/carriers.mjs` in headless Chrome against the
shipped catalogue, in-page wasm at API 8:

| case | scenario, controls | driven until | sim t [s] | the drawn cell | its group's two currents | note printed |
| --- | --- | --- | --- | --- | --- | --- |
| parallel split | `cc_cv_charge_pack` (4S2P NMC, 2 % capacity and 3 % R0 scatter), 6 A | 600 s | 604.5 | (0,0), 3.0019 A | 3.0019 / 2.9981 A | "this cell is carrying 50.0% of its group's current" |
| weak cell | the same, with (0,1) replaced through the fault panel at t = 0 by a half-capacity, 1.5× R0 cell; 6 A; (0,1) pinned | 600 s | 603 | (0,1), 2.0454 A | 3.9546 / 2.0454 A | "this cell is carrying 34.1% of its group's current" |
| rest, circulating | the same, 6 A to 900 s and then `Rest` | 1500 s | 1526.5 | (0,0), +0.0049 A | +0.0049 / −0.0049 A (the other three groups ±0.0206, ±0.0143, ±0.0037 A) | "this group's cells are circulating current between themselves — this one at 0.005 A while the group's net is zero" |

Every group's two currents sum to the group current to the digits printed — 6.000 A under
load, 0.000 A at rest — and the strips in the pack band follow the numbers: equal on the
scattered pack, the weak cell's visibly the shorter of its pair, and opposite in sign at
rest. The head line prints the drawn cell's own C-rate against its own capacity, so the
half-capacity cell reads 1.36 C at 2.05 A where its sibling reads 1.33 C at 3.95 A.

The whole driver set — the twelve cases the carrier-diagram slice shipped plus these three —
was run once more against the rebuilt bundle: all fifteen held their conditions, none raised a
banner, and the twelve older cases print the captions they printed before. The single-cell
cases print no share sentence — on a 1S1P pack there is nothing to share — and the four
parallel-pack cases print theirs in front of the caption: 50.4 % on the shorted cell, 50.2 %
while balancing, 50.8 % and 50.1 % on the runaway pair. The contactor-open case now reads as
a circulating group, which is right: with the contactor open the terminals carry nothing and
the mismatched cells of each group trade current between themselves. Server: the `sim-server.exe` built on 2026-09-05, which serves the page and the
catalogue and runs no physics for it.

## Predictions, scored

* **P1 — confirmed.** `crates/sim-core/tests/snapshot.rs` is not in the diff and passes;
  `SNAPSHOT_VERSION` is 21; `WASM_API_VERSION` 7 → 8 and `WASM_API_MIN` with it;
  `sim_server::API_VERSION` is 2 with a paragraph saying why.
* **P2 — confirmed, at the predicted 1e-9 after the shipped test had been written at 1e-6. The tighter number passed all 128 cases on the first try, so the test now asserts the tolerance the prediction named rather than the looser one it was drafted with.** The comparison lives in `properties.rs` as planned, and the
  two derivations are independent: perturbation B (an average instead of the split) and C
  (the magnitude) both redden it.
* **P3 — confirmed.** Both arms, the shorted sum and the clean contrast, hold to 1e-9. The
  short's current is compared against `Telemetry::i_internal_short_a` rather than
  recomputed, so the two accounts of the leakage are checked against each other.
* **P4 — half right, and the half that was wrong was the design, not the test.** A fresh
  pack reports `None`, a pack read back from bytes reports `None` until it steps, one step
  fills every entry: all as predicted. Two things were not. First, "a restored pack" is two
  different packs: an in-process `Pack::restore(&pack.snapshot())` is a `Clone` and keeps
  the reading, and only a restore through serialized bytes comes back cold; the test pins
  both. Second, **the `dt > 0` gate was removed before it shipped.** The family the plan
  put it in — vent latch, sensor clock, aging sub-clock, fault queue — exists to stop an
  observation *mutating state*, and this field is not state; and the browser page samples a
  freshly loaded session with exactly `step(0.0, …)`, so a gated field would have read
  `None` on the one frame the client most needs it. A probe therefore reports the split it
  was probed at, which is what it does for the terminal voltage and the pack current too.
  The predicted perturbation ran in mirror image: *adding* the gate reddens exactly the
  probe test and nothing else (row D).
* **P5 — confirmed for the allocation, and the byte count was stale before this slice
  began.** `a_warm_step_allocates_nothing` is green, and perturbation E shows it has teeth
  on the new buffer: dropping the `Vec` instead of handing it back reddens it. `Cell` is
  untouched — the diff adds no field to it — but the in-tree assertion reads
  `size_of::<Cell>() == 192`, at `HEAD` as well as here. The 176 in the prediction was
  copied from `cell-size.md`, which was right when written and was overtaken by the
  hysteresis state (`155fad1`, v17 → v18) without the record moving; see "Still open".
* **P6 — the test was missing, as predicted, and the perturbation reddened one test more
  than predicted.** Deriving `PartialEq` on `CellCurrents` reddens
  `a_pack_equals_its_own_serde_round_trip` — the new guard — **and**
  `snapshot.rs::zero_length_step_does_not_mutate_state`. The second is the finding: a probe
  writes the split it solved into the buffer, so under a derived equality a probe *is* a
  mutation, and the always-true `PartialEq` is what keeps "a zero-length step mutates
  nothing" true. The impl was load-bearing for two contracts, and the plan had counted one.
* **P7 — confirmed for the drawing, and falsified for the scenario it named.** The note about not knowing the share is gone and the panel prints the share; the pack grid offers `current [A]`. But no shipped scenario carries a weak cell — the weak-cell scenario the plan named is a Rust test, not a catalogue entry — and on the one scattered parallel pack the catalogue ships, `cc_cv_charge_pack`, the split at 600 s is **50.0 / 50.0** to the panel's precision. That is physics, not a defect: the split an R0 mismatch makes is a transient, because the cell taking more current runs down faster until the OCV gap it opens closes the gap in current, and 3 % scatter has decayed to a tenth of a percent by 600 s. A capacity mismatch is different in kind — equal SOC rates need currents in the ratio of the capacities, so the share persists — and a half-capacity cell injected through the page's own fault panel prints **34.1 %** at 600 s (the pure capacity share would be 33.5 %; the 1.5× R0 is holding a little of its transient). That case is now in the driver, and it is the frame the prediction was about.**
* **P8 — confirmed.** `path_claims` is in the 76 binaries above and green; no lesson prose changed.**

## Perturbations

Each edit was applied to `crates/sim-core/src/pack.rs`, five sim-core binaries were run
(`cell_current`, `properties`, `topology`, `step_allocations`, `snapshot`) with
`--no-fail-fast`, and the file was restored byte-exact before the next row. Nothing
compiled red; every row is a runtime catch.

| perturbation | what reddened |
| --- | --- |
| A. `#[derive(PartialEq)]` on `CellCurrents` | `a_pack_equals_its_own_serde_round_trip` **and** `zero_length_step_does_not_mutate_state` — see P6. |
| B. report `i_g / parallel` (an average, not the split) | the shorted-sum test, the probe test (the two averages are equal, so the `assert_ne!` fires), `parallel_currents_sum_to_group_current`, and the circulating-current test. Four independent catches. |
| C. report `i_k.abs()` | the probe test (a charge reads positive), `parallel_currents_sum_to_group_current`, and the circulating-current test (the sink cell reads positive). |
| D. gate the write on `dt > 0.0` | exactly the probe test. The mirror of P4's registered perturbation, with the same shape of result. |
| E. never hand the buffer back (`drop` it at the end of the step) | the deserialized, shorted and probe tests, the properties and topology comparisons, **and** `a_warm_step_allocates_nothing` — the allocation harness sees the fresh `Vec` every step. |

One side effect worth recording: rows B and C made `properties.rs` fail, and proptest wrote
its shrunk counterexample to `crates/sim-core/tests/properties.proptest-regressions`
(`parallel = 2`, a −2.28 A charge, 5.5 % R0 scatter — the smallest pack on which an
average is distinguishable from the split). The file was deleted rather than committed: it
records a failure of a deliberately broken engine, and its header would have told the
next reader it was a real one.

## Deliberately not done

* **No `dt > 0` gate**, for the reasons under P4. A probe reports the split it was probed
  at.
* **No terminal current and no end-of-step re-derivation.** The field is the internal
  branch current the cell was advanced with, and it does not sum to the pack current on a
  shorted or balancing pack; `CLAUDE.md`'s one-line summary of the property now says so.
* **Nothing in `Telemetry`.** The split is per cell and on request through `Pack::cell()`,
  like every other per-cell number; the cheap per-step summary is unchanged.
* **No server route and no `API_VERSION` bump.** `GET /sessions/{id}/cells` gains the key
  because `CellView` is serialized verbatim; nothing else on that boundary moved.
* **No guided-path step.** The lesson that this split *is* the imbalance physics — a weak
  cell taking less of the load and the pack grid's SOC spread being its integral — is a
  path slice, with claims about numbers the page prints rather than about a canvas.
* **The Godot adapter is untouched.** `sim-godot` reads `CellView` through its own
  accessors and none of them was asked for the current; an in-process restore there keeps
  the reading anyway, since it is a clone.

## Still open

* **The reading after a real step is one step behind the state shown beside it.** After
  `step(dt, …)` the page draws `current_a` — the start-of-step split — against the cell's
  end-of-step SOC and voltage. At 1 C and a 1 s step the gap is a part in a thousand (the
  table above); at the coarse steps an aging fast-forward uses it grows with `dt`. A client
  that wants the split at the displayed state can probe with `dt = 0` and read again, which
  the page already does on a fresh session and not after every step. Whether that matters
  is a question for the first lesson that quotes a per-cell current.
* **`cell-size.md` records `Cell` at 176 B and the tree asserts 192.** Not this slice's
  doing — `HEAD` already said 192, and `git log -S` puts the change at `155fad1`, the
  hysteresis state (v17 → v18) — but the plan copied the stale number, and the size record
  has not been updated for the 16 B that state cost. A one-line re-measurement.
* **A restore through JSON draws one stale sample.** The documented price of not bumping:
  the page falls back to the pack-current drawing for the first 250 ms poll after a
  restore. Nothing in the trajectory moves.
* **The pack band's strips normalise within the group**, so two groups with the same
  imbalance look the same whatever their absolute current. The pack grid's `current_a`
  metric is where the absolute numbers are; the band shows the share.
* **The browser run used the server binary built on 2026-09-05.** That is sound for what
  the driver measures — the page runs the engine in-page from the rebuilt wasm bundle, and
  the server only serves the static files and `GET /scenarios` — but the server's own
  `cells` route carrying the new key was checked by its unit tests, not over a socket.
