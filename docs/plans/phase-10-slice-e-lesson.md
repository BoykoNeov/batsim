# Phase 10, slice E — the shelf, drawn and grown

**Status: BUILT 2026-10-15. Phase 10 is complete.** Plan: `phase-10-sei.md` §E and its last
addendum, which records the measurements taken before asking and the owner's three decisions.
Harness (out of tree, not kept): `W:/temp/claude/phase10e/` — `measure/` the probe, `walk.mjs`
the browser walk, `perturb.py` the table below.

## The owner's decisions, recorded

Asked and answered on 2026-10-15, before any lesson prose (full text in `phase-10-sei.md`):

1. **A new chapter at the end; step 8 stays where it is.** Nothing renumbers. Step 8 runs the
   equivalent circuit, which `Pack::new` refuses the film on, so it could not be one half of
   the comparison; step 38 points back to it.
2. **Raise the page's top speed** to 1 000 000×, these steps at an hour a step.
3. **Three steps**, one LG M50 `Spm` cell resting at 25 °C for 365 days: full under the drawn
   curve, full under the film, at 30 % under the film with the drawn curve's same comparison
   as a control arm.

## What was built

* **Four scenario files**, `shelf_drawn_full`, `shelf_film_full`, `shelf_film_low`,
  `shelf_drawn_low`: one `PackConfig` with `calendar` or `initial_soc` changed, which
  `shelf_files_differ_only_where_their_headers_say` asserts. 1S1P, `Spm` with 20 shells,
  isothermal at 298.15 K, no BMS, no scatter, no faults. The headers carry the table below.
* **Guided-path steps 38–40** (`drawn-on-the-shelf`, `grown-on-the-shelf`, `stored-low`) in a
  ninth chapter, "Aging, drawn and grown". `Rest`, `dt = 3600`, `speed_x = 1000000`,
  `until_s = 31536000`. Every numeral is tied: 17 claims (2 of them on the one arm), 24 ledger
  rules; all three steps are in the ledger whole (16, 11 and 21 numbers) and in
  `spelled` at 0. The word "year" is in `BANNED_UNITS`, so the prose says "365 days", and the
  harness gained `Tie::Days` (`Tie::Hours` with 86 400) to tie it to the mark.
* **`crates/sim-data/tests/shelf_lesson.rs`**, for the sentences with no number: the four files
  are one; the room is pinned (an ambient 20 K hotter moves nothing, bit for bit); the film
  stored full lies between `√t` and a straight line and nearer `√t` (2.24 for four times the
  time), stored low nearer the line (2.98); full over low falls with the span (11.7 at 30 days,
  8.8 at the quarter, 6.65 at the mark); at 85 % the film loses what it loses full, to 1 %.
* **Three page changes**, each found by this slice:
  * **The speed slider reaches 10⁶×** (`max="6"`), **held** above 10 000× to 20 000 steps a
    second (`heldSpeed`): a million times real time at an hour a step, never two million
    steps a second at the default half second. At or below 10 000× nothing is held, so every
    multiplier the page offered before runs as it did. The label says when it is held
    (`1000000×, held to 10000× at this dt,`).
  * **A step without `dt` puts back the default when an earlier *step* wrote the box.** Before,
    it kept whatever the last step left: a chapter jump from step 35 to step 1 already ran
    step 1 at 10 s steps while every number on it was measured at 0.5, and from the shelf it
    would have run at an hour. A length the reader typed is theirs and stays (walked: typed 3,
    Next, still 3).
  * **On a step that opts in (`rearm: true`), Load, Restart and the BMS toggle re-arm the
    step's mark** (`rearmMark`). The prose sends readers to "run it to the same mark" and
    nothing stopped them there; at 1 000 000× the control arm ran on to 374.5 days before a
    hand could pause it, and the 90.66 % the prose quotes was out of reach. **Opt-in, after a
    review caught the first version re-arming every step**: an audit of all 33 arms that
    rebuild the pack found nine that run past their step's mark, and four of those (steps 2,
    14, 16 and 25 — to a cut-off or a second leg) are written for a run that carries on, so a
    stop there would be a pause the prose never mentions. Steps 40, 11 and 19 opt in. Older steps were already written as if it stopped — step 19 says "Press Run at the
    mark" after a BMS restart, and step 11 "press Run again when the mark passes" after one.
    Re-arming makes both sentences true where they were not; no claim moved, because the
    harness has always run an arm to its stated `to_s`.
* README's guided-path paragraph, the `Start — 40 steps` button and the harness's self-counts.

## The numbers, measured on the page's shape

Out-of-tree probe (`measure/`, the scenario loader, `Demand::Rest`, 1 h steps):

| file | `soh cap` at the quarter (7884000 s) | at the mark (31536000 s) | `soh res` at the mark |
| --- | --- | --- | --- |
| `shelf_drawn_full` | 92.07 % | 84.14 % | 1.2378 × |
| `shelf_film_full` | 99.37 % | 98.60 % | 1.0140 × |
| `shelf_film_low` | 99.93 % | 99.79 % | 1.0033 × |
| `shelf_drawn_low` | 95.33 % | 90.66 % | 1.1401 × |

600 s and 60 s steps give the same values to seven digits; no flag is raised; a year costs about
40 ms native.

### Four things the draft got wrong

1. **2.98, not 2.97.** The draft's ratio for the film stored low was worked out from rounded
   readings; the ledger computes it from the claimed values, 2.9753.
2. **The control arm's reading was unreachable** until the mark was re-armed (above). The first
   browser walk read `soh cap` at 374.5 days.
