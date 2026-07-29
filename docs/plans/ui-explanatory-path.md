# UI / pedagogy — the explanatory path

Third client slice, after `docs/plans/ui-pedagogy.md` (the pack grid) and
`docs/plans/ui-bms-view.md` (truth beside belief). Still not a numbered phase: Phases 0–6
built an engine, and this is the client catching up to the *first* of `CLAUDE.md`'s two
stated purposes — "let users experiment … and **see** what happens".

## What is missing, stated precisely

The page is a complete instrument panel and an empty lesson. Every control `CLAUDE.md`'s
pedagogy sentence asks for exists — demand, environment, BMS on/off, faults, per-cell
grid, sensor frame, snapshots — and nothing anywhere tells a reader **which knob to turn
or what to look at afterwards**. The panels carry excellent explanatory prose about
*themselves* (`index.html:332`, `364`, `385` are three of the best paragraphs in the
repo), but a reader who does not already know battery physics has no entry point: they
land on a paused 4S2P pack with a scheduled fault and no reason to press anything.

That gap is not a missing panel. It is missing **content**, and this slice is about the
cheapest possible way to carry content.

## The form: a lesson is a record, not a screen

The single decision this plan exists to make. Each lesson step is data:

```js
{ id, title, scenario, transport, demand: {mode, value}, ambient_c, bms,
  until_s, watch, prose, expect }
```

A `LESSONS` array holds them; one renderer walks any record. **No lesson gets its own DOM
or its own handler.**

The argument is not tidiness — it is the diagnosis this whole slice sits downstream of.
`docs/plans/ui-pedagogy.md` already recorded why there are only two scenarios: the
`<option>` list is hardcoded, so *every added scenario costs an HTML edit*. Writing
bespoke markup per lesson reproduces that disease one layer up, and reproduces it in the
part of the page most likely to grow. As records, three things follow for free:

