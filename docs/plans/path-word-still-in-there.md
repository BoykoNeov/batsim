# The step that is nothing but hours

`docs/plans/path-word-batch-three.md` closed with a hand-off: eleven word-blind steps left,
and the largest of them — `and-it-is-still-in-there`, step 24 — deliberately **not** in that
batch. Six spelled quantities, every one an hour or a fraction of one, and three of them
stated relative to a moment that is not the start of the run. That is a slice, not a batch
member, and this is it.

**Status: LANDED 2026-09-01.** `spelled` 20 → 21, `word_blind` 12 → 11. Six vocabulary rules,
five `v_at` claims tagged with the instants they read at, one sentence rewritten in digits,
and three stale self-descriptions in Rust doc comments repaired — two of which no scanner in
this repo can see and one of which had been contradicted by a rule comment in the same file
for four days.

## What the scan reads on this step, and what it does not

**As the scan found it, before the one reword below.** `spelled_numbers` read six quantities
in the prose — five today, because the second row moved into digits. (`Lesson::text` starts
at `prose: [`, so the title's "four hours" is invisible to every scanner here and always
was, as is everything in the `//` comments above the block.)

| phrase | what it is | where it sits |
| --- | --- | --- |
| *four hours* | the rest leg, in the sentence that introduces the pulse program | free prose |
| *four and a quarter hours* | the step's own span | free prose |
| *half an hour* | an instant, 1800 s after the current went off | inside a claim's literal |
| *at four* | an instant, at the far end of the rest | inside a claim's literal |
| *four hours* | the rest leg again, in the sentence about the charge row not moving | inside a claim's literal |
| *Half an hour* | the same instant as the third row, in the sentence that splits the overpotential | free prose |

The `[[english]]` backlog listed **eight** phrases for this step then and seven now, and the
two the reader never sees are the shape it has never had: *"a tenth of a volt"* and *"an hour"*, both quantities
spelled as an **article**. The ban reads those and the reader does not, which is the
asymmetry `BANNED_UNITS` exists to license. They stay in the backlog.

## The three that are relative, and why they do not go through `ReadAt`

Three of the six name an instant **counted from where the load came off** rather than from
the origin of the clock. The rest begins at 737 s, so *half an hour* is 2537 s on the panel
and *at four* is the far end of a 14400 s off-leg.

`Accounted::ReadAt` already carries exactly this frame — "since the current stopped" — and it
is **restricted to one quantity**, `pulse_rebound_arrived`, with the restriction written into
its own comment: every other pulse quantity is read at a leg boundary, where the reading
returns the leg length itself and a token has two accountings.

**That fence holds on this step and the collision is not hypothetical.** One of this step's
claims is read at 737.5 s, half a second into the rest, and this step's `dt` is 0.5 s. A
generalised rest-relative frame would offer a timed `0.5` to `Accounted::Setting` and
`Accounted::ReadAt` in the same sentence group — the exact two-readings hazard that arm's
`clash` assert exists for, on a function thirty-two steps share.

So the three instants are carried by **vocabulary rules**, which are per-sentence and reach
nothing else:

```rust
Tie::Difference(&[
    Tie::Instant { step: "and-it-is-still-in-there", arm: None, quantity: "v_at:2537" },
    Tie::Setting(Control::PulseOn),
])
```

The claim's own instant, less the leg the current was on for. Order is the claim, as under
every `Tie::Difference`: reversed, *half an hour* would be half an hour **before** the rest
began, which is inside leg one.

## Two claims were unaddressable, so the instants are tagged

`Tie::Instant` refuses a quantity two claims answer at two instants — file order would
otherwise decide which one a sentence means. This step files **five** `v_at` claims at
737, 737.5, 2537, 4337 and 15136.5 s, all under one name, so not one of them could be pointed
at. All five are tagged now (`v_at:2537`, and so on), which is the move
`docs/plans/path-instant-tagged-readings.md` describes and which `looks-fine-from-outside` has already
paid for once. Tagging all five rather than the two this slice needs is deliberate: a step
whose voltages are addressable in two of five places is a step where the next sentence to
quote it picks the wrong one.

