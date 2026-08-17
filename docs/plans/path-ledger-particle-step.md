# The particle step, scanned whole, and two fences that were refusing the wrong thing

`docs/plans/path-ledger-last-two-steps.md` closed by naming the next targets: *"Sixteen of
twenty-four steps are still claims-only, and the two dense DFN/SPM steps (13, 16) are the
next expensive ones: their numbers are nearly all measurements, so ledgering them is a
claim-writing slice rather than an arm-building one."*

**Half of that is wrong, and measuring the two steps first is what showed it.** Step 13
(`particle-remembers`) is not a claim-writing slice at all: it printed twenty numerals when
this was written — the ledger entry says **nineteen**, because the last row of the table
below is a numeral this slice reworded out of the prose — ten
of them are already claimed, and of the ten that are not, **none is a measurement**. They
are the scenario's initial charge, its shell count, two digit runs inside the chemistry's
own provenance string, two RC time constants the chemistry file's own arithmetic gives,
and three numbers **the step next door measured**. It cost no new claim and one prose
reword.

Step 16 (`the-electrolyte-starves`) is the expensive one, it is expensive for reasons that
are not claim-writing either, and it is **not ledgered here**. The measurement and the
blocker are at the end of this document.

## Step 13, token by token

Twenty numerals as found, nineteen as shipped. Ten sit inside a claim's literal already — the five rebounds, the
four-part decomposition of the first tooth, and the 8 % tail. The other ten:

| token | what decides it | arm |
| --- | --- | --- |
| `90 %` | `pack.initial_soc` | `Tie::Scenario`, pow10 2 |
| `20` shells | `pack.cell_model.Spm.shells` | `Tie::Scenario` |
| `2020` in `Chen2020` | the chemistry's `meta.provenance` | `Tie::Name`, prefix `Chen` |
| `0` in `[r0]` | the same string, which is where those blocks are *called* placeholders | `Tie::Name`, prefix `[r` |
| `74.8` (first) | step 12's first rebound | `Tie::Quoted` |
| `74.8` (second) | step 12's fifth rebound | `Tie::Quoted` |
| `0.5 %` | step 12's arrival fraction, complemented | `Tie::Quoted`, new |
| `9` s | `[[rc]]`#1's `r_ohms × c_farad` | `Tie::Product` over an indexed path |
| `72` s | `[[rc]]`#2's, the same way | `Tie::Product` over an indexed path |
| `6` in "Phase 6" | nothing in the tree | **reworded away** |

### `Phase 6` is reworded, on the `R0` precedent

"a model that has been in this engine since Phase 6" points into `CLAUDE.md`'s frozen
build plan. It is not a quantity — it is a section number — and the failure a tie would
catch (the plan renumbering itself) is not live the way a lesson insertion is, which is
what `Tie::Ordinal` exists for. `Tie::Name`'s docs record the same disposal for the `0` of
`R0`: "that one had no field to point at and was reworded away instead." So: *"since its
porous-electrode phase"*.

The `0` of `[r0]` goes the other way in the same step, and the contrast is the point. That
digit **does** have a field to point at, because the chemistry file's provenance says in
so many words that `[r0]/[[rc]]/[aging]/[safety] are labelled placeholders` — which is
exactly what the sentence is telling the reader. Delete the label from the chemistry and
the sentence is a claim about a file that no longer makes it, and the tie goes red.

The prose says "This file's `[r0]`", where the file that holds `[r0]` is the *chemistry*
and "this file" reads as the scenario. Corrected to "The chemistry file's" as part of the
same edit, so the tie's target and the sentence's subject are the same file.

### Two fences that were refusing the wrong thing

Both of `Tie::Quoted`'s guards blocked a legitimate quotation of step 12, and neither
guards what its own comment says it guards.

