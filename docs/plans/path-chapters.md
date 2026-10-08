# The guided path, in chapters

*2026-10-08. Owner decision; a client-only slice. No engine, scenario or claim changed.*

## The question

`docs/ROADMAP.md` (§4) asked for a decision before Phases 10 and 11 add lessons: the path had
grown to 37 steps with no argument about its shape, and the honest options were a longer
single path or short tracks per theme. Three were put to the owner:

| option | cost | what it buys |
| --- | --- | --- |
| **chapters over the existing order** | one small slice | a reader can find and jump to a topic; nothing reordered |
| separate themed tracks | several slices | each track stands alone — but the lessons are reordered, the prose's cross-references by step number (41 of them, counted with `grep -o "step [0-9]+"`) rewritten, and lessons that lean on the one before them ("two steps from now…") rewritten to stand alone |
| one long path, as it was | none | nothing; Phase 10's lessons go on the end |

**The owner chose chapters over the existing order.**

## What was built

* `const CHAPTERS` in `web/app.js`, right after `const LESSONS`: eight `{ first, title }`
  records, each naming its chapter's first lesson **by id**. Outside the lesson array on
  purpose: `path_claims.rs` splits `LESSONS` on its `id:` fields and reads every string
  literal in a block as that lesson's prose, so a chapter title inside a lesson record
  would have entered the uniqueness and number scans as a sentence the reader is shown.
  Keyed by id, an inserted lesson cannot slide a boundary.
* The path panel shows `chapter N of M — title` above the lesson title, computed from the
  list. `step N of 37` stays as it was: steps keep **one numbering across the whole
  path**, because the prose addresses its neighbours by those numbers.
* A chapter menu beside Back and Next. Choosing a chapter goes to its first step **on a
  freshly built pack** (`applyStep(L, fresh = true)`), whatever step the reader was on.
* The sidebar note under *Start* and the README's path paragraph say the chapters exist.

The chapters, by first step: what a cell does (1), a pack and its gauge and protection
(3), wear and charging (8), inside the cell (12), abuse — shorts and past empty (18),
other chemistries (22), gauges that read the curve (30), LFP's particles (35).

## Why a jump always rebuilds

Every chapter happens to open on a step whose scenario differs from the step before it, so
arriving by Next already reloads (`applyStep` reloads on a changed scenario). A jump is not
arriving by Next: it can come from anywhere. Paused part-way through step 4 at 66 s, the
chapter menu's "chapter 2" goes to step 3 — **the same scenario**, with the clock short of
step 3's 300 s mark — so neither reload clause fires and step 3 would have carried on with
step 4's half-run pack. Hence `fresh`.

Measured in the page (headless Chrome over CDP, the server's `/app/` route):

| arm | clock before the jump | clock just after |
| --- | --- | --- |
| with `fresh` | 66 s | 7 s (rebuilt, running from zero) |
| control: `fresh` forced to `false` | 67 s | 67 s (inherited) |

Also checked there: eight menu entries in order; Start shows chapter 1 / step 1 of 37;
Next across a boundary moves the chapter line and the menu; Back across one moves them
back; jumping to the last chapter lands on step 35; Exit restores `Start — 37 steps`, and
Start again opens chapter 1. No console errors.

## The test

`crates/sim-data/tests/path_chapters.rs`, scraping `web/app.js` the way the claims test
does. Perturbation table — each break made on a copy of the file, restored and compared
byte for byte afterwards:

| perturbation | test that reddened |
| --- | --- |
| chapter 2 starts at step 4, which shares step 3's scenario | `every_chapter_opens_on_a_step_that_builds_its_own_pack`, with its own message |
| a `first` that names no lesson | `every_chapter_names_a_lesson_and_they_run_in_order` (the pack test panics too, on the same missing id) |
| the last chapter's `first` moved to step 9 (out of order) | `every_chapter_names_a_lesson_and_they_run_in_order` |
| a title with a count word ("Four other chemistries") | `chapter_titles_state_no_count` |
| chapter 1 starts at step 2 | `every_chapter_names_a_lesson_and_they_run_in_order` |

`path_claims` (69 tests) still green: nothing it reads moved.

## Deliberately not done

* **No reordering.** Step 8 (wear while idle) sits in the charging chapter because that is
  where it runs. When Phase 10 adds aging lessons, an aging chapter is the natural home
  for it — moving it then changes the step numbers the prose quotes, so it is that
  slice's decision, not this one's.
* **No per-chapter numbering** ("chapter 3, step 2"): the 41 cross-references would all
  need rewriting.
* **No chapter count in words anywhere.** The page computes it; titles are tested for
  digits and number words.
