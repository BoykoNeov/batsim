# The scenario catalogue, and the two scenarios nobody could load

The first two items of the queue `docs/plans/ui-explanatory-path.md` left open, taken
together because the first one on its own exposes nothing: a listing route over two
files is a route with nothing to list.

Not a numbered phase. Phases 0–6 built an engine; this is the client catching up to it,
the same footing as the three `ui-*.md` slices before it.

## What is missing, stated as source

`web/index.html` hardcodes the scenario picker:

```html
<select id="scenario">
  <option value="cc_discharge_lfp.toml">…</option>
  <option value="soft_short_under_a_lying_sensor.toml" selected>…</option>
</select>
```

and `routes.rs` serves `scenarios/` as a `ServeDir`, which answers a file by name and
cannot answer *what files exist*. So every scenario costs an HTML edit, and the repo has
two. `app.js` already names this as a disease and says where the cure goes — the guided
path was built as records rather than markup "because the `<option>` list is hardcoded",
and its comment ends *"when the scenario-listing route lands it serves this same shape."*
This slice lands it.

The second half is the reason it matters. **Two of the three shipped chemistry files are
reachable from no client at all**: `nmc_18650_generic` and `nmc_21700_lgm50` are parsed
by `sim-data` tests and by nothing a reader can press. And the only `[pack.aging]` in the
repo — in `soft_short_under_a_lying_sensor.toml` — sets a sub-clock and is never the
subject of anything; aging shipped in Phase 3 and has never been *shown*.

## The measurement that came before the design

Recorded because the last slice made three claims about what a reader would see and all
three were wrong, and because the one before it paid a version bump on an assumption.
Everything below is from a throwaway harness against `sim-core` (outside the repo, not
committed), reading coefficients from `chemistries/*.toml` rather than from `CLAUDE.md`'s
illustrative block — that block is a schema, not a source, and it has already cost this
repo two wrong numbers.

**Calendar fade is not merely visible, it is fast.** LFP at 100 % SOC, thermally coupled,
`sub_clock_period_s = 10`:

| sim time | 25 °C | 45 °C |
|---|---|---|
| 10 ks | 99.76 % | 99.14 % |
| 40 ks | 99.51 % | 98.27 % |
| 160 ks | 99.03 % | 96.54 % |
| 600 ks | 98.11 % | 93.30 % |

(That table is the probe's own 100 % pack. The scenario this slice ships rests at 95 %
for the reason in Part B, which costs it a little: 1.83 points at 25 °C and 6.51 at 45 °C
over 600 ks, re-measured against the committed file.)

The speed slider tops out at 10⁴×, so 600 ks is **one minute of watching**. The `√t` law
is exact in the trace — each doubling of elapsed time multiplies accumulated fade by
√2 (0.87, 1.22, 1.73, 2.45, 3.46, 4.90, 6.92 points at 45 °C) — and resistance tracks it
at exactly `1 + 1.5 × capacity_loss`, the chemistry's `r_growth_per_capacity_loss`. The
+20 K ambient step is worth **3.5× the fade**, and 100 % SOC against 50 % is worth 1.4×,
which is `cal_soc_stress` read straight back out.

Two things this measurement *killed*:

- **"NMC ages faster than LFP" is false here** and must not be written anywhere. At
  600 ks the three chemistries sit at 98.11 / 97.81 / 97.81 % (25 °C) and 93.30 / 93.15 /
  93.15 % (45 °C). The two NMC files are identical in fade by construction — the 21700's
  `[aging]` comment says it carried the coefficients over unchanged — and LFP's higher
  activation energy nearly cancels its higher pre-exponential. **The aging lesson is
  about temperature and charge level within one chemistry, not about chemistry.**
- **These coefficients are labelled placeholders in all three files**, and 12 % loss in
  23 days at 45 °C is aggressive. Every number the lesson quotes must therefore be
  quoted as this parameter set's behaviour, not as a claim about real cells. Raising or
  lowering a coefficient to make the demo read better is out of the question: it is a
  silent physics lie in a teaching tool, and the provenance rule governs a change like
  that as much as a new constant.

**The chemistry comparison rests on the discharge curve, and it is a strong contrast.**
1S1P, 0.5 C, 25 °C, isothermal, no BMS, terminal voltage from 90 % to 20 % SOC:

