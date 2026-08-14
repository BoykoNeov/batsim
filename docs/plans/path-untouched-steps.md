# The five steps nothing was about

Of the guided path's 24 lessons, five carried neither a claim nor a ledger entry.
`web/path-claims.toml`'s `unledgered` list named them one line each, which is what kept
them visible; nothing else in the repo touched them. Their prose prints a little over a
hundred numbers between them, and every one of those numbers was free to rot under a
fully green suite — which is not a hypothetical, because that is exactly how six figures
in step 19 went stale and how a contrast in step 14 that was never true survived four
slices.

The five, by the id `web/app.js` gives them:

| step | id | subject |
| --- | --- | --- |
| 9 | `two-legs` | a CC-CV charge, and where the constant-voltage leg begins |
| 12 | `circuit-repeats-itself` | a pulse train on the equivalent circuit |
| 13 | `particle-remembers` | the same train on the single-particle model |
| 14 | `three-times-the-current` | the same train at 3 C, and the floor under each model |
| 16 | `the-electrolyte-starves` | the same discharge on the DFN, dead in half the time |

## This is encoding, not discovery, and the distinction is the point

`docs/plans/path-prose-ledger.md` swept all fourteen unclaimed steps two days before this
one, measured ~145 figures, and found two defects — both repaired there. So the numbers in
these five steps were **known good going in**, and this slice re-measured every one of them
and found no third defect.

That is not an argument for skipping the work. It is the argument for doing it. That sweep
was a temporary module inside `path_claims.rs` that wrote CSVs to a scratch directory and
was deleted, which is the same shape as the four instruments before it: *"every one of those
findings came from an instrument that lived outside the tree and never ran again."* A
measurement ages the moment it is taken. What these steps needed was not another look but a
standing one.

## What went in, and in what order

Three commits, each with a step's worth of claims and whatever the harness was missing to
make them:

