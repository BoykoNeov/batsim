# The zero-length probe row

> **A harness that only samples stepped rows cannot reproduce a number the page's probe
> produced**, and the failure is silent and one-sided — every affected number reads
> slightly *low*, which is exactly what a plausible drift looks like.
>
> — `docs/plans/path-prose-ledger.md`, which found six such numbers and could not check
> any of them.

Three of the guided path's twenty-four steps quote a reading taken **before the reader
presses Run**, and four more decompose a pulse with one. `crates/sim-data/tests/path_claims.rs`
drove each step by stepping the engine, so it had no such reading to offer and those
numbers were unclaimable. This slice gives it one, and closes step 18's opening sentence
with it.

## What the probe is

`applyStep` in `web/app.js` dials in a step's controls and then awaits `readNow()`, which
steps the engine by `dt = 0` under that step's own demand and paints the result. It is the
engine's own answer to "what is this pack doing now" — the same contract
`docs/plans/phase-7-dfn.md` records `probe_at` answering for `dt <= 0`. For a `Pulse` step
the demand is the leg the pack's own clock is on, not `Rest`, which matters: step 18 is a
`Pulse` and its probe is taken under 15.459594 A.

**It mutates nothing, and that was measured rather than assumed.** `phase-3-aging-faults.md`
records a case where end-of-step temperature had no `dt > 0` gate and a zero-length probe
on a pack did move something, so the doc comment was not taken at its word. Probed twice on
step 18's SPM, on `belief-drifts`'s BMS pack and on step 12's circuit, the second telemetry
is bit-identical to the first and the serialised snapshot is unchanged across both.

That is what makes the page's **two** probes on a reloading step reproducible by modelling
only the second: `loadScenario` takes one under whatever the demand box still holds, then
`applyStep` takes this one under the step's own demand. If a probe moved the pack, both
would have to be reproduced and in order.

## Why it is not a row in `rows`

The plan doc that asked for this called it "a zero-length probe **row**", and building it
that way would have been wrong twice over. `Run` carries it as a separate field and a claim
*declares* it reads the probe, the way `after_mark` is declared rather than inferred from
`read_at_s > until_s`.

**Time cannot name it.** `Run::row_at` addresses rows by nearest end-of-step time. A probe
shares its instant with a stepped row exactly — at the start of a run with the first step's
`t = dt` beside it, and (were probes ever taken mid-run) with the step that ends at the
same instant. The two differ by precisely the thing the probe exists to measure: one step
of relaxation, which on the pulse steps is 2.9 mV out of 74.8. An addressing scheme that
cannot tell them apart hands an author whichever was stored first.

**It would have moved a claim that nothing else touched.** `first_flag`,
`flags_arriving_at`, `delivered_ah`, `deficit_zero_s` and `soc_gap_pts_min` all fold over
`rows`. The casualty was predicted and then measured: on `belief-drifts` the probe reads a
BMS estimator gap of **3.000000000000025** points against that run's minimum of
**3.0182** — a probe has no step to lag the truth by, and `path-estimator-gap.md` had
already recorded that one-step lag as the reason the gap does not start at the exact 3.0000
the scenario hands it. A probe row would therefore have stolen last slice's minimum claim
and moved its recorded instant from 0.5 s to 0.0, with no prose changing anywhere. The
perturbation table below contains that exact case, and the failure message it produces is
the claim's own.

## What went in

* **`Run::probe`**, a `Row` taken before the first step under the lesson's own demand.
* **`Claim::probe`**, opt-in, with three fences in
  `every_probe_claim_is_taken_before_the_run`: `read_at_s` must be 0, the claim may not
  also be `after_mark`, and the step must set `reload: true`. The last is the one worth
  naming — `run()` always builds a fresh pack, which a stepped trajectory mostly absorbs,
  but the probe *is* the fresh pack. On a step that inherits its pack from whichever step
  the reader arrived from, there is no fresh reading to claim. `applyStep` also reloads
  when the scenario file differs, but that depends on where the reader came from and is not
  knowable here, so the flag is the conservative half.
* **A fourth fence in `measure`**, which is split into `measure_row` (anything readable off
  one row) and the reductions. A probe is one row with no history, so a claim asking for a
  flag's arrival or an amp-hour total off it is refused rather than quietly answered from
  the stepped rows behind its back. A quantity added to `measure_row` becomes
  probe-readable automatically; one added to the reductions stays refused, with a message
  naming itself.
* **The `surface gap` readout row, now mirrored.** It sat beside `past empty` in the "not
  mirrored" list, and only one of the two belonged there: `past empty` is sampled on a
  250 ms *wall*-clock throttle, so "what does that row show at simulation time t" has no
  answer, and `surface gap` carries no throttle. `Row` now carries the pair and
  `render_row` prints it, which is what lets step 18's headline be a display claim.

## Step 18's opening sentence, closed

> Both numbers read **0.00 / 0.00** before you press Run, beside 3.927 V and 100.0 %.

Four claims, all `probe = true`, all reading at t = 0. Every number in the sentence is
claimed, which is what the accounting check wants of a claimed sentence.

**The zero is not a hard zero, and the page knows it.** The negative electrode's gap reads
`-1.11e-16` on a particle nobody has asked for a current: the bulk side of the difference
goes through a volume-weighted mean that sums and divides while the surface side returns
the outermost shell untouched. `toFixed` on that prints `-0.00`. `gapPts` in `web/app.js`
has a guard that spells it `0.00`, with a doc comment saying why — and this slice is the
first thing that ever depended on it. Its sibling, the positive electrode, reads an exact
`0.0`: the asymmetry is real and the guard is what keeps a reader from seeing it as a
direction.