**"Exactly one claim on that step may name it."** Step 12 states its first rebound in
**three** sentences — the headline (*"It is 74.8 mV on the first pulse…"*), the
decomposition (*"…74.8 mV climbs back slowly (the RC pairs)…"*) and the closing line
(*"Hold onto 74.8 mV"*) — so `pulse_rebound_mv:1` carries three claims, at one instant,
with one value. This file guessed two, and the perturbation harness's own match-count
assertion said three before a single case ran: an edit that matches a different number of
places than the author expected is the same silent-failure shape as an edit that matches
nothing. The hazard the fence names is real and is about a
*different* shape: `v_at` on step 15 carries six claims at six instants, and "step 15's
`v_at`" would be decided by file order. (That example expired one slice later: step 15's
readings now name their instants — see `path-instant-tagged-readings.md` — and the fence's
standing case moved to step 20.) What separates the two cases is not how many
claims there are but whether they answer differently. So the fence becomes: **every claim
on that `(step, quantity)` must carry the same `value`**, and a disagreement is refused
with the message the old one gave. That is strictly more coverage for the quantities a
rule quotes: two claims on one of them that have drifted apart is now an error, where the
old fence refused the quotation outright and so could never notice. It says nothing about
a quantity no rule quotes, and it is not a general agreement check over the claims file.

**"A quoted claim that spells its own number scaled is refused, or the two scalings would
multiply silently."** This one cannot happen. `Tie::Quoted` resolves to `claim.value`,
which is in the engine's units; the claim's `spells_pow10` describes how *that other
step's prose* renders it, and this scan never reads it. The only scaling applied is the
quoting rule's own `pow10`, against this step's own sentence. The fence was refusing a
composition it had ruled out by construction — and the sentence it refused is step 13's
`0.5 %`, whose source claim spells `99.5 %` and therefore carries `spells_pow10 = 2`.
Removed, with the reasoning recorded in its place.

### `Tie::Quoted` learns one frame, and it is the claims file's own word

Step 12 claims that **99.5 %** of the first rebound has arrived by the half-way point of
the rest. Step 13 prints the other side of that: *"8 % of the fifth rebound arrives in its
final five minutes, against the circuit's 0.5 %."* The claims file already has a word for
a sentence printing how far below one a value sits — `states = "complement"` — and step
13's own 8 % claim uses it on this very quantity. So the tie gains the same word rather
than a second vocabulary: `QuotedAs::{Same, Complement}`, applied to the value before the
rule's `pow10` scales it. `1 − 0.995238 = 0.004762`, times a hundred, rounded to the one
decimal place the sentence commits to, is `0.5`.

**The frame gap is stated rather than hidden.** Step 12's claim is on the *first* rest and
step 13's sentence is about the *fifth*. They are the same number only because the circuit
is linear and time-invariant, which is step 12's entire lesson and which step 12 separately
claims — five rebounds identical to four decimal places. That is the arm's stated cost: a
re-fit that made the circuit's late rests differ from its early ones would leave this
sentence green and wrong. The alternative — a claim on step 12's fifth rest — has no
admissible `tol_from`: `spelled` and `tighter` both require a spelled number and step 12
prints none for that rest, and `grid` is fenced to step-grid times.

### An indexed path, because the sentence names both pairs

*"the RC pairs are 9 s and 72 s"* is two products of two chemistry fields each, and
`numbers_at_path`'s only array walk was `*`, which is strict: `rc.*.r_ohms` reaches both
resistances, and a `Tie::Product` factor reaching two values resolves to nothing by design.
So the walker learns a numeric segment — `rc.0.r_ohms`, `rc.1.c_farad` — which names one
element where `*` names all of them. The two arms already in the file stay what they were:
`*` is for a sentence about every member, an index is for a sentence about one.

## Perturbations, registered before the run

All twenty-one ran and all twenty-one reddened. Rows 10–21 ran first; rows 1–9 were re-run
afterwards from a script that replaces one literal, runs the binary, records the failing
test names and restores the file byte for byte — which is how the working tree came back
to the same diffstat it started with. Every row below reddens
`every_numeral_in_a_ledgered_step_is_accounted_for`; the file-side edits redden
`every_claim_matches_the_engine` with it, because moving a scenario field moves the run
the claims are checked against.

