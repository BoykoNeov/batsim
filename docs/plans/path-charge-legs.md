# The charge legs: the half of two steps nothing could measure

Status: **landed 2026-08-12.** Follows `docs/plans/path-display.md`, which closed the
formatter gap and left this one at the top of its deferred list:

> **The charge legs of steps 20 and 21 are unreachable by this harness**, and they are
> where both original defects live. Those legs begin when the reader puts the demand box
> to −2 A mid-run; the harness drives one demand program per step. The `0.0 %` defect
> itself is therefore still not asserted anywhere — only the class it belongs to is.

---

## What the gap was

`crates/sim-data/tests/path_claims.rs` builds each lesson's pack, drives it with that
lesson's one demand program to that lesson's mark, and checks the prose against the run.
Steps 20 and 21 are not one demand program. Both discharge to their mark and then tell the
reader, in the middle of the sentence, to reverse the demand box and press Run again — and
both spend their last third describing what happens after that.

So the two defects that motivated the previous slice — a clock reading `16m` where the
sentence gave a time to a tenth of a second, and a charge row still reading `0.0 %` where
the prose said the debt had cleared — were still not asserted by anything. The slice that
found them built the check that would catch their *class*; the instances themselves lived
on a trajectory the harness could not produce.

---

## The one assumption everything rests on, checked before anything was built

"The reader changes the demand box at the mark and presses Run" had to mean *the same pack
continues, at the same step length, with nothing rebuilt*. That was inferred from a note
string and a comment, which is not the handler. It is now checked in the page:

* `demand-value` has **no change handler at all**. It is read fresh inside `advance`
  (`Number($("demand-value").value)`) on the frame that uses it, so typing in it moves
  nothing on its own.
* `pathArrived` sets `path.until = null`, `state.running = false` and re-renders. It
  rebuilds nothing and resets no clock. The next Run resumes the same pack, and with the
  mark gone the frame loop's step-count clamp is gone with it — "nothing will stop it but
  you", as the step's own note says.
* `dt` is unchanged, and for a plain `Current` demand `advance` takes the frame's steps in
  one call, so the leg stays on the same uniform 0.5 s grid the pre-mark numbers were
  measured on. That matters more than it looks: `aging.rs` carries a partial sub-clock
  period rather than dropping it, so step 21's trajectory depends on the *sequence* of
  step sizes and not only on their sum.

Had any of that been false — a rebuild, a clock reset, a re-chopped grid — the leg would
be a different pack and this slice a different shape.

---

## What landed

**`web/path-claims.toml`** gains a second table type and one more claim field:

```toml
[[leg]]
step        = "what-it-cost"
instruction = "Now put the demand box to **−2** and press Run"
demand_a    = -2.0
run_for_s   = 400.0
```

and `after_mark = true` on a claim, which says it is read on that leg. `read_at_s` stays
**absolute** simulation time throughout.

The leg is declared here rather than as a `then:` field in the lesson block, because the
page does not do it — the reader does. A field in `const LESSONS` that the page never
reads would sit among fields the page acts on and read as behaviour.

**Three ties keep the leg honest**, and they are the only non-circular content it has:

1. `instruction` must appear **verbatim in that step's own prose**. Reword the sentence
   and the leg goes red.
2. `demand_a` must be spelled **as a number inside that instruction**. Not `contains`:
   every leg so far is a reversal, and `+2` would find its `2` inside the prose's own `−2`
   and pass — a leg run backwards, tied to a sentence saying the other direction, with
   every claim on it re-measured to match. `contains_number` refuses a match flanked by a
   digit, a decimal point, or a leading minus.
3. A leg must carry at least one `after_mark` claim. A leg that asserts nothing is a
   longer simulation that looks like coverage.

**`crates/sim-data/tests/path_claims.rs`** grows a `Row` (time, telemetry, and the pack's
largest per-cell deficit), a `drive` helper so a run is one or two legs of the same pack,
the leg loader and its checks, a two-directional reachability check, and two quantities:
`deficit_pts_at` and `deficit_zero_s`.

