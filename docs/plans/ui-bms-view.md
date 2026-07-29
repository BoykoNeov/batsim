# UI / pedagogy — truth beside belief

The slice `docs/plans/ui-pedagogy.md` named and priced at its end. Still not a numbered
phase: Phases 0–6 built an engine, this is the client catching up to it.

## What is being exposed and why it was missing

`Bms::sensors() -> &SensorFrame` has been public in `sim-core` since Phase 2 and **no
adapter exposes it**. `SensorFrame` is the only thing the BMS is allowed to see —
principle 8 in `CLAUDE.md` says ground truth ≠ BMS view, and the gap between them is "a
feature to expose, not a bug to hide". Until now the page could expose exactly one
scalar of that gap: `soc_bms` against `soc_true`.

This slice exposes the frame on both client transports and draws the gap channel by
channel.

`SNAPSHOT_VERSION` does **not** move (10): no saved pack changes shape, and every
accessor this needs — `Pack::bms`, `Bms::sensors`, `Bms::config`, `Bms::soc_estimate`,
`Bms::contactor_open`, `BmsConfig::temp_probes` — was already public.

## The version pair was mispriced, and the constants' own text is why

Both the previous plan and the memory of it say this slice costs `API_VERSION` **and**
`WASM_API_VERSION` 2 → 3, "bumped together". That price was set without reading either
constant's doc comment, and the two do not say the same thing:

- `sim_server::API_VERSION` states a bump **rule**, with an explicit additive exemption:
  *"Bumped when a client would break: a renamed route, a renamed `ErrorCode`, or a renamed
  field on one of the engine types that crosses the wire. Adding a field or an error code
  does not bump it."* A new route is an addition and breaks nobody, so it **stays at 2**.
- `sim_wasm::WASM_API_VERSION` states the contract's **scope** — *"method names, and the
  JSON field names of the engine types that cross this boundary"* — with no exemption. A
  new method changes that set, so it goes **2 → 3**.

The asymmetry is textual rather than a judgement call, but it also happens to match the
one real difference between the two adapters: **`web/pkg` is a build artifact loaded
separately from the JS that calls it**, gitignored, and rebuilt by hand after any Rust
change. A page can be newer than the wasm it loads; it cannot be newer than the server it
fetched itself from. So the wasm number has a job here — `app.js` compares it against a
minimum and names the rebuild command — and the server's would have had none.

**A bump with nothing reading it is decoration.** The check in `app.js` is what makes
this one load-bearing, and without it the failure mode stays what it has always been:
`TypeError: this.sim.sensors is not a function` from a stale bundle. Feature detection
(`typeof sim.sensors === "function"`) would answer "is it there", and the version answers
the more useful question — "is the bundle stale, and what do I run".

This is the first time the two constants have parted. Both doc comments say so.

## The panel's order is the physics, and it is not the order that was drafted first

The obvious layout leads with per-group voltage bars against the true envelope. That
panel is **degenerate by construction** and would have shipped looking broken.
`SensorFrame`'s own doc says it, and `pack.rs` proves it: the true `v_g` computed at
end-of-step is *moved into* `bms.sample()`. Voltages and probe temperatures are **exact**
reads. The always-on error is elsewhere. So the channels are ordered by which of them
actually diverges:

1. **Current** — sensed `i_pack_a` vs `Telemetry::i_actual`. Wrong on every step, by
   `current_offset_a` plus a noise draw. This is the root cause, so it goes first.
2. **SOC** — `soc_bms` vs `soc_true`. Drifts *because* of (1): the estimator coulomb-counts
   the lying sensor. The panel draws the causal arrow rather than leaving two numbers
   side by side. On LFP the flat mid-range blocks the OCV correction that would heal it,
   which is `min_ocv_slope_v_per_soc` doing its designed job.
3. **Temperature** — `max_probe_k()` vs `Telemetry::t_max`. The probes read exactly, but
   only where they are. Once the thermal network builds a gradient the hottest
   *instrumented* cell is not the hottest cell, and the error is **spatial** — so the
   probes are also ringed on the existing per-cell grid, where the gap is visible as a
   position rather than a number.
