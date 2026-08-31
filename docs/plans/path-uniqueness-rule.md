# The uniqueness rule

A lesson may not say that something is the only, the first, the last or the most of its kind
**in this path** or **in this repo** unless something derives it. Six sentences say it today,
across four steps; two of them are false, and both went false the same way — a later lesson
added a cell that broke an earlier lesson's claim to be exceptional.

This is a slice against a defect class, not against a number. Every check in
`crates/sim-data/tests/path_claims.rs` is about whether a **quantity** is right. Nothing in
the repo asks whether a claim about the **path's own shape** is still true, and "the only cell
here that does X" is exactly the sentence a lesson wants to write the day a chemistry lands.

## Where it came from

`docs/plans/sodium-ion-chemistry.md` closed by naming it. That slice found two shipped
sentences that a seventh chemistry had made false — step 28's *"the last this path adds"* and
step 29's *"a thing no other parameter file in this repo has to say about itself"* — and wrote
down why nothing caught them:

> the checks in this repo are all about whether a number is *right*. Nothing in them asks
> whether a **claim about the path's own shape** is still true […] The cheap discipline is a
> grep for superlatives — `the last`, `the only`, `no other` — over `const LESSONS` on any
> slice that adds a step.

It recommended the grep as a discipline. This slice runs it, finds two more, and makes it a
check instead — because a discipline nobody has written down as a test is a discipline that
lasts exactly as long as the author who remembers it.

## The two false statements

Both are on `ten-c-costs-a-point`, the LTO lesson (step 26), and both were true when written:

* **Shipped prose**, `web/app.js`: *"A 20 Ah prismatic cell whose datasheet rates it at 10 C
  in both directions, which no other cell in this path comes close to in either."*
* **A code comment** on the same step, above `demand:`: *"Nothing else in this path goes above
  3 C."*

`chemistries/nimh_subc_3ah_generic.toml` is rated **`max_discharge_c = 10.0`**. It is taught
at steps 27–29, which landed **after** the LTO lesson. So the nickel cell does not merely go
above 3 C, it **ties LTO exactly** in one of the two directions the sentence names. What is
still true is the pair: nothing else in the path is rated for 10 C in *both* directions.

Neither statement contains a numeral that moved, which is why every existing check is green on
them. The `10` in the prose is ledgered and tied to `cell.max_discharge_c`; it is right. The
sentence around it is wrong.

**And one of the two is invisible to every check in the file even in principle.**
`lesson_text` slices a lesson's block from `prose: [` onward, so a comment above `demand:` is
not in `lesson.text` at all. The scanner this slice builds reads a lesson's comments as well as
its strings, and that is a deliberate widening: this repo's recorded failures include a stale
self-count inside a panic message and four inside doc comments. Prose about the path rots
wherever it is written, and a `//` is not a place a claim goes to be safe.

## The instrument

A **ban with a backlog**, copied from the digits rule (`docs/plans/path-digits-rule.md`) —
which is itself the shape `[[english]]` uses — rather than a reader:

* A **sentence** of a lesson fires if it contains both a scope phrase (`this path`, `the
  path`, `this repo`, `the repo`) and a uniqueness word (`no other`, `nothing else`, `the
  only`, `the one`, `the first`, `the last`, `the sole`, `the widest`, `the highest`, `the
  most`, `unique`).
* Sentences come from a lesson's **comment lines and string literals**, each split
  independently, so a sentence never spans a prose item or drags JavaScript syntax in with it.
* Every firing sentence must appear in `[[unique]]` in `web/path-claims.toml`, matched **both
  ways**: a sentence in the prose and not in the table fails, and a table entry no sentence
  matches fails too. Rewording a claim therefore forces the entry to be re-read, and the entry
  cannot outlive the sentence.
* An entry may carry a derivation (`over`). One that does is **checked**. One that does not is
  **backlog** — named as unchecked rather than left to look covered, which is the distinction
  `unledgered` and `word_blind` already draw one axis over.

**The argument for a ban is the digits rule's argument and it transfers exactly.** Deciding
whether a superlative is a claim about the path or a turn of phrase is a hard problem in both
directions — *"the last step"* is a pointer at a neighbour and *"the last chemistry"* is a
claim. A false alarm under a ban costs one sentence its author re-reads. A miss under a reader
is silent, and a false sentence ships behind a green suite — which is what happened twice
already, and what is happening on the LTO step right now.

## The arm

One, because one sentence needs one. `Unique::TopOf { fields, mode }` compares the step's own
chemistry against **the chemistries the path teaches** — derived by walking every lesson's
scenario file to its `chemistry` id, not by globbing `chemistries/`. The claim is about the
path; a parameter file that ships and is taught nowhere would make a true sentence red.

