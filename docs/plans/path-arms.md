# The buttons the reader is told to press

> Two capabilities the harness needs first, both established by this slice:
>
> 1. **A zero-length probe row**, or a fifth of these steps cannot be checked at all.
> 2. **Instructed continuations and control changes.** `Leg` covers one demand change after
>    the mark. These steps also ask for a BMS toggle, two step lengths, an ambient change,
>    `Clear queued`, `Clear latched BMS fault`, and "press Run again" — all reproduced by
>    the temporary harness here, none of them by `run()`.
>
> — `docs/plans/path-prose-ledger.md`

The probe landed one slice ago. This is the second one. `crates/sim-data/tests/path_claims.rs`
could reproduce exactly one thing a reader does after a run stops — type a new current into
the demand box — and everything else the guided path instructs was unclaimable. Four steps'
worth of prose tells the reader to uncheck the BMS, put `dt` up to 5 and press **Restart**,
clear a latched fault, clear the fault queue, press **Step 1**, or simply press **Run**
again, and not one of those trajectories existed anywhere in the tree.

**The temporary harness the plan doc refers to was never committed.** The prose-ledger slice
landed 52 lines of `path_claims.rs` — the CC-CV window fix — and threw the rest away. Its
measurements are on the record and the code that made them is not, so this slice rebuilds
the six control changes rather than recovering them.

## What went in

`[[leg]]` is gone, replaced by `[[arm]]`: an instructed control change plus the trajectory
that follows from making it.

* **`start`** — `mark` continues the step's own pack past its mark, which is what
  `pathArrived` leaves in front of a reader; `restart` builds a fresh pack under the arm's
  controls, which is what the page's **Restart** button and the BMS checkbox both do.
* **Overrides** — `demand_a`, `dt`, `bms`. The old leg was `start = "mark"` with a
  `demand_a`, and the two legs in the file migrated to exactly that.
* **`actions`** — the buttons, in order: `clear_queued`, `clear_latched`, `step_1`, `run`.
  What separates them is the thing step 18 is about — whether the pack advances.
* **`identical_to`** — one arm's end state must equal another's, bit for bit.
* **`Claim::arm`** replaces `Claim::after_mark`, and is a name rather than a flag.

Claims: 73 → 88. Every new one is read on an arm.

### Why `after_mark` had to become a name

A leg only ever *appended* to the step's own trajectory, so "is this claim past the mark"
answered "which trajectory is this claim on" as a side effect. A restart arm breaks that:
step 18's `dt = 5` arm ends at the step's own 90 s mark and reports a different number
there. Time cannot name the trajectory, so the claim does.

The two frames that were fenced against each other — a duration since the mark, and a
duration remaining to it — are now fenced on `start == "mark"` rather than on the flag, and
a restart arm is excluded from both. On an arm that rebuilds the pack the mark is not an
origin.

### The fence that actually matters, and it is not the instruction

`every_leg_is_instructed_by_its_own_step` required the sentence to be in the prose and the
current to be spelled inside it. With one control that was the whole of it. With three, the
same check goes slack in a way worth naming: an arm citing "uncheck the BMS" could also
have run at `dt = 5`, and its claims would have been true of a trajectory nobody is told to
produce.

So every override now needs its own anchor in the sentence it cites, and every override has
to be a real change from what the step configures:

| override | anchored by | and must |
| --- | --- | --- |
| `demand_a` | the current spelled in the instruction | — |
| `dt` | the step length spelled in the instruction | differ from the step's, and be on a `restart` |
| `bms` | the word `BMS` in the instruction | differ from what the step configures |

The `bms` direction is *derived* rather than word-matched, and that was the second design
worth having. A word list ("uncheck", "off", "without") is a declaration that can disagree
with the fact. Requiring the value to differ from the step's own means the direction is
forced by the step: `nothing-to-clamp` ships a BMS, so the only change available is off.

## Step 18's sentence that is not a number

> Press those same two buttons in the other order, still without running, and you get an
> identical pack: the move you cannot take back is the Run.

No `quantity` states that, and picking one that happens to match — the terminal voltage,
say — would assert something weaker than the reader is told. It is asserted instead as a
pair of arms that stop at the buttons and whose serialised snapshots must be equal.

**Stopping at the buttons is load-bearing.** Comparing after an identical Step 1 and a
minute of running would be green if the two orders left different packs and the run washed
the difference out — which is the exact failure the sentence warns about.

It is also the only thing in the tree asserting that `Pack::clear_faults` and
`Pack::clear_bms_fault` commute. They touch different state today and nothing else says
they must.

## What was found

### A third number, wrong at its own precision

