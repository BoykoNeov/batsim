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
   arrives on that step, and the `ReadAt` fence refuses an accounting at an instant a flag
   arrives. So the number needed a claim of its own, which is what `t_at_v_below:` is for —
   and deleting that claim reddens check 6 by name. Second, the threshold in that quantity
   is barely pinned: moving it from 2.5 V to 2.6 V changes nothing, because the collapse is
   216 mV in one 2 s step and every threshold in (2.4218, 2.6381] lands on the same row.
   What the claim pins is the step the voltage falls off on. Written into the claim's note
   rather than left to be discovered, because a check nobody has perturbed is a check
   nobody knows the strength of.

   *And two stale counts it exposed.* Adding a first claim to a previously unclaimed step
   moves every tally, which is the derived self-counts working as intended — but two
   sentences turned out to state counts nothing derived, each sitting immediately beside a
   derived twin: `2 of 69` grid claims in this test's docs (four, of 134) and `Five steps
   carry neither a claim nor a ledger entry ... the other sixteen` (four and seventeen).
   Both are now tallies. A count next to a derived count is not thereby derived.
2. **Step 9, the first CC-CV taper claims.** One new quantity, `cccv_taper_s`, with an
   invariant under it that decides whether the page's stop is a fact about the simulation
   or a fact about the browser's frame schedule.
3. **Steps 12, 13 and 14's own-pack half.** One new family of quantities for a pulse
   train's decomposition, all keyed off the leg boundaries the `Pulse` program already
   defines.

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
