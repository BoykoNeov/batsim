# The client's redraw — paint on change, fold to the pixel, one cursor across six panels

**Status: built and measured, 2026-09-05.** No Rust in the engine moved; one test grew a
variant; the page's `web/app.js` and `web/index.html` changed; a measuring instrument
landed under `tools/client-perf/`. The section at the foot is a worked plan for what was
deliberately left, written so that a less capable model can execute it slice by slice.

## What this is about

The browser page is where a person *sees* the simulator, so it is where its performance is
felt and where its visuals are judged. Both had a defect nobody had measured:

* **The page redrew six canvases on every animation frame whether or not anything had
  changed.** With a full history (the page caps `history` at 200 000 samples) each redraw
  scanned every sample of every trace twice — once for the y-range and once, strided, to
  draw. A *paused* page with nothing moving held a whole core.
* **The traces were thinned by stride.** Once a run had more samples than pixels the draw
  took every k-th sample. A pulse train at 10 000x has 60 s pulses two samples wide against
  a 200 000-sample history; at a stride of ~160 the plot showed roughly one pulse in three,
  irregularly, and read as a resting pack with the odd glitch.
* **There was no way to read a value off a plot.** The panels are meant to be read against
  each other — the current plot explains the voltage plot, the temperature plot explains
  both — and the only numbers on the page were the readout rows for the newest sample.

## Predictions, registered before the change ran

Written down before the first measurement of the new code, scored below.