3. **"At 85 % the film loses what it loses full, to 0.03 %"** was written into a scenario header
   from three-decimal readings. Measured, 1.4036 against 1.4027 points: 0.06 %.
4. **"Its resistivity is PyBaMM's value, labelled a placeholder"** was written into the headers
   from the plan's decision 3. The chemistry file says `FITTED` (5.594e4 Ω·m, slice A's joint
   fit to the WMG pulse resistance, flagged weak). Read off `[spm.sei]` itself; corrected. The
   plan's text was superseded by slice A and was never the shipped value.

### What the slice found and recorded, not fixed

* **The spike's "about 14 % resistance per 1 % capacity" is not what the engine reports** (about
  1.0 at full and 25 °C, up to about 3 at 85 % and 45 °C). Most of it is stale: the spike used
  PyBaMM's resistivity (2e5 Ω·m) and slice A refitted it to 5.594e4, 3.6× less. The rest is the
  divisor — a 10 s pulse resistance there, the `Spm`'s own lumped resistance at the cell's state
  here. The lesson quotes the readout, not the ratio. Recorded in `phase-10-sei.md`.
* **`soh res` is not comparable across storage levels**: at full and at 85 % the film is the same
  to three decimals of capacity and the readout says 1.0140 against 1.0205, because the reference
  resistance moves with state. The lesson compares resistance only at one charge level.
* **Stored at 30 % the film's lithium comes out of the stored charge**: `soc (true)` reads 29.9 %
  at the mark. Stored full it reads 100 % throughout, because the loss is counted off the top of
  the range. In the headers; not in the lesson.

## Perturbations

Predictions written into the harness before the run; each case edits one file, runs
`path_claims`, `shelf_lesson`, `path_chapters` and `scenario` (93 tests) below normal priority,
and restores the file from the bytes read before the edit, checked byte for byte.

| case | change | predicted to redden | reddened |
| --- | --- | --- | --- |
| **A** | step 39's `98.60 %` printed `98.61 %` | the literal check, the ledger | `every_claim_appears_in_its_own_step`, `every_numeral_in_a_ledgered_step_is_accounted_for` — held |
| **B** | `shelf_film_low` at 35 % | the file-pair test, the claims, the 30 % rules | those three, **and** three `shelf_lesson` tests that read the low file — more than predicted |
| **C** | `shelf_film_full` back to the drawn curve | the file-pair test, the claims, three `shelf_lesson` tests | all but `the_gap_between_full_and_low_narrows_with_time`, which stays green: the drawn curve over the film stored low also falls with the span |
| **D** | `[spm.sei] d_li` raised 30 % | the claims on steps 39–40 | `every_claim_matches_the_engine` alone — held; the shape tests do not see a size change, by design |
| **E** | step 38's `speed_x` 100 000 | the ledger (the speed and the 32 s) | the ledger — held |
| **F** | `Tie::Days` divides by 3600 | the ledger (every 365) | the ledger — held |
| **G** | step 40's `2.98` written `2.97` | the ledger | the ledger — held |
| **H** | control: an unclaimed sentence reworded | nothing | nothing — held |
| **I** | step 39's `dt` 3600 → 1800 | nothing (a stated gap) | nothing: no prose on step 39 prints its step length and the film agrees to seven digits at 1800 s, so nothing *should* see it |

The page changes (`heldSpeed`, the `dt` reset, `rearmMark`) have no Rust-side test; they are
what the browser walk below measures.

## Gates

* `cargo test --workspace --no-fail-fast` (below normal priority): **815 passed, 0 failed**, 98
  test binaries. `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --all --check` clean.
* `web/pkg` rebuilt (it was from 2026-10-08, before Phase 10's engine); `node
  tools/wasm-parity/parity.mjs`: **172/172 bit-identical** (86 runs, 86 snapshot handoffs), the
  four shelf files among them.
* **Walked in a real page** (`walk.mjs`): `sim-server` on its own port, headless Chrome on its own
  profile and port. Start shows `Start — 40 steps`; the chapter menu, driven by a typed digit,
  lists nine chapters and lands on step 38. Each step reaches 365.0 d in 30 s of wall clock with
  `dt` 3600 and the label `1000000×`, and reads: step 38 `84.14 %` / `1.2378 ×`; step 39
  `98.60 %` / `1.0140 ×`; step 40 `99.79 %` / `1.0033 ×`. The picker offers the four files;
  `shelf_drawn_low` loaded and run stops at 365.0 d and reads `90.66 %`; on step 39, which does
  not opt in, Restart then Run carries on past the mark (463 d after 40 s), as every older step does. Jumping to chapter 1
  puts `dt` back to 0.5; the slider at its top then reads `1000000×, held to 10000× at this dt,`;
  a typed `dt` of 3 survives Next. No console errors. Chrome closed through CDP, the server
  stopped by the PID recorded at launch.

## What this slice does not do

* **The arm is walked, the quarter-mark readings are not.** The walk reads the four panels at the
  mark; the quarter readings are claims checked natively in `path_claims.rs`.
* **No temperature step.** The owner chose three steps over a fourth at 45 °C; step 8 teaches
  temperature, and these files are isothermal.
* **The prose does not state the WMG cells' near-full numbers.** It says the cells "do not settle"
  whether a little below full is kinder, which is `phase-10-sei.md`'s exit 5 addendum; no test
  holds those numbers, so no numeral says them.
* **The drawn curve's placeholder is not refitted.** The lesson says the size gap is the
  placeholder's; fitting `[aging]`'s calendar pair would move every LG M50 aging number in the
  suite, and is Phase 11's kind of work.