Nothing outside this step quoted `v_at` here, so the tags cost nothing and the step is
quotable now, which it was not.

## The sentence that had to move

> This step covers **four and a quarter hours**, so it runs fast…

The mark is 15374.5 s, which is **4.27** hours. A quarter of an hour is not a rounding of
that at any precision this file compares at: a computed tie reads 4.3 to one place and 4.27
to two, and never 4.25. `Tie::Setting(Control::Until)` compares exactly and refuses it
outright.

So the figure is in digits — *"This step covers 4.3 hours"* — which is what the digits rule
asks for anyway, and it is tied by `Tie::Hours(&Tie::Setting(Control::Until))`. This is the
**second** rule in the table to reach that variant directly. See the next section for why
that sentence is worth writing down.

The reword moves four counts, and the order matters: **re-derive after the edits, not
before.** The word scan now reads **five**, not six; the step's digit count goes 26 → 27; the
`[[english]]` block header goes 8 → 7; and the ledger's own numeral tally goes 770 → 771 in
two files, only one of which was watched.

## Three self-descriptions that were false under a green suite

None of these is a number a scanner can reach. All three are prose in Rust doc comments.

1. **The module header said the word reader "finds nothing at all today".** True when
   written — the digits rule had emptied every step then in `spelled` — and false since the
   third batch refilled it four days later. It now says what it said and when it stopped
   being true, and points at the two derived tallies in `path-claims.toml` rather than
   restating their numbers.
2. **`the_ban_sees_every_quantity_the_reader_reads` said the whole English backlog "sits on
   steps the reader is not turned on for".** That stopped being true the moment a batch
   opened a step with a backlog entry left on it — `belief-drifts` keeps two — and it is a
   claim about what makes the check live, so a reader would have taken the check's own word
   for its coverage.
3. **`Tie::Hours`'s own doc said the variant is "unreachable through the vocabulary
   today".** Two rules reach it directly. The first was step 8's, built two batches ago,
   and *that rule's own comment says so in as many words* — ten thousand lines from the
   paragraph it falsified. Nothing points the author of a new rule at the note their rule
   makes stale, which is the whole shape.

