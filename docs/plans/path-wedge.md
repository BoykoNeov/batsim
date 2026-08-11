# The guided path's last step, and a freeze nobody has attributed

*Everything down to "What the build found" is the plan as written **before** the work, in
the future tense, kept because two of its predictions were wrong in ways worth reading. The
findings section corrects it in place and says which parts it overturns. It was not a freeze
and it was not that lesson.*

## Context

The guided path is 19 steps. Entering the last one — `nothing-to-clamp`, the 100 mΩ
external short — stops the page answering, and it never recovers. Three measurements are
already on record in `docs/plans/surface-vs-bulk.md`:

* steps 1–18 all answer; the page dies on the transition **into** 19, permanently
  (re-checked at +5 s and +15 s);
* the scenario is not the cause — loading `external_short_100_milliohm.toml` directly over
  the in-page engine runs clean through 88 A, the contactor opening, and thirteen simulated
  minutes;
* a worktree at `aa22c2d` — before the surface-gap slice, its own 18-lesson page on its own
  port — dies on **exactly the same transition**, entering *its* last step from the one
  before. Same lesson, same symptom, different number.

So it is that lesson's *entry path*, and it is not inherited from the last slice. What has
never been established is whether the **page** freezes or only the **driver** does. All
three measurements were taken through the same CDP script, and "evaluates stop being
answered" is equally the signature of a dead debugger session or a starved render loop —
both of which this repo has hit before and recorded (`spm-scenario.md`, `ui-pedagogy.md`:
rAF does not fire at all when the window is occluded; a timed-out evaluate keeps clicking;
pace the poll loop or it starves the page's rAF).

The consequence is concrete rather than cosmetic: `surface-vs-bulk.md`'s exit criterion
has an unmet clause because of this — *"the whole path walked forward and back"* — and
**no one has ever walked the path end to end**, in either direction, with every step run to
completion. Whatever else the freeze is, it is the thing standing between the shipped
tutorial and its first complete traversal, and it may be masking more than one broken step.

Intended outcome: the last step is enterable, the freeze is attributed by measurement
rather than argued, and the whole path is walked forward and back with each step running to
its own mark — the criterion the last three slices have each deferred.

## Phase 1 — page or driver (blocking; do nothing else first)

Reading the code did not find it. `applyStep` has no loop; `frame` clamps its step count to
the step's own mark; `advance`'s two `while` loops both decrease strictly
(`pulsePhase().toEdge` is ≥ 1 in both legs, checked). So the cause is not visible statically
and the discrimination has to be measured.

**What the symptom does and does not admit.** `Runtime.evaluate` runs on the renderer's
main thread and does **not** need an animation frame. A rAF loop that never fires leaves the
main thread idle and evaluates answering promptly — so the occlusion trap this repo keeps
citing does *not* explain a dead evaluate channel, and neither does a promise that never
resolves: `frame` re-schedules itself on its first line, so a hung `await advance(n)` leaves
a page that is visibly stuck but perfectly responsive. Permanently unanswered evaluates are
consistent with exactly three things — **the main thread blocked in a synchronous loop**,
**the renderer gone (crash / OOM)**, or **the debugger transport dead**. Plan the arms
against those three, not against rAF.

Two checks were already done while planning, and both go in the record:

* `M:\claud_projects\temp\surface-cdp\chrome.log` is **empty, 0 bytes** — the browser's own
  output was never captured in the run that recorded the freeze. No crash evidence either
  way. Redirect Chrome's stderr to a real file this time; it may end the investigation for
  free.
* `walk.py`'s `Target.js` does use `awaitPromise=True`, but `wedge.py`'s snippets return
  plain values (`click(); return 1`), so nothing there waits on a frame. That makes
  "the driver waited on a promise" a weaker explanation than it first appeared — keep it as
  an arm, not the leading theory. What *is* worth knowing: the driver's socket carries a
  90 s timeout and `send` blocks in `recv` until its own id comes back, so "permanently
  unresponsive" currently means "one 90 s read timed out".

The instrument must survive a dead evaluate channel, so it must not *be* an evaluate:

1. **Crash and error events, subscribed before the transition.** `Inspector.targetCrashed`,
   `Runtime.exceptionThrown`, `Log.entryAdded` — push channels, nearly free, and any one of
   them ends the investigation in a single run. The existing `Target` already enables
   `Runtime` and installs a page-level error hook; it does not listen for these, and its
   `send` loop *discards* every event message that is not its own reply, so they have to be
   drained deliberately.
2. **A heartbeat that carries state, not a tick.** A bare frame counter only distinguishes
   a blocked event loop, which the CPU sample below already tells you. Write the state
   instead — `document.title = \`${n}|i=${path.i}|busy=${path.busy?1:0}|run=${state.running?1:0}|be=${state.backend?1:0}\`` —
   and read it by polling Chrome's `GET /json/list`, which the **browser** process serves,
   not the renderer. That answers "did `applyStep` reach its `finally`", "did the backend
   get replaced", "did the run arm", with no evaluate at all, and so covers most of Phase 2's
   bisect as well. It is a temporary patch to `frame` in `web/app.js`: copy the file first
   and restore from the copy, never `git checkout` — this repo has destroyed uncommitted
   work that way once.
3. **Corroboration, free:** sample the Chrome renderer process's CPU time
   (`Get-Process chrome | Select-Object CPU`) across the transition. A JavaScript loop that
   never yields burns a core; a dead transport leaves an idle process.
4. **Ground truth, once:** drive the last transition by hand in a normal, focused, visible
   Chrome window with no automation attached at all. The only check with no shared failure
   mode with the other three.

Existing harness to reuse rather than rewrite: `M:\claud_projects\temp\surface-cdp\walk.py`
(`Target`, `READ`) and `wedge.py`, which already jumps to the step by the scenario picker so
the sixteen lessons in front of it cost nothing.

**If it is the harness**, this slice collapses to "make the walk trustworthy" — fix the
driver, run the full forward-and-back walk (Phase 3), correct the three plan documents that
record the wedge as a page fault, and stop. Say so plainly; a measurement that overturns a
recorded finding is the result.

**If it is the page**, continue to Phase 2.

## Phase 2 — attribute it, by bisecting the record and not by reading

The recorded evidence already rules out the scenario file, so the cause is in the *entry*:
`applyStep` (`web/app.js:3013`) run against this lesson's record (`web/app.js:2911`, the
last entry of `LESSONS`). Bisect over that record's fields, one at a time, entering the step
from step 18 each time — the transition that fails — and keeping the page's own heartbeat
running so each arm is answered by the instrument from Phase 1 rather than by an evaluate:

* `transport: "wasm"` — the only field that sets `path.switchedTransport`, which forces
  `reload` and takes the load path even when the scenario already matches. Note that
  assigning `$("use-socket").checked` does **not** fire the change handler, so the page can
  be mid-load with one backend live and the checkbox describing another.
* `bms: true` against the branch that calls `$("reset").onclick()` directly and awaits it —
  a rebuild at t = 0 layered on top of a load that is already in flight.
* `until_s: 200` with `speed_x: 20` and `dt: 0.5` — the arming of the run itself.
* the predecessor: enter the same step from step 17 and from a cold `Start`, to confirm the
  claim "from the one before" is about step 18 specifically and not about being last.

Two structural suspects worth checking while in there — but **second-tier by the reasoning
above**, since neither can by itself silence an evaluate. Both would produce a page that is
stuck and still responsive, so if the heartbeat says the main thread is alive they become the
explanation, and if it says the main thread is blocked they are not it:

* `refreshCells` (`web/app.js:1565`) is deliberately **not** awaited by `frame` and guards
  itself with `state.cellsBusy`. An exception thrown between setting that flag and the
  `finally` that clears it would be a permanent stall of the grid; `loadScenario` replacing
  `state.backend` mid-read is exactly the interleaving it is written against.
* `loadScenario`'s `loadSeq` ticket (`web/app.js:2141`) returns early on an overtaken load —
  and an overtaken load leaves `state.backend` null while `applyStep` proceeds to arm a run.

First-tier, and untested so far: **the renderer simply dying** on this transition. Phase 1's
crash subscription is what answers it, and nothing in Phase 2 should run before that answer
is in.

## Phase 3 — fix, then walk the whole path both ways

Fix the cause found in Phase 2 in `web/app.js`. Expect no Rust change and no version
constant to move; if the fault turns out to be in an adapter, that changes the shape of the
slice and should be re-planned rather than absorbed.

Whatever the cause, add the guard the failure argues for. `applyStep` already has an
unconditional `finally` that clears `path.busy` precisely so a throw cannot wedge Back and
Next — the same reasoning applies to whichever flag or await turns out to be the one that
never completes.