| edit | must redden |
| --- | --- |
| prose `the same 90 %` → `91 %` | the initial-charge tie |
| scenario `initial_soc = 0.90` → `0.91` | the same tie from the file side, and step 13's claims with it |
| prose `20 shells deep` → `21 shells deep` | the shell-count tie |
| scenario `shells = 20` → `21` | the same tie from the file side |
| prose `Chen2020` → `Chen2021` | the `Chen` name tie |
| chemistry `meta.provenance`'s `Chen2020` → `Chen2019` | the same tie from the file side |
| prose `[r0]` → `[r1]` | the `[r` name tie |
| chemistry `meta.provenance`'s `[r0]/[[rc]]` → `[r1]/[[rc]]` | the same tie from the file side |
| prose `where the circuit's was 74.8` → `74.9` | `Quoted`, on step 12's first rebound |
| all three of step 12's `pulse_rebound_mv:1` claims' `value` → `74.9` | the same tie from the claim side, and check 7 with them |
| **one** of those three claims' `value` → `74.9` | the new agreement fence |
| prose `its 74.8 mV was the same five times` → `74.9` | `Quoted`, on step 12's fifth rebound |
| prose `the circuit's 0.5 %` → `0.6 %` | `Quoted`, complemented |
| that rule's `QuotedAs::Complement` → `Same` | the same tie |
| step 12's `pulse_rebound_arrived:1` claim's `value` → `0.99` | the same tie from the claim side, and check 7 |
| prose `9 s and 72 s` → `10 s and 72 s` | the first RC product |
| prose `9 s and 72 s` → `9 s and 73 s` | the second RC product |
| chemistry `[[rc]]`#1's `c_farad` → `950.0` | the first product from the file side |
| the second rule's `rc.1.r_ohms` → `rc.0.r_ohms` | the second product's operand pinning |
| prose "since its porous-electrode phase" → "since Phase 6" | the ledger, on a numeral nothing accounts for |
| `Tie::Quoted` pointed at a `(step, quantity)` two claims answer differently | that fence's `should_panic` test |

## Step 16, measured and not ledgered

The measurement was taken the cheap way and it is worth recording how, because the
obvious way is thirty times more expensive. The accounting check panics on the *first*
numeral nothing accounts for, so enumerating a step's gaps normally means one
edit-and-rerun cycle per gap. Turning that panic into a print-and-continue for one
throwaway run lists all of them at once: **43 numerals, of which 11 are already inside the
literals of this step's ten claims and 32 are not.** Forty-three is the densest step in the
path by a wide margin — the previous high was 26 — and the count came from the file's own
derived tally rather than from counting, by writing `1` beside the entry and letting it
correct me.

The thirty-two sort into four piles, and only the last is a blocker.

