# The panel's own numbers: closing the formatter gap

Status: **landed 2026-08-12.** Follows `path-claims.md`, which closed the "nothing here
is asserted by a test" hole and left this one open at the top of its deferred list:

> **The formatter gap is the real remaining hole**, and it is the one that produced two
> defects in the most recent slice to look. Closing it means running the page's
> formatters, so it is a browser slice or a port of `fmtTime` and friends — and a port is
> a third source of truth, which is exactly what `MIRRORED` exists to discourage.

---

## What the gap was

`web/path-claims.toml` checks a lesson's number three ways: the sentence still contains
it, the engine still produces it, and the run still reaches the moment it is read at.
None of those three knows what the *page* shows at that moment.

That is not a theoretical gap. Both defects found by the previous slice were of exactly
this shape — the engine was right, the sentence was arithmetically right about the
engine, and the reader could not see it:

* `fmtTime` prints whole minutes above 120 s, so a sentence saying "at t = 983.0 s" is
  pointing at a clock that reads `16m`.
* `soc (true)` prints one decimal, so it still reads `0.0 %` at the instant a sentence
  said the charge level had come back — the cell is 0.004 % full there, and the row does
  not reach `0.1 %` for another two seconds.

The general form: **a readout row is a step function of the number behind it.** The value
check compares against a tolerance, and a drift well inside that tolerance can still move
the printed digit. `99.45 %` becoming `99.44 %` is invisible to a check with a tolerance
of 5e-5 on the fraction, and visible to every reader.

---

## Three ways to close it, and why this one

| | runs in `cargo test --workspace`? | sources of truth |
|---|---|---|
| Drive the real page in a browser | no — needs Chrome | one |
| Evaluate `app.js` under a JS engine in the test | yes, with a large new dependency | one |
| Mirror the formatters in Rust, pinned line by line | yes | two, pinned |

The browser option fails the constraint that decides this: a check that needs a toolchain
the default gate does not have is a check that does not run, which is the disease this
whole file was built to cure. Node is on this machine; it is not in `cargo test`, and a
skip-if-absent test degrades toward green.

The JS-engine option (`boa_engine`) keeps one source of truth, and was declined on cost
that is real rather than speculative: it is a large build every clone pays, its
availability offline was unverified, and the formatters are **not** one scrapeable region
— `K` and `toC` are at line 300, `fmtTime` at 922, `READOUTS` at 1004, `gapPts`/`isPorous`
at 1090. Extracting them would mean adding sentinel comments to `app.js` *and* an engine.

The mirror is what the file already does twice. `pulsePhase` and `ccCvDemand` are
reimplemented in Rust and every constant they depend on is pinned in `MIRRORED`, which is
not a discouragement of mirroring but the thing that makes mirroring safe. This slice adds
twenty-three rows to that table: absolute zero, `toC`, `fmtTime`'s four branches, thirteen
readout rows, two quiet placeholders, and — deliberately — the two rows it does *not*
mirror, so that a page change turning one of them into a telemetry-only row is noticed
instead of leaving a mirrorable row unmirrored.

---

## What landed

**`web/path-claims.toml`** gains three optional fields:

* `display` — a readout row label, exactly as `READOUTS` labels it.
* `shows` — what that row prints at `read_at_s`, as a string, unit and spacing included.
* `quoted` — the prose quotes that string verbatim, so the test also demands it be in the
  sentence. This is the authoring rule "a sentence that tells the reader to look at a row
  says what the row says", made checkable one claim at a time.

Half a display claim is rejected: `display` without `shows` asserts nothing while looking
like coverage, `shows` without `display` names no row, and `quoted` without either has
nothing to quote. All three panic.

**`crates/sim-data/tests/path_claims.rs`** gains the mirror (`to_fixed`, `fmt_time`,
`to_c`, `render_row`), two new quantities (`q_gen_at`, `i_rejected_at`), and two
assertions: the rendered string must equal `shows`, checked inside the existing grouped
run, and — under `quoted` — `shows` must appear in the prose, checked in the literal test
where no run is needed.

Coverage goes from **11 claims over 4 steps to 32 over 7**. The three new steps are 10
(the CC-CV charge with no constant-voltage leg — also the first claimed step whose demand
is the page's own controller rather than a plain current), 20 (past empty) and 21 (what
over-discharge costs).

