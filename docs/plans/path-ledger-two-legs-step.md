# The charge, ledgered — and a sentence that counted the path wrong

`two-legs` is step 9: a CC-CV charge on an empty NMC cell, the one step where the
constant-voltage leg actually engages. It is the twelfth of twenty-four steps scanned whole,
and the first whose subject is the **page's own policy** rather than the engine's behaviour.
`CC-CV` is not a demand `sim-core` has — it is two of them with a rule between, which is where
`CLAUDE.md` puts a charge policy — so its numbers are the policy's: the current it asks for,
the voltage it aims at, the current it stops below, and how often the rule may fire at all.

It cost one new tie, one new control, one new quantity, one new row field, one new claim, five
instant tags, one number off the page and one sentence corrected.

## What was open, and who closed it

The step's own note in `path-claims.toml` had listed what it left unchecked. Reading it before
starting was worth more than the work it saved:

| the note said | true when written | true on 2026-08-17 |
| --- | --- | --- |
| the headline needs the `Derived` accounting arm, "and it does not exist" | yes | **no** — built 2026-08-15, two slices later |
| "the third slice running to be blocked on that arm" | yes | no |
| the cutoff, the starting charge and the decision window have no arms | yes | two of three were reachable |

**A note about the code is a claim, and nothing in this repo checks one.** The `Derived`
sentence sat false for five days through every green run, because no check reads a sentence
*about* the tree. That is the fourth instance this project has recorded; the previous three are
in `path-self-description-sweep.md` and the commit that repaired `Accounted::Setting`'s
paragraph.

And the arm it named turned out not to be the one the headline needed. `Tie::Derived` reads a
sentence's **own printed siblings**, and this sentence prints its answer two paragraphs from
its operands:

> **The last leg is 13 % of the time for 5 % of the charge**

Neither `6210` nor `5420` nor `95.3` is anywhere in it. So the headline is the *other* family —
`Tie::Ratio` over `Tie::Difference` over `Tie::Quoted`, the arms that read files and claims —
and the note's five-day-stale advice would have sent the next author down the wrong branch.

## The instant the leg changes was measurable and unmeasured

`5420 s` was accounted for, before this slice, only as *the instant three other claims are read
at*. Every one of them — the current, the voltage, the charge — would still be true if the
controller had gone on asking for a constant current for another hour. Nothing said the leg
changed there, which is the sentence's actual subject.

So the slice adds `cccv_cc_ends_s`, and with it `Row::voltage_hold`: the demand the controller
issued, recorded rather than inferred. `|i| < the box` would have been the easy measurement and
a different statement — it is also true of a BMS derate, of a clamped pack, of a protection
trip. Read off the demand, the quantity says what the prose says.

**It is better behaved than the taper instant two sentences later, and the contrast is the
point.** `cccv_taper_s` is about when the page *stops*, which `ccCvDone` decides at the end of a
chopped chunk — a fact about the browser, except where the crossing lands on a decision-window
boundary, which is why that quantity asserts it does and refuses to answer otherwise. This is
about when the page changes *leg*, which `ccCvDemand` decides on the decision grid and nowhere
else. `advance` holds one demand across a whole window, so the boundary is a multiple of 10 s by
construction and no invariant is needed.

**The harness was once wrong about this instant and nothing could tell.** Before `drive`
chopped its steps at the page's windows it decided the demand per step, and the leg changed a
step early — `cccv_cc_ends_s` reads 5419.5 with the window forced off, against the 5420.0 the
page has. It was invisible because the only *claimed* CC-CV step then was
`leg-that-is-not-there`, whose LFP cell never reaches the band at all and is on a constant
current under either rule; this step's own claims arrived nine hours after the fix.

**And the claim as first written would not have caught it.** `tol_from = "spelled"` gives half
a unit in the last printed place — 0.5 s — and this quantity moves in whole 0.5 s steps, so a
boundary one step out sits *exactly* on the tolerance and passes. Measured: at 0.5 the value
check goes green and the run is caught two claims later by a display check on the `terminal`
row, which is a different sentence being wrong about a different thing. Half a step (0.25) is
the tightest meaningful bound for a grid time — what `grid` would give if the sentence spelled
no number, declared as `tighter` because it does. **A claim can be right, green, and
decorative against the one failure it was written for**, and the only way to find out is to
break the thing and watch which assertion fires.