* `Mode::Each` — for **every** named field, this chemistry's value is strictly greater than
  every other taught chemistry's. This is what *"comes close to in either"* says.
* `Mode::Both` — this chemistry's **minimum** across the named fields is strictly greater than
  every other's minimum. This is what *"rated for 10 C in both directions"* says.

Both modes are built, and the order matters: `Each` is what makes the shipped sentence go red.
Rewording the sentence first and then building only the mode that passes is a green that
proves nothing — the shape `docs/plans/path-instant-tagged-readings.md` records as a
registered green on the wrong claim.

## Predictions, registered before the engine ran

1. **The scanner finds exactly six sentences on four steps** — `one-step-that-got-through`,
   `and-it-is-still-in-there`, `ten-c-costs-a-point` (two), `which-way-it-was-driven`,
   `a-curve-worth-reading`. Two are in shipped prose and four are in comments. *(Measured with
   an out-of-tree prototype before the Rust was written; the Rust must agree with it, and a
   disagreement is a finding about the port rather than a licence to edit the number.)*
2. **The `Each` arm on the two LTO sentences is RED with the prose untouched**, and the
   failure names `cell.max_discharge_c` and the nickel chemistry, because 10 ties 10.
3. **Lowering `nimh_subc_3ah_generic`'s `max_discharge_c` to 3 turns that same red green**,
   with the prose still untouched. This is the perturbation that proves the arm reads the
   *other* cells: an arm that only ever read its own chemistry would pass the shipped sentence
   and this test could not tell.
4. **The universe derived from the path's scenario files is all seven chemistry files.** The
   path-derived set and a glob of `chemistries/` coincide today; the derivation still follows
   the path, so that it stays right the day one does not.
5. **The reword breaks no existing check.** The ledger rule `"rates it at {n} C in both
   directions"` keeps its phrase; no numeral in either sentence moves; the comment is not in
   `lesson.text`, so nothing ledgers it.
6. **`Mode::Both` is not vacuous:** setting LTO's `max_charge_c` to 1 must redden the reworded
   claim.
7. **The four backlog sentences stay backlog.** *"The first step in this path whose headline
   quantity is one step long"*, *"the one place in this path where the `overpotential` tile is
   the fitted mechanism"*, *"the first lesson in this path where the interesting quantity is a
   memory"* and *"the first parameter file in this repo fitted from a laboratory's own raw
   measurements"* are all true, and **none is rewritten to clear the scan**.
   `docs/plans/path-twin-arm.md` records that deleting a true sentence to satisfy a check is
   the same defect as inventing a false one.

## What this will NOT cover, stated before it is built

* **A uniqueness claim that names no scope.** *"A sixth chemistry, and the last of the
  flat-curve cells"* quantifies over the path and says neither "this path" nor "this repo", so
  this scanner never sees it. It is true today. Widening the trigger to superlatives over
  repo furniture (`the last … cells`, `the first … lesson`) was measured at fifteen hits of
  which most are neighbour-pointers, and a backlog that is mostly noise is a backlog nobody
  reads. The narrow trigger is the whole of the coverage, and the authoring rule that goes
  with it is: **if you claim it over the path, say "in this path"**.
* **Whether a backlog sentence is true.** The four are unchecked by construction. The table
  says so; a green run says nothing about them.
* **Prose outside `const LESSONS`.** The header of `path_claims.rs`, the plan documents, and
  `CLAUDE.md` all describe the path's shape too, and none of them is scanned.

---

*(Everything above this line was written before the check was built. What follows was written
after the engine ran.)*

## What happened

`no_lesson_claims_uniqueness_over_the_path_undeclared` (the ban, matched both ways),
`every_derived_uniqueness_claim_holds` (the derivations) and
`the_uniqueness_scanner_reads_comments_and_stops_at_a_piece_boundary` (the one property of
the scanner no shipped sentence proves) are in `crates/sim-data/tests/path_claims.rs`. The
table is `[[unique]]` in `web/path-claims.toml`: 6 sentences, 5 steps, 2 with a derivation and
4 in the backlog. Both false sentences on `M:\claud_projects\battery\web\app.js` are fixed.
`tests/path_claims.rs` is 67 tests green, and so is the workspace.

### Prediction 2 held, and the red arrived in the right order

With `mode = "each"` and the shipped prose untouched:

> lesson `ten-c-costs-a-point` says this, and it is not true of the chemistries this path
> teaches:
>
>   Nothing else in this path goes above 3 C.
>
> `nimh_subc_3ah_generic` has `cell.max_discharge_c` = 10, and this cell's is 10 — not
> strictly more.

That is the whole finding: a **tie**, on a cell taught two steps later, in a sentence with no
numeral that moved. The prose was reworded only after this red existed.