> and running on from there drifts up to **13.25 V** over the following minute

The engine says **13.255344 V** a minute after the Step 1 that repairs the pack. The
`terminal` row prints `13.255 V`. Two decimals of 13.2553 is 13.26, not 13.25 — so the
sentence was wrong in both available readings: not the string the panel shows, and not a
correct rounding of the value either.

The tell is inside the sentence itself. Its sibling number, `**13.236 V**`, is given to
three decimals *because that is what the row prints* — the author quoted the panel for the
first number and rounded by hand for the second. Repaired to `13.255 V`, and both halves
are now display claims quoting the row.

This one was not a drift. Like `three-times-the-current`'s missing 600-second contrast, it
was wrong when written, and for the same structural reason: nothing in the repo had ever
produced the trajectory it describes.

### The five stale currents are now guarded

`nothing-to-clamp`'s post-clamp currents — 58.04, 40.33, 17.03, 1.94, 0.103 A — and its
376.4 K peak were repaired one slice ago after going stale at `2e306b2`, where aging began
growing the RC resistances. They were *measured* then and guarded by nothing; a measurement
ages the moment it is taken. They now carry claims, as does the `SOC_CLAMPED_LOW` arrival
at 235.5 s that ends the run and the pair of currents that establish the two arms are
identical until they are not.

All eleven reproduce to the digit the prose prints.

### The peak instant is claimed as a read time, not as a peak

The trajectory is flat across the two steps either side of the maximum — 376.37081 K at
245.0 s against 376.37083 at 245.5 — so the engine's argmax is 245.5 by two parts in 10^8
and a panel printing one decimal cannot tell them apart. Claiming the argmax would pin
something no reader can check, which is the "right but unreachable" defect this repo has
shipped twice.

So `245.5` is accounted for as the temperature claim's *read instant* — "we measured then",
which is as much as the panel supports. What is not guarded is the peak moving to a
different step while the reading at 245.5 stays inside 0.05 K. Named in the claim's own
note rather than left to be found.

### The harness is no longer what blocks step 18's headline

> 0.56 points at 0.5 s, **5.57 at 5 s**, 11.14 at 10 s, where the cell ends 19 K hotter
> instead of 1.

Both trajectories now exist and both are exact: the base run gives 0.55719 points and the
`dt = 5` arm gives 5.57188. The sentence still cannot be claimed, and the blocker has moved
one check along. It prints `0.5`, `5` and `10` as **control settings**, and check 6 —
"every number in a claimed sentence is tied to something" — knows only `spelled`, `read at`
and `shown`. A step length is none of the three, so claiming the sentence would mean
leaving three of its numbers tied to nothing (refused, on purpose) or inventing a waiver
(also refused).

That is the `Setting` arm in `docs/plans/path-prose-ledger.md`, and it belongs to whoever
builds the ledger's taxonomy. **This slice's contribution to it is the evidence that it is
now the binding constraint**, which is not what the plan doc predicted — it listed the
harness capabilities as the blocker and the accounting arms as a separate axis.

## What was measured

Nineteen perturbation cases, each launched with a real exit code, each recording *which*
test reddened. `start /belownormal` is not used anywhere in the harness: it is on the
record twice as exit-code-blind, and `docs/plans/surface-vs-bulk.md` records five green
perturbations that were all lies.

| perturbation | reddens |
| --- | --- |
| no perturbation at all — the null | nothing, exit 0 |
| the 17.03 A claim loses its `arm` (so it reads the protected run) | reachable, and the value |
| the 93.29 A claim loses its `arm` — an instant both runs share | **nothing — green** (below) |
| the BMS arm becomes a continuation instead of a restart | the arm fence, reachability and accounting |
| the BMS arm asks for the BMS the step already has | the arm fence, and the value |
| the `dt` arm declares the step's own 0.5 | the arm fence, and the value |
| the `dt` arm declares a 7 its instruction does not spell | the arm fence |
| the BMS arm's instruction reworded by one word | the arm fence |
| the reversed-order arm drops one of its two buttons | **the identity assertion** |
| the reversed-order arm drops `identical_to` | the arm fence — nothing reads `buttons` |
| **Clear queued** made a no-op in the harness | the value, on the repaired pack |
| **Clear latched BMS fault** made a no-op in the harness | the value, on the second tooth |
| **Step 1** made to advance two steps | **nothing — green** (below) |
| the cleared arm stops before the minute it claims | reachable, and the value |
| REVERT THE REPAIR: prose, literal and `spells` all say 13.25 V again | the value, the stated check, and the tolerance rule |
| the peak temperature moved 0.06 K | the value |
| DELETION: the 40.33 A claim removed | literal, accounting, and the tolerance rule |
| DELETION: the 87.02 A claim removed | accounting, and the tolerance rule |
| CONTROL: the peak claim reads at 260 s instead of 245.5 | the value, and accounting |