### `toFixed` is not `{:.*}`

The one piece of the mirror that is not transcription. ECMA-262 splits the sign off first
and then picks, of the two candidates equally near the value, **the larger** — round-half-
up on the magnitude. Rust rounds half to **even**. They differ only on exact ties, and
exact ties are reachable here: `fmtTime` divides simulation time by 60, and a half-second
grid lands on `x.5` minutes at every odd half-minute. `format!("{:.0}", 4230.0/60.0)` is
`70`; the page shows `71m`.

The rounding is therefore done on the decimal string, not by scaling the float. Rust's
formatter renders a double's *exact* decimal expansion given enough places (a binary
fraction always terminates in decimal, in at most 1074 places), so the digit at the cut is
the true one and the rule is simply "round up if it is 5 or more".

Twenty-two cases pin it, and **every expectation was produced by node v24 rather than
reasoned about** — which was worth doing, because one of the predictions was wrong:
`(9.9995).toFixed(3)` is `9.999`, not `10.000`, since the double is really
9.99949999999999938. Its neighbour `(99.995).toFixed(2)` *is* `100.00`, because that
double is really 99.99500000000000454. Nothing about the decimal a human types predicts
which; only the expansion does. The shipped test needs no node — the table is static.

---

## What it found

**One prose defect, in step 10.** The sentence named the `heat` readout and gave figures
the row cannot print:

> The `clamp` readout starts saying `refused 1.150 A`, and `heat` steps from 0.041 W to
> 4.181 W

The row is `toFixed(2)`. A reader watching it sees `0.04 W` step to `4.18 W`. Now:

> ... and `heat` steps from `0.04 W` to `4.18 W` — two decimals is all that row prints,
> and the figures behind them are 0.041 and 4.181 W

Both halves are now claimed: `shows` holds the row's string with `quoted = true`, and
`value` holds the engine's figure with a tolerance set by the finer number the sentence
still states.

**And one defect in this slice's own first draft, caught by the check being built.** The
`shows` for step 20's collapse was written by rounding the *sentence's* number — 1.2055 →
`1.206 V`. The engine is at 1.205497, so the row prints `1.205 V`. Rounding a rounded
number is precisely how a display claim goes wrong, and it is the empirical case for the
rule the check is built on: **format the value you just measured, never the value the
claim stores.** Had the assertion formatted `claim.value`, this draft would have shipped
green and wrong.

Everything else agreed. The eleven pre-existing claims all render as their sentences imply,
and step 21 — the step the previous slice hand-measured through the formatters — is exact
in all ten of its new claims. That is the expected outcome for prose that was measured
carefully once; the point of this slice is that it stays that way without anyone
remembering to measure it again.

Two findings worth recording that are not defects:

* The clock reads `69m` at three separate claimed knees — steps 1, 2 and 20 — including
  the two whose 7.5 s difference is the whole point of the comparison between them. The
  sentences give seconds because the panel cannot; that is now recorded rather than
  assumed.
* `protection-on`'s current claim has a tolerance of 0.2 A — two hundred times the last
  digit the row prints — because the prose hedges ("just under 14 A"). The value check
  there is structurally unable to see the panel change. It is the clearest case for the
  display half being a separate assertion rather than a tighter tolerance.

---

## Reddening

Twelve perturbations, each applied alone, run alone, and reverted from a byte-for-byte
backup. Harness: `M:\claud_projects\temp\path-display\redden.py`.

| # | perturbation | test | expected | got |
|---|---|---|---|---|
| 1 | `to_fixed` rounds ties down instead of away from zero | `to_fixed_matches_javascript` | red | red |
| 2 | `fmt_time`'s seconds/minutes boundary moved to 121 | `fmt_time_matches_the_page` | red | red |
| 3 | `soh cap`'s decimals changed on the page | `mirrored_constants_still_match_the_page` | red | red |
| 4 | the unmirrored `past empty` row stops reading `soc_deficit` | same | red | red |
| 5 | a claim has `display` and no `shows` | `every_claim_matches_the_engine` | red | red |
| 6 | a claim has `shows` and no `display` | same | red | red |
| 7 | a claim is `quoted` with neither | `every_claim_appears_in_its_own_step` | red | red |
| 8 | `shows` disagrees with the row by one digit | `every_claim_matches_the_engine` | red | red |
| 9 | a claim is `quoted` whose string the prose does not contain | `every_claim_appears_in_its_own_step` | red | red |
| 10 | the typographic-minus normalisation removed | same | red | red |
| 11 | a claim names `past empty` | `every_claim_matches_the_engine` | red | red |
| 12 | a claim's `value` moved inside its tolerance | same | **green** | green |