4. **Group voltage** — last, and labelled for what it is: *exact until a sensor fault
   lies about it*. It stays in because it is the only place `SensorOffset`/`SensorStuck`
   on `GroupVoltage(k)` becomes visible, and last slice's fault panel can already inject
   one. A channel that reads "exact by construction" is itself the teaching point: the
   physics does not lie to the BMS here, broken hardware does.

## Staleness is part of the contract, not a defect to paper over

`pack.rs` gates sampling on `sensor_tick`, which is exactly `dt > 0.0`. There is no
configured sensor period, so on a running pack the frame is one step old at most. But
`readNow()` steps with `dt = 0`, so **a paused pack's frame does not resample** — the
same zero-length-read contract that means a probe step fires no queued fault.

The panel therefore renders `sampled_at_s` always, and marks the frame stale when it
lags `sim_time_s`. Last slice's record says a stale read looks exactly like a feature
that failed; naming the lag on screen is the cheapest possible defence against reading
this panel's own honesty as a bug.

## Payload shape

One shape, both transports — `sim-wasm`'s module doc commits to "one engine should not
have two dialects", and `Cells`/`CellsResponse` already set the precedent of two
field-identical structs (the server does not depend on `sim-wasm`).

```json
{ "v_group": [3.31, 3.32, ...], "temp_probe_k": [301.4, ...],
  "temp_probe_at": [[0,0], [2,1]], "i_pack_a": -4.98,
  "sampled_at_s": 120.0, "soc_est": 0.713, "contactor_open": false }
```

`null` for a pack with no BMS — a pack with no BMS has no sensors, which is a supported
mode (principle 7), not an error.

Three shape decisions worth the words:

- **`temp_probe_at` rides in the dynamic payload though it is static config.** It belongs
  in `PackFacts` by rights, but `PackFacts` is `#[derive(Copy)]` on both transports and a
  `Vec` would end that for a field read four times a second next to the values it labels.
- **No `series` field**, unlike `Cells`. `v_group.len()` *is* the series count and cannot
  disagree with itself; a restated one can go stale. `Cells` carries `parallel` because
  its consumer does index arithmetic — this payload's consumer does not.
- **No non-finite guard.** `pack.rs` uses `f64::NAN` as the probe-read fallback, and JSON
  cannot spell NaN (Phase 4 slice C). But `validate_bms` rejects any probe outside the
  topology at construction, so the fallback is unreachable, and `cells_of` guards nothing
  for the same reason. An unreachable guard would imply the range check is not trusted.

## Deliberately not in this slice

- **No `sim-godot` exposure.** A third adapter surface with no client asking for it is
  precisely the anti-pattern last slice's finding was written to correct — six entry
  points built ahead of their client and never called. When a Godot scene wants the BMS
  view, that is when it gets one.
- **No per-group *truth* array.** It would make the voltage channel non-degenerate on a
  healthy pack, and it is not free: the true `v_g` is a solve output, not stored state,
  so exposing it costs either a per-step `Vec` in `Telemetry` (against a step budget
  already marginal) or a new field in the snapshot. The true min–max envelope from
  `Telemetry` bounds the sensed bars, which is what makes a lying sensor visible, and
  that is the whole job.
- **No sensor-frame history.** The panel samples the present, on the same 4 Hz clock as
  the cell grid.

## The evidence this slice owes, and the two paths it takes

Last slice exercised `SensorStuck` only against a BMS-less pack, where the engine
*refused* it — so the `GroupVoltage(k)` fault path has never actually landed. This
slice's voltage channel is unverifiable without it, and the two things verify each other:
the bar must leave the true envelope while the band stays exactly where it was.

There are **two** code paths to that fault and they must both be walked, because they
share nothing but the engine's `Fault` type:

1. **Scenario TOML → `build_pack` → fault queue.** Already authored:
   `scenarios/soft_short_under_a_lying_sensor.toml` schedules
   `SensorOffset { sensor = { GroupVoltage = 1 }, offset = 0.12 }` at `t = 600 s`,
   alongside the short it exists to hide. Its own header says
   *"Telemetry::soc_bms against Telemetry::soc_true is where to watch it"* — because when
   it was written, the scalar pair was the only place it *could* be watched. This slice
   is that scenario finally getting the instrument it was authored for.
2. **`Pack::schedule_fault` from the live fault panel.** A different path, and the one
   the record says has never landed. Cheap now that the panel exists.