Coverage goes from **32 claims to 46**, over the same 7 of 21 steps.

### Reachability on a leg is a weaker claim, and it is circular

Said here, in the file header, and in the test's module docs, because it will not be
obvious from a green run. Before the mark, `until_s` is a stop the page enforces; the
check is a fact about the page. After the mark the page stops for nothing, so `run_for_s`
is bounded only by what the prose asks the reader to do — and if it is set to just cover
the furthest claim, "reachable" reduces to "I ran long enough to reach it". Read a green
leg claim as *true of the trajectory a reader following this sentence produces*, not as
*a reader will get this far*. This is the `tol` hazard's shape again: a field whose
correctness rests on the author, in a file whose whole purpose is not to rest on the author.

### The deficit is claimable; the row that shows it still is not

`deficit_pts_at` and `deficit_zero_s` read `Pack::cell` ground truth. They may name no
`display`, and the two claims that use the first say so in their notes — because `past
empty` samples the same quantity on a quarter-second **wall**-clock throttle, and step 21's
own prose records that row reading 9.438 points at an instant the engine was at 9.704. A
future reader must not take a green 9.704 for a checked panel row.

`deficit_zero_s` is measured rather than written down for a specific reason: five of this
slice's claims are read at the instant the debt clears, and a hardcoded 983 would quietly
become fiction if the trajectory moved. Now the instant is checked before anything is read
at it.

### Relative prose, absolute claims

The leg prose switches to time-since-the-mark — "383.0 s later", "0.062 V a minute in",
"95.14 % from 390 s in" — while `read_at_s` is absolute. A claim authored with the relative
number in both halves would pass green and be measuring the wrong instant: the "measured
past the mark" defect from `docs/plans/reversal-ui.md` in a new costume. Every leg claim's
note records both readings.

---

## What it found

**Two prose defects, both in step 20, both in the direction a reader would notice.**

* **`0.026 V on the first step` was the second step's reading.** The terminal is 0.024988 V
  at t = 4400.5 s — the first step after the reversal, and what a reader pausing and
  pressing Step 1 once gets — and 0.025951 V at 4401.0. The row prints `0.025 V` and then
  `0.026 V`. Both numbers are true of the run; only one of them is the first step. Now
  `0.025 V on the first step`, claimed and displayed.
* **`1.732 V at 240` is 1.731450 V**, which is 5.5e-4 from the sentence — outside the
  tolerance this file's own rule would give it (half a unit in the last printed place of
  `1.732` is 5e-4) — and, worse, the row prints `1.731 V`. A reader told to watch the trace
  come back up was being handed a string the panel never shows. Now `1.731 V at 240`.

**Step 21's charge leg was clean, to a degree worth recording**, because it is the step
whose numbers were most recently re-measured:

* the debt clears 383.0 s after the mark, exactly;
* the clock reads `16m` there and `10m` at the mark;
* `soc (true)` still prints `0.0 %` at that instant, the cell is 0.003974 % full (prose:
  0.004 %), and the row first prints `0.1 %` at t = 985.0 s — two seconds later, exactly;
* `soh cap` first prints `95.15 %` at t = 660.0 s and `95.14 %` at t = 990.0 s — the "from
  one minute into the charge" and "from 390 s in" of the prose are both *transitions*, not
  approximations, though the claims read one instant each and do not assert that.

Also checked and left alone: `254 s at 2 A is 0.1410 A·h`. As arithmetic that is 0.1411,
but the quantity the sentence is naming — the debt actually repaid — is 0.14098 A·h,
because the last step overshoots into a cell that is 5.1e-5 full. The stated figure is the
better of the two and the arithmetic is a hair loose. Not worth an edit; worth not
"fixing".

---

## Reddening

Sixteen perturbations, **each run as its own `cargo test` invocation and judged by its exit
code**, never by grepping a combined log — `cargo test --workspace` stops at the first
failing binary, and a harness that read output instead once reported 5/5 green when all
five were lies (`docs/plans/surface-vs-bulk.md`). Script kept out of tree at
`M:\claud_projects\temp\wedge\redden.py`; the tree is restored after each case and was
verified clean at the end.