1. **Step 16, the first DFN claims.** Ten of them, all on its own pack. No new capability
   at all — the zero-length probe, `flag_first_s:` and the readout mirror already covered
   everything it states. One new quantity, `t_at_v_below:`, for the instant the terminal
   crosses a cut-off.

   *Cost, measured.* One trajectory is built per `(step, arm)` and all ten claims read the
   same rows, so this is one 250-step DFN run: the default `cargo test` gate goes from
   4.0 s to 6.4 s, and release from 0.40 s to 0.57 s. That is the most expensive single
   thing in the file and it is the reason the 1 C rerun the step's last paragraph instructs
   stays out — 1742 steps on each of two models is roughly fifty times this.

   *Two things the perturbation pass turned up.* First, `464` cannot be accounted for as
   "the instant this sentence's claims are read at": `OPERATING_POINT_OUT_OF_WINDOW`
   arrives on that step, and the fence refuses a `read at` accounting at an instant a flag
   arrives. So the number needed a claim of its own, which is what `t_at_v_below:` is for.
   Second, the threshold in that quantity is barely pinned: moving it from 2.5 V to 2.6 V
   changes nothing, because the collapse is 216 mV in one 2 s step and every threshold in
   (2.4218, 2.6381] lands on the same row. What the claim pins is the step the voltage falls
   off on. Written into the claim's note rather than left to be discovered, because a check
   nobody has perturbed is a check nobody knows the strength of.

   *And two stale counts it exposed.* Adding a first claim to a previously unclaimed step
   moves every tally, which is the derived self-counts working as intended — but two
   sentences turned out to state counts nothing derived, each sitting immediately beside a
   derived twin: `2 of 69` grid claims in this test's docs (four, of 134) and `Five steps
   carry neither a claim nor a ledger entry ... the other sixteen` (four and seventeen).
   Both are now tallies. A count next to a derived count is not thereby derived.
2. **Step 9, the first CC-CV taper claims.** Eight of them. One new quantity,
   `cccv_taper_s`, with an invariant under it that decides whether the page's stop is a
   fact about the simulation or a fact about the browser's frame schedule.

   The page's completion test is evaluated at the end of each chopped chunk, and a chunk
   ends at a decision-window boundary *or* wherever the frame's step budget ran out — so in
   general the instant the page stops is somewhere between the crossing and the next
   boundary, and which is a fact about the browser. That is why `drive` did not model it and
   why no claim had ever read past a taper. It is claimable on this step for a reason
   particular to this trajectory: the crossing lands **on** a boundary (6210 s is step 12420
   at `dt = 0.5`, and 20 steps is the window), a chunk never crosses one, so the test is
   certainly evaluated there and the step before is still over the taper. The quantity
   asserts that property and **panics rather than answering** anywhere else. Perturbing the
   page's taper from 0.15 A to 0.152 moves the crossing to 6205 s, off the grid, and it
   refuses — which is the perturbation that makes the invariant a fact rather than a
   paragraph.

   ### Where the deletion pass corrected me

   The right way to ask whether a claim is load-bearing is to delete the whole claim and
   report *every* test that reddens. Replacing its `literal` with a placeholder — the first
   thing tried here — also reddens the literal check, so "check 6 caught it" cannot be told
   apart from "check 1 caught the placeholder". With whole blocks removed and the
   claim-counting tallies set aside, the answer is:

   | deleted claim | what notices |
   | --- | --- |
   | step 16's `t_at_v_below:2.5` | `every_claim_matches_the_engine` — the flag fence, **not** check 6 |
   | step 9's `cccv_taper_s` | **nothing** |
   | step 16's collapse voltage | check 6 |
   | step 9's charge at the end of the CC leg | check 6 |

   Two corrections fall out. The flag fence lives inside the engine test, because that is
   the only place a trajectory exists — a flag's arrival is not knowable from the prose —
   so deleting step 16's instant claim reddens *that*, and check 6 stays green on its own.
   And step 9's stop claim is required by nothing: `6210` falls straight back to being the
   instant the charge-level claim beside it is read at, because no flag arrives there to
   make the fence refuse. The `read at` accounting reaches every instant the run is quiet
   at, and only an event forces a sentence's moment to be claimed. The sentence says the
   charge *stops* at 6210, which is a great deal more than "we measured then", and the only
   thing making that the checked statement is the claim being there.
3. **Steps 12 and 13, the pulse decomposition.** Twenty claims and five new quantities —
   `pulse_sag_mv`, `pulse_jump_mv`, `pulse_rebound_mv`, `pulse_lost_mv` and
   `pulse_rebound_arrived`, each taking a tooth number — all keyed off the leg boundaries
   the `Pulse` program already defines, and all resting on `Row::rest_v`.

   **This is where the third defect turned up.** Step 13's tooth decomposition says
   *"135.7 mV of sag: 113.9 mV back instantly, 17.3 mV climbing, and 4.5 mV that has not
   returned"*. The engine says the last part is **4.559 mV**, which rounds to 4.6. The
   reason is visible in the sentence's own arithmetic: 135.7 − 113.9 − 17.3 is exactly 4.5,
   so that figure was subtracted from the three *rounded* parts instead of being read off
   the run. The unrounded parts do add up — 135.723 − 113.895 − 17.269 = 4.559 — and the
   sibling sentence on step 12 shows what a measured decomposition looks like: its four
   parts round to 212.8, 132.8, 74.8 and 5.3, which do *not* add up (132.8 + 74.8 + 5.3 =
   212.9), precisely because each was taken off the engine separately. Prose corrected
   to 4.6. A number that makes its sentence's arithmetic come out exactly right is worth a
   second look; the one that doesn't was the honest one.

   Two smaller shaping facts. `I·R0` in step 12's prose contains the numeral `0`, which
   nothing can account for, so that decomposition is claimed as two literals rather than
   one — and both carry a claim on the 74.8 mV rebound, which is the sanctioned cost of
   splitting a sentence. And `pulse_rebound_arrived` is stored as a fraction rather than a
   percentage so that `states = complement` does the work for step 13's *"8 % arrives in
   its final five minutes"*: it is the same measurement as "92 % had already arrived", and
   `complement` is how this file already spells the other side of a number.

   The perturbation that matters here replaces the zero-length `Rest` read with the stepped
   row's own terminal voltage — the reading a naive harness takes — and the suite reddens.
   Without that, the whole `rest_v` mechanism would be decoration.

4. **Step 14's own-pack half.** Six claims and one arm — and a change to the number
   scanner, which is the finding this commit is really about.

   `11 880 s` was **two numbers** to `written_numbers`: `11` and `880`. Nothing could spell
   either, no accounting arm could tie either to anything, and so any sentence containing
   one was unclaimable. That was silent, and it had been shaping the file for seven slices.
   The lesson prose contains exactly four space-separated numbers — `10 000`, `11 280`,
   `11 880`, `200 000` — and **not one of them appeared in any claimed literal** before this
   commit. Authors met the blocker, wrote a shorter literal, and moved on; nothing recorded
   why.

   `join_thousands` fixes it, narrowly. A group joins only when the separator is exactly one
   ASCII space, the group is exactly three digits, and neither side carries a decimal point,
   so `at 2 s, 464 s` and `11 880.5` are untouched. The narrowness is the point and it is
   what `the_scanner_joins_thousands_groups_and_nothing_else` asserts: joining two numbers a
   sentence wrote separately would make check 6 demand an accounting for a figure nobody
   printed, which no author could ever satisfy. Reverting the rule reddens check 6 by name,
   which is what shows the `11 880` claim really depends on it.

   The joined token keeps its space, so `spells = "11 880"` stays "written exactly as the
   sentence writes it"; everything that turns a token into a number now goes through one
   `number_of` helper that strips separators.

   The arm is a plain "keep pressing Run" continuation to 13 920 s — eight marks' worth
   past where the page stops — and it carries the floor claims. *"It pins at 0.3095 V and
   stays there"* is asserted as two claims at instants one whole tooth apart, both reading
   0.309467 V bit for bit, on the same principle as step 11's "at the same instant": two
   pins are what an equality looks like when the instrument cannot say "equal".

## Where this leaves the file

Every one of the twenty-one steps in `[ledger].unledgered` now carries at least one measured
claim. Two steps in the repo still carry none, and they are both **ledgered** ones — scanned
end to end for numerals and measured nowhere, which is the opposite gap and a smaller one.

Two tally phrases had to be reworded during this arc because their counts were heading for
zero.

### Three things a review caught that nothing here could

Named rather than quietly fixed, because two of them are about the shape of this arc's own
checks.

* **`join_thousands` had a false-join the unit test did not cover.** The gap between two
  digit runs was measured from the *untrimmed* run, which on `at 5769. 880 s` covers the
  full stop — so the gap landed on the space and joined `5769` to `880`, a figure the
  sentence never printed. The test's must-not-join list had a decimal point on the right
  (`11 880.5`) and none on the left. Latent, not live: the suite was green with it in.
  The fence is now measured from the trimmed token's own end, and the case is in the test.
* **The plan doc said "Nineteen claims" where the derived count says twenty** — a
  hand-maintained number going stale inside the arc whose immediately preceding commit was
  about freezing plan-doc numbers so they cannot rot. Corrected. The lesson is the one that
  commit already drew and this one re-earned: a count in a plan doc is not derived, and
  writing one is choosing to maintain it.
* **The corrected 4.5 mV was stated in a second place.** `scenarios/pulse_train_spm.toml`'s
  header describes its own tooth at length and repeated the figure, unchecked, under the
  provenance rule. Corrected there too, with the reason. There is precedent for exactly
  this — the `60.8 %` figure lives in a `ccCvDone` doc comment nothing checks — and the
  general point is that a prose fix has to be a repo-wide grep and not a file-wide one.

### Deferred, with a price (added by this arc)

* **The ledger scan is the one consumer of `written_numbers` the new unit test does not
  reach.** `cover_by_rule` matches tokens against scenario and chemistry field values, and
  none of the three ledgered steps contains a spaced thousands group, so the joined-token
  path is untested there. It fails toward red — a token carrying a space will not match a
  file number formatted without one — which is the safe direction, but the first author to
  ledger `wearing-out-while-idle` will meet it, because that step's prose carries both
  `10 000` and `200 000`. `"{W} steps carry neither a claim nor a ledger entry"` breaks at one and reads as
nonsense at zero — which is precisely the moment the work it describes is finished. A tally
that cannot survive its own count reaching zero forces a rewrite at the worst possible time,
so both now put the count last, and `HEADER_WORDS` learned to say "none".

## The measurement, and the one thing it settled

Every figure was re-derived before any of it was written down, with a temporary module in
`path_claims.rs` reusing `lessons()` / `build` / `demand_now` / `pulse_on` — the recipe
`path-prose-ledger.md` records as the one that worked — writing one CSV per trajectory to
`M:\claud_projects\temp\untouched-steps`.

The thing it settled is the pulse decomposition, which is what had steps 12 and 13 blocked.
Their prose breaks one tooth into three parts:

> 212.8 mV of sag, of which 132.8 mV returns the instant the current stops (`I·R0`, pure
> resistance), 74.8 mV climbs back slowly (the RC pairs), and the last 5.3 mV never returns
> at all

Read off the stepped rows a reader actually sees, the rebound is **71.8 mV**, not 74.8. The
first rest sample is at `t = 60.5` and is already half a second into the relaxation, so
every one of these numbers comes out slightly low — which looks exactly like plausible
drift, and is the trap `path-prose-ledger.md` names. Take a **zero-length `Rest` read** at
`t = 60` instead and all ten figures across the two steps reproduce to the digit:

| | circuit (step 12) | particle (step 13) |
| --- | --- | --- |
| sag | 212.815 mV → `212.8` | 135.723 mV → `135.7` |
| back instantly | 132.776 mV → `132.8` | 113.895 mV → `113.9` |
| climbs back | 74.767 mV → `74.8` | 17.269 mV → `17.3` |
| never returns | 5.272 mV → `5.3` | 4.559 mV → `4.5` |

So the quantity is defined against a zero-length read at the leg boundary, and
`Pack::step(0.0, Demand::Rest, ..)` is what takes it. That is the same instrument the
pre-run probe uses and it is sound for the same reason — `a_zero_length_probe_moves_nothing`
already asserts the engine's `dt = 0` contract — but it is a probe the *page* does not take
on its own. See "Deferred, with a price".

## Deferred, with a price

* **Everything cross-pack.** `run()` builds one pack per arm, so a sentence comparing two
  scenario files cannot be claimed. That leaves out step 16's readings of its twin
  (`3.918`, `3.471`, `3.437`, `6.33 W`) and its 1 C boundary, and step 14's circuit figures
  (`132.8 → 397.3`, `74.8 → 224.3`, `×2.99`, `×3.00`, `1.84 V`, `−0.45 V`, `−0.66 V`) —
  every one of which was measured here and is correct. Step 14's prose does instruct the
  swap ("Load `pulse_train_ecm` and do the same thing"), so the arm has a sentence waiting
  for it; what it needs is a `scenario` override on [`Arm`].
* **Everything cross-step.** Step 14's ratios (`×1.87`, `×6.01`, `×2.48`) divide its own 3 C
  figures by step 13's 1 C ones. Both packs exist in this file; nothing lets a claim on one
  step read a run built for another, and widening that is a larger change than a scenario
  override because it makes a claim's trajectory no longer a function of its own step.
* **The two accounting arms are still missing**, so several literals stop short of a
  fragment rather than covering their whole sentence. `Chemistry` would account step 16's
  `2.50 V cut-off` (it is `v_min` in `nmc_21700_lgm50.toml`) and step 13's `9 s` and `72 s`
  RC time constants; `Derived` would account step 9's *"13 % of the time for 5 % of the
  charge"* and the `212.8 = 132.8 + 74.8 + 5.3` identity in step 12. Each dodge is recorded
  in its claim's own note, because a literal that stops short reads as a covered sentence.
* **Three perf ratios no trajectory can settle.** Step 14's "about ten times the circuit's
  arithmetic" and step 16's `140×` and `500×` are deliberately ratios rather than durations,
  for the reason step 14's own prose gives. No benchmark ran here.
* **A flag the prose does not mention.** Both step 16 and step 14 raise
  `OPERATING_POINT_OUT_OF_WINDOW` at instants their sentences describe without naming it —
  464 s on the DFN, 11 280 s on the particle. Not a defect: the flag says the demand landed
  outside the window the pack can serve, which is what both sentences are about in words.
  Recorded because the sibling step `looks-fine-from-outside` had the same discovery and it
  is worth knowing that "no flags" claims on these steps are about their own marks.
* **A mid-run zero-length read is an instrument the page offers only sideways.** The page
  takes a `dt = 0` probe when the demand-mode dropdown changes (`$("demand-mode").onchange`),
  so a reader who pauses at a leg boundary and switches the mode to `Rest` sees exactly the
  number these claims are measured against — but pausing *at* `t = 60` is not something the
  page makes easy, and the prose does not ask for it. These claims are therefore about the
  model rather than about a panel: they name no `display`, on the same terms as
  `delivered_ah`, which has no row either.