### Prediction 1 was half wrong, and the file's own check is what said so

The scanner found exactly the six sentences the out-of-tree prototype had found — the port
agreed. **But the plan says "four steps" — twice, in its opening paragraph and in prediction 1 — and it is five.** That was a hand-count off the
prototype's output, registered as a prediction, and wrong; the same wrong number went into the
table's header comment; and `every_count_these_files_state_about_themselves_is_derived` failed
on the first run with the derived number beside the written one. The prediction stands as
written above, because that is what the line is for.

Worth naming plainly: **the only reason this was caught in minutes is that a count written in
a comment above a table is a checked number in this repo.** The identical mistake in the plan
document — a file nothing scans — is still there five paragraphs up, and would have been there
for good.

### The second red was a collision, and it says something about an old boundary

`every_count_above_an_english_block_is_derived` failed too, claiming `[[english]]` "still
heads a block `ten-c-costs-a-point`" that has no phrases left. It does not: my new section's
per-step headers use the same `# <step> — <count>:` shape the English backlog uses, and
`english_block` cuts **from the English banner all the way to `[ledger]`** — roughly 1,600
lines, most of which are arms and claims, not English phrases. Anything written between the
two is inside the English section as far as that check is concerned.

The section was moved above the English banner and the red cleared. The boundary was left
alone: it has no failing case now, and widening a check to fit a section that has moved is a
change with nothing behind it. It is written down here instead, because the next author to add
a section in that gap will meet the same confusing red.

### The perturbation table

| perturbation | reddened |
| --- | --- |
| nickel cell's `max_discharge_c` 10 → 3, prose untouched | **nothing — and that is the result** |
| LTO's `max_charge_c` 10 → 1 | `every_derived_uniqueness_claim_holds` |
| one `[[unique]]` entry deleted | the ban, and the self-count tally |
| a `[[unique]]` entry no sentence matches | the ban, and the self-count tally |
| the LTO sentence reworded to drop "in this path" | the ban |
| CONTROL: a non-uniqueness sentence reworded | nothing |

Run with real exit codes and `--no-fail-fast`, and the first row's message was read by hand
rather than taken off the exit code — both are standing rules here after a harness once
reported five greens that were all lies.

**Row one is prediction 3 and it is the load-bearing one.** An arm that only ever read its own
chemistry would pass the shipped sentence and no other row in this table could tell. Dropping
the *other* cell's rating turns the red green with the claimed sentence untouched, which is
the only evidence that the comparison is a comparison.

**Row two answers the question row one raises about the fix.** After the reword the mode
changed, and a mode that passed because it compares nothing would look identical from outside.
Breaking LTO's own charge rating reddens it, naming `lfp_26650_generic` — the cell that then
ties it — so `both` is doing arithmetic and not nodding.

### What is NOT closed

* **The escape is closed against an accident, not against an author.** Rewording a claim to
  drop "in this path" reddens, because the table entry goes stale — but an author who edits
  the sentence *and* deletes the entry has removed the claim from the rule's sight, and
  nothing notices. That is the same guarantee `[[english]]` gives and it is the honest limit
  of a both-ways list: it stops drift, not intent.
* **The four backlog sentences are unchecked and two of them could be settled.** *"The one
  place in this path where the `overpotential` tile is the fitted mechanism"* would fall out
  of comparing `[diffusion]` sections across the taught chemistries — the same shape `over`
  already has, one field-kind over. Nothing else would use it today, which is why it was not
  built: this file's rule is that an arm nothing accounts anything with does not get built.
* **A superlative with no scope phrase is still invisible**, and one is live: *"A sixth
  chemistry, and the last of the flat-curve cells"*. It is true — sodium-ion's open-circuit
  curve spans 2.094 V against nickel's 0.400 — and it will be false the day a flat cell is
  added, with nothing watching. The narrow trigger was a deliberate choice against a backlog
  of mostly noise, and this is what it costs.
* **Prose outside `const LESSONS` claims uniqueness too.** `chemistries/lto_20ah_generic.toml`
  says its charge rating is *"an order of magnitude above every other chemistry shipped
  here"* — true today, checked by nothing, and in a file this rule does not read.

### One more thing the grep found and this rule cannot

The scan that started the slice was run over the shipped lesson prose and it turned up ~60
uniqueness phrases, of which six are claims about the path. The other fifty-odd are local —
*"the last step"*, *"the only thing moving"*, *"there is no other candidate"* — and every one
of them is a sentence that could go the same way for a different reason: they are claims about
a **trajectory**, and a trajectory moves when a coefficient does. Nothing here reads them, and
the ledger reads only their numbers. That is a bigger and vaguer hole than the one this slice
closed, and it is named rather than scoped.