**Thirteen fall to arms that already exist.** The scenario's `initial_soc` (`100 %`), the
demand box (`15.459594 A`, and `15.46` again as the same box at two decimal places), the
ambient box (`25 °C`), the chemistry's `cell.v_min` (the `2.50 V` cut-off) and its
`cell.capacity_ah` (`5.153198`, and `5.15` at two places), a `Tie::Ratio` of the box over
that capacity (`3 C`, on step 22's `C/20` precedent), a difference (`64` seconds) and a
product (`1.99 A·h`) — and two quotations that this slice's own arm can serve: **`3.927`
twice**, off step 17's probe claim, and **`6.33 W`**, off step 15's `q_gen_at`. Both are
quotable for the same reason: each is the only claim naming that quantity on that step.

`1 C` looked like the same shape as `3 C` and is not, which is worth writing down before
someone spends the arm on it. `Tie::Ratio` divides two *file reads*, and at 3 C both halves
are real files — the demand box over the capacity. At 1 C the current is not in any box:
the sentence is telling the reader to *type* `5.153198 A`, which is the capacity itself, so
the ratio would be that field over itself. It resolves to `1` and it can never be anything
else, which makes it an arm that cannot fail. Either it gets a control-shaped arm on the
step's own instruction — the same machinery `[[arm]]` already uses to change a box — or the
`1 C` gloss is reworded. That is a decision, not work.

**One is reworded away**, on the precedent this slice set for `Phase 6`: "the model this
engine's Phase 7 was built for" is a section number in a frozen build plan, not a quantity.
The reword belongs to the slice that ledgers the step, not to this one.

**Five are claim-writing on this step's own pack** and are therefore only work: the two
instants of "over the 64 seconds from 400 s to 464 s", that `464` again where the amp-hour
figure is worked out, the `535 mV` drop across that interval, and the second printing of
`2.808` inside a parenthetical that this step's existing probe claim does not reach.

The `2.808` carries a constraint worth stating: it is the last of three figures in
*"(3.798 V for the circuit, 3.927 for the particle, 2.808 here)"*, and the other two are a
third pack and a quotation. A claim literal reaching it must therefore stop short of both —
`"2.808 here"` and not the whole parenthetical — or the literal covers a token a vocabulary
rule also covers, which is the double-accounting panic this file already has code for.

**Twelve are blocked, and the blocker is not the one this file predicted.**
`path-claims.toml` records the reason as "its twin readings and its 1 C boundary need a
second pack", and a second pack is indeed unavailable — a claim is checked by running its
*own* step's scenario, so nothing on step 16 can read step 15's cell. But `Tie::Quoted`
now exists precisely to borrow another step's measurement without re-running it, and the
sharper obstacle is that **the agreement fence built in this slice refuses step 15.** Its
eight voltage readings all share one quantity name, `v_at`, and resolve to
`[3.9267, 3.9180, 3.4706, 3.4491, 3.4386, 3.4178, 2.5020, 2.4953]`; `soc_at` carries two
that likewise disagree. So even the twin figures step 15 *does* measure — `3.918` and
`3.471` — cannot be quoted until those readings are split into per-instant quantity names
the way step 12's rebounds already are (`pulse_rebound_mv:1`). That rename is mechanical,
it is the price of the twin half of this step, and it is a cost the next slice pays rather
than a wall.

What is left after the rename is genuinely without an arm:

* **`3.437 V`, `596` seconds, and the `34 mV` fall** are readings of step 15's pack at
  instants step 15 never claims. New claims on step 15 would supply them; that is a second
  step's slice, not this step's.
* **`3.798 V`** is the circuit's zero-length probe — a *third* pack. The claims file
  already says so in the note on step 17's probe claim.
* **`12` seconds in `3484` — `0.34 %`** is the 1 C boundary, and it needs both models run
  to a full-discharge cut-off at a current that is not this step's box. That is an arm and
  a second pack at once, and roughly an hour of simulated time on the most expensive cell
  model in the engine. The scenario file's own header states all three numbers as measured
  — but in a `#` comment, and a tie reads TOML *fields*, not prose.
* **`2.28`** is the ratio of the two models' delivered charge: two packs by construction.
* **`140×` and nearly `500×`** are per-step cost ratios. No trajectory settles a cost, and
  this is the same shape already named beside `what-protection-costs`.

So the honest headline for step 16 is: 11 numerals already claimed, 13 reachable by arms
that exist, 1 reworded, 5 written as claims on its own pack, and **13 that are not work but
decisions** — the twelve above plus the `1 C` gloss. About two thirds of it is affordable
today; the last third needs a per-instant rename on step 15, new claims on step 15, and a
decision about whether the cost ratios get an arm or get reworded. It is not a claim-writing slice,
which is what the previous plan predicted for it.

## Learned while building

**A red baseline that runs concurrently with another job is not a baseline.** A
`should_panic` test came back "did not panic" on a full-suite run, which reads exactly like
a fence that was never wired up — and in isolation the same test passed. The full run had
overlapped the tail of the *previous session's* perturbation harness, which mutates the
tree in place and restores it afterwards. The test binary compiled against a neutered
source. Nothing about the failure said "contention", and the only reason it was caught was
running the single test on its own before believing it. Perturbation harnesses restore what
they touch, so a stale mutation leaves no trace to find afterwards.

**Turn the first-failure panic into print-and-continue before enumerating a step's gaps.**
The accounting check is built to stop at the first unaccounted numeral, which is right for
an author fixing one at a time and wrong for measuring a step you have not started. One
throwaway edit — `panic!` becomes `println!` and `continue` — listed all thirty-two gaps in
a 0.2 s run, with the surrounding sentence beside each. The alternative was around thirty
edit-and-rerun cycles, and it would have produced the same list.

**A fence built for one step's convenience can price another step's slice.** The agreement
fence was written here to let step 13 quote step 12, whose three sentences all state one
value. Its side effect is that step 16 cannot quote step 15 at all, because step 15 names
eight readings with one word. That is the fence working — file order would otherwise pick
which reading the sentence meant — but it means the cost of the *next* slice moved, and it
moved because of an edit made in this one. Worth stating in the plan rather than
discovering at the top of the next.