Running only the scenario would demonstrate the panel while leaving the API path exactly
as unverified as the last slice left it.

## The bug the panel found in itself, on its first paint

On a freshly loaded scenario **every group was tagged "outside truth"** — four
simultaneous accusations on a pack where nothing was wrong.

The cause is not a comparison bug, it is a category error about *when* a frame exists.
A pack that has never advanced has never sampled: it carries the construction-time
open-circuit read, every group at `OCV(initial_soc)` and the current sensor an exact
zero it has not earned. Meanwhile `readNow()` reports telemetry under the page's
standing demand — 3.3132 V sensed against a true envelope of 3.292–3.293 V, the
difference being simply `I·R`. Both are labelled `t = 0`, and **both labels are correct**,
so no timestamp check can catch it: the clocks agree and the instants do not.

The panel now refuses to compare until the pack has stepped at least once, and says
which of the two states it is in ("boot read — the sensors have not sampled yet"). The
general form is worth keeping: **a staleness check on timestamps cannot see a frame that
is stale in state but current in time.** The `dt = 0` probe read is exactly the operation
that produces one.

## What was verified, and how

| claim | evidence |
| --- | --- |
| the scenario fault path | ran `soft_short_under_a_lying_sensor` past t = 600 s in the page: g1's dot left the band to 3.3748 V and was tagged, g0/g2/g3 stayed inside at ~3.265 V, and the band did not move |
| the live API path | injected `SensorOffset { GroupVoltage(2), −0.25 }` from last slice's panel: g2 read 3.0126 V, *below* the band, tagged — two liars in opposite directions at once, one from a file and one from the API. This is the path the record said had never landed |
| both in Rust, not only on screen | `a_lying_group_sensor_reads_outside_the_true_envelope` walks both paths and asserts the envelope claim directly; perturbed (fault moved to another group) and it fails as designed |
| the always-on channels | current +0.028 A against a truth of 2.000, SOC +4.26 pt, and probes reading 1.01 K *below* the true `t_max` because neither sits on the cell that shorted |
| both transports | the identical panel over the in-tab wasm engine and over the server's REST + WebSocket session |
| the stale-bundle banner | set `WASM_API_MIN` to 4, reloaded, and read the banner naming the rebuild command — which is how the **second** bug below was found |
| console | clean; nothing from the page |

**The stale-bundle check was decoration when first written.** It called `showBanner`
and carried on, and boot continues into an automatic scenario load, which calls
`clearBanner` — the warning was erased about 200 ms after it appeared, and the first
perturbation showed no banner at all. It now throws, like the missing-bundle path does
and for the same reason, and it lives *outside* that path's `try` so its message is not
relabelled as a bundle that would not load. A version check nothing acts on would have
made the whole `WASM_API_VERSION` bump ceremony.

## Colour, checked rather than judged

The two entities already have colours: the SOC plot has drawn truth in `--accent` and
the BMS estimate in `--warn` since Phase 4. Colour follows the entity, so the panel
reuses them rather than picking its own — a reader who has learnt "amber is what the BMS
thinks" on one panel keeps it on the next.

Run through the `dataviz` validator against this page's dark surface, that pair returns
**PASS** on CVD separation (ΔE 21.7 protan, 24.7 tritan), the normal-vision floor (26.2),
the chroma floor and contrast — and **FAIL** on the lightness band (0.789 and 0.824
against a dark-mode band of ≈0.48–0.67). Both colours are brighter than the band wants.
That is a pre-existing property of the page's whole chart palette, not something this
panel introduces, and fixing it means re-stepping every trace colour and the grid ramp —
a page-wide restyle, and a different slice. Recorded rather than quietly inherited.

One form decision came from the same source: the group channel is a **dot plot, not
bars**. The interesting window is tens of millivolts on a 3.3 V cell, so the axis cannot
start at zero, and a bar on a truncated axis is the textbook way to draw a difference
that is not there. A dot carries a position and claims nothing about an origin it never
touches.

## Gate

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all`. Unlike last slice this one *can* move the Rust tests: the new route
gets a test beside the existing route tests. The out-of-tree trajectory instrument still
cannot move — it builds its nine cases from scenario TOML through `parse_scenario` /
`build_pack`, and no physics changes here.