## `Tie::Page`: the arm that reads the client

"The rule is checked every **10 s** of *simulation* time" is a number in no scenario, no
chemistry and no lesson block. It is `CCCV_PERIOD_S` in `web/app.js`, and the sentence's whole
point is that the page keeps a grid of its own.

The arm parses it, on `default_dt`'s terms — that function has read the page's default timestep
out of the markup since this file was written, "so a change to the page's default moves this
test with it". Widen the window to 30 s and the sentence goes red on sight.

**It also closed a hole the file complains about in four places.** `CCCV_PERIOD_S` had sat in
`MIRRORED` since day one with *nothing reading it* — the "pinned, and consulted by nothing"
shape cited as a cautionary example by `WORD_NUMERALS`, by `Op`, by `Tie::Hours` and by two
rule-usage checks — while `cccv_window_steps` carried its own copy of the `10`. Both halves are
kept and they now say different things: the pin says the page still *spells* the constant the
way the parser looks for it, the parser says what the number *is*. Three doc paragraphs that
used it as a present-tense example were fixed with it; the ones written in the past tense
("sat pinned for six slices, and the mirror it guarded was wrong the whole time") are still
true and still worth citing.

## Two numbers with no honest arm, handled two ways

**`1×` left the page.** "The same experiment would take a different path at 1× and at 800×" —
the `800` is the speed slider, and the `1` is the identity, the way a reader says "real time".
No file decides it. On the terms step 16 set when five of its numbers left, the sentence now
reads *"a different path in real time than at 800×"*: same statement, one number instead of two.

**The `5 %` denominator is only half recovered from the token, and it is worth being exact
about which half.** The charge added in the second leg is 4.24 points. Over the charge this run
actually put in (99.52 − 20) that is **5.33 %**; over a full charge from this start (100 − 20)
it is **5.30 %**; over the cell's whole capacity it is **4.24 %**. The third prints `4`, so the
token does rule that reading out — the sentence is about a fraction of the charging, not of the
cell. The first two both print `5` and nothing here separates them. The measured pair is the one
the arm can express and the one the sentence means, and the rule's comment says exactly this:
the green proves the arithmetic rounds to what the prose prints, and proves one rival wrong out
of two.

**The two denominators are deliberately not parallel**, which looks like a defect until stated:
the time is measured from t = 0, the charge from the 20 % the pack started at. A charge fraction
over the whole 100 % would count 20 points the reader never put in; a time fraction from a
non-zero origin would be meaningless.

**Known granularity.** Both tokens commit to one digit, so `13` survives anything in
[12.5, 13.5) — about 50 s of movement in the leg boundary — and `5` anything in [4.5, 5.5). The
arms are as tight as the prose is, which is the rule `tol_from = "spelled"` already keeps on the
claims side. Tightening past the sentence's own precision would be inventing a claim the page
does not make.

## The measurement, in one table

Everything the two headline arms rest on, measured on this step's own trajectory:

| quantity | value |
| --- | --- |
| constant-current leg ends | 5420.0 s (`90m` on the panel) |
| first voltage-hold step | 5420.5 s |
| charge there | 95.277778 % |
| taper crossed, page stops | 6210 s |
| charge there | 99.519515 % |
| last leg as a fraction of the run | 790 / 6210 = **12.72 %** → `13` |
| last leg as a fraction of the charge added | 4.2417 / 79.5195 = **5.33 %** → `5` |

## The word numeral that was wrong

The step opened:

> Eight steps of taking charge out; this one puts it back.

`two-legs` is the ninth lesson, so eight steps do precede it — but the eighth is
`wearing-out-while-idle`, whose demand is `Rest` and whose own prose says "no current flows
anywhere". **Seven** take charge out. The sentence now says so.