| # | perturbation | reddens |
|---|---|---|
| 0 | a leg claim's value drifts | value |
| 1 | a leg claim's `shows` drifts (`0.0 %` → `0.1 %`) | displayed |
| 2 | a leg claim's `shows` drifts on the clock (`16m` → `17m`) | displayed |
| 3 | `after_mark` claim on a step with no leg | reachable |
| 4 | `after_mark` claim reading *before* the mark | reachable |
| 5 | `after_mark` claim reading past the leg's end | reachable |
| 6 | pre-mark claim reading past the mark (the old check still bites) | reachable |
| 7 | `instruction` is not in the step's prose | instructed |
| 8 | the *prose* is reworded away from the instruction | instructed |
| 9 | leg current is not the one the sentence says (−3 vs −2) | instructed |
| 10 | leg current is the right digit with the **wrong sign** (+2 vs −2) | instructed |
| 11 | a leg with no claims on it | instructed |
| 12 | two legs declared on one step | instructed |
| 13 | a leg naming a step that does not exist | step exists |
| 14 | **the leg is not run at all** — the harness keeps the discharge demand | value |
| 15 | the deficit is read 0.1 % low | value |

Case 14 is the one that says the mechanism does work: with the leg silently not run, the
leg claims fail rather than quietly measuring the wrong trajectory.

Case 11 had to be rewritten. Un-flagging one claim's `after_mark` reddened the run, but
under the *reachability* check — the claim became a pre-mark claim reading past the mark —
which proves nothing about "a leg must assert something". Isolating it needed the whole
step-20 leg block deleted, leaving a leg with no claims. It was then hand-validated by
reading the message: `7 passed; 1 failed`, and the failure is
`step 'past-empty' declares a charge leg and no claim reads it`. That is the one case whose
output was read rather than counted.

---

## Versions

**Nothing moves.** No engine state, no wire field, no stored layout, no schema, no version
constant. `web/pkg` needs no rebuild: the only Rust in this diff is a test, and `app.js` is
served as a file rather than embedded in the wasm bundle. The page change is two numbers in
one sentence.

---

## Deferred, with a price

* **`run_for_s` is unbounded by anything outside this file**, which is what makes the leg's
  reachability check circular. The honest fix is not a tighter number, it is a different
  kind of evidence — the prose saying how far to run, parsed. Step 19 does say it ("let it
  go to about 400 s"); steps 20 and 21 do not.
* **Only the demand box.** Steps 18 and 19 ask the reader to change `dt`, uncheck the BMS,
  press Restart, and press two clear-buttons in a stated order — and step 19's most
  interesting arm ("keep running, everything worth seeing happens after the mark") is a leg
  this mechanism could nearly reach, except that it also unchecks the BMS. A leg carrying an
  optional BMS override is a small change and is not made here.
* **One leg per step, and no change part-way along one.** The mechanism is a second demand,
  not a demand *sequence*. Nothing in the path needs a third leg today.
* **`past empty` and `surface gap` still cannot be displayed**, unchanged by this slice.
  `past empty` is now measurable as a value, which is strictly more than before and strictly
  less than what its sentences claim: the prose describes a row a reader watches counting
  down, and what is checked is the number behind it.
* **Fourteen of twenty-one steps still carry no claim, unchanged by this slice.** All
  fourteen new claims went to two steps that were already covered; the count of covered
  steps did not move. Those fourteen steps are unchecked, not passing.
* **`tol` still has no enforcement**, and this slice adds a second field with the same
  property: `run_for_s`. Both are author-set numbers that a careless value makes vacuous,
  and neither has a check.
* **The two corrected numbers were corrected by this harness's own measurement**, which is
  the same instrument that now asserts them. If the mirror of the `terminal` formatter is
  wrong in some way `MIRRORED` does not pin, both the correction and its check are wrong
  together. The independent evidence is that `0.025`/`0.026` is a *step boundary* question,
  not a formatting one, and the ECM arithmetic for it is written out in that claim's note.