## What was measured

Thirteen perturbation cases, each launched with a real exit code and each recording *which*
test reddened rather than only that something did.

| perturbation | reddens |
| --- | --- |
| no perturbation at all — the null | nothing, exit 0 |
| `probe = true` dropped from the surface-gap claim | value — the engine reads **1.35 points**, not 0 |
| the page's negative-zero guard removed | the mirror table (see below) |
| the probe pushed into `rows` as row zero | **the estimator-gap minimum**, by name |
| the negative gap's value moved 0.01 points | value and stated |
| prose, literal and `spells` all say 3.928 V | stated (and the display half, which quotes the panel) |
| prose and literal say 3.928, `spells` left at 3.927 | tolerance, literal, accounting |
| the surface-gap row's `shows` says `0.01 / 0.00` | display |
| `probe = true` on a whole-run reduction | the probe fence, and `measure`'s refusal |
| a probe claim given a `read_at_s` of 100 | the probe fence |
| `reload: true` taken off step 18 | the probe fence |
| CONTROL: a surface-gap claim moved to a circuit step | the no-electrodes refusal |
| CONTROL: the `soc (true)` probe claim reads 99.9 % | value and stated |

Two are worth reading twice, and both were hand-validated against the failure text rather
than trusted from an exit code — `docs/plans/surface-vs-bulk.md` records a harness that
reported five green perturbations that were all lies.

**Dropping `probe = true` moves the surface gap from 0 to 1.35 points**, not by a rounding.
Two seconds into a 3 C discharge the gradient has already stood up, so the first stepped
row is not a slightly-stale version of the probe — it is a different measurement. This is
the case that says the probe channel is doing work rather than decorating one.

**The probe-as-a-row case fails on the claim it was predicted to**, with the message
`the smallest BMS gap on this run is 3.000000000000025 points at t = 0 s, and the claim
reads at t = 0.5 s`. That assertion was written one slice ago for an unrelated reason — a
reduction with a decorative instant — and it is what caught this.

## Found on the way: a hand mirror is only as good as what ties it to the page

Deleting the negative-zero guard from `web/app.js` left the whole suite **green**. The
display check renders through `fmt_gap_pts`, a hand copy of `gapPts`, so the mirror went on
printing what the page no longer printed — and the claim's own note said, in writing, that
the display half was "what actually holds the page to it". It was false when written and
the perturbation is what showed it.

The mechanism to fix it already existed and had simply not been extended: `MIRRORED` pins
one source line of `web/app.js` per line `render_row` reimplements, and the `surface gap`
row's line was in it only as an example of a row *not* mirrored. Four rows now stand there
— the row itself, its circuit placeholder, `isPorous`, and the guard — and the perturbation
reddens `mirrored_constants_still_match_the_page`.

Two general lessons, and the second is the one that generalises:

* A green perturbation on a display check is not evidence the display is checked. It can be
  evidence that both sides of the comparison are yours.
* **The claim's note asserted the fence that was missing.** A note is authoring context and
  nothing checks it, which is recorded elsewhere in this file as a known limitation — this
  is the first time that limitation cost something, and it cost a false statement sitting
  inside the very claim it was wrong about.

## Deferred, with a price

* **The pulse decomposition is blocked on reachability, not on the probe.** The 74.8 /
  132.8 / 5.3 mV family on `circuit-repeats-itself`, and the 17.3 → 37.2 mV rebounds on
  `particle-remembers`, are engine-exact from a probe at the instant the current stops.
  Whether a *reader* can take that probe is a separate question and is now the blocker:
  `readNow()` fires on load, on reset, on a demand-mode change and on entering a path step
  — **not on pause**. A reader who pauses at t = 60 s sees the last stepped row, 2.9 mV
  short. There is a path to it (pause on the instant, toggle the demand mode away and back;
  a `Pulse` probe reads its leg off `sim_time_s`, so it would land on the rest leg and give
  the right number) and calling that "reachable" is a stretch. The honest outcome may be a
  corrected sentence rather than a green claim, and this repo has shipped
  "right but unreachable" twice. Do not write those claims before answering it.
* **Step 16's three-model probe sentence needs three packs.** "3.798 V for the circuit,
  3.927 for the particle, 2.808 here" quotes probes from three different scenario files in
  one sentence, and `run()` builds one pack per step. The 3.927 is claimed here because it
  is read on its own step; the other two are not.
* **The `surface gap` display claim is one row, two numbers.** The positive-electrode claim
  names no `display`, because its sibling names the row and the row prints both — two
  display claims on one row would assert the same string twice. That is right, and it means
  the positive number's rendering is checked only through a string that also contains the
  negative's.
* **Ten of the twenty-four steps remain wholly unchecked**, and what they still need is
  unchanged: instructed continuations and control changes (a BMS toggle, two step lengths,
  an ambient change, `Clear queued`, `Clear latched BMS fault`, "press Run again"), and a
  claim each. `[ledger].unledgered` names them one line apiece.
* **`the-gradient-itself` is claimed, not ledgered.** Four numbers out of the step's many
  are tied to the engine. The whole-prose scan is a separate contract and this step's
  remaining figures are measurements rather than file constants, which is what the ledger
  has no arms for yet.
