# The file's account of itself

`web/path-claims.toml` and `crates/sim-data/tests/path_claims.rs` describe their own
contents in numbers. How many claims take each tolerance rule, how many frames each
`states` variant is used in, how many numbers check 6 scans, how many steps the ledger
covers, how many claims sit beside each step it does not scan. Every one of those was
hand-maintained, and nothing in the repo read any of them.

They drifted. `docs/plans/path-ambient-arm.md` found the header's tallies five slices out
of date (`same 59` against an actual 110, `spelled 53` against 102), re-derived them by
hand, and wrote down that the next slice should re-derive rather than adjust. That is a
measurement, and a measurement ages the moment it is taken: two slices later the same
paragraph was wrong again in four new places.

This slice makes the counts derived. **The phrase is declared; the number never is** —
the contract `spells` keeps for a claim and `ScenarioRule` keeps for the ledger.

## What went in

Two tests in `path_claims.rs`:

* `every_count_these_files_state_about_themselves_is_derived` — a table of `Tally`
  entries, each a sentence with `{n}` / `{w}` / `{W}` / `{o}` where a count sits and a
  derivation per placeholder. The sentence is rendered with the derived numbers in place
  and required to appear in the file **exactly once**, matched against a flattened copy
  (comment markers stripped, whitespace runs collapsed) so column padding and line breaks
  are not part of the assertion.
* `every_count_beside_a_ledger_entry_is_derived` — the per-step counts in the `[ledger]`
  lists, which are the ones that move most often. Nothing is declared at all here: the
  comment beside `"past-empty",` says a number, the file says a number, and they have to
  be the same one. A ledgered entry's leading count is the numerals its prose prints; an
  unledgered entry's `claims: N` is the claims on that step, and an entry with no count
  must have no claims.

The tally table covers the counts in both files' headers; the ledger test covers every
entry in both `[ledger]` lists, so it needs no list of its own and grows with them. Sizes
are deliberately not written here: a figure in this paragraph would be a hand-maintained
count of the thing that exists to end hand-maintained counts, and it would be wrong the
next time a tally is added. `TALLIES` and `NOT_DERIVED` in `path_claims.rs` are the
authority. Of the two tests the ledger one is the stronger — it declares **nothing**,
comparing a number in a comment against a number in the file — and it is the one that
caught `past-empty`. If a later slice has to cut scope here, cut tallies, not that.

### Derived from the check's own functions, never re-scanned

The three numbers in "116 of the 139 numbers the 86 claimed sentences print" are outputs
of check 6, not properties of the parsed file. They are produced by calling `sentences()`,
`numeric_tokens()` and `accounting_for()` — the same functions the accounting test runs.
A second scan of the same prose could disagree with the first while both stayed green,
which is the defect class this whole file exists to kill.

`Facts::accounting_arms` counts how many of check 6's arms the claims actually **use**,
rather than how many the enum has. That is the honest reading of the sentence it pins:
this file's rule is that an arm nobody accounts anything with does not get built. It is
kept from going stale by omission by `Accounted::arm_name`, an exhaustive match — a fifth
variant does not compile until it is named.

### A separate word table, on purpose

`HEADER_WORDS` (and `HEADER_ORDINALS`, for "and no fifth") is not `WORD_NUMERALS`. That
table is the *lesson prose's* vocabulary and every entry in it must be spelled by a claim,
so putting "twelve" there to render a header sentence would redden a test about the page.
Where the two overlap they are asserted to agree, so there are two tables and one meaning.

### The gap is written down

`NOT_DERIVED` lists the self-counts this check does not derive, each with its reason and
its sentence. A declared list of exclusions is a free-text waiver unless something makes
it go stale, so each entry's phrase must still be in the file: reword the sentence and the
waiver reddens. Every entry is one of three shapes — a count of past slices, a past-tense
measurement ("fourteen of the twenty-four steps *were* in that position", whose figures
are frozen with the sentence and must not be refreshed), or an estimate no scan reproduces
("about 145 measurement-shaped numbers").