Then the walk, which is the deliverable and the thing that has never happened:

* every step 1 → 19 **run to its own mark**, not paused past it (the previous walk skipped
  each mark to save time, so only one step ever ran to completion);
* then 19 → 1 backwards, which exercises `applyStep`'s reload inequality in the direction it
  does *not* hold on its own — a mark behind the pack forces a rebuild, a mark ahead does
  not, and the pulse steps 12–14 are where that asymmetry bites;
* driven from a script kept out of tree under `M:\claud_projects\temp\`, extending
  `surface-cdp\walk.py`, so it is re-runnable by the next slice.

Known traps to honour in the driver, all previously paid for: never `await setTimeout` in an
injected script — pace with `await fetch`; one debugger session per target; IIFE-wrap
evaluates; a screenshot forces a paint on an occluded window; and anything read off a page
driven fast under an occluded window must be re-read at rest.

## Verification

* The discrimination of Phase 1 stated as a measurement with its three arms, including
  which one overturned or confirmed the record.
* A forward pass and a backward pass over all 19 steps, each step reaching its own
  `until_s`, with `path-where` read at every step — the transcript is the evidence.
* `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace` still clean (expected untouched, but the gate is the gate). Note
  `cargo test --workspace` stops at the first failing binary.
* **Scope cap on the prose.** Running every step to its mark will very likely surface
  lesson text whose numbers do not match, since most steps have only ever been checked at
  their marks. **Record what moved; do not fix it here.** Nineteen steps of dense numeric
  prose is a slice of its own and the ask was the frozen step. The one exception is prose
  the fix itself falsifies, which this repo's convention says must not survive the commit
  that breaks it.
* The exit criterion in `docs/plans/surface-vs-bulk.md` is closed or re-qualified in place,
  on that file's own precedent.

## What the build found

Written after the fact, as the other plans in this directory are. Instruments live in
`M:\claud_projects\temp\wedge\`: `phase1.py` (discrimination), `phase2.py` (the stack),
`walk_full.py` (the walk), and `app.js.orig`, the copy taken before anything was edited.

### It was never a freeze — the renderer was being killed

The three-armed discrimination answered on the first run that reached the transition, and
it answered something none of the three arms was written to expect:

    Inspector.detached  {"reason": "Render process gone."}

The page does not hang. **The render process dies**, and the debugger channel goes with
it — which is why every earlier measurement, all of them taken through evaluates, read as
"the page stopped answering, permanently".

The plan's own reasoning about the symptom was half right and half wrong, and both halves
mattered. Right: an evaluate does not need an animation frame, so occlusion — the trap
this repo keeps citing — could never have explained it, and neither could a promise that
never resolves. Wrong: of the three remaining candidates the plan named, it ranked "the
main thread blocked in a synchronous loop" first and treated a crash as the outside
chance. It was the crash. The subscription that caught it cost four lines.

The `document.title` heartbeat carried its weight anyway, because it dated the death
precisely. It stops at

    hb2401|i=18|on=1|busy=0|until=200|run=1|be=1|t=9.5|cb=0

— `busy=0` says `applyStep` ran to its `finally`, `run=1|until=200` says the step armed
normally, and `t=9.5` says the pack had already advanced 9.5 simulated seconds. So the
step's *setup* is not the cause and never was; the page dies in an ordinary frame, a
second into an ordinary run.

**Memory named the mechanism.** Adding a working-set sample to the poll turned the CPU
arm into an unambiguous read:

| t after the click | max chrome working set |
| --- | --- |
| 0.2 s | 532 MB |
| 2.0 s | 777 MB |
| 4.9 s | 2028 MB |
| 9.6 s | 3483 MB |
| 14.8 s | 4309 MB |
| 16.5 s | **531 MB — the process is gone** |

A renderer allocating four gigabytes in fifteen seconds and then being killed.

**Chrome's own stderr is still 0 bytes, and that is a fact about the capture, not about
Chrome.** This run passed `--enable-logging=stderr` and redirected the launched process's
output to `chrome.log`, and the file is empty across three runs — but the process that
died is a *child* of the one that was redirected, and on Windows that flag does not
reliably reach a redirected pipe anyway. So the honest statement is "no crash line was
captured, and the capture is unproven", not "Chrome said nothing". The instrument that
did answer was the CDP event subscription, which is a push channel from the browser
process and needed no log at all. Left recorded because the tempting version of this
sentence — a green-looking 0-byte log read as evidence of no crash — is the same trap
`ANCHORS.md` names as "check what a green gate covers", and it would have pointed away
from the answer rather than towards it.

### The line, taken from the running loop

`Debugger.pause` is delivered to V8 as an interrupt and fires between bytecodes, so it
stops a loop that is *running* rather than needing an idle main thread. Three seconds
into the runaway it produced the whole answer in one stack:

    drawPanel   line 750
    draw        line 1992
    frame       line 2088
    locals: y0 = 13.2568, y1 = 13.256800000000002, yStep = 5e-16

`web/app.js:750` was

```js
for (let v = Math.ceil(y0 / yStep) * yStep; v <= y1; v += yStep) {
  yTicks.push({ v, text: v.toFixed(decimals) });
}
```

The two endpoints are **two ULPs apart** — the same voltage twice, as far as anything
physical is concerned — and one ULP at 13.25 is 1.8e-15 against a tick step of 5e-16. So
`v += yStep` **does not move `v`**, the loop cannot terminate, and it has a `push` in it.
That is not a slow loop; it is a memory leak with an exit condition that can never fire.

The value is the giveaway: 13.2568 V is a 4S LFP pack at rest. Step 19's pack sits
resting for its first sixty seconds before the short fires, and successive samples of a
resting terminal voltage differ by a float step or two. **The trigger is a nearly-flat
trace, not this lesson** — which is why "it belongs to that lesson's entry path" was as
far as three earlier measurements got: they varied the lesson, and the lesson was never
the variable.

Two guards existed and neither covered it. `if (!Number.isFinite(y0)) [y0, y1] = [0, 1]`
catches an empty trace, and `(y1 - y0) * 0.08 || Math.max(...)` catches endpoints that
are *exactly* equal — the `||` falls through on zero. Two ULPs is not zero, so it took
the tiny-pad branch and produced a nonzero, meaningless axis. **A guard written for the
degenerate case can be missed by one ULP.**

### The fix, and which part stops what

* **The cause — a range floor.** A span too small to be a range is treated as flat and
  padded like a flat one, replacing the `|| 0` idiom that only ever caught exact equality.
  **This is the part that stops the crash.**
* **The loop — counted, not accumulated.** Both tick loops index from a first tick instead
  of adding into the induction variable, so the variable cannot fail to advance. With the
  floor in place this catches nothing; it is a backstop, and `MAX_TICKS` bounds the one
  range `drawPanel` does not compute for itself — `yFixed`, whose only caller today passes
  the literal `[0, 100]` of the state-of-charge panel.
* **The x axis had the identical defect**, at what was line 787: `spanned` only ruled out
  `x1 === x0`, so two sample times a couple of ULPs apart reached `t += xStep` exactly as
  two voltages reached `v += yStep`. It now takes the same floor, which drops it into the
  single-label branch that "no span" already meant there. Same bug, same commit, not a
  precaution.

**The floor is two floors, and the relative half alone was wrong.** The first version
tested `span > Math.abs(y1) * 1e-12` — relative only, and keyed on one endpoint. A trace
straddling zero has no magnitude to be relative to: `y0 = -1e-18, y1 = 1e-18` beats a
threshold of 1e-30 and sails through into an axis two attometres wide. No crash there (the
tick cap holds it at 64 gridlines) but a meaningless axis, and a floor that misses the case
it exists for. The shipped test takes the larger of both endpoints and floors it
absolutely: `span > Math.max(mag * 1e-12, 1e-9)`. A nanovolt is four orders below the tens
of microvolts a resting pack's cell-voltage panel legitimately spans.

Checked over the edge cases (`M:\claud_projects\temp\wedge\floor.mjs` — an arithmetic
check of the predicate, with the walk below as the end-to-end proof): the crash's own pair
and its negative twin go flat to a 1.33 V axis with 2 ticks; straddling zero and both
exactly-equal cases land on a 1.0-wide axis; and a 0.6 mV cell-voltage span, a 1 µV span,
an ordinary 2.0–4.2 V and a 298–344 K temperature range all stay live with 3 or 4 ticks.

Verified against the crash it was written for: the transition into step 19 now runs
through 9.5 s, past the fault at 60 s, with the working set flat at 533 MB.

### The path walked, both ways, for the first time

`walk_full.py`, on the shipped page with the instrument removed: **37 of 37 steps reached
their own marks — 19 forward, 18 back — and `window.__errs` is empty at the end.** Arrival
is the page's own statement rather than the driver's opinion: `applyStep` ends by setting
the Run button to "Pause" and `pathArrived` sets it back to "Run", so a step that never
armed is reported as such instead of being counted.

Three things the walk itself taught:

* **There is no such thing as entering the last step from behind.** The first run scored
  36/37 and spent 420 s — its entire per-step budget — waiting for step 19 to re-arm at
  the turnaround. It had just arrived there at the end of the forward pass, so no "Pause"
  was ever coming. The backward pass starts at 18 by construction, and the driver says so
  now rather than scoring a bookkeeping artefact as a failure.
* **One step reads differently depending on which side you arrive from, by design.**
  Step 5 finishes its mark at **40.8 % / 12.628 V** going forward and **37.9 % /
  12.593 V** coming back, at the same `t = 20m`. That is `applyStep`'s reload rule being
  an inequality: steps 4, 5 and 6 share a scenario, so forward from 4 (mark 600) into 5
  (mark 1200) *continues* a pack that has drawn 6 A for ten minutes, while backward from 6
  — which reloaded, because its mark is behind — continues a pack that has spent 60 s at
  40 A instead. Both are the rule working. Its lesson survives either way: the fault is
  scheduled at t = 600 s in the file, and both approaches cross it. Nothing in the prose
  is falsified; **the numbers a reader sees are a function of their route**, which is worth
  knowing before anyone writes a number into that step.
* **Every other step is identical in both directions**, to the digit, and the runs
  reproduce each other — the values above come from three independent walks, the last of
  them **against the final tree**, after the two-part floor replaced the relative-only one.
  Re-running the verification on the tree that ships, rather than on the tree the first
  version of the fix was measured on, is this repo's own rule (`dfn-aging-gap-closed`).

The steps that were previously unreachable now report what they were written to: step 18
`CONTACTOR_OPEN` at 89.4 %, step 19 `CONTACTOR_OPEN` at 39.6 %, which is the fifty-point
contrast that lesson exists for.

### What is not covered

There is no JavaScript test harness in this repo and this slice does not add one — the
page has never had one, `drawPanel` is module-scoped, and exporting it to make it testable
is a larger change than the fix. The guard's evidence is the walk plus the crash
reproduction, both re-runnable from `M:\claud_projects\temp\wedge\`. Recorded as a gap
rather than left implied.

## Exit criterion, met

The page enters step 19 from step 18 and keeps running; the freeze is attributed to a named
cause by measurement; and the 19-step path is walked forward and then backward with every
step reaching its own mark, from a script that can be re-run.

All three met, with one clause worth stating precisely rather than generously: "walked
backward" is 18 steps, not 19, because the last step has nothing to be entered from behind.
The forward pass covers it, and the phrasing above is the honest form of what
`surface-vs-bulk.md` deferred.

Gates: `cargo fmt --all --check` clean, `cargo clippy --workspace --all-targets -- -D
warnings` clean, `cargo test --workspace` 60 binaries all green — expected, since no Rust
was touched, and run because the gate is the gate. `SNAPSHOT_VERSION` 13 → 13,
`API_VERSION` 2 → 2, `WASM_API_VERSION` 5 → 5: a page-only fix moves no constant, and
`web/pkg` needs no rebuild because no Rust changed.

## Named follow-ups, deferred here

* **Verify the path's numbers.** The walk above will run every step to completion for the
  first time; anything it contradicts gets recorded, and correcting it is its own slice.
* **The solver blow-up.** A `Demand::Voltage` far outside a cell's window drives the pack
  solve to −2.9e6 V, negative kelvin and NaN — flagged `SOLVE_UNCONVERGED`, but that flag's
  own doc promises a client "approximate", which this is not. The iteration has a cap and no
  damping or bracketing, where `CLAUDE.md` specifies a bisection fallback for `Power`. It has
  also blinded the out-of-tree trajectory instrument in that region since Phase 7 slice A
  (`ANCHORS.md`, "Blind spots"). May be two slices if each cell model has to declare a valid
  window.
* **Lead-acid and NiMH.** Phase-sized rather than slice-sized: needs OCV hysteresis, which
  is per-cell state, so a `SNAPSHOT_VERSION` bump with a migration — and sourced constants
  for two chemistries that have none in the tree.