| chemistry | 90 % | 50 % | 20 % | span |
|---|---|---|---|---|
| `lfp_26650_generic` (2.303 Ah) | 3.279 | 3.230 | 3.111 | **168 mV** |
| `nmc_18650_generic` (3.0 Ah) | 4.064 | 3.765 | 3.583 | **481 mV** |
| `nmc_21700_lgm50` (5.153 Ah) | 3.984 | 3.641 | 3.366 | **618 mV** |

That is the plateau, in one number, and it is the missing first half of a lesson the
guided path already teaches the second half of: step 3 shows the LFP estimator declining
to correct itself because `min_ocv_slope_v_per_soc = 0.15` is not met. Until now a reader
had to take the flatness on trust.

**An SPM scenario is deferred, with its numbers.** `nmc_21700_lgm50` is the one chemistry
carrying an `[spm]` section, and Phase 6's cell model is reachable from no client — the
same "built ahead of its client" shape the UI slices exist to repair. It is affordable:
0.80 µs/step at 12 shells against 0.12 µs/step for the same cell's ECM (1S1P, native
release), not the 26× the Phase 6 note records for a larger pack. But at 0.5 C the two
models agree to ±20 mV through the middle of the discharge and only part near empty
(−119 mV at 3 % SOC), so a scenario that just selects `Spm` shows a reader almost nothing.
The comparison that would pay — high C-rate diffusion limitation, and relaxation after a
pulse — is a slice with its own design, not a TOML file. **Named, priced, and not taken
here.**

## Part A — the listing route

`GET /scenarios` returns a JSON array, one entry per `*.toml` in the served scenario
directory.

**Route shape mirrors `/app`, which is already in this file for exactly this reason.**
`routes.rs:46-52` explains that `nest_service` at a bare path swallows it. So:

```rust
.route("/scenarios", get(list_scenarios))
.nest_service("/scenarios/", ServeDir::new(&dirs.scenarios))
```

No redirect on the bare path, unlike `/app` — it answers JSON, so there is no relative
URL underneath it to resolve against the wrong base. `GET /scenarios/<file>.toml` must
keep serving text after the nest moves (the prefix that gets stripped changes), and
`GET /scenarios/` with no file gets whatever it gets — pinned by a test so it is
documented rather than discovered later.

Each entry carries what a picker and a lesson need, and nothing that would make this a
second copy of the scenario format:

| field | source |
|---|---|
| `file` | directory entry, e.g. `"cc_discharge_lfp.toml"` |
| `name`, `description` | `[meta]` |
| `chemistry` | the id, or `"inline"` for a `chemistry_toml` scenario |
| `series`, `parallel`, `initial_soc`, `initial_temp_k` | `[pack]` |
| `cell_model` | `"Ecm"` / `"Spm"` — so the deferred slice above needs no route change |
| `bms`, `aging` | `bool`: is the section present |
| `thermal` | `"Isothermal"` / `"Network"` |
| `faults` | count of `[[faults]]` entries |

Two rules that are not decoration:

- **Sort by file name.** `fs::read_dir` order is not specified, and this repo does not
  ship order that depends on the machine (`CLAUDE.md`'s determinism section bans the
  same thing one layer down).
- **A malformed scenario appears as an entry carrying its parse error**, rather than
  being filtered out. A file that vanishes from a listing because it is broken is the
  worst possible report: the author sees a picker that simply does not mention their
  scenario. Same family as the erased banner and the note that only rendered in the
  paused branch — the diagnostic exists, and the path through it that matters never runs.
  A directory that cannot be read at all is a 500 in this server's error shape.

`api_root` gains a pointer to the route beside the `scenario_dir` it already reports, per
that handler's stated habit of answering in-band to whoever is holding curl.

**No version constant moves.** `sim_server::API_VERSION`'s own doc states the rule and
its exemption — *"Bumped when a client would break: a renamed route, a renamed
`ErrorCode`, or a renamed field… Adding a field or an error code does not bump it"* — and
a new route is an addition. `WASM_API_VERSION` is untouched because `sim-wasm` gains
nothing: the page fetches this route from the server on both transports. `SNAPSHOT_VERSION`
is untouched because no engine state changes. Each checked against its own doc comment,
which is the correction this repo already paid for once.

## Part B — the scenarios

Three files under `scenarios/`, each with the header comment the two existing ones set
the standard for.