This is opt-in per sentence, like the ledger, and for the same reason: nothing can decide
automatically whether a number in a paragraph is *about this file*. A green run is not a
claim that every self-count is checked, and the docs say so where a reader meets them.

## What it found

Nine wrong numbers, all of them written by slices that ran a green suite:

| where | said | is |
| --- | --- | --- |
| header, `quoted` claims | 37 | 53 |
| header, `WORD_NUMERALS` entries | one | two |
| header, check 6's arms | "Three accountings, and no fourth" | four — `setting` was missing from the list entirely |
| header, check 6's scan | 64 of 73 numbers, 60 sentences | 116 of 139, 86 |
| header, sentences needing no waiver | 60 | 86 |
| header, unclaimed steps left | eleven | five |
| foot, unledgered steps carrying claims | Twelve / nine | Sixteen / five |
| `path_claims.rs`, `TolFrom` and `States` docs | 53 / 14 / 59 of 69 | 103 / 17 / 111 of 124 |
| `path_claims.rs`, arms on step 18 | four | six (four of which branch off the mark, which *was* right) |

The `setting` one is the interesting failure. The header's list of accountings still said
there were three and named three; the fourth had been built two slices earlier, was
documented at length in the Rust enum, and was invisible in the file an author actually
reads before writing a claim. A count of a list is a cheap thing to check and it caught a
missing list entry.

**Two counts, one label.** The `unledgered` entries for `what-it-cost` and `past-empty`
said 20 and 14, and the file holds 19 and 13 claims on those steps — plus one arm each.
They were counted with a tool that swept `[[arm]]` blocks too, under a label that says
"claims". Not drift: a different instrument. The plain reading of the label wins, so they
are 19 and 13, and the failure message names the hazard so the next author counting by
hand does not repeat it.

## Verification

Nine perturbations, each requiring a non-zero **exit code** and the expected assertion in
the output; the harness also fails a case whose search pattern matches zero or several
times, because a scripted edit that matches nothing is silently green. Both directions,
which is what makes the result mean anything:

* **Prose side** (the sentence moves, the file does not): a tally number edited in each
  file, a summary sentence reworded, a per-step count edited, a ledgered step's numeral
  count edited, and a `NOT_DERIVED` sentence reworded. All red.
* **File side** (the contents move, the sentence does not): one claim's `quoted` flipped
  to false, one claim's `tol_from` moved from `spelled` to `tighter`, one claim removed.
  All red, each naming the sentence that had stopped being true.

The file-side three are the ones that matter: they prove the check tracks the file rather
than just the paragraph. One of them was also validated by hand — flipping `quoted` on the
481 mV claim reddens with "this file's own prose should say `in all 52 claims below that
set `quoted``".

## Deferred, with a price

* **Coverage is opt-in and cannot be made exhaustive by this design.** Nothing decides
  automatically that "24" in a sentence is a count of lessons rather than a temperature.
  A new self-count added tomorrow joins the unchecked majority in silence — the same
  weakness the ledger's `unledgered` list has, and the same answer: it is written down.
* **`NOT_DERIVED` phrases are pinned by presence, not by count.** Where the sentence lives
  in `path_claims.rs`, the table's own copy of it is one of the matches, so the assertion
  is "more than the table's own" rather than "exactly one". A duplicate of one of those
  sentences would pass.
* **A tally with a plural in it can be forced into bad English.** "the translation, two
  entries" is rendered from `WORD_NUMERALS.len()`; if it ever drops back to one, the check
  demands "one entries" until the phrase is reworded. The tally is what tells the author,
  which is the right time to find out, but the phrase is not grammar-aware and will not
  become so.
* **Nothing checks the counts in the plan docs.** `docs/plans/*.md` state figures about
  the claims file constantly and none of them is derived. They are a record of a moment
  rather than a description of the tree, which is the argument for leaving them — but it
  is the same argument that was made about the header before it went stale.