Nothing in this repo could have found it. `written_numbers` scans digits, so a quantity spelled
in letters is invisible to the ledger; check 6 never sees it because no claim quotes that
sentence; and the tallies only check counts someone has declared in `TALLIES`. It was found by
hand, because ledgering a step means reading its prose, and the word `Eight` was sitting in the
first sentence.

That is the second class of defect a word numeral has produced here — the first was nine stale
self-counts, four of them in words — and it is the argument for the word-numeral slice being one
piece of work and not three: a vocabulary that can see "half a point" can see "eight steps".

## Perturbations

Registered before running, and every one of them reddened as predicted. The mis-pointings are
what the design turns on — an arm that names the wrong field must fail *on sight*, because the
number is never declared:

| change | outcome |
| --- | --- |
| the taper rule reads `DemandValue` (1.5) instead of `Taper` (0.15) | red, ledger scan |
| the starting-charge rule reads `initial_temp_k` (298.15) instead of `initial_soc` | red, ledger scan |
| the ordinal names `bare-curve` (position 1) instead of step 2 | red, ledger scan |
| the prose says the rule is checked every 20 s where the page says 10 | red, ledger scan |
| the `cccv_cc_ends_s` claim deleted | red — the 13 % rule resolves to no number, **and** the claim count |
| `drive`'s decision window forced off (the pre-2026-08-14 harness) | red — `i_at:5420` reads −1.5552 A, and `cccv_cc_ends_s` reads 5419.5 |
| the 13 % ratio's operands reversed (6210 / 790 = 786 %) | red, ledger scan |
| both charge readings untagged, the rules quoting bare `soc_at` | red — the quote is refused as ambiguous |

Two are worth keeping. The **deletion** is what proves the new claim is load-bearing rather than
decorative — `path-probe-row.md` records a case where one claim of four turned out not to be
required, and this is the same question asked and answered the other way. The **untagging** is
what proves the five instant tags are not bookkeeping: a step that files two readings under one
name is unquotable, and the fence refuses rather than picking by file order.

What the perturbations do **not** reach: none of them perturbs a shared input. Moving the page's
taper makes `cccv_taper_s` *panic* on its window-boundary invariant before any new rule is
consulted, and moving `initial_soc` or `CCCV_PERIOD_S` moves the trajectory — so a red would be
some other check reddening, which this project has twice mistaken for a passing test of the
thing it was actually changing.

## What is still not checked here

* **`Tie::Ordinal` pins the step number, not the sameness.** "The same NMC cell as step 2"
  claims two scenarios name one chemistry; the arm checks only that
  `same-discharge-other-chemistry` is still second in the path. No arm in this taxonomy compares
  two scenarios' chemistries, and building one for a single sentence was not worth it — stated
  rather than left for a reader to assume.
* **`cc_cv_charge_nmc.toml`'s header is an unguarded second copy** of half this step's numbers:
  5420, 3.66 → 4.20, 95.3, 13 %, 5 %, "90 minutes", "13 more", "the last 4 points". Checked by
  hand against the measurements above and it agrees. Nothing scans scenario headers, and the
  shape — one figure asserted in two files with no comparison between them — is exactly what
  produced the defect `path-ledger-last-two-steps.md` found, where a ratio was asserted three
  times over and was wrong in both files.
* **The page's completion test is still frame-dependent**, and no claim reads past a taper
  except the one this step is entitled to, for the reason its own note gives.
* **`Row::voltage_hold` is `false` on the probe row**, which is a true statement (a probe is
  taken before the run is armed) but not an interesting one. Nothing reads it there.

## What the ledger looks like now

Twelve of twenty-four steps scanned whole, 201 numerals, eighteen arms. Twelve steps left, all
of them carrying claims on their claimed sentences and none of them scanned end to end.

The cheapest remaining, by the measured table in `path-self-description-sweep.md` (which counts
*unaccounted* numerals, not numerals): `bare-curve` and `nothing-to-clamp` at 12 each, then
`protection-off` at 15. The expensive tail is `the-gradient-itself` at 38 and the two 35s. That
table was measured before three of the arms below it existed, so it now over-states every count
by whatever those arms reach — it is a ranking, not a budget.