Case 12 is a green result and not a gap: it is the demonstration that the display
assertion reads the engine and never `value`. Moving `protection-on`'s stored value from
13.8207 to 13.75 leaves every check green, because the row is still rendered from the
13.820706 the engine produced. A version that formatted `value` would print `13.750 A`
there.

Case 10 was additionally hand-validated by eye, because a harness that reads its own
output rather than an exit code has lied in this repo before — the failure message names
the right claim (`what-it-cost`, the `terminal` row, `-0.065 V`) and the right reason.

**The harness itself failed first, in the way this repo keeps rediscovering.** Its first
version launched cargo as `cmd /c start /belownormal /b /wait cargo test`, which is the
form this project uses everywhere to keep test runs off the user's CPU — and `start` does
not propagate the child's exit code. Every case came back exit 0. It reported 1/12 while
its own captured output said `test failed` in eleven of them. Fixed by launching cargo
directly with `creationflags=BELOW_NORMAL_PRIORITY_CLASS`, which sets the priority on the
process rather than on a wrapper. **`start /wait` is not a way to run something you intend
to check the status of.**

---

## Versions

**Nothing moves.** No engine state, no wire field, no stored layout, no schema. `web/pkg`
needs no rebuild: no Rust that the wasm bundle embeds changed — the Rust in this diff is a
test. The one page change is a sentence of prose.

---

## Deferred, with a price

* **The charge legs of steps 20 and 21 are unreachable by this harness**, and they are
  where both original defects live. Those legs begin when the reader puts the demand box
  to −2 A mid-run; the harness drives one demand program per step. The `0.0 %` defect
  itself is therefore still not asserted anywhere — only the class it belongs to is.
  Closing it means teaching the mirror a scripted demand *sequence*, which is a second
  mirror of `applyStep`'s controls rather than of its formatters.
* **`past empty` and `surface gap` cannot be claimed at all.** Both are formatted from
  per-cell state rather than telemetry, and `past empty` is sampled on a 250 ms *wall*-
  clock throttle, so "what it shows at simulation time t" is not a function of t. Step
  21's own prose has to warn a reader about that lag. Claiming those rows needs per-cell
  state carried through the run and an honest model of the throttle; naming one today
  panics rather than passing.
* **Fourteen of twenty-one steps still carry no claim**, down from seventeen. Unchecked,
  not passing.
* **`quoted` is opt-in and nothing suggests where it is missing.** A sentence that tells a
  reader to watch a row and does not quote it is exactly the defect class this slice is
  about, and finding those is still a human reading the prose.
* **`quoted` also asserts nothing today that `literal` does not.** In all 32 claims
  `shows` is a substring of `literal`, and `literal` must already appear in the prose, so
  the quoted check follows from it. The tell is in the reddening table above: case 9 could
  only be made red by setting `quoted` on a claim that does not have it, a configuration
  no shipped claim uses. It is kept as a forward guard and as the place the authoring rule
  is written down, and it starts doing independent work the first time a sentence quotes a
  row string outside its own claimed literal. Recorded here rather than left for a reader
  to infer coverage that is not there.
* **`tol` still has no enforcement**, and this slice instantiated the hazard on its own
  first new claim: the two `heat` entries were written with a tolerance that matched
  neither the sentence's precision nor the stored value's. They now follow the file's rule
  (the sentence's number, half a unit in its last place), but nothing would have caught
  it — the previous slice named this and it is still true.
* **The mirror is a second source of truth for thirteen row formatters.** `MIRRORED` makes
  a page-side change fail loudly, which is the whole defence — but a change made on *both*
  sides carelessly still passes, and the mirror cannot notice a row being added.