- A new lesson is an array entry, reviewable as a diff of prose.
- When the scenario-listing route lands (still the next slice by the ordering in
  `[[ui-bms-view]]`'s "still open"), it serves this same shape, and the lessons that are
  blocked today become entries rather than a rewrite.
- The blocked lessons below can be *written now and left commented*, because a record is
  legible before it is runnable.

**Linear, with Next/Back, not a menu.** "Try this, watch that" is a sequence — lesson 3
assumes the reader has seen the grid respond in lesson 2. A menu is a superset and can be
added later over the same array; committing to linear now costs nothing and is recorded
here as the open question the user may reverse.

## The five lessons that are buildable today

All five run on the two scenarios already in `scenarios/`, with zero engine, `sim-data`,
or `sim-server` work. The "new machinery" column is what each one costs *beyond the
renderer*, and the answer is the point of the form decision above.

| # | lesson | scenario | what it teaches | new machinery |
| --- | --- | --- | --- | --- |
| 1 | **The bare curve** | `cc_discharge_lfp` | A 1S1P discharge with every model off: the instant `I·R0` step down at load, the RC tails, then LFP's famously flat plateau. Establishes that the plots are the ECM and nothing else. | none |
| 2 | **The pack disagrees with itself** | `soft_short…` | 4S2P: the group solve splits current by state, so cells diverge. Grid on SOC, then on overpotential. | none |
| 3 | **Belief drifts from truth** | `soft_short…` | `soc_bms` vs `soc_true`. The current sensor is wrong on *every* step and the estimator integrates it; LFP's flat OCV means the resting correction is weak mid-range — the failure `CLAUDE.md` calls "intended and visible". | none |
| 4 | **A sensor that lies** | `soft_short…` | Let the scenario's own scheduled `SensorOffset` on `GroupVoltage(1)` fire and watch the group-voltage row separate from truth — the one channel where a fault becomes visible, since it reads exactly otherwise. | none |
| 5 | **Protection, and its absence** | `soft_short…` | The same demand, BMS on then off: derate → contactor open, versus a limit simply violated. `CLAUDE.md` calls this contrast a core teaching scenario. | transport gate (below) |

Lessons 2–4 ride one scenario deliberately: the reader keeps one pack in their head and
changes what they look at, which is also why lesson 1 is the only one that reloads into a
different topology.

Nothing here is speculative. Lesson 4's fault has been scheduled in that TOML since
Phase 3 and its instrument landed in `[[ui-bms-view]]`; lesson 5's physics is Phase 2's
own exit criterion ("protection scenarios pass with BMS on, and the same demands violate
limits with BMS off"), so only the framing is new.

## Deliberately not in this slice

Three more lessons are already written as records and blocked on content that does not
exist. They are named here so the ordering is explicit, not to be built:

- **Chemistry comparison** — blocked. Three TOMLs ship (`lfp_26650_generic`,
  `nmc_18650_generic`, `nmc_21700_lgm50`) and **two are reachable from no client**: both
  scenarios pin LFP. Needs new scenario files, therefore the listing route first.
- **Aging: the fade curve** — blocked. The engine is complete since Phase 3, and the only
  `[pack.aging]` in the repo (`soft_short…:42`) sets `sub_clock_period_s` and nothing
  else. Needs an aging scenario *and* a cycles-vs-SOH view, which is a second history
  buffer — a separate slice on its own.
- **CC-CV charging** — blocked, and unbuilt anywhere. `CLAUDE.md` designates it
  client-side policy; it is genuinely wanted and genuinely independent of this.

Also out: no new scenario files, no server route, no per-lesson URL/deep-link, and no
`web/pkg` commit (gitignored as always — `start-frontend.bat` rebuilds).

## Mechanics, against verified line numbers

A step applies itself through the controls that already exist, and **never in parallel to
them**, so the sidebar and the lesson cannot disagree about the pack's state:

| what a step sets | how | why that way |
| --- | --- | --- |
| scenario | write `$("scenario").value`, `await loadScenario()` | `loadScenario` reads the select (`app.js:1513`); it is the only builder of a backend |
| demand | write `#demand-mode` / `#demand-value`, dispatch nothing | demand is re-read from the inputs on every step and every read (`app.js:1385`, `1423`) — there is no handler to call |
| ambient | write `#ambient`, call `applyEnv()` | `applyEnv` (`app.js:1575`) also repaints the label and routes through whichever backend is loaded |
| BMS | write `$("bms").checked`, `$("bms").dispatchEvent(new Event("change"))` | the handler is `() => $("reset").click()` (`app.js:1620`) and rebuilding at t = 0 is the documented behaviour, not a side effect to route around |
| run to a time | set `state.running = true`, and stop in `draw()` when `sim_time_s >= until_s` | the accumulator already owns pacing; a lesson must not introduce a second clock — principle 3 |

`watch` names an element id (`#pack`, `#bms`, `#plot-soc`, …) and the renderer outlines
it. Colour comes from the existing `--accent`, unchanged by this slice: `3f4ffdd` just
re-stepped the whole palette against the `dataviz` validator, and an outline is chrome
around data ink rather than a new entity.

## Three traps this slice walks into if unguarded

**1. Step text must not live in `#banner`.** `loadScenario` calls `clearBanner()` at
`app.js:1534`, and every lesson step that changes scenario passes through it. This is
exactly the bug `[[ui-bms-view]]` fixed — a warning erased ~200 ms later by the next
`clearBanner` — except that there it fired rarely and here it would fire on nearly every
step. The path gets its **own persistent element**, and the banner keeps its single job:
errors and engine refusals.

**2. Lesson 5 cannot run over the WebSocket, and that is a fact about the server.**
`afterFactsChange` disables the checkbox when the socket is on (`app.js:1562`), noting
"the server builds the pack the scenario asked for, and this page will not rewrite it".
The same line disables it when `scenario_has_bms === false`, which is true of
`cc_discharge_lfp` — it ships no `[pack.bms]`. So a step's `transport` field is load
bearing: it declares `"wasm"` where required, and the renderer says why rather than
presenting a dead control. Better to state the asymmetry than to hide a lesson.

**3. A step must survive the reader touching the sidebar mid-lesson.** Since every step
re-applies its full control set on entry, Back-then-Next is a repair. `expect` is prose,
never an assertion — a page that argues with its reader about a slider they moved on
purpose is worse than one that says nothing.

## Version constants: none of the three move

Read rather than assumed, because `[[ui-bms-view]]` recorded these two parting once:

- `sim_server::API_VERSION` (2) — states a bump rule for a renamed route/`ErrorCode`/wire
  field. No route changes. **Stays.**
- `sim_wasm::WASM_API_VERSION` (3) — scopes method names and wire field names. No new
  method; this slice adds no Rust at all. **Stays**, and `WASM_API_MIN` in `app.js` stays
  at 3 with it.
- `sim_core::SNAPSHOT_VERSION` (10) — no stored state changes shape. **Stays.**

A client-only slice that pays a bump would be paying ceremony. The one thing worth
re-checking at implementation time is that `WASM_API_MIN` is still perturbed by the
existing throw path, since this slice adds boot work ahead of the first scenario load.

## Verification

The highest-risk part, and budgeted as such: a lesson is mostly scenario loads, a full
path is many browser round-trips, and the rAF-occlusion trap has now recurred twice
(`[[ui-pedagogy-slice]]`, `[[ui-bms-view-slice]]`). The recorded workarounds apply — a
temp probe file under `M:\claud_projects\temp`, and a screenshot to force the paint.

Minimum to claim this works:

- Every one of the five lessons walked start to finish, with the *stated* observation
  actually visible — not merely "the step applied without an error". The distinction is
  the one `docs/plans/ui-pedagogy.md` drew between **landed** and **accepted**, and the
  same table format is owed here.
- One full path over **both** transports, with lesson 5 correctly refusing the socket
  and saying why. Scenario loading is the thing that differs between the in-tab wasm and
  a server session, and it is what a lesson does most.
- Back/Next past a step the reader has perturbed by hand.
- Console clean.

## Gate

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all` — and the expectation is that all three land **exactly** where they are
now, because this slice adds no Rust. The out-of-tree trajectory instrument cannot move
either: it builds its nine cases from scenario TOML through `parse_scenario` /
`build_pack`, and no physics, no chemistry, and no scenario file changes here.

## What the build changed, and what it found

Written after the fact, because three of these contradict the plan above.

**Six steps, not five.** Protection ships as two records (`protection-on`,
`protection-off`) rather than one. It is a *comparison*, and a comparison needs two
states; folding it into one step would have meant a lesson whose instruction was "now go
and change a toggle yourself", which is the thing this slice exists to remove.

**Two numbers in the prose were wrong, and the engine was right.** The first draft said
"a 2.5 Ah cell" and "15 A pack limit" — both taken from the illustrative TOML in
`CLAUDE.md` rather than from `chemistries/lfp_26650_generic.toml`, which says
`capacity_ah = 2.303451` (the *usable* Prada2013 stoichiometry window, and deliberately
not a round number). The real figures are 0.87 C at 2 A, and 3 C × 4.607 Ah = **13.82 A**
at pack level, which is what the readout showed. Caught because the cell emptied at 4146 s
where the arithmetic predicted 4500. **`CLAUDE.md`'s parameter block is a shape, not a
source** — the provenance rule applies to reading constants as much as to writing them.

**Lesson 3's claim did not survive its own instrument.** The draft said the estimate
"walks away from the truth and never walks back", implying visible integration of the
20 mA sensor offset. Measured over its ten minutes, the offset is worth about 0.07 points
while the boot error is 3.00 — the gap is essentially constant, and a reader watching for
a widening one would conclude the panel was broken. The text now leads with the error that
is *there* and names the offset as the mechanism that runs away over a longer drive.

**Lesson 2 gained a paragraph, and then lost it — it was reading a stale grid.** Mid-build
the pack readout `soc (true)` appeared to sit about two points *below* every tile, and
that got written up as a real semantic gap: `Telemetry::soc_true` is `rem_ah / nom_ah`,
against nominal, while a tile is the fraction of what that cell holds today. The
*semantics* are right; **the observation was not**. `refreshCells` self-throttles, and at
the multipliers used to buy frames during verification the grid was simply several
seconds behind the headline. On a freshly sampled grid the tiles bracket the aggregate
exactly — 73.83…74.32 around a headline of 74.09. The paragraph now says what is true and
useful instead: the aggregate sits in the middle of the spread and says nothing about it,
which is what an aggregate *is* and why the grid exists.

Twice in one slice, then: a claim about what a reader will see, written from reasoning
rather than from the screen, and wrong both times. **Measure first is not a style note.**

**`applyStep` needed `try`/`finally`, and the first version had neither.** Any throw left
`path.busy` set, which disables Back *and* Next: a permanently dead path showing "setting
up…" and no error. That is a worse failure than whatever caused it. The body is now
wrapped, the catch banners the message, and the `finally` clears `busy` unconditionally.

**The transport note only rendered in the paused branch.** So the one moment a reader most
needs to be told their transport was switched under them — while the step is running, just
after it happened — was the moment it said nothing. Appended in every branch now. Same
family as the erased-banner bug this plan was already guarding against: the check existed,
and the path through it that mattered did not run.

### What was verified, and how

Same **landed** / **accepted** distinction `docs/plans/ui-pedagogy.md` drew: *landed*
means the step's own stated observation was read off the screen, not that it applied
without erroring.

| step | on wasm | on socket |
| --- | --- | --- |
| 1 bare curve | **landed** — arrived exactly on 70m; flat plateau then the knee, `SOC_CLAMPED_LOW`, terminal 1.936 V | **landed** — same trajectory, clock advancing under `1S1P · server (WebSocket)` |
| 2 pack disagrees | **landed** — tiles fanned from identical to 74.68…75.14 | **landed** — 73.83…74.32 around a 74.09 headline |
| 3 belief drifts | **landed** — truth 63.17 vs BMS 66.24 (+3.07 pt); sensed current 6.035 against a true 6.000 | **landed** — +3.07 pt, bit-for-bit the same figures |
| 4 lying sensor | **landed** — `SHORT (INT)` 0.630 A, g1's dot 113 mV clear of the other three and labelled *outside truth*, its true-spread bar shortest, 27.2/29.2 °C, **no flags** | **landed** — identical, including the *outside truth* label |
| 5 protection on | **landed** — 13.821 A delivered against a demand of 40, `OC` raised (3 C × 4.607 Ah = 13.82 A) | **landed** — same 13.821 A and `OC` |
| 6 protection off | **landed** — 40.000 A obeyed exactly, cell V 1.427, 62.6/65.7 °C, `SOC_CLAMPED_LOW` the only flag | **n/a by design** — the gate fires and switches transport; verified twice from a live socket session |

Also landed: Back/Next repairing a sidebar perturbed to `Rest` / 999 / −20 °C / 1×;
Pause mid-step re-rendering the note to *"paused at …, part-way to …"* and back on resume;
the wasm-only gate naming its reason in the note while the step runs; console clean of
anything from the page; `cargo test --workspace` 53 suites, clippy and fmt unmoved.

### Verification traps, for the next client slice

The rAF-occlusion trap recurred a third time, and this session pins it down harder than
"take a screenshot":

- **rAF does not fire *at all* while the automation window is occluded.** Thirty seconds
  of `wait` advanced the clock by zero. A screenshot forces exactly one paint, hence one
  frame — so frames are a currency you spend one screenshot at a time.
- **Never `await requestAnimationFrame` in an injected script.** It never settles, and the
  evaluate dies on the 45 s CDP timeout looking like a frozen renderer.
- **A timed-out CDP evaluate keeps running in the page.** Its loop went on clicking `Next`
  underneath later probes, producing step numbers that made no sense. Reload before
  re-testing; a timeout abandons the *result*, not the script.
- Raising the speed multiplier is a safe way to buy frames: it changes how many steps a
  frame takes, never `dt`, so the trajectory is bit-identical.
- At very high multipliers the pack grid can trail the headline by one frame, because
  `refreshCells` self-throttles. Pre-existing, self-correcting on the next frame, and
  invisible at the speeds the lessons actually set.

## Ordering, after this

Unchanged from `[[ui-bms-view]]`, except that the path now gives the listing route a
second customer: **scenario-listing route → new scenarios (chemistry, aging) → CC-CV →
Phase 7 (Dfn)**. Each of the first three unblocks exactly one of the three lessons parked
above, which is a better argument for their order than "smallest first".