Two were hand-validated against the failure text rather than trusted from an exit code.
The identity case fails with the two snapshots printed, and they differ in exactly one
field — `external_short_g` is `33.33` on the arm that dropped **Clear queued** and `0.0`
on the one that pressed it. The repair-revert case fails on the *display* half first, with
`path-claims.toml says it prints 13.25 V` against `the page's formatter prints 13.255 V`,
which is the defect stated in the check's own words.

### The two greens

**Dropping `arm` from the 93.29 A claim changes nothing**, and that is the sentence being
right rather than the check being weak. "The trajectory is identical — the same 93.29 A on
the first frame after the fault" says the protected and unprotected packs agree there, so a
claim pointed at either reads the same number. It is worth naming because it means the
`arm` field on that one claim is not *forced* by anything: an author could delete it and
meet a green suite. The same shape as the positive-electrode claim in
`docs/plans/path-probe-row.md` — real coverage that nothing compels. It is named in the
claim's own note.

**Making `Step 1` advance two steps left the suite green**, and that one was a genuine gap.
The claims on those arms read one step past the mark, and an extra row *after* the one they
read changes nothing they look at — so how many steps the button takes was asserted by
nothing at all. Closed the way this file closes every mirror question: `MIRRORED` now pins
`await advance(1);` from `$("stepone").onclick`, so the page cannot change what the button
does without failing here. Verified by perturbing the page to `advance(2)`, which reddens
`mirrored_constants_still_match_the_page` by name.

That is the pin doing what it can rather than what one would want. It catches the *page*
moving; it does not catch this harness's copy moving, which is the standing limitation of
every mirrored formatter in the file and is recorded as such in the module docs.

## Deferred, with a price

* **Ten steps still carry no claim at all**, and two of the four this slice was scoped
  around are still among them. `looks-fine-from-outside` needs a plain continuation (press
  Run again past 500 s to reach 2.502 V at 1058 s) — cheap now, and only left out to keep
  this slice to two steps. `the-electrolyte-starves` needs a 1 C rerun of two different
  scenario files in one sentence, which is a *second pack*, not an arm: `run()` builds one
  pack per trajectory and the sentence compares two models.
* **`what-protection-costs` needs the ambient slider**, which this slice deliberately did
  not build. `docs/plans/path-prose-ledger.md` attributed a −5 °C ambient change to
  `nothing-to-clamp`; it is not in that step's prose at all, it is in step 11's. Building an
  `ambient_c` override with no caller is the shape this file rejects everywhere else, so it
  waits for the step that instructs it — and that step is a CC-CV charge whose completion
  test is frame-dependent, which is a separate unsolved thing.
* **The protected half of `nothing-to-clamp` is still unclaimed.** `OT` at 133.5 s, the
  latch at 156.0 s, 39.62 %, 344.5 K and the 1.3 K of somebody else's temperature. They
  need no arm — they are the step as configured — and they were simply not this slice's
  subject.
* **An arm's `run` length is still this file's own choice**, exactly as a leg's was, and
  there are now six of them rather than two. The non-circular half is unchanged and is now
  stricter (the override anchors above), but a reader following the prose is not guaranteed
  to run as far as an arm does. Read a green arm claim as "true of the trajectory a reader
  following this sentence produces", never as "a reader will get this far".

  Two arms were first written stopping exactly where their furthest claim reads, which
  makes "reachable" mean only "I ran long enough to reach it" — the tautology the module
  docs name, arrived at by accident. Both now run past it (the repaired pack to 160 s
  against a claim at 150.5, the unprotected arm to 420 s against claims at 400), following
  step 20's charge leg, whose note says it runs 26 s further "so the furthest claim is not
  sitting on the final row of the run". Extending changed no value, which is itself worth
  having checked.
* **The event fence now runs per trajectory rather than per step**, because a flag arriving
  on the unprotected arm says nothing about a sentence read on the protected one. Verified
  that this cost no existing sentence its check: of the 69 claimed sentences, **none** has
  claims split across two trajectories, so every one is scanned against exactly the run it
  is read on, exactly as before.
* **A `Step 1` on a windowed CC-CV program is untested.** It is expressed as a `drive` of
  one `dt` rather than as a second stepping loop, precisely so it cannot diverge from a
  `run` of the same length — but no arm exercises it, and the CC-CV window state resets per
  `drive` call, which would matter to an arm that ran a partial window. Named because the
  next CC-CV arm will meet it.