| # | prediction | outcome |
| --- | --- | --- |
| P1 | A paused page with a full history drops from ~100 % of the main thread busy to under 10 %, redrawing about four times a second (the cell sampler's 250 ms clock plus the one-second fallback). | **Confirmed.** 100 % → 6.4 %, 44.5 → 4.5 draws/s. |
| P2 | One full `draw()` costs about the same as before: the fold is one pass where the range scan was one pass, and the stride pass it replaces was cheap. | **Wrong, in the good direction.** 19.8 → 12.4 ms. The old draw's *two* passes over every sample were the cost; the stride pass was not cheap, it was `times.length / stride` calls into the canvas path API plus the full-length range scan. |
| P3 | Running at 10 000x stays at ~100 % busy until the redraw is paced, because every frame brings new samples and so every frame repaints. | **Confirmed**, 99.6 % unpaced; then **49.7 %** with the pacing rule (a redraw waits at least twice the last draw's cost). |
| P4 | The pacing does not slow the simulation: at 10 000x the engine is a few percent of the thread and the accumulator hands it the same steps either way. | **Confirmed.** 10 333 → 10 066 simulated seconds per wall second, inside run-to-run noise (three unpaced runs spanned 10 066–10 333). |
| P5 | Every pulse of a 227-pulse train is visible after the change and about one in three before. | **Confirmed by screenshot** (`tools/client-perf/shots.mjs`, `SPEED=4 RUN_MS=16000`, `cc_discharge_lgm50.toml` in `Pulse` mode). The old page's current panel shows ~50 irregular strokes; the new one is a continuous comb. |

## What was built

All in `web/app.js` unless said otherwise.

1. **A redraw flag.** `view.dirty` is set by everything that changes what `draw` shows —
   `record` (new samples), `resetHistory`, `refreshCells` (new cell read, which feeds the
   grid, the BMS panel and two readout rows), any `input`/`change` event on the sidebar
   (the demand notes are re-rendered by `draw`), a window resize, and the cursor. `frame`
   only calls `draw` when the flag is up, with a **one-second fallback** so a path this
   file forgot to mark costs a stale second rather than a stale screen. `draw` clears the
   flag and records its own duration.
2. **Pacing.** A redraw is not taken until at least twice the last draw's cost has passed
   since the last one, so painting never holds more than a third of the thread. At 1x a
   draw is a few milliseconds and the rule never bites; at 10 000x it halves the busy
   share with no visible change to a run moving that fast.
3. **Min/max folding** (`bucketize`). Each trace is folded onto one column per pixel in a
   single pass, keeping each column's lowest and highest sample *and the order they came
   in*, plus a flag for a column holding a `null` (the "no estimate" hole, which still
   breaks the line). The y-range falls out of the same pass. `drawPanel` then emits two
   points per column at most.
4. **A shared cursor.** Pointing at any panel marks the sample nearest that instant on all
   six: a hairline, a ring on each trace where it crosses, and a box with the time (in a
   finer clock than the axis, `fmtTimeAt`) and each trace's reading at two more decimals
   than the axis ticks. The time is read back through the layout the panel recorded when
   it last drew (`layouts`, a `WeakMap` from canvas to `{padL, plotW, x0, x1}`), so there
   is one mapping, not two kept in step. `#plots canvas { cursor: crosshair }` in
   `index.html` says the panels are pointable.
5. **A dot on each trace's newest sample**, so a paused panel shows where "now" is.
6. **Flag chips rebuilt only when the flag string changes** (`renderFlags` used to
   `replaceChildren` every frame).
7. **An instrument handle**, `window.batsim = { state, history, view, draw, invalidate }`,
   read by nothing on the page; it is how `tools/client-perf/measure.mjs` asks how many
   samples the plots hold and what a repaint costs.
8. **The start button's fallback label is checked.** `index.html` says `Start — 32 steps`
   before `app.js` replaces it with one built from `LESSONS.length`; its own comment
   admitted it had shipped wrong (24 against 26) and that nothing governed it. The claims
   test's self-count table (`every_count_these_files_state_about_themselves_is_derived` in
   `crates/sim-data/tests/path_claims.rs`) gained a `Prose::IndexHtml` source and a tally
   for that phrase.

Not touched: `fmtTime`, the readout formatters, `CCCV_*`, `CELLS_PERIOD_MS` — every literal
the claims test pins in `MIRRORED` is where it was.

## What was measured

Headless Chrome 152, 1600×1000, page served by the release `sim-server`, the default
scenario (`soft_short_under_a_lying_sensor`, 4S2P). "Busy" is the share of a five-second
CPU profile not in `(idle)`; `draw_ms` is twenty forced full repaints, averaged.

| state | before | after (unpaced) | after (paced) |
| --- | --- | --- | --- |
| paused, full history — busy | 100 % | 6.4 % | 6.4 % |
| paused — draws per second | 44.5 | 4.5 | 4.5 |
| one full `draw()` at ~176 k samples | 19.8 ms (35.7 ms in an earlier run) | 12.4 ms | 12.2 ms |
| running 1x — busy | 99.9 % | 12.0 % | 10.0 % |
| running 10 000x — busy | 99.9 % | 99.6 % | 49.7 % |
| running 10 000x — top frame | `drawPanel` 77 % | `bucketize` 74 % | `bucketize` 35 % |
| running 10 000x — sim s per wall s | 10 333 | 10 300 | 10 066 |

Read the last two rows together: even at full fast-forward the engine (`wasm-function`,
`step`) is **about 2 % of the main thread**. In the browser the paint is the cost, not the
physics, and `pack-step-perf.md`'s single-digit-microsecond hunt is not what a reader of
this page is waiting on.

Screenshots from the run are not committed (they are build output); the pair worth seeing
is regenerated by the `shots.mjs` line under P5, once against the page at `HEAD~1` and once
against `HEAD`.

## The instrument, and what it taught

`tools/client-perf/measure.mjs` and `shots.mjs`, Node 24 with no dependencies (the
`WebSocket` global talks to the DevTools protocol). Both need a running server and a
headless Chrome you launched yourself on a debugging port. Four things cost time and are
written down so they cost it once:

* **`Performance.getMetrics` was the wrong instrument.** Its `ScriptDuration` reported
  ~870 ms of script per second on the *new* page while a CPU profile of the same page
  showed 91 % idle. The number that tracks the profile is the profile; `busy()` takes one.
* **A DevTools target that is not the foreground one gets no animation frames**, and the
  whole page loop hangs off `requestAnimationFrame`. `PUT /json/new` then
  `/json/activate/<id>` before driving it. (This is the occluded-tab rule from
  `ui-explanatory-path.md`, met from the other side.)
* **The history cap trims to nine tenths.** `record` drops the oldest tenth when the cap is
  reached, so `history.t.length >= 200000` is never observed; wait for 175 000.
* **Chrome spawned from Node under Git Bash exited silently on this machine**; the same
  command line from PowerShell (`Start-Process -PassThru`) stays up and hands back a PID,
  which is also the only handle a shutdown may use (`CLAUDE.md`, never kill by name).

And two about editing the page: `web/app.js` is checked out with CRLF endings under
`core.autocrlf=true` while the index holds LF, so a tool that appends LF leaves the file
mixed — normalise on write. `node --check` accepts the module (it detects top-level
`await`), so a syntax slip is caught before a browser is involved.

## Perturbations

No automated test reads the canvas code; the page's behaviour is asserted by the
instrument above and by eye. What *is* tested:

| perturbation | what reddened |
| --- | --- |
| `index.html` start button label 32 → 31 | `every_count_these_files_state_about_themselves_is_derived` (the new `Prose::IndexHtml` tally); nothing else. |
| any `MIRRORED` literal moved | `mirrored_constants_still_match_the_page`, unchanged from before — none moved. |

## Deliberately not done

* **No incremental fold.** `bucketize` is one pass over every sample per trace per redraw.
  With pacing that is a third of the thread at 10 000x and a few percent at 1x, which is
  the case a reader is in. A hierarchical fold would take it to a few percent at any
  speed; it is the first item of the plan below, not this slice, because the pacing rule
  bought most of the win for eleven lines.
* **No change to how frames cross the wasm boundary.** `step_many` still returns a JSON
  string the page parses. The profile puts `JSON.parse` under 1 % at 10 000x, so it is
  not where the time is; it is the plan's second item because it is where the *next*
  time would be once the fold is cheap.
* **No engine work.** See the measurement: the engine is ~2 % of the browser's thread.
* **No narrow-screen layout, no light theme, no touch or keyboard cursor.** Each is
  scoped below.

## Still open — the plan, for a less capable model

Each item is self-contained: the change, the files, how to know it worked, and what not
to touch. Run the measurement before and after with the same label scheme
(`node tools/client-perf/measure.mjs before-<item>` … `after-<item>`), and keep the
pinned literals in `MIRRORED` (`crates/sim-data/tests/path_claims.rs`) exactly as they
are — `cargo test -p sim-data --test path_claims` is the check that they still are.

### A. A hierarchical min/max fold, so a redraw stops depending on the history length

**Why.** `bucketize` is O(samples) per trace per redraw; at 200 000 samples and nine
traces that is 1.8 million comparisons a paint. A two-level summary makes it O(columns ×
log samples) ≈ 17 000.

**Change.** In `web/app.js`, beside `history`, keep for every numeric column a pyramid:
level 0 is the samples, level k holds `min`, `max`, `minIndex`, `maxIndex`, `hasNull` of
each aligned block of 2^k samples (k up to the block size where a level has under ~64
entries). Maintain it in `record`: push to level 0; whenever a block at level k completes,
fold it into level k+1. When `record` trims the oldest tenth, rebuild the pyramid from
scratch (once per 20 000 samples — amortised nothing). Then `bucketize` maps each column to
its sample index range (`nearestIndex` on the column's two edge times), and answers the
column's min/max/order/null by walking the pyramid over that range — the standard sparse
range-minimum query, largest aligned blocks first. Return the same `{lo, hi, loI, hiI, brk,
min, max}` so `drawPanel` does not change.

**Files.** `web/app.js` only. `history`'s key list is the pyramid's key list; `soc_bms`
carries `null`, which the pyramid must record as `hasNull` and not as a number.

**Acceptance.** `running_max.busy_pct` ≤ 25 % and `draw_ms` ≤ 5 at a full history;
`bucketize` gone from `running_max.top`. The two pulse screenshots must be
indistinguishable from the ones this slice produced — the fold's *answer* is unchanged,
only how it is computed. A property to assert in a scratch harness before wiring it in:
for random `(times, values, x0, x1, nb)`, the pyramid's `bucketize` and the current
one-pass `bucketize` return identical arrays. Keep the one-pass version in the file as
that reference until the property has been run.

**Do not** change the pacing rule to compensate; measure with it as it is.

### B. Frames across the wasm boundary as numbers, not JSON

**Why.** Once A lands, the top of the 10 000x profile will be `JSON.parse` in
`WasmBackend.step` (300 frames of ~20 fields per call, 60 calls a second).

**Change.** In `crates/sim-wasm/src/lib.rs`, add `step_many_flat(dt, n_steps, demand_json,
report_every) -> Vec<f64>` returning `frames × FIELDS` numbers in a documented column
order, with `soc_bms` encoded as `NaN` when absent and `flags` as the bitflags' integer
value cast to `f64` (it fits: the flag set is well under 2^53). Bump `WASM_API_VERSION`
to 7 (the crate's rule: any addition the page can depend on is a bump) and raise
`WASM_API_MIN` in `web/app.js` to 7 with a comment saying what v7 is, in the style of the
v6 comment above it. Keep `step_many` (JSON) — the socket backend and
`examples/experiment.mjs` use the JSON frame, and the page's `SocketBackend` must go on
working unchanged. In `WasmBackend.step`, build the frame objects `record` expects from
the flat array; `record` itself does not change.

**Acceptance.** `cargo test -p sim-wasm` green, including a new test that `step_many_flat`
and `step_many` agree field by field on a short run (this is the host-target `engine`
module, no JS needed). `JSON.parse` gone from the 10 000x top frames. A `Rest` step on a
BMS-less scenario must still draw `soc_bms` as a broken line, not as a line at zero —
that is the `NaN` → `null` conversion, and the SOC panel is where it shows.

**Do not** touch `sim-core`; the flattening is an adapter concern.

### C. A layout below 760 px

**Why.** `body` is a fixed 320 px sidebar beside the main column; on a phone the plots are
100 px wide.

**Change.** In `web/index.html`, a `@media (max-width: 760px)` block: `body` becomes one
column, `#sidebar` a top block with its own `max-height: 45vh` and vertical scroll, and
`#plots` one column at 220 px (the 1250 px breakpoint already makes it one column; this
tunes the height). The `#pack-grid` tile width (84 px, set in `buildGrid`) is fine.

**Acceptance.** `shots.mjs` with the emulation width edited to 400 px: no horizontal
scrollbar on `body`, every plot title clear of its legend (the 1250 px comment in
`index.html` explains the collision to avoid and how to measure it). The claims test does
not read CSS, so it cannot redden; the page-level check is by eye.

### D. A light theme

**Why.** The page is dark only; `index.html` has the palette as CSS variables but the
canvas colours are string constants in `app.js` (`PLOT_BG`, `PLOT_GRID`, `PLOT_INK`, the
per-trace colours, and the cursor box's `rgba`).

**Change.** Read the canvas palette from `getComputedStyle(document.documentElement)` once
at boot and again on a `matchMedia("(prefers-color-scheme: light)")` change, into a `PALETTE`
object `drawPanel` reads instead of the constants. Add the light values under
`@media (prefers-color-scheme: light) { :root { … } }` in `index.html`, keeping the two
accent roles (truth = `--accent`, belief = `--warn`) — the BMS panel's legend depends on
that pairing. Call `invalidate()` on the media-query change.

**Acceptance.** Both schemes screenshotted (`Emulation.setEmulatedMedia` with
`prefersColorScheme`) with every trace distinguishable on both. No number in any lesson
moves, so the claims test is unaffected.

### E. Keyboard and touch for the cursor

**Change.** `pointermove` instead of `mousemove` (covers touch); arrow keys on a focused
canvas (`tabindex="0"`) step `view.cursor` to the neighbouring sample by `nearestIndex`
± 1; `Escape` clears it. Small, and it is what makes the cursor usable in a lesson on a
tablet.

**Acceptance.** By eye in `shots.mjs` with `Input.dispatchKeyEvent`. Do not let the key
handler run while `#sidebar` has focus.

### F. What the cursor prints, tied to what the readouts print

**Why.** The cursor box shows a trace at `axis decimals + 2`; the readout rows have their
own formatters, and the claims test pins *those*. A reader comparing the two can see
`3.2841 V` beside `3.284 V`.

**Change.** Give each trace an optional `fmt` (the readout row's formatter for that
quantity where one exists) and use it in the cursor box; fall back to the current rule
otherwise. The formatters live in `READOUTS`; do not copy their bodies, reference them.

**Acceptance.** `mirrored_constants_still_match_the_page` stays green (nothing in
`MIRRORED` is edited); cursor and readout agree to the digit at the newest sample.

### G. Engine performance — not from here

The browser measurement says the engine is ~2 % of the page's thread at 10 000x. Anything
about `Pack::step` belongs to `pack-step-perf.md` and its protocol ("Measuring a change on
this machine"); the client cannot see single-digit microseconds and should not be used to
claim them. `docs/ROADMAP.md` H9 is the open item.