1. **`cc_discharge_nmc.toml`** — 1S1P `nmc_18650_generic`, otherwise character-for-character
   the conditions of `cc_discharge_lfp.toml`: 100 % SOC, 298.15 K, isothermal, no BMS, no
   aging, no faults. **The point is that it differs in exactly one field**, so the curve
   difference cannot be anything but the chemistry. It also exercises the 2-RC path,
   which that file's own header advertises and no scenario used.
2. **`cc_discharge_lgm50.toml`** — the same again for `nmc_21700_lgm50`, whose header must
   say which half of that file it reads: `[ocv]` and `capacity_ah` are fitted from
   Chen2020 and are what this scenario shows; `[r0]`/`[[rc]]` are labelled placeholders;
   the `[spm]` section is not touched by an `Ecm` pack and awaits the slice named above.
   The file itself says the two halves have different provenance and that "this comment is
   where that is written down" — a scenario against it inherits the obligation.
3. **`calendar_fade_hot.toml`** — 4S2P LFP, 95 % SOC, `[pack.scatter]`, `[pack.thermal.Network]`,
   `[pack.aging]`, **no BMS**, no faults, starting at 298.15 K.
   - Thermal coupling is load-bearing rather than decorative: without it the pack is
     isothermal and the ambient slider drives nothing, which is the whole interaction.
     Cells reach ambient with a ~271 s time constant (95 J/K over 0.35 W/K), so a few
     seconds of watching at speed.
   - No BMS because nothing here needs protecting — a resting pack trips nothing — and
     because `soc_bms` beside a fade curve is a second lesson competing with this one.
   - Scatter because a pack is never eight identical cells — **not** because it shows at
     rest, which was the reason drafted first and is wrong. Measured after the files were
     written: calendar fade depends on temperature and SOC, both uniform in a resting
     pack, so the eight health tiles end up agreeing to three parts per million. The
     scenario header says so rather than implying a spread that is not there.
   - **95 % SOC, not 100 %, and that came out of running it.** At exactly 1.0 this pack
     does not rest: its parallel twins develop equal and opposite millivolt overpotentials,
     drift apart in SOC, and hold the pack 0.44 K above ambient with nothing connected.
     Cells sit on the SOC clamp, and the LFP OCV table climbs 180 mV between 98 % and
     100 %, so a rounding-sized charge difference becomes a millivolt of open-circuit
     voltage and the group solve does exactly what it should with it. Correct, and it
     would read to a student as self-discharge, which this engine does not model. At 0.95
     every cell holds `0.950000` and sits at exactly ambient. The stress factor is 1.36
     against 1.40 — not a difference worth defending.

A test walks `scenarios/*.toml` and `parse_scenario`s each. That test is what makes the
listing route trustworthy and it does not exist today; `crates/sim-data/tests/load.rs` is
where the chemistry equivalent lives and gets checked first so this is not a duplicate.

## Part C — the client

**The picker is populated from the route.** On load, fetch `/scenarios`, build the
`<option>` list from it (label: file stem, topology, and the short "what is on" summary
the two hand-written labels already carry), and keep the current selection if it survives.
**If the fetch fails, the hardcoded options stay and the failure is on screen.** A new
page against an old server otherwise gets a silently empty picker — the same shape as the
version check that only called `showBanner` and was erased by the next `clearBanner`.

**A sixth plot panel, `plot-soh`.** The fade is currently a two-decimal number in the
readout row and a grid ramp; neither is a curve, and "the fade curve" is the thing to be
shown. One panel, unit `%`, two traces on one axis:

- `soh_capacity × 100` — falls from 100
- `soh_resistance × 100` — rises from 100

Same units by construction ("% of new"), which is what lets them share an axis, and their
divergence *is* `r_growth_per_capacity_loss`: 1.5 points of resistance for every point of
capacity, readable straight off the panel. With aging off the panel is two flat lines at
100 %; `drawPanel` already handles a degenerate y-range (it pads by `max(|y1|·0.05, 0.5)`),
so that renders as a flat pair on a 95–105 axis rather than as a broken axis.

**Two lesson records**, appended to `LESSONS` — array entries, no new markup, which is the
design that comment predicted:

- *The plateau, and the price of it* — loads `cc_discharge_lfp.toml`, then the NMC one at
  a matched C-rate, quoting the spans above. Known limitation, stated rather than hidden:
  **loading a scenario resets the history**, so the two curves cannot be seen at once and
  the step compares by number instead. A ghost trace of the previous scenario is the
  obvious fix and is not in this slice.
- *Nothing is happening, and it is still wearing out* — loads `calendar_fade_hot.toml`,
  rests it at 10⁴×, watches `plot-soh` bend, then asks the reader to push ambient to 45 °C
  and watch the slope steepen. The `√t` shape and the temperature factor are the two
  claims, and both are measured rather than reasoned. **Re-measured against the file as
  committed**, which moved them: 1.83 points lost at 25 °C and 6.51 at 45 °C over 600 ks,
  so **3.6×**, not the 3.5× the first probe gave for a different initial condition.

## Verification, and the traps this page has already sprung

- **Every new `<option>` gets a real click**, not a synthesised `loadScenario()`. The
  last slice shipped a wrong note precisely because every transition in verification came
  from its own code and the button a reader presses was never pressed.
- **rAF does not fire at all under an occluded automation window.** One screenshot buys
  one frame; never `await requestAnimationFrame` in an injected script; a timed-out CDP
  evaluate keeps running in the page, so reload between probes.
- **Anything read at a high speed multiplier is re-read at rest before it becomes prose.**
  `refreshCells` self-throttles, and that is what turned a correct piece of reasoning into
  a false observation last time.
- Both transports. The socket path builds the pack server-side, so the aging scenario runs
  there unchanged; the picker is served by the same server either way.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all` before the commit, per `CLAUDE.md`.

## What the build changed

Written after the fact, as the last three plans were.

**Three claims in this document did not survive being run**, all of them about the aging
scenario, and all three were corrected in the files rather than left standing:

1. **100 % SOC was wrong as an initial condition.** See Part B — the pack does not rest
   there, and the reason is the SOC clamp meeting the steepest two percent of the LFP OCV
   table. 95 % instead.
2. **Scatter shows nothing at rest.** Drafted as "so the SOH tiles fan out"; measured, the
   eight tiles agree to three parts per million.
3. **The temperature factor a reader sees is 2.7×, not 3.6×.** This is the one worth
   keeping. 3.6× is the ratio between two packs aged *from new* at the two temperatures.
   The lesson raises the ambient **part-way through a run**, and a second leg of equal
   length then costs 2.84 points against the first leg's 1.06. `√t` means an already-aged
   pack pays less for the same stress. Writing 3.6× would have put a number on screen that
   a reader measuring for themselves would have found wrong — and it would have hidden the
   more interesting fact.

**The picker's failure notice moved out of the banner before it was ever written.**
`showBanner` at boot is erased by `loadScenario`'s own `clearBanner` a few lines later —
the identical mechanism that erased the stale-bundle warning in `ui-bms-view.md`. It lives
in the scenario note instead, which is rewritten on every load and therefore survives.

**Verification, and what was and was not driven by hand.** All five scenarios were loaded
through the picker by **real keystrokes** on a focused `<select>`, and the guided path was
walked from step 1 to step 8 by **real clicks** on Next — the control whose handler went
unexercised last slice. `plot-soh` was watched drawing the two curves live.

The degraded path was **staged rather than reasoned**: a worktree at the previous commit,
built and run on port 8081 with `--web-dir` pointed at the current page, giving exactly
"new page, old server" — `GET /scenarios` 404, `GET /scenarios/<file>.toml` 200. The page
kept its built-in list (visible in the label, which still read the stale `4S2P, BMS,
faults` where the served listing says `BMS, aging, thermal, 2 faults`), carried the reason
in the note, and loaded scenarios normally.

Two things were **not** driven on screen, stated rather than glossed: the full 200 ks
aging run (rAF does not fire under an occluded window, and one screenshot buys one frame —
25 ks was reached, and the trajectory beyond it is measured natively against the committed
file), and one final `change` event on the old server, where arrow keys stopped reaching
the focused select. Every number in the two new lesson steps comes from a native run of
the shipped scenario files, not from the page.

## Exit criterion

A reader who has never edited a file can load every chemistry in the repo from the
picker, see why LFP's charge state is hard to measure, and watch a pack lose capacity by
sitting still — and adding the next scenario costs one TOML file and no HTML.
