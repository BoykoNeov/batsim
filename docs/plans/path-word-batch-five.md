# Word batch five — the millivolt, and the amp that was never about the word

**Landed 2026-09-01.** One unit noun admitted, one measured and refused, one vocabulary
rule, one claim given an instant tag, one new test. No engine code, no snapshot bump, no
wasm rebuild — `crates/sim-data/tests/path_claims.rs` and `web/path-claims.toml` only.

The scoping came from `docs/plans/path-word-batch-four.md`, which named both words and said
each was "a rule due the same afternoon it is admitted". That is right about `millivolt` and
wrong about `amp`, and the reason it is wrong turned out to be the more interesting half.

## The forced half: `millivolt`

`("millivolt", 0.001)` joins `UNIT_NOUNS`. Singular only, on `volt`'s terms. It forces
exactly one phrase path-wide — step 21's *"which is about a tenth of a millivolt here"* —
which was predicted from the `[[english]]` inventory before any code, and confirmed by
admitting the noun alone and reading the panic: that phrase, no second.

**It is the first noun on the list whose scale is neither a time nor one.** `tie_agrees`
computes `10^pow10 / unit`, so the noun's own scale carries the conversion and the rule
needs no `pow10` of its own. Nothing had exercised that division before: every previous
non-time noun (`point`, `time`, `percent`, `volt`) scales by one, so a conversion that
silently dropped the scale would have been green on every case in the scanner test. There is
a case for it now.

**The rule, and the wrong operand that was available.** The sentence is about the RC pair
lagging the resistance growth it is chasing, and the number is how far short of the settled
arithmetic the row actually falls:

    |(-0.0640 x 1.0726455) - (-0.068551)| = 9.8312e-5 V = 0.0983 mV

so `Tie::Magnitude` over a `Tie::Difference` of a `Tie::Product` of two quotations and a
third quotation. The tempting operand is the sentence's own printed `-0.0686`, two clauses
earlier — and it gives 4.9e-5 V, which `to_fixed(0.049, 1)` renders `"0.0"`. It fails red
rather than green, which is the direction that matters, but the warning is written into the
rule rather than into this file: **the number this sentence states exists only between two
values one of which the prose never prints in full.** That is what makes it a difference of
ties rather than a `Tie::Derived` over siblings.

**One claim grew an instant tag to make the rule possible.** `what-it-cost` files three
readings under `v_at` — 1.9306 V at the knee, -0.065024 at 300 s, -0.068551 at the mark —
and `Tie::Quoted` refuses an address three claims answer differently. The mark's reading is
now `v_at:600`. The other two stay untagged, on the rule step 20 already wrote down: a tag
is an address a quoting sentence needs, not a decoration every reading gets.

## The refused half: `amp`, and what it is actually about

`nothing-to-clamp` writes *"until the current is down to hundredths of an amp"*. Admitting
the noun makes the reader value that at **one ampere** — measured, `("1", 1.0, "amp")` — for
a sentence stating a hundredth of one. Both shapes in `spelled_numbers` walk FORWARD from
their head (a numeral, or an article) and step over fillers and scale words on the way to a
noun. **Neither looks behind the head**, so a partitive that stands its scale word alone,
ahead of the article, is read as the article alone.

**The hole is in the reader, not in the word, and that was worth measuring rather than
assuming.** `spelled_numbers("moves by hundredths of a point in")` returns
`[("1", 1.0, "point")]` **today**, on a noun `UNIT_NOUNS` has admitted since it was written.
The scanner has had this blind spot the whole time; it is safe only because no lesson writes
the shape. The full inventory of partitives in the path, taken with one regex:

| phrase | head | read as |
| ------ | ---- | ------- |
| `a tenth of a second` | article | declined — `second` is the article shape's one limit |
| `two hundredths of a point` | numeral | 0.02 point |
| `a hundredth of a point` | article | 0.01 point |
| `a tenth of a volt` | article | 0.1 V |
| `a tenth of a millivolt` | article | 1e-4 V — **this batch** |
| `to hundredths of an amp` | **nothing** | would be 1 A |

One bare head, path-wide, and it is the amp sentence.

So the precondition is now a check rather than a hope.
`no_lesson_stands_a_scale_word_alone_before_a_unit_this_reader_reads` scans every lesson for
a scale word with no numeral and no article in front of it, and asks **the reader itself**
whether it reported a reading in the four words behind — a hand-written walk would inherit
whichever blind spot the walk it was copied from has. It is green today and goes red the
moment `amp` is admitted, with the reason attached.

**Building the shape instead was the other road and it leads nowhere.** A reader that valued
the phrase at 0.01 A would then need an arm for it, and what that sentence states is a
magnitude rather than a figure — where to stop watching, not what the current is. No file in
this tree decides 0.01 A. The arm's own note already records the neighbouring version of
this: the instruction used to say "about 400 s" and that digit came out when the step was
ledgered, because nothing decided it either. (`Tie::Instant` has since been built for
exactly that shape, so the note's argument is now stale about the 400 — but the current is
still undecided, and rewriting a stopping condition into an instant would be changing the
lesson to suit the checker.)

## Perturbations — eight cases, each naming the check that fired

Control captured green first, exit codes read from the process rather than through `start`,
tree byte-verified restored after each case.

| perturbation | reddens |
| ------------ | ------- |
| the new rule's phrase matches nothing | accounting + `every_ledger_rule_is_a_phrase_and_is_used` |
| that rule's `pow10` 0 -> 3 | accounting (value) |
| the quoted reading loses its `:600` tag | accounting (`Tie::Quoted` refuses the ambiguity) |
| `millivolt` taken back out of `UNIT_NOUNS` | both self-counts, the scanner case, rule-unused |
| `millivolt` scaled 1.0 instead of 0.001 | accounting + the scanner case |
| **`amp` admitted** | **the new prose guard**, plus both self-counts, the scanner case, accounting |
| prose says *"two tenths of a millivolt"* | accounting, on the VALUE — panic prints `9.83e-5` |
| the claimed clause deleted | both self-counts, rule-unused, the ban |

Two of these are the ones that mattered. The amp case is the only evidence the new guard is
not decorative, and the "two tenths" case is the only evidence the rule forces the number
rather than the shape — its panic text was read rather than its exit code, because a red
exit code can be the wrong check reddening.

## The counts that moved

`spelled`'s entry for `what-it-cost` 6 -> 7; the whole-list tally thirty-one -> thirty-two
quantities, still on eight nonzero steps. `word_blind` unchanged at nine. The ledger's arm
count is unchanged at 28 — `Magnitude`, `Difference`, `Product` and `Quoted` are all already
in use — and `[[english]]` is unchanged at 45 phrases, because a phrase that gets a rule
stays on the list.

## Still unheld

Nine word-blind steps, unchanged by this batch. Inside `spelled`, one phrase remains tied to
nothing: `nothing-to-clamp`'s *"an amp"*, and it is now tied to nothing **on the record**
rather than by omission. The other unit noun the last batch named as blocking —
`what-it-cost`'s *"a tenth of a millivolt"* — is what this batch closed.

The next word-scan batch has no forced work left in it: every remaining phrase is on a step
the scan is not turned on for. The largest are `pack-disagrees` (three, all about a spread
across the grid that nothing measures) and `three-times-the-current` (three, one of which is
a per-step cost ratio between two cell models that no file in this tree holds).