The third is the one worth generalising: **a note recording that nothing does X is falsified
by the first thing that does X, and the thing that does it is written somewhere else.** This
file has caught the same shape before (`Tie::Name`'s docs record it about the `0` of `R0`).

## The tally that was not registered

`n_ledgered_numerals` is derived and asserted where `path-claims.toml` states it. The module
header states the same pair of numbers in a different sentence, and that one was registered
nowhere — so this slice's reword moved the claims file's copy and left the header saying 770.
It is a `Tally` now (`"today it is {w} steps and {n} numbers"`), which is the same repair
`docs/plans/path-self-description-sweep.md` made nine times and the reason this one was cheap
to find: the pattern is "one fact, two files, one of them watched".

## Perturbations

See the foot of this file for the measured table. The two design points it prices:

* **Two of the six numbers sit inside a claim's literal.** A prose-only perturbation on one
  of those dies on the literal check first and reddens for the wrong reason, which is
  `path-word-batch-three.md`'s finding. Both are moved **with** their claim's `literal`,
  leaving `read_at_s` and `value` alone, so only the new rule can object.
* **The rule table is walked against the case list by hand**, because the accounting check
  panics in prose order: a case reddening the first rule says nothing about the fifth.

---

# The measured table

Ten cases, run 2026-09-01 against a baseline the harness verified green before it started
(`M:\claud_projects\temp\path-word-still-in-there\perturb.py`, real exit codes, below-normal
priority, tree digest checked at the end — **restored: True**). Every case ran the whole
`path_claims` binary; the column after the verdict is the tests that reddened, and the line
under each is the panic, which is what says the red landed on the right number.

| case | verdict | reddened |
| --- | --- | --- |
| control — nothing changed | GREEN | — |
| A — *four hours* → *five hours* (the program sentence) | RED | ledger, on `PulseOff` |
| B — *4.3* → *4.5* (the step's span) | RED | ledger, on `Until, in hours` |
| C — *at half an hour* → *at two hours*, prose + literal | RED | ledger, on `v_at:2537` less `PulseOn` |
| D — *at four* → *at three*, prose + literal | RED | ledger, on `v_at:15136.5` less `PulseOn` |
| E — *the entire four hours* → *three hours*, prose + literal | RED | ledger, on `PulseOff` |
| F — *Half an hour into the rest* → *Two hours* | RED | ledger, on `rc_overpotential_mv_at` less `PulseOn` |
| G — the tag `v_at:2537` moved to `v_at:2538` | RED | `every_claim_matches_the_engine` **and** the ledger |
| H — `Tie::Hours` lifted out of the rounding group | RED | `an_hours_tie_rounds_the_way_a_computed_tie_does` **and** the ledger |
| I — *at four* tied to `Setting(PulseOff)` instead of the claim | **GREEN** | — |

**Six rules, six cases, six distinct panics.** The accounting check panics in prose order, so
a table like this understates itself unless each case's message is read: all six name the rule
or the tie they died on, and no two name the same one, which is the hand-walk of the rule
table against the case list that `path-word-batch-three.md` asks for.

**A, C, D, E and F each redden a second check, and it is not noise.** The perturbed phrase is
not in `[[english]]`, so `no_lesson_spells_a_quantity_in_english` fires too — the ban
matching the backlog both ways, working exactly as designed on a sentence an author just
changed.

**Case G is what the tags cost and what they buy.** Moving one tag by a second reddens the
tag's own assert in `measure` *and* leaves the rule that reads that instant resolving to
nothing. Two independent objections to one edit.

**Case H is a measurement that retires a paragraph.** `an_hours_tie_rounds_the_way_a_computed_tie_does`
carried "lifting `Tie::Hours` out of the rounding group leaves all 28 tests green" — true when
written and false since step 8's rule landed two batches ago. Lifting it now reddens that test
**and** step 24's `4.3`. The doc says so, with the date.

**Case I is the finding.** `Setting(Control::PulseOff)` is 14400 s exactly, so *at four* tied
to the pulse program is green — tighter than the arm actually used, and wrong. It decides the
number off the **program** rather than off the claim, so moving that reading to two hours into
the rest would leave the prose saying "at four" and the rule green. The arm in the tree is the
looser one on purpose: the claim's own instant, less the leg the current was on for, compared
at the precision the prose prints (none), which is ±half an hour where the two siblings that
print a tenth are held to three minutes. A green perturbation is a result, and this one names
the alternative a later author would reach for.

## The enumeration this slice got wrong, and where it was caught

The count beside the `spelled` entry is derived and was right. **The sentence beside it was
not**: it read *"the rest leg twice, and two instants inside it"*, which sums to four against
a derived five. There are three READINGS at two distinct instants — *half an hour* and *Half
an hour* are the same moment in two sentences — and the enumeration lost one in the re-derive
after the reword. The first draft, written before the reword, summed correctly.

That is this file's own recurring shape landing inside the slice that documents it: `# 5` is
watched by `every_count_beside_a_word_list_entry_is_derived` and the words beside it are read
by nothing. `first_count` takes **one** number per note, so everything after it is prose. The
same pass removed two counts this slice had planted in words — *"FIVE OF THE SEVEN ARE TIED
NOW"* in the `[[english]]` header and *"The twenty-seventh"* in the ledger note — by
rewording them to carry no numeral, which is the cheap repair the module header took.

And the claim in `Tie::Hours`'s repaired doc was **checked before it shipped**: of the seven
places that construct the variant, five sit inside a `Tie::Product` and exactly two are a
rule's outermost tie. Writing a fresh false count into the doc comment whose false count this
slice was fixing is the one mistake that would have been unanswerable.
