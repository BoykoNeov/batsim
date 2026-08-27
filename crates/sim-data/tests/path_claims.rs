//! Check the numbers the guided path's lesson prose claims, against the engine that
//! is supposed to produce them.
//!
//! # What this is for
//!
//! `web/app.js`'s `const LESSONS` is 24 teaching steps whose prose states hundreds of
//! specific quantities. Until this test existed, not one of them was checked by
//! anything in the repo. Four slices found numbers in that prose that had drifted, or
//! were never true, or were true about a quantity no reader can see — and every one of
//! those findings came from an instrument that lived outside the tree and never ran
//! again. `web/path-claims.toml` is that instrument's findings turned into assertions;
//! read its header for the checks it describes and why the literal is stored as a string
//! rather than formatted from the value. It said "the four checks" until this sentence was
//! swept: that count was written when there were four, and the header it points at has
//! listed six for five slices. It carries no number now, which is the other way to stop a
//! count rotting — the one this file already took for the `grid` tally's twin.
//!
//! # The six checks, and why none of them is redundant
//!
//! * **Literal** — the claim's text appears verbatim in that step's prose. A prose edit
//!   that changes the number now fails here even though the engine never moved. This is
//!   the half that would have caught all four historical failures; a golden-value table
//!   would have caught none of them.
//! * **Value** — the engine, driven the way `applyStep` drives it, produces the number.
//! * **Stated** — the number the *sentence* prints is the number the engine produced,
//!   read through the frame the sentence uses it in ([`States`]). This is the check that
//!   joins the two above, and until it existed they were two green halves with nothing
//!   between them: `literal` is a substring test against the prose and `value` is a
//!   comparison against the engine, so re-measuring a drifted `value` without re-wording
//!   the sentence left both passing and the prose wrong. It is not the value check at
//!   lower precision either — it runs the other way, from the prose's digits to the
//!   engine's, and the frames are not all scales: `0.53 points are gone` is a
//!   complement, `refused 0.822 A` a magnitude, `383.0 s later` a duration off the mark.
//! * **Reachable** — the claim is read at a time the step actually runs to. "Right but
//!   unreachable" is a defect class this repo has shipped twice.
//! * **Displayed** — optional, and the newest. A claim may name a readout row; the row's
//!   own formatter, mirrored from `web/app.js`, must then turn that instant's telemetry
//!   into exactly the string the claim records in `shows`. This is not the value check
//!   again at lower precision: a panel row is a *step function* of the value, so a drift
//!   well inside a claim's tolerance can still flip `0.0 %` to `0.1 %` or `99.45` to
//!   `99.44` and break the sentence around it. Two shipped defects were of exactly that
//!   shape — a time of 983.0 s that the clock renders `16m`, and a charge level the SOC
//!   row still prints as `0.0 %` at the instant the prose says it has arrived. The
//!   formatters are mirrored rather than executed, on the same terms as `pulsePhase` and
//!   `ccCvDemand` below: every one of them is pinned in [`MIRRORED`], so the page cannot
//!   change a decimal place without failing here. Running the real ones would need a
//!   JavaScript engine in the default `cargo test` gate, or a browser, and a check that
//!   needs either is a check that does not run.
//!   With `quoted = true` the claim additionally asserts that `shows` appears verbatim in
//!   the prose — the authoring rule "when a sentence tells the reader to read a row, it
//!   quotes what that row prints", made checkable one claim at a time. On today's claims
//!   that assertion is *implied* by the literal check, because every `quoted` claim's
//!   `shows` sits inside its own `literal` — re-verified when the charge-leg slice added
//!   ten more of them; it is a forward guard and a statement of the rule,
//!   not an independent result. Said plainly here because an uncovered check under a green
//!   test reads as a covering one.
//! * **Accounted** — every number a claimed sentence prints is tied to something. The
//!   five above are all about the number a claim *spells*, and said nothing about the
//!   other figures in the same sentence: `**99.98 %** when the cell empties at **207.5 s
//!   and 1.9306 V**` carried three numbers and two claims, and the third was checked only
//!   as characters that had to still be there — so the prose and the literal could drift
//!   to `210 s` together with every check green. That is the *original* hole this file was
//!   built to close, surviving on the one number in a claimed sentence nobody had claimed.
//!   [`every_number_in_a_claimed_literal_is_accounted_for`] scans each literal and
//!   requires an [`Accounted`] for every figure in it. The accounting is *derived*, not
//!   declared — unlike `states` and `tol_from`, each arm is an exact numeric fact, and a
//!   declaration that could disagree with the fact would be a fresh instance of the very
//!   defect `tol_from` exists to catch. There is no waiver variant, on purpose.
//!   One arm needs a trajectory and so is fenced from inside the value check: an instant
//!   the run raises a *flag* at is an event, and a sentence that names when something
//!   happens is claiming that moment rather than merely reading a row at it. That fence is
//!   not decoration — without it, deleting the `207.5 s` claim this check was written to
//!   force left the whole suite green.
//!
//! Beside all six sits **the ledger**, which is about a *step* rather than about a claim:
//! [`every_numeral_in_a_ledgered_step_is_accounted_for`] scans a step's whole prose and
//! requires every numeral in it to be tied to something, claimed or not. **Numeral used to
//! be the operative word**, and for as long as it was, a quantity spelled in English was
//! invisible: "about half a point across the whole grid", "a gap of about three points",
//! step 14's "about ten times the circuit's arithmetic per step". A ledgered step was
//! digits-closed, which is less than closed.
//!
//! **The answer is now a rule about the prose rather than a second scanner.**
//! [`no_lesson_spells_a_quantity_in_english`] refuses a quantity spelled in letters
//! anywhere in a lesson — the digits rule — so "numeral" and "quantity" mean the same thing
//! in the prose the ledger scans, and the digit scanner, which has covered all twenty-four
//! steps for many slices, is the whole of the coverage. `docs/plans/phase-8-chemistries.md`
//! records the decision and `docs/plans/path-digits-rule.md` the slice: reading English was
//! seven of twenty-four steps after two slices, and finishing it was seventeen more rounds
//! of the same work. The argument for banning instead is about which way the mistakes fall.
//! A false alarm under a ban is one sentence rewritten by its own author that minute; a miss
//! under a reader is silent, and a wrong number ships behind a green suite.
//!
//! [`spelled_numbers`] survives as the READER it always was, opt-in per step through the
//! ledger's `spelled` list and merged into the same scan so that one number cannot have two
//! readings. It finds a numeral and a measure noun ("three minutes", "half an hour", "an
//! eighteen-minute discharge"), a numeral written after its unit ("the first minute and a
//! half"), a numeral written before it with a fraction between ("four and a half seconds"),
//! and a list item carrying a unit an earlier item stated ("5.80 at six"). **It finds nothing
//! at all today**: every quantity it used to read has been rewritten in digits, and the
//! per-step counts beside `spelled` are all zero, which is the result rather than a gap.
//!
//! **What is still spelled in English is named phrase by phrase**, in `[[english]]` in
//! `web/path-claims.toml` — 48 of them across twelve steps, matched both ways so
//! that the list can only get shorter. They are the half that was tied to nothing: rewriting
//! one into digits makes the ledger see it, and the ledger has no waiver, so each is a rule
//! or a claim rather than an edit. Two shapes stay out of the ban and are declared rather
//! than inferred from a green run: the prose counting its own furniture — "two electrodes",
//! "Four footnotes", "six gap figures" — which is a list's length rather than a measure and
//! wants an arm that reads a list, and a count of ENGINE STEPS — "eight steps" — which the
//! run length over `dt` settles. Nothing here says either is tied to anything.
//! Check 6 can only
//! reach the sentences a claim already quotes, and fourteen steps had no claim at all when
//! this was written — which is how six figures in step 19 went stale, and how a contrast in
//! step 14 that never existed survived, both under a fully green suite. Two steps are
//! still in that position. Coverage is opt-in per step
//! (`[ledger]` in `path-claims.toml`) and today it is twenty-four steps and 663 numbers —
//! which for one slice collided with the fourteen above and no longer does: that fourteen
//! is the steps that had no claim when this paragraph was written and is frozen, and this
//! count is the steps scanned whole today, which moves every time one is.
//! Twenty-six arms exist — a scenario field, a chemistry field, a control on the lesson block,
//! the sentence's own arithmetic over those as a product, a ratio, a difference or a sum,
//! one of their durations read in hours, the span of a
//! chemistry table, a node of one, digits inside a name, digits inside the name of a file an
//! arm picks ([`Tie::Picker`]), the label of a control read out of `web/index.html`, how many
//! entries an array of the scenario has, the open-circuit voltage at a charge one of them
//! names, the magnitude of any of them, the position of another lesson,
//! the panel's clock at the step's mark, a constant of the page's own policy parsed out of
//! `web/app.js` ([`Tie::Page`]), a figure the sentence works out from its own
//! siblings, any of those read on **another lesson** ([`Tie::Elsewhere`]), a control read off
//! **one of this step's arms** rather than off the step — or, where that arm loads another
//! file from the picker, a field of **that** file ([`Tie::OnArm`]), a number
//! **another step measured** ([`Tie::Quoted`], which reads that step's claim), **when** that
//! step's claim was read rather than what it came to ([`Tie::Instant`]), a period the page
//! holds said as the rate a sentence writes ([`Tie::PerSecond`]), and a claim
//! whose literal contains the number ([`claimed_accounting`], which is
//! check 6's own accounting asked about a number the ledger found). **None of the taxonomy
//! is missing any more**: the last of its six kinds — the figure derived from its siblings —
//! is [`Tie::Derived`], built for step 22's "six of these in series is the 12 V battery".
//! Check 6 has an arm of that name too; the two scans are separate, as they are for
//! `setting`. The rest of the design is in `docs/plans/path-prose-ledger.md`.
//!
//! Behind the value check sits a seventh, about this file rather than about the page:
//! [`every_tolerance_follows_its_declared_rule`]. `tol` is what decides how much of a
//! claim is actually claimed, and it was the one field nothing checked — a careless value
//! makes the value check pass on anything. Each claim now declares which rule its
//! tolerance follows ([`TolFrom`]) and the derivation is checked, because the same defect
//! — a note citing "half a unit in the last printed place" beside a tolerance that was a
//! half-step — shipped in two consecutive commits, the second on the slice fixing the
//! first.
//!
//! One more check is about *prose* rather than about any claim, and it is the one that
//! keeps the paragraphs you are reading honest:
//! [`every_count_these_files_state_about_themselves_is_derived`]. Both this file and
//! `path-claims.toml` describe their own contents in numbers — how many claims take each
//! tolerance rule, how many sentences check 6 scans, how many steps the ledger covers,
//! how many claims sit on each step it does not. Every one of those was hand-maintained
//! and none was checked, so they drifted: a slice found the header's tallies wrong by
//! five slices' worth, re-derived them by hand, and left the same hole open behind it.
//! Now the *phrase* is declared and the *number* is derived, which is the contract
//! [`LedgerRule`] and `spells` both keep. It is opt-in per sentence, like the ledger,
//! so the counts it does not derive are written down in [`NOT_DERIVED`] with the reason
//! and the sentence they excuse.
//!
//! # What this test does NOT cover
//!
//! Named here rather than left to be inferred, because an uncovered step under a green
//! test reads as a verified one.
//!
//! * **Only the steps and quantities listed in `path-claims.toml`.** Coverage is
//!   partial by design and grows one measured claim at a time. A step with no entry is
//!   an unchecked step, not a passing one; [`every_covered_step_exists`] guards the
//!   reverse direction only (no claim may name a step that is gone).
//! * **One readout row: `past empty`.** It is formatted from `Pack::cell` output rather
//!   than from telemetry *and* sampled on a 250 ms **wall**-clock throttle, so what it
//!   shows at a given simulation time is not a function of that time at all. It is not
//!   mirrored: a claim naming it panics rather than passing. See [`render_row`].
//!
//!   `surface gap` stood here beside it until the probe slice and no longer does. It is
//!   per-cell in the same way, but it carries no throttle, so it does have a value at a
//!   given simulation time — [`Row`] carries the pair and [`render_row`] prints it.
//! * **The BMS panel's strings, though not its numbers any more.** [`Row`] now carries what
//!   the BMS *measured* as well as what is true ([`Sensed`]), so a claim can read a sensor —
//!   `t_gap_k_at` is the panel's `temperature` gap, belief minus truth, and it is what
//!   settles "protection is late by 1.3 K of somebody else's temperature". What is not
//!   mirrored is that panel's rendering: [`render_row`] covers `READOUTS` and nothing else,
//!   so a claim measured on a sensor may name no `display` and the truth-beside-belief
//!   columns are checked by nothing. Only the temperature channel is carried, because it is
//!   the only one a claim reads — the current and charge channels would be fields with no
//!   consulting code, which is the shape this file rejects everywhere else.
//! * **A slider dragged mid-run, and a sentence needing two packs.** A step may declare any
//!   number of `[[arm]]`s — an instructed control change and the trajectory that follows —
//!   covering the demand box, the `dt` box, the BMS checkbox, the ambient slider,
//!   **Restart**, **Clear queued**, **Clear latched BMS fault**, **Step 1** and **Run**.
//!   `ambient_c` may be dragged **at the mark**: [`run`] keeps two [`Env`]s, the step's
//!   slider before the mark and the arm's after it. That split and the accounting arm that
//!   pays for it were one deadlock — the sentence needing the split prints `20 K` and
//!   `2.7×`, figures derived from their siblings, so neither half was buildable alone.
//!   A sentence comparing two *scenario files* used to be left out here, and is not any
//!   more: [`Arm::pack_from`] names the lesson a reader walks **Back** to, so an arm can
//!   rebuild another lesson's pack under this step's typed current. Step 16's 1 C rerun of
//!   both porous models is what it was built for, and the three numbers that sentence had
//!   to delete are back on the page. What is still left out is the third pack in the same
//!   step — the equivalent circuit's zero-length probe at this current, which no lesson in
//!   the path runs. It is reachable in principle now; what it needs is a lesson to walk to.
//! * **One arm compared against another.** `identical_to` compares two arms' end *states*,
//!   and nothing compares two arms' *events*. Step 11 says a charge inhibit and a plating
//!   flag arrive "at the same instant" on two different trajectories; that equality is
//!   asserted only in the weak sense that two claims pin the same number, so moving both
//!   arrivals together would keep it green.
//! * **How far an arm runs is this file's own choice, and the reachability check on an arm
//!   claim is therefore weaker than the one on a pre-mark claim.** Before the mark the
//!   page stops at `until_s` whatever the reader does, so "reachable" is a fact about the
//!   page. After it the page stops for nothing — `pathArrived` sets `path.until = null` —
//!   so an arm's `run` length is bounded only by what the prose asks the reader to do, and
//!   if it is set to just cover the furthest claim then "reachable" says only "I ran long
//!   enough to reach it". The non-circular half is
//!   [`every_arm_is_instructed_by_its_own_step`]: the sentence telling the reader to make
//!   this exact change must be in this step's prose, and every control the arm overrides
//!   must be anchored in that sentence and must be a real change from the step's own.
//! * **Sentences no claim is about, in the steps the ledger has not reached — none of them.**
//!   Check 6 closed the half of this that lived *inside* a claimed literal, and the ledger
//!   has now closed twenty-four whole steps — but only twenty-four. Steps here carrying
//!   neither a claim nor a ledger entry: none. With claimed sentences checked and the rest
//!   of the prose free: none. `[ledger].unledgered`
//!   names what is left — none of the twenty-four — one line each, so this list cannot go
//!   quietly out of date; it is empty, and it stays in the file so that the next lesson
//!   added to the path has somewhere to say it is not checked.
//!   **What that closes is one axis and not the gap.** Every numeral in every step of the
//!   path is now tied to a file, a control, a claim or a sentence's own arithmetic. What is
//!   still open is what a *digit scan cannot see*: this path states dozens of its
//!   quantities in English — "half a point", "three thousandths of a point", "more than six
//!   times", "about a quarter of the run" — and every one of them is a measurement a reader
//!   leans on. [`every_word_numeral_is_read_by_something`] guards the small table of words
//!   a claim or a rule has actually needed; it says nothing about the ones no claim spells.
//!   Every arm the two scans were waiting for has landed, and that is why the gap that is
//!   left is not an arm: the zero-length probe, instructed control changes, and the last of
//!   check 6's five accounting arms, [`Accounted::Derived`].
//!   **What check 6 still has no arm for is a configured constant** — a threshold a
//!   scenario file declares — which is why step 11's literal still has to stop short of the
//!   fragment naming its `343.15`. Check 6 could
//!   refuse a waiver variant because its claimed sentences happen to need none; a
//!   whole-prose ledger cannot, and it still refuses one.
//! * **Page-behaviour claims.** Anything about what a control does, what a legend
//!   prints, or what a button orders. Those need a browser.
//! * **The client-side demand programs are mirrored, not shared.** `Pulse` and `CcCv`
//!   are policies that live in `web/app.js`; the engine has no such demands. This file
//!   reimplements them, which makes it a second source of truth that can drift from the
//!   first. [`mirrored_constants_still_match_the_page`] pins every constant copied out
//!   of the page so a change there fails here instead of diverging quietly.

use std::path::{Path, PathBuf};

use sim_core::{Demand, Env, Pack, Telemetry};
use sim_data::{parse_chemistry, parse_scenario, Scenario};

/// Kelvin at 0 °C. The lesson files speak Celsius; the engine speaks Kelvin.
const K: f64 = 273.15;

// ---------------------------------------------------------------------------
// The repo, found from the manifest rather than from an absolute path
// ---------------------------------------------------------------------------

/// Repository root, derived from this crate's manifest directory.
///
/// The out-of-tree instrument this test grew from hardcoded `M:\claud_projects\battery`,
/// which is exactly the thing that stopped it being a test.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root resolves from CARGO_MANIFEST_DIR")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn app_js() -> String {
    read(&repo_root().join("web").join("app.js"))
}

fn index_html() -> String {
    read(&repo_root().join("web").join("index.html"))
}

// ---------------------------------------------------------------------------
// Constants copied out of the page
// ---------------------------------------------------------------------------

/// Every value this file mirrors from the browser client, with the source text that
/// must still contain it.
///
/// This table is the whole defence against the mirror drifting from the page. Each row
/// is `(what it is, the file, the literal that must still be there)`.
const MIRRORED: &[(&str, &str, &str)] = &[
    ("default dt", "index.html", r#"id="dt" value="0.5""#),
    // The one row with a reader beside it: [`cccv_period_s`] parses this constant's
    // value, and this pin says the page still SPELLS it the way that parser looks for it.
    // Both are kept because they fail differently — delete the row and a rename becomes an
    // unexplained panic in the parser instead of a named failure here. For six slices this
    // row was the whole of it, and the mirror it was meant to guard was wrong the whole
    // time; that is the history the rest of this file keeps citing.
    (
        "CcCv sub-clock period",
        "app.js",
        "const CCCV_PERIOD_S = 10",
    ),
    (
        "CcCv band per cell",
        "app.js",
        "const CCCV_BAND_V_PER_CELL = 0.001",
    ),
    // The pack-grid sampler's clock, which is also what fills the surface-gap readouts —
    // step 17's "sampled four times a second" is this constant said the other way up.
    (
        "cells sampler period",
        "app.js",
        "const CELLS_PERIOD_MS = 250",
    ),
    // --- the readout formatters, mirrored in `render_row` -------------------
    //
    // One row per line of `web/app.js` that `render_row` reimplements. These are what
    // make the display check a check: a decimal place moved on the page and not here
    // would otherwise leave a green test asserting the page's *old* rendering.
    ("absolute zero", "app.js", "const K = 273.15;"),
    ("toC", "app.js", "const toC = (k) => k - K;"),
    (
        "fmtTime: seconds branch",
        "app.js",
        "if (s < 120) return `${s.toFixed(0)}s`;",
    ),
    (
        "fmtTime: minutes branch",
        "app.js",
        "if (s < 7200) return `${(s / 60).toFixed(0)}m`;",
    ),
    (
        "fmtTime: hours branch",
        "app.js",
        "if (s < 172800) return `${(s / 3600).toFixed(1)}h`;",
    ),
    (
        "fmtTime: days branch",
        "app.js",
        "return `${(s / 86400).toFixed(1)}d`;",
    ),
    (
        "the `sim time` row",
        "app.js",
        r#"["sim time", (m, f) => fmtTime(f.sim_time_s)],"#,
    ),
    (
        "the `terminal` row",
        "app.js",
        r#"["terminal", (m) => `${m.v_terminal.toFixed(3)} V`],"#,
    ),
    // The **Step 1** button takes exactly one step, which is the whole of what
    // `Action::Step1` models. Pinned after a perturbation that made the action advance
    // two steps and left the suite GREEN: the claims on those arms read an instant one
    // step past the mark, and an extra row after it changes nothing they look at. The
    // number of steps a button takes is a fact about the page, so this is where it is
    // held — the same argument as `pulsePhase` and the readout formatters.
    ("the `Step 1` button", "app.js", "await advance(1);"),
    (
        "the `current` row",
        "app.js",
        r#"["current", (m) => `${m.i_actual.toFixed(3)} A`],"#,
    ),
    (
        "the `soc (true)` row",
        "app.js",
        r#"["soc (true)", (m) => `${(m.soc_true * 100).toFixed(1)} %`],"#,
    ),
    (
        "the `soc (bms)` row",
        "app.js",
        r#"["soc (bms)", (m) => (m.soc_bms === null ? null : `${(m.soc_bms * 100).toFixed(1)} %`)],"#,
    ),
    (
        "the `cell v` row",
        "app.js",
        r#"["cell v", (m) => `${m.v_cell_min.toFixed(3)} / ${m.v_cell_max.toFixed(3)} V`],"#,
    ),
    (
        "the `cell t` row",
        "app.js",
        r#"["cell t", (m) => `${toC(m.t_min).toFixed(1)} / ${toC(m.t_max).toFixed(1)} °C`],"#,
    ),
    (
        "the `heat` row",
        "app.js",
        r#"["heat", (m) => `${m.q_gen_w.toFixed(2)} W`],"#,
    ),
    (
        "the `soh cap` row",
        "app.js",
        r#"["soh cap", (m) => `${(m.soh_capacity * 100).toFixed(2)} %`],"#,
    ),
    (
        "the `soh res` row",
        "app.js",
        r#"["soh res", (m) => `${m.soh_resistance.toFixed(4)} ×`],"#,
    ),
    (
        "the `balancing` row",
        "app.js",
        r#"["balancing", (m) => `${m.q_balancing_w.toFixed(3)} W`],"#,
    ),
    (
        "the `short (int)` row",
        "app.js",
        r#"["short (int)", (m) => `${m.i_internal_short_a.toFixed(3)} A`],"#,
    ),
    (
        "the `clamp` row",
        "app.js",
        r#"(m) => (m.i_rejected_a === 0 ? null : `refused ${Math.abs(m.i_rejected_a).toFixed(3)} A`),"#,
    ),
    // The `clamp` row's own quiet placeholder, which is a third element rather than the
    // default below it. Pinned as its indented source line — every literal in this table
    // is deliberately confined to a single line, because a checkout that rewrote line
    // endings would break a multi-line one for a reason that has nothing to do with the
    // page.
    (
        "the `clamp` row's quiet placeholder",
        "app.js",
        "    \"none\",",
    ),
    (
        "the default quiet placeholder",
        "app.js",
        r#"for (const [key, fn, quiet = "no BMS"] of READOUTS) {"#,
    ),
    // The one row deliberately NOT mirrored, pinned so that a page change which turns it
    // into a telemetry-only row is noticed rather than silently leaving a mirrorable row
    // unmirrored.
    (
        "the `past empty` row reads per-cell state",
        "app.js",
        "const d = Math.max(...cs.map((c) => c.soc_deficit));",
    ),
    // --- the `surface gap` row, and the three lines it is made of ------------
    //
    // It sat beside `past empty` as unmirrored until the probe slice, and these four rows
    // are what moving it cost. The last one is the reason they are here at all: with only
    // the row's own line pinned, deleting the page's negative-zero guard left the whole
    // suite green — the mirror kept its own copy of the guard and went on printing what
    // the page no longer printed. Measured, in the perturbation table of
    // `docs/plans/path-probe-row.md`, and it is the same failure this table exists for.
    (
        "the `surface gap` row",
        "app.js",
        "return `${gapPts(c.surface_gap_neg, 2)} / ${gapPts(c.surface_gap_pos, 2)} pts`;",
    ),
    (
        "the `surface gap` row's placeholder on a circuit",
        "app.js",
        "    \"circuit — no electrodes\",",
    ),
    (
        "isPorous: which cells have the quantity at all",
        "app.js",
        "return !!cell && cell.surface_gap_neg !== null && cell.surface_gap_neg !== undefined;",
    ),
    (
        "gapPts: the negative-zero guard",
        "app.js",
        "return (Math.abs(v) < 0.5 * 10 ** -dp ? 0 : v).toFixed(dp);",
    ),
];

/// `web/index.html`: the `dt` input's default. Parsed from the markup rather than
/// declared here, so a change to the page's default moves this test with it.
fn default_dt() -> f64 {
    let html = index_html();
    let marker = r#"id="dt" value=""#;
    let start = html
        .find(marker)
        .unwrap_or_else(|| panic!("web/index.html has no `{marker}` — the dt input moved"))
        + marker.len();
    let rest = &html[start..];
    let end = rest.find('"').expect("dt value attribute is quoted");
    rest[..end].parse().expect("dt default parses as a float")
}

/// `web/app.js`: how often the CC-CV controller is allowed to change its mind \[s\].
///
/// Parsed rather than declared, on [`default_dt`]'s terms exactly — and here that closes a
/// gap this file's own docs complain about in four places. `CCCV_PERIOD_S` has sat in
/// [`MIRRORED`] since the day this test was written with *nothing reading it*, which is the
/// "pinned, and consulted by nothing" shape rejected everywhere else; [`cccv_window_steps`]
/// carried its own copy of the `10`.
///
/// **The pin and the parse are both kept, and they say different things.** The `MIRRORED`
/// row says the page still spells this constant the way the mirror expects to find it; this
/// says what the number is. Delete the row and a rename becomes a silent panic here instead
/// of a named failure there.
///
/// Anchored on the name **and** the `=`, so a longer identifier ending in the same characters
/// cannot answer for it.
/// The `cells` sampler's period \[ms\], read out of `web/app.js`.
///
/// The surface-gap readouts are filled from the pack-grid sampler rather than from the
/// per-frame telemetry, and step 17's prose is about exactly that: *"sampled four times a
/// second while everything else is redrawn every frame"*. Its own reader, on
/// [`cccv_period_s`]'s terms and for the reason [`Tie::Page`] gives: a tie able to read any
/// expression out of the page would find a number for almost any token.
fn cells_period_ms() -> f64 {
    let app = app_js();
    let marker = "const CELLS_PERIOD_MS = ";
    let start = app
        .find(marker)
        .unwrap_or_else(|| panic!("web/app.js has no `{marker}` - the sampler's clock moved"))
        + marker.len();
    let rest = &app[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    rest[..end]
        .parse()
        .expect("CELLS_PERIOD_MS is a numeric literal")
}

fn cccv_period_s() -> f64 {
    let app = app_js();
    let marker = "const CCCV_PERIOD_S = ";
    let start = app
        .find(marker)
        .unwrap_or_else(|| panic!("web/app.js has no `{marker}` — the decision window moved"))
        + marker.len();
    let rest = &app[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    rest[..end]
        .parse()
        .expect("CCCV_PERIOD_S is a numeric literal")
}

/// A constant that must still read the way the mirror below assumes it reads.
#[test]
fn mirrored_constants_still_match_the_page() {
    let app = app_js();
    let html = index_html();
    for (what, file, literal) in MIRRORED {
        let src = match *file {
            "app.js" => &app,
            "index.html" => &html,
            other => panic!("MIRRORED names an unknown file: {other}"),
        };
        assert!(
            src.contains(literal),
            "the {what} is mirrored in path_claims.rs from web/{file}, which no longer \
             contains `{literal}`. The page changed and this test's copy did not — fix \
             the copy, do not relax this assertion."
        );
    }
    assert!(
        (default_dt() - 0.5).abs() < f64::EPSILON,
        "default dt parsed as {} — MIRRORED says the markup still says 0.5, so the \
         parser is what broke",
        default_dt()
    );
}

// ---------------------------------------------------------------------------
// The panel's formatters, mirrored
// ---------------------------------------------------------------------------
//
// Everything in this section reimplements a few lines of `web/app.js` so that a claim
// can assert what a readout row *prints*, not only what the engine computes. Every
// mirrored line is pinned in `MIRRORED` above. See the module docs for why this is a
// mirror rather than a JavaScript engine.

/// `Number.prototype.toFixed`, which is not what Rust's `{:.n}` does.
///
/// The two disagree on exact ties, and only on exact ties. ECMA-262 splits the sign off
/// first and then picks, among the two candidates equally near the value, **the larger**
/// — so on the magnitude it is round-half-**up**, and `(0.25).toFixed(1)` is `0.3`.
/// Rust's formatter rounds half to **even**, and gives `0.2`. A tie needs the double's
/// exact value to terminate one digit past the cut, which is rare but entirely reachable:
/// a half-second simulation grid divided by 60 lands on `x.5` minutes at every odd
/// half-minute, which is precisely the argument `fmtTime` hands to `toFixed(0)`.
///
/// The rounding is done on the decimal string rather than by scaling the float, because
/// `v * 10f64.powi(dp)` introduces an error of its own right where this function has to
/// be exact. `GUARD` extra places are asked of Rust's formatter, which renders the
/// double's *exact* decimal expansion (a binary fraction always terminates in decimal),
/// so the digit at the cut is the true one and not a rounded one.
fn to_fixed(x: f64, dp: usize) -> String {
    assert!(x.is_finite(), "to_fixed on a non-finite value: {x}");
    // A double's fractional part is at most 1074 decimal places long, so this many extra
    // digits is not a heuristic: the expansion below is exact, and every digit this
    // function inspects is the double's own.
    const GUARD: usize = 1080;
    // `x < 0.0` and not `is_sign_negative`, matching the spec's `If x < 0`: negative zero
    // prints without a sign in JavaScript, and `(-0).toFixed(3)` is `0.000`.
    let neg = x < 0.0;
    let wide = format!("{:.*}", dp + GUARD, x.abs());
    let (int_part, frac) = wide
        .split_once('.')
        .expect("GUARD > 0, so there is a point");

    let mut digits: Vec<u8> = int_part
        .bytes()
        .chain(frac.bytes().take(dp))
        .map(|b| b - b'0')
        .collect();
    let first_dropped = frac.as_bytes()[dp];
    // `>= 5` is the whole of the spec's rule: above the halfway point rounds up, and a
    // tie — 5 with nothing but zeros behind it — rounds up too.
    if first_dropped >= b'5' {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }

    let text: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
    let int_len = text.len() - dp;
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&text[..int_len]);
    if dp > 0 {
        out.push('.');
        out.push_str(&text[int_len..]);
    }
    out
}

/// `fmtTime`: the page's simulation clock, and the formatter behind the first of the two
/// defects this check exists for — above two minutes it prints whole minutes, so a claim
/// about t = 983.0 s is a claim about a row that reads `16m`.
fn fmt_time(s: f64) -> String {
    if s < 120.0 {
        format!("{}s", to_fixed(s, 0))
    } else if s < 7200.0 {
        format!("{}m", to_fixed(s / 60.0, 0))
    } else if s < 172_800.0 {
        format!("{}h", to_fixed(s / 3600.0, 1))
    } else {
        format!("{}d", to_fixed(s / 86400.0, 1))
    }
}

/// `toC`: kelvin to celsius, the page's own way round.
fn to_c(k: f64) -> f64 {
    k - K
}

/// `gapPts`: a surface gap in points of charge, with negative zero spelled `0.00`.
///
/// The guard is the page's and is mirrored rather than simplified away, because it is
/// exactly what step 17's headline turns on. A uniform particle does not read a hard zero:
/// measured on that step's own probe, the negative electrode reads `-1.11e-16` — the bulk
/// side of the difference goes through a volume-weighted mean while the surface side
/// returns the outermost shell untouched. `toFixed` on that gives `-0.00`, and the step
/// says the reader sees `0.00`. Without this the display claim would be red on a minus
/// sign the page does not show.
fn fmt_gap_pts(x: f64, dp: usize) -> String {
    let v = x * 100.0;
    let shown = if v.abs() < 0.5 * 10f64.powi(-(dp as i32)) {
        0.0
    } else {
        v
    };
    to_fixed(shown, dp)
}

/// What a readout row prints for this frame — `READOUTS` in `web/app.js`, including the
/// placeholder a row falls back to when its formatter returns `null` on a running pack.
///
/// One row is missing on purpose and panics instead of returning something plausible:
///
/// * **`past empty`** is formatted from per-cell `soc_deficit`, which is not in
///   `Telemetry` at all, and the page samples it on a 250 ms *wall*-clock throttle rather
///   than per frame. There is therefore no such thing as "what that row shows at
///   simulation time t" — at speed it can be a dozen seconds of simulation behind, which
///   is a fact step 21's own prose has to warn a reader about. A mirror that answered
///   anyway would be asserting a number the page does not show.
///
/// It is named in the module docs as uncovered rather than left to be inferred.
///
/// **`surface gap` used to sit beside it and no longer does, and the reason given here for
/// nine slices was wrong.** It said that row "carries no throttle, so unlike `past empty` it
/// *does* have a value at a given simulation time". Both are formatted from `cells`, and
/// `cells` is sampled on `CELLS_PERIOD_MS` — see [`Row::surface_gap`], where the correction
/// and what it costs are written out. What actually separates them is narrower: a throttled
/// row is only behind while something is MOVING, so `surface gap` has a value at an instant
/// the reader can stop on — the zero-length probe, and the mark — and `past empty` has none
/// at all, because nothing in the path claims it anywhere but mid-run. That is what lets
/// step 17's headline (`0.00 / 0.00` before the reader presses Run) be a display claim
/// rather than a number with no panel behind it, and it is why the sixteen mid-run gap
/// claims on that step are value-only.
fn render_row(label: &str, row: &Row) -> String {
    let (t, sim_time_s) = (&row.telemetry, row.t_s);
    match label {
        "sim time" => fmt_time(sim_time_s),
        "terminal" => format!("{} V", to_fixed(t.v_terminal, 3)),
        "current" => format!("{} A", to_fixed(t.i_actual, 3)),
        "soc (true)" => format!("{} %", to_fixed(t.soc_true * 100.0, 1)),
        "soc (bms)" => match t.soc_bms {
            None => "no BMS".to_string(),
            Some(s) => format!("{} %", to_fixed(s * 100.0, 1)),
        },
        "cell v" => format!(
            "{} / {} V",
            to_fixed(t.v_cell_min, 3),
            to_fixed(t.v_cell_max, 3)
        ),
        "cell t" => format!(
            "{} / {} °C",
            to_fixed(to_c(t.t_min), 1),
            to_fixed(to_c(t.t_max), 1)
        ),
        "heat" => format!("{} W", to_fixed(t.q_gen_w, 2)),
        "soh cap" => format!("{} %", to_fixed(t.soh_capacity * 100.0, 2)),
        "soh res" => format!("{} ×", to_fixed(t.soh_resistance, 4)),
        "balancing" => format!("{} W", to_fixed(t.q_balancing_w, 3)),
        "short (int)" => format!("{} A", to_fixed(t.i_internal_short_a, 3)),
        "clamp" => {
            if t.i_rejected_a == 0.0 {
                "none".to_string()
            } else {
                format!("refused {} A", to_fixed(t.i_rejected_a.abs(), 3))
            }
        }
        // `isPorous` first, and it is not a formatting detail: on a circuit this row does
        // not print a zero, it prints the reason there is no number. A mirror that
        // rendered `0.00 / 0.00` there would let a claim assert "measured, and flat" on a
        // model with no electrodes to measure.
        "surface gap" => match row.surface_gap {
            None => "circuit — no electrodes".to_string(),
            Some((neg, pos)) => {
                format!("{} / {} pts", fmt_gap_pts(neg, 2), fmt_gap_pts(pos, 2))
            }
        },
        "past empty" => panic!(
            "`past empty` is a readout row this test deliberately does not mirror: it is \
             formatted from per-cell state and sampled on a wall-clock throttle, so it has \
             no value at a given simulation time. Claiming what it displays needs a \
             different instrument — see the module docs."
        ),
        other => panic!(
            "path-claims.toml names a readout row that is not in web/app.js's READOUTS: \
             `{other}`. Known: sim time, terminal, current, soc (true), soc (bms), \
             cell v, cell t, heat, soh cap, soh res, balancing, short (int), clamp, \
             surface gap."
        ),
    }
}

/// `toFixed` against the engine that actually runs the page.
///
/// Every expectation here was produced by node v24 (V8) rather than reasoned about, and
/// the cases were chosen for the two things that separate a faithful mirror from
/// `format!("{:.*}")`: exact ties, which Rust rounds the other way, and the sign rule.
#[test]
fn to_fixed_matches_javascript() {
    // (value, dp, what `(value).toFixed(dp)` prints in V8)
    let cases: &[(f64, usize, &str)] = &[
        // Ties. Every one of these is a value a double holds exactly, so the rounding
        // rule decides the digit rather than the representation error.
        (0.5, 0, "1"),
        (1.5, 0, "2"),
        (2.5, 0, "3"),
        (0.25, 1, "0.3"),
        (0.75, 1, "0.8"),
        (2.125, 2, "2.13"),
        (-0.25, 1, "-0.3"),
        (-2.5, 0, "-3"),
        // The one that is not a tie however much it looks like one: 1.005 is really
        // 1.00499999999999989... and V8 prints the digit the double has, not the digit
        // the decimal literal suggests.
        (1.005, 2, "1.00"),
        (8.575, 2, "8.57"),
        // Carries, including one that grows the integer part. The pair in the middle is
        // the same lesson as 1.005 above, twice over: `9.9995` is really
        // 9.99949999999999938..., so it rounds *down* despite the literal, while `99.995`
        // is really 99.99500000000000454... and rounds up. Nothing about the decimal a
        // human typed predicts which; only the double's own expansion does.
        (0.999_9, 3, "1.000"),
        (9.999_5, 3, "9.999"),
        (99.995, 2, "100.00"),
        // Zeroes and signs. Negative zero has no sign in JavaScript; a small negative
        // number keeps one even when every printed digit is a zero.
        (0.0, 3, "0.000"),
        (-0.0, 3, "0.000"),
        (-0.000_1, 3, "-0.000"),
        // dp = 0 drops the point entirely rather than leaving a trailing one.
        (69.108_333_333_333_33, 0, "69"),
        (16.383_333_333_333_33, 0, "16"),
        // Readings of the kind the claims below actually make.
        (0.638_698_5, 3, "0.639"),
        (95.155_1, 2, "95.16"),
        (1.072_6, 4, "1.0726"),
        (-0.068_6, 3, "-0.069"),
    ];
    for &(v, dp, want) in cases {
        assert_eq!(
            to_fixed(v, dp),
            want,
            "to_fixed({v}, {dp}) — JavaScript prints `{want}`"
        );
    }

    // The half-minute case that makes this function necessary rather than tidy: a run on
    // a 0.5 s grid reaching 4230 s is 70.5 minutes, and `fmtTime` asks toFixed(0) for it.
    // Rust's own formatter rounds that to 70; the page shows 71.
    assert_eq!(format!("{:.0}", 4230.0 / 60.0), "70");
    assert_eq!(fmt_time(4230.0), "71m");
}

/// The number scanner's thousands rule, and every near miss around it.
///
/// The cases that matter are the ones that must **not** join. Joining two numbers that the
/// sentence wrote separately makes check 6 demand an accounting for a figure nobody printed,
/// and no author can satisfy that — so the rule is narrow and the narrowness is what is
/// asserted here. `at 2 s, 464 s` is the case this was written against: a comma and a space
/// between two numbers, the second of them three digits.
#[test]
fn the_scanner_joins_thousands_groups_and_nothing_else() {
    for (text, want) in [
        // Joins: exactly one space, exactly three digits, no decimal point either side.
        ("clamp at 11 880 s", vec!["11 880"]),
        ("over the next 200 000 s", vec!["200 000"]),
        ("watching at 10 000×", vec!["10 000"]),
        ("1 234 567 of them", vec!["1 234 567"]),
        // Does not join: a comma, or any other character, sits between.
        ("at 2 s, 464 s", vec!["2", "464"]),
        ("0.5 s, 5 s, 10 s", vec!["0.5", "5", "10"]),
        ("3.66 V to 4.20", vec!["3.66", "4.20"]),
        // Does not join: two spaces, a group of the wrong size, or a decimal point.
        ("5  880", vec!["5", "880"]),
        ("5 1234", vec!["5", "1234"]),
        ("5 88", vec!["5", "88"]),
        ("1.5 880", vec!["1.5", "880"]),
        ("11 880.5", vec!["11", "880.5"]),
        // The trailing dot the scanner trims off `at 5769.` is still in the SOURCE, so the
        // gap to the next run has to be measured from the trimmed token's own end. Measured
        // from the untrimmed one it lands on the space and joins two numbers a sentence
        // wrote as one figure and a sentence boundary.
        ("at 5769. 880 s", vec!["5769", "880"]),
    ] {
        assert_eq!(
            numeric_tokens(text),
            want,
            "the scanner read `{text}` wrongly"
        );
    }

    // `len` is the TOKEN's extent and not the run's, which is what both of its readers
    // want — see [`Written`]. Asserted here rather than left to the one lesson sentence
    // that happens to exercise it: step 18 prints a claimed number at the end of a literal
    // with a full stop behind it, and while the field carried the run that number tested as
    // sitting outside its own literal. A cover that survives only while one sentence keeps
    // its shape is not a cover.
    for (text, want) in [
        ("instead of 1. Put", vec![("1", 11usize, 1usize)]),
        ("at 5769. The", vec![("5769", 3, 4)]),
        ("1.2.3 of them", vec![("1.2", 0, 3)]),
        ("over the next 200 000 s", vec![("200 000", 14, 7)]),
        ("3.66 V", vec![("3.66", 0, 4)]),
    ] {
        let got: Vec<(String, usize, usize)> = written_numbers(text)
            .into_iter()
            .map(|w| (w.token, w.at, w.len))
            .collect();
        let want: Vec<(String, usize, usize)> = want
            .into_iter()
            .map(|(t, at, len)| (t.to_string(), at, len))
            .collect();
        assert_eq!(got, want, "the scanner placed `{text}` wrongly");
        for (token, at, len) in &got {
            assert_eq!(
                &text[*at..*at + *len],
                token,
                "`len` has to be an extent into the source, not a length in the abstract"
            );
        }
    }
}

/// The clock's four branches, at and around each boundary.
/// The word scanner reads the three shapes it claims to, and nothing else.
///
/// The instrument test, on [`the_scanner_joins_thousands_groups_and_nothing_else`]'s terms:
/// this scanner decides which of a step's sentences have to answer for themselves, so a
/// case it quietly misses is coverage nobody asked for and nobody gets.
#[test]
fn the_word_scanner_reads_quantities_and_not_pronouns() {
    // The NOUN is returned beside the token and the scale, and that is not decoration.
    // `Written::unit` is what `Reading` carries into the gate on check 6's duration arms,
    // and `unit_is_time("")` is TRUE - an empty unit means "the sentence wrote digits", which
    // those arms have always been asked about. So a shape that stopped setting the noun, or
    // a conversion that dropped it, would re-open the collision this slice closed and every
    // test in this file would stay green, because the one sentence that collided has since
    // been repaired. This helper is the only place the wire is watched.
    let one = |text: &str| {
        let found = spelled_numbers(text);
        assert_eq!(found.len(), 1, "`{text}` -> {found:?}");
        let w = found.into_iter().next().expect("checked just above");
        assert!(
            !w.unit.is_empty(),
            "`{text}` scanned to a quantity with no unit noun. Empty means \"written in \
             digits\" to `unit_is_time`, so a spelled quantity carrying it is offered the \
             arms that read a step length and an instant - which is the accounting this \
             field exists to refuse."
        );
        (w.token, w.scale, w.unit)
    };

    // Shape 1: a numeral and a unit, with the fillers a sentence puts between them.
    assert_eq!(
        one("5.71 at three minutes,"),
        ("3".to_string(), 60.0, "minutes")
    );
    assert_eq!(
        one("rest for half an hour."),
        ("0.5".to_string(), 3600.0, "hour")
    );
    assert_eq!(
        one("jumps of roughly fifty simulated seconds and"),
        ("50".to_string(), 1.0, "seconds")
    );
    // Hyphenated attributive: the unit is fused onto the numeral, so neither half is a word
    // of its own to walk from.
    assert_eq!(
        one("inside an eighteen-minute discharge"),
        ("18".to_string(), 60.0, "minute")
    );
    // A compound cardinal.
    assert_eq!(
        one("takes twenty-four minutes of"),
        ("24".to_string(), 60.0, "minutes")
    );
    // A scale word folds into the value rather than standing as the unit. This is the case
    // that fails toward a WRONG NUMBER rather than toward silence: read as a unit, three
    // thousandths of a point comes out as the number three.
    assert_eq!(
        one("moves by three thousandths of a point in"),
        ("0.003".to_string(), 1.0, "point")
    );

    // Shape 2: the numeral follows its unit, because English puts it there.
    assert_eq!(
        one("after the first minute and a half, 5.71"),
        ("1.5".to_string(), 60.0, "minute")
    );
    // Any fraction, not only a half. Read only halves once, and a perturbation turning
    // this very phrase into a quarter went green because the scanner saw nothing there.
    assert_eq!(
        one("after the first minute and a quarter, 5.71"),
        ("1.25".to_string(), 60.0, "minute")
    );

    // Shape 4: shape 2 the other way round — the numeral ahead of its fraction and both
    // ahead of the unit. Not a variant of shape 2: the phrase is a different sequence of
    // words and only the quantity is the same.
    assert_eq!(
        one("40.33 A four and a half seconds later,"),
        ("4.5".to_string(), 1.0, "seconds")
    );
    assert_eq!(
        one("about two and a half days — which is"),
        ("2.5".to_string(), 86400.0, "days")
    );
    // ...and it consumes its own tail. Read by shape 1 instead, `half seconds` is 0.5 s, and
    // on `nothing-to-clamp` — whose step length IS 0.5 s — the ledger tied a sentence saying
    // 4.5 to a control holding 0.5 and went green. The assertion is `len == 1` inside `one`,
    // so this case is the whole finding.
    let both = spelled_numbers("40.33 A four and a half seconds later,");
    assert_eq!(both.len(), 1, "the tail is not a second quantity: {both:?}");

    // A noun that is NOT a duration, read off the shape the gate was found by. Everything
    // above scales by a time; this one scales by one, exactly as `seconds` does, which is
    // why the scale cannot stand in for the noun.
    assert_eq!(
        one("against the twin's half a percent,"),
        ("0.5".to_string(), 1.0, "percent")
    );

    // Shape 3: a list item inheriting the unit an earlier item stated.
    let list = spelled_numbers("5.71 at three minutes, 5.80 at six, and 5.81 later");
    assert_eq!(
        list.iter()
            .map(|w| (w.token.as_str(), w.scale, w.unit))
            .collect::<Vec<_>>(),
        vec![("3", 60.0, "minutes"), ("6", 60.0, "minutes")],
        "the second item carries the first item's unit"
    );

    // ...and the ellipsis stops where the list does. `at three decimals` states a noun of
    // its own, which is not a measure this scanner admits, so it must not inherit the
    // seconds from a sentence upstream. Caught in authoring: the first draft of this rule
    // looked only for a numeral with no unit after it, and read that as three minutes.
    assert!(
        spelled_numbers("some thirty seconds earlier. Hover the tile at three decimals,")
            .iter()
            .all(|w| w.phrase != "at three"),
        "`at three decimals` is not a list item"
    );

    // The pronouns and the articles. They are 146 of the 293 English number-words in this
    // path's prose, and they are the whole reason it is the unit noun that decides and not
    // the numeral: a scan keyed on the number word alone would demand an accounting for
    // every one of these and be waived to death inside a slice.
    for text in [
        "one of these",
        "the one that got through",
        "two of them disagree",
        "Two numbers because there are two electrodes",
        "Four footnotes, and the first",
        "a pack with more than one cell",
        "does not move by one digit",
        "the start of a second 3 C discharge",
        "for the whole second half of the discharge",
        "Both times you were asked to infer",
    ] {
        assert!(
            spelled_numbers(text).is_empty(),
            "`{text}` is not a spelled quantity, but the scanner read {:?}",
            spelled_numbers(text)
        );
    }
}

/// A numeral this scanner cannot value is a **failure**, never a skip.
///
/// The hazard is specific and this repo has paid for it once: a harness that silently passes
/// over what it does not understand reports a green meaning "I found nothing I recognised",
/// which reads exactly like "there was nothing to find". `docs/plans/surface-vs-bulk.md` is
/// the five-green version of that story.
///
/// So on a word-scanned step, a numeral word no shape consumed may not have a measure noun
/// sitting just behind it. *"four and a half seconds"* is the shape it was written pointing
/// at — a numeral, a fraction and a unit, which none of the shapes read at the time — and
/// **it did its job**: opening the scan on five more lessons, this guard named six such
/// phrases and refused rather than certifying them, which is what bought shape 4. That the
/// example in this paragraph is now *read* is the point of the guard, not a hole in it: what
/// it watches is the gap between what the prose spells and what the scanner has learned, and
/// that gap is refilled by every lesson added to the list.
///
/// What it does **not** see is a quantity with no numeral word in it at all. *"a minute in"*
/// and *"an hour of simulation"* spell one as an article, and there is nothing here to key
/// on. [`english_quantities`] does see that shape, because refusing one needs no value.
///
/// **It is vacuous on today's prose, and saying so is the point of this paragraph.** Every
/// step in `spelled` now spells nothing at all, so this guard walks seven lessons and finds
/// no numeral to ask about. It is kept rather than deleted because it guards the READER, and
/// the reader is still wired in: a spelled quantity coming back to one of those steps would
/// meet [`no_lesson_spells_a_quantity_in_english`] first, and this second. A green here is
/// worth exactly what a green over an empty list is worth, which is nothing, and that is not
/// what it used to be worth.
#[test]
fn no_spelled_quantity_is_silently_skipped() {
    let lessons = lessons();
    let ledger = ledger();
    for step in &ledger.spelled {
        let lesson = lessons
            .iter()
            .find(|l| l.id == *step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));
        let text = ascii_minus(&lesson.text);
        let found = spelled_numbers(&text);
        let words = words_of(&text);
        for (i, (at, _, word)) in words.iter().enumerate() {
            if numeral_word(word).is_none() {
                continue;
            }
            if found.iter().any(|w| *at >= w.at && *at < w.at + w.len) {
                continue;
            }
            // Only where every word between the numeral and the unit is a CONNECTIVE.
            // Without that, *"two numbers, in points of state of charge"* trips this: the
            // `two` counts the row's figures and the `points` belongs to the phrase after
            // it, three words along. They are not one quantity, and nothing about the
            // distance says so. What does say so is that `numbers` and `in` are not words a
            // quantity phrase is built from, where *"five and a half minutes"* is nothing
            // but those words.
            let connective = |w: &str| {
                FILLERS.contains(&w) || w == "and" || FRACTIONS.iter().any(|(f, _)| *f == w)
            };
            let mut unit_hit = None;
            for (_, end, w) in words[i + 1..].iter().take(4) {
                if lookup(UNIT_NOUNS, w).is_some() {
                    unit_hit = Some((*end, w.clone()));
                    break;
                }
                if !connective(w) {
                    break;
                }
            }
            let Some((unit_end, unit)) = unit_hit else {
                continue;
            };
            panic!(
                "step `{step}` spells `{word}` and then `{unit}` within four words, and no \
                 shape this scanner reads consumed it:\n  …{}…\n\
                 That is a quantity written in English which this scan would otherwise pass \
                 over in silence, reporting a green that says only that it understood \
                 nothing here. Either it is a shape `spelled_numbers` should read — add it, \
                 with a case in `the_word_scanner_reads_quantities_and_not_pronouns` — or \
                 the sentence is doing something with a number word that is not a \
                 measurement, in which case the fillers or the unit nouns should say so, \
                 rather than leaving the scanner to decide by accident.",
                &text[*at..unit_end],
            );
        }

        // The other direction, and it is a SHAPE rather than a word. `<unit> and a X` puts
        // the numeral BEHIND its unit, so there is nothing ahead of it for the loop above
        // to find - and X need not be a numeral this file knows, which is exactly when the
        // silence matters. Keyed on `tenth` being a numeral, this missed
        // *"a minute and a tenth"* completely: the guard skipped the word before it ever
        // asked what was in front of it.
        //
        // Written as the literal sequence "and a" rather than as "connectives nearby",
        // because the loose version fires on *"the whole second half of the discharge"*,
        // where `second` is an ordinal, `whole` is a filler, and the two are not one
        // quantity.
        for (i, (at, _, word)) in words.iter().enumerate() {
            if lookup(UNIT_NOUNS, word).is_none() {
                continue;
            }
            let tail: Vec<&str> = words[i + 1..]
                .iter()
                .take(3)
                .map(|(_, _, w)| w.as_str())
                .collect();
            if tail.len() < 3 || tail[0] != "and" || tail[1] != "a" {
                continue;
            }
            if found.iter().any(|w| w.at == *at) {
                continue;
            }
            panic!(
                "step `{step}` writes `{}`, which is the shape `<unit> and a <numeral>` \u{2014} \
                 a quantity whose numeral sits behind its unit \u{2014} and `spelled_numbers` \
                 did not read it:\n  \u{2026}{}\u{2026}\n\
                 `{}` is not a fraction this file knows, so the sentence states a length \
                 nothing can check while the scan reports that it found nothing here. Add \
                 the fraction to `FRACTIONS`, or reword the sentence.",
                text[*at..words[i + 3].1].trim(),
                text[*at..words[i + 3].1].trim(),
                tail[2],
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The digits rule - a quantity in lesson prose is written in digits
// ---------------------------------------------------------------------------

/// The unit nouns the BAN refuses, which is deliberately a superset of [`UNIT_NOUNS`].
///
/// **A forbidder has no arm, and that is what licenses the width.** [`UNIT_NOUNS`] is narrow
/// on purpose: every noun in it is one some tie can answer about, because a reader that
/// finds a quantity it cannot tie either fails loudly or - worse - hands it to whatever arm
/// happens to hold the right number, which is the collision
/// `docs/plans/path-word-batch-two.md` records twice. This list ties nothing. A word in it
/// is refused, the sentence is rewritten in digits, and the cost of a false alarm is one
/// reworded sentence that its author sees the same minute. So the only reason to leave a
/// unit out of it is that the word is not a unit.
///
/// **Furniture is still out, and for the reader's reason rather than by inheritance.** The
/// prose counts its own lists - *"two electrodes"*, *"Four footnotes"*, *"three decimals"* -
/// and those are a list's length rather than a measure. Admitting them here would forbid a
/// sentence this repo has no arm for, so the rewrite would move the number into the digit
/// ledger and leave it unaccountable *there*, which is a worse place for it than the one the
/// module header already declares it to be in.
const BANNED_UNITS: &[&str] = &[
    // Everything the reader reads.
    "second",
    "seconds",
    "minute",
    "minutes",
    "hour",
    "hours",
    "day",
    "days",
    "point",
    "points",
    "time",
    "times",
    "percent",
    "percentage",
    // ...and the units a lesson could state that no tie has ever been asked about. Not one
    // of these is spelled anywhere in the path today, which is the point of having them:
    // they cost nothing now and they are what a NEW lesson would reach for.
    "volt",
    "volts",
    "millivolt",
    "millivolts",
    "amp",
    "amps",
    "ampere",
    "amperes",
    "milliamp",
    "milliamps",
    "ohm",
    "ohms",
    "milliohm",
    "milliohms",
    "watt",
    "watts",
    "milliwatt",
    "milliwatts",
    "joule",
    "joules",
    "kelvin",
    "degree",
    "degrees",
    "millisecond",
    "milliseconds",
    "week",
    "weeks",
    "month",
    "months",
    "year",
    "years",
];

/// The ban knows every unit the reader knows.
///
/// One direction only, and that is the whole content of it: [`BANNED_UNITS`] may grow past
/// [`UNIT_NOUNS`], because refusing a word needs no arm behind it, but it may never fall
/// short. A noun the reader would read and the ban would pass is a quantity that can be
/// spelled in English and still satisfy every check in this file.
#[test]
fn the_ban_refuses_every_unit_the_reader_reads() {
    for (noun, _) in UNIT_NOUNS {
        assert!(
            BANNED_UNITS.contains(noun),
            "`{noun}` is a unit `spelled_numbers` reads and `BANNED_UNITS` does not \
             refuse. A sentence spelling a quantity in that noun would be read, tied, and \
             never told to write itself in digits."
        );
    }
}

/// Every phrase in a lesson's prose that spells a quantity in English, loosely.
///
/// **Loose on purpose, and it is the opposite of [`spelled_numbers`] in the one way that
/// matters.** That function has to produce a *value*, so it refuses what it cannot parse and
/// [`no_spelled_quantity_is_silently_skipped`] exists to catch the refusals. This one
/// produces no value at all - it answers "does this sentence spell a quantity", and a phrase
/// it cannot parse is exactly as banned as one it can. So it reads a shape the reader never
/// had: the quantity spelled as an **article**, *"an hour of simulation"*, *"a minute in"*,
/// which has no numeral word in it to key on and which the reader's own docs have declared
/// unread since it was built.
///
/// **There is no fence anywhere in it, and that was a decision.** The first draft refused
/// the article shape after `of`, so that *"a fraction of a point"* would not read as the
/// number one; a partitive names the unit of a quantity stated in front of it. It bought
/// three entries off the backlog list and cost a false NEGATIVE the same afternoon -
/// *"empty is three and a half minutes away instead of an hour"* states an hour and is not
/// a partitive, and the fence swallowed it. That is the shape this file has now built three
/// times: a fence written against the cases its author thought of, inheriting exactly the
/// blind spot it was guarding against. A ban does not need one. A partitive costs an entry
/// on a list; a missed quantity costs a number nothing checks.
fn english_quantities(text: &str) -> Vec<(usize, String)> {
    let words = words_of(text);
    let banned = |w: &str| BANNED_UNITS.contains(&w);
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut out: Vec<(usize, String)> = Vec::new();

    for (i, (at, end, word)) in words.iter().enumerate() {
        // A word inside a phrase already reported is not the head of a second one - the
        // overlap skip [`spelled_numbers`] carries, for its reason.
        if spans.iter().any(|(a, b)| at > a && at < b) {
            continue;
        }
        // "eighteen-minute", "half-second": the unit is fused onto the numeral, so neither
        // half is a word of its own to walk from.
        if let Some((head, tail)) = word.split_once('-') {
            if numeral_word(head).is_some() && banned(tail) {
                spans.push((*at, *end));
                out.push((*at, text[*at..*end].to_string()));
                continue;
            }
        }
        // "<unit> and a <anything>" - the numeral sits behind its unit, and what follows
        // "and a" need not be a fraction this file knows. Keyed on the shape for that
        // reason; see the second half of [`no_spelled_quantity_is_silently_skipped`], which
        // was keyed on the numeral and missed "a minute and a tenth" completely.
        if banned(word) {
            let tail: Vec<&str> = words[i + 1..]
                .iter()
                .take(3)
                .map(|(_, _, w)| w.as_str())
                .collect();
            if tail.len() == 3 && tail[0] == "and" && tail[1] == "a" {
                let stop = words[i + 3].1;
                spans.push((*at, stop));
                out.push((*at, text[*at..stop].to_string()));
                continue;
            }
        }
        // A numeral - or an article, which is how English spells one without a numeral -
        // then connectives, then the unit.
        let article = word == "a" || word == "an";
        if numeral_word(word).is_none() && !article {
            continue;
        }
        let mut j = i + 1;
        let mut walked = 0;
        while let Some((_, stop, w)) = words.get(j) {
            if banned(w) {
                spans.push((*at, *stop));
                out.push((*at, text[*at..*stop].to_string()));
                break;
            }
            let connective = FILLERS.contains(&w.as_str())
                || w == "and"
                || FRACTIONS.iter().any(|(f, _)| f == w)
                || lookup(SCALE_WORDS, w).is_some();
            if !connective || walked == 4 {
                break;
            }
            j += 1;
            walked += 1;
        }
    }
    // The list item whose unit an earlier item stated - *"0.4819 V at sixty,"*, *"5.80 at
    // six, and"*. Last, because it reads what the shapes above did not: a numeral with no
    // unit behind it at all, closing on the punctuation of a list rather than on a noun.
    //
    // The reader has this shape too ([`spelled_numbers`]'s third), and it is the one its own
    // plan doc calls the shape that mattered most - four of the six instants the surface-gap
    // step reads at were written this way. A ban that could not see it would be narrower
    // than the reader in one place, which is the one thing
    // [`the_ban_sees_every_quantity_the_reader_reads`] refuses.
    for (i, (at, end, word)) in words.iter().enumerate() {
        if word != "at" {
            continue;
        }
        let Some((n_at, n_end, n_word)) = words.get(i + 1) else {
            continue;
        };
        if numeral_word(n_word).is_none() {
            continue;
        }
        if spans.iter().any(|(a, b)| n_at >= a && n_at < b) {
            continue;
        }
        let closes = text[*n_end..].starts_with(',')
            || text[*n_end..].starts_with('.')
            || text[*n_end..].starts_with(" and");
        if closes {
            let _ = end;
            spans.push((*at, *n_end));
            out.push((*at, text[*at..*n_end].to_string()));
        }
    }

    out.sort_by_key(|(at, _)| *at);
    out
}

/// The ban sees every quantity the reader reads, and more.
///
/// [`the_ban_refuses_every_unit_the_reader_reads`] asks this of the two unit tables; this
/// asks it of the two scanners, over the real prose, which is the half a table comparison
/// cannot reach - a shape one reads and the other does not would pass that test and fail
/// here. Run over **every** lesson rather than the word-scanned ones, which is what makes it
/// a live check today: the thirty-five quantities still spelled in English all sit on steps
/// the reader is not turned on for.
///
/// One direction, on [`BANNED_UNITS`]'s terms. The ban is allowed to be wider - it reads the
/// article shape, which the reader never has - and is never allowed to be narrower.
#[test]
fn the_ban_sees_every_quantity_the_reader_reads() {
    for lesson in &lessons() {
        let text = ascii_minus(&lesson.text);
        let banned = english_quantities(&text);
        for read in spelled_numbers(&text) {
            assert!(
                banned
                    .iter()
                    .any(|(at, p)| read.at >= *at && read.at < at + p.len()),
                "on lesson `{}` the reader finds the quantity `{}` and the ban does not \
                 see it. A shape `spelled_numbers` reads and `english_quantities` passes \
                 over is a quantity that can be spelled in English and never told to write \
                 itself in digits.",
                lesson.id,
                read.phrase,
            );
        }
    }
}

/// The article shape fires, which is the whole of what the reader could never do.
///
/// *"an hour of simulation"*, *"a minute in"* - a quantity with no numeral word in it at
/// all, which is why [`no_spelled_quantity_is_silently_skipped`] cannot see one either: that
/// guard walks numerals. It is the shape the reader's own plan doc declined to build, on the
/// grounds that reading it would need three fences none of which could be watched. Refusing
/// it needs none.
#[test]
fn the_article_shape_reads_a_quantity_with_no_numeral_in_it() {
    let found: Vec<String> = english_quantities("this file runs for an hour of simulation")
        .into_iter()
        .map(|(_, phrase)| phrase)
        .collect();
    assert_eq!(
        found,
        vec!["an hour".to_string()],
        "the article shape did not read a quantity spelled as an article"
    );
}

/// No lesson's prose spells a quantity in English - the digits rule.
///
/// **The rule and the reason.** `docs/plans/path-word-numerals.md` and
/// `path-word-batch-two.md` built a reader for the quantities this path spells in words, and
/// got it as far as seven of twenty-four steps. Finishing it was seventeen more rounds of
/// the same work, and `docs/plans/phase-8-chemistries.md` records the owner's decision to
/// **ban the practice instead of reading it**: the prose writes its numbers in digits, the
/// digit ledger already covers all twenty-four steps, and this is the check that keeps it
/// that way.
///
/// **The argument is about which way the mistakes fall, not about which instrument is
/// cleverer.** Deciding whether an English number word is acting as a quantity is the same
/// hard problem either way - *"one of the cells"* is not one and *"one volt"* is. What
/// changes is the cost of being wrong. A false alarm under a ban is one sentence rewritten
/// by the author who just wrote it, with the failure naming the phrase. A miss under a
/// reader is silent and a wrong number ships behind a green suite, which is a shape this
/// file has shipped and written down more than once.
///
/// **The list is the backlog, at one entry per phrase.** Thirty-five quantities were
/// rewritten into digits by the slice that added this check - every one that had an arm
/// behind it - and what is left is named in `[[english]]` in `web/path-claims.toml`, phrase
/// by phrase. It is matched **both ways**: a phrase in the prose and not in the list fails,
/// and a phrase in the list and not in the prose fails too, so repairing a sentence means
/// deleting its entry and the list can only get shorter. `word_blind`'s per-step counts say
/// something similar one resolution coarser, and each is derived from its own scan rather
/// than from the other.
#[test]
fn no_lesson_spells_a_quantity_in_english() {
    let listed = english_exceptions();

    let mut found: Vec<(String, String)> = Vec::new();
    for lesson in &lessons() {
        let text = ascii_minus(&lesson.text);
        for (_, phrase) in english_quantities(&text) {
            found.push((lesson.id.clone(), phrase));
        }
    }

    for entry in &found {
        let (step, phrase) = entry;
        let seen = found.iter().filter(|f| *f == entry).count();
        let allowed = listed.iter().filter(|e| *e == entry).count();
        assert!(
            allowed >= seen,
            "lesson `{step}` spells a quantity in English: `{phrase}`.\n\
             Write it in digits. The path's prose states its numbers in digits so that the \
             ledger, which scans every one of the twenty-four steps, can see them - a \
             quantity spelled in letters is invisible to it, and a green ledger on a step \
             that spells one says less than it looks like it says.\n\
             If this really is not a quantity (`one of the cells`, `a second opinion`), \
             reword it rather than reaching for a waiver: there is none. `[[english]]` in \
             web/path-claims.toml is the backlog of phrases that were already in the prose \
             when this rule landed, and not a place to put new ones."
        );
    }
    for (step, phrase) in &listed {
        assert!(
            found.iter().any(|(s, p)| s == step && p == phrase),
            "`[[english]]` lists `{phrase}` on lesson `{step}` and no scan of that step's \
             prose finds it. The list is a backlog and may only get shorter: if the \
             sentence has been rewritten in digits, delete the entry. A stale entry leaves \
             a step reading as though it still owes something it does not."
        );
    }
}

/// A step may not have its words scanned without having its digits scanned.
///
/// The two lists are one contract, in the direction `steps` and `unledgered` already are: a
/// step whose digits nothing ties has no business claiming its words are tied, and a green
/// word scan standing beside an unledgered step reads as coverage it is not.
/// Every lesson is word-scanned or is named as not, and no lesson is both.
///
/// [`every_lesson_is_ledgered_or_named_as_not`] one axis over. The argument is the same one
/// that file makes for its own pairing and it is worth restating, because the temptation to
/// leave a bare opt-in list is strongest exactly while it is short: an opt-in list says what
/// is covered, and only a partition says what is *not*. A lesson written next month joins
/// neither list and this check is what turns that into a red rather than into a step whose
/// English quantities nothing ever reads.
#[test]
fn every_lesson_is_word_scanned_or_named_as_not() {
    let lessons = lessons();
    let ledger = ledger();

    let mut listed: Vec<&str> = ledger
        .spelled
        .iter()
        .chain(&ledger.word_blind)
        .map(String::as_str)
        .collect();
    let before = listed.len();
    listed.sort_unstable();
    listed.dedup();
    assert_eq!(
        before,
        listed.len(),
        "a step is in both `[ledger].spelled` and `[ledger].word_blind`. The two \
         lists are one partition; a step in both makes the file say its words are \
         read and are not."
    );

    for lesson in &lessons {
        assert!(
            listed.contains(&lesson.id.as_str()),
            "lesson `{}` is in neither `[ledger].spelled` nor `[ledger].word_blind`. \
             A lesson that says nothing about its English quantities is word-blind \
             and unremarked, which is the state this pairing exists to make \
             impossible — put it in one of the two.",
            lesson.id,
        );
    }
    for id in &listed {
        assert!(
            lessons.iter().any(|l| l.id == **id),
            "`[ledger]` names `{id}`, which is not a lesson. Both lists follow the path."
        );
    }
}

#[test]
fn every_word_scanned_step_is_ledgered() {
    let ledger = ledger();
    for step in &ledger.spelled {
        assert!(
            ledger.steps.contains(step),
            "`{step}` is in the ledger's `spelled` list but not in `steps`, so its words are \
             scanned and its digits are not. Ledger the step first."
        );
    }
}

/// The two tables of number words agree about what a word means.
///
/// [`WORD_NUMERALS`] is what an *author* may name in a claim's `spells` or a rule's operand,
/// and every entry of it must be used; [`CARDINALS`] and [`FRACTIONS`] are what the *scanner*
/// reads, consulted for every word of a step whether or not anyone named it. Keeping them
/// apart is what stops the scanner trivially satisfying
/// [`every_word_numeral_is_read_by_something`] and disarming it while looking like coverage.
///
/// The cost of two tables is that they can drift, and a word meaning 3 to a claim and
/// something else to the ledger is exactly the two-readings hazard this taxonomy is arranged
/// against. So where they overlap they are held equal here.
#[test]
fn the_two_word_tables_cannot_disagree() {
    for (word, value) in WORD_NUMERALS {
        let Some(scanned) = numeral_word(word) else {
            continue;
        };
        assert_eq!(
            scanned, *value,
            "`{word}` is {value} to a claim and {scanned} to the scanner. One word, two \
             numbers, and which of them a sentence got would depend on which scan reached it."
        );
    }
}

#[test]
fn fmt_time_matches_the_page() {
    for (s, want) in [
        (0.0, "0s"),
        (59.5, "60s"),
        (119.999, "120s"),
        (120.0, "2m"),
        (983.0, "16m"),
        (4146.5, "69m"),
        (7199.0, "120m"),
        (7200.0, "2.0h"),
        (172_799.0, "48.0h"),
        (172_800.0, "2.0d"),
    ] {
        assert_eq!(fmt_time(s), want, "fmtTime({s})");
    }
}

// ---------------------------------------------------------------------------
// One lesson step, read out of `const LESSONS`
// ---------------------------------------------------------------------------

/// A lesson step's machine-readable setup, scraped from `web/app.js`.
///
/// The prose is carried alongside so the literal check reads the same block the setup
/// came from — a claim can never be matched against a different step's sentence.
#[derive(Debug, Clone)]
struct Lesson {
    id: String,
    scenario: String,
    demand: Prog,
    ambient_c: f64,
    bms: Option<bool>,
    dt: f64,
    until_s: f64,
    /// The speed slider's multiplier — steps per frame, and **nothing the trajectory can
    /// see**.
    ///
    /// Scraped for one reader: the ledger's [`Control::Speed`], because step 8's prose prints
    /// it ("twenty seconds of watching at 10 000×") and no file but this block holds it.
    /// Absent in most lesson blocks, which is the page's own default rather than a missing
    /// value.
    ///
    /// **Adding this field changes what one paragraph elsewhere can claim.**
    /// [`Accounted::Setting`] argued that a generous version of *itself* — accounting a token
    /// against any numeric field of the lesson block — "cannot be built, or perturbed into
    /// existence, without adding the field first", and this is that field. The argument is
    /// now the design rather than the absence: check 6's arm ties a token to the step length
    /// of a **trajectory** a claim is read on, and no trajectory has a speed.
    speed_x: Option<f64>,
    /// Does `applyStep` rebuild the pack on the way into this step?
    ///
    /// Read for one purpose: to fence a `probe` claim. [`run`] always builds a fresh pack,
    /// which for a *stepped* row is a simplification the trajectory mostly absorbs — but a
    /// probe claim is a claim about the pack **before the first step**, and on a step that
    /// inherits its pack there is no such thing as a fresh one. `applyStep` also reloads
    /// when the scenario file differs from the one on screen, so `reload: true` is the
    /// stronger condition and the only one a claim can rely on from either direction.
    reload: bool,
    /// `prose` and `expect` concatenated: everything a reader is shown for this step.
    text: String,
}

/// The page's three client-side demand programs. Two of them are not demands the
/// engine has — they are policies `web/app.js` runs on top of it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Prog {
    Current(f64),
    Rest,
    CcCv { i: f64, v_cell: f64, taper: f64 },
    Pulse { i: f64, on_s: f64, off_s: f64 },
}

/// The demand program an **arm** runs under: the step's, unless the reader typed a current.
///
/// The two current boxes behave differently on purpose, and the difference is the page's.
/// `demand_a` is the *simple* box, which carries a mode of its own — typing into it leaves
/// whatever program was selected, so a number there on a `Pulse` step ends the pulse train.
/// `cc_cv_a` is the CC-CV group's charge current, one field of three; typing into it changes
/// that field and nothing else, so the charge stays a charge. Collapsing the two would turn
/// step 11's *"ask for 6 A"* into a 6 A **discharge**, because the simple box is
/// discharge-positive and the CC-CV one is not.
///
/// Both being set is refused where the arms are checked, so the order of these arms is not a
/// preference.
fn arm_prog(arm: &Arm, step_demand: Prog) -> Prog {
    match (arm.cc_cv_a, arm.pulse_a, arm.demand_a, step_demand) {
        (Some(i), _, _, Prog::CcCv { v_cell, taper, .. }) => Prog::CcCv { i, v_cell, taper },
        // Unreachable on a well-formed file — `cc_cv_a` is refused off a CC-CV step where
        // the arms are checked — and written out rather than left to a catch-all so that
        // adding a fourth program cannot silently drop the field.
        (Some(_), _, _, other) => other,
        // The pulse box: the current changes and the legs do not, which is exactly what the
        // page's three-field group lets a reader do. Refused off a `Pulse` step in
        // `check_pulse_current`, and written out here for the same reason its neighbour is.
        (None, Some(i), _, Prog::Pulse { on_s, off_s, .. }) => Prog::Pulse { i, on_s, off_s },
        (None, Some(_), _, other) => other,
        (None, None, Some(i), _) => Prog::Current(i),
        (None, None, None, other) => other,
    }
}

/// Pull `key: <number>` out of a lesson block.
fn num_field(block: &str, key: &str) -> Option<f64> {
    let marker = format!("\n    {key}: ");
    let start = block.find(&marker)? + marker.len();
    let rest = &block[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Pull `key: "<text>"` out of a lesson block.
fn str_field(block: &str, key: &str) -> Option<String> {
    let marker = format!("\n    {key}: \"");
    let start = block.find(&marker)? + marker.len();
    let rest = &block[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Pull a number out of the single-line `demand: { ... }` object.
fn demand_num(line: &str, key: &str) -> f64 {
    let marker = format!("{key}: ");
    let start = line
        .find(&marker)
        .unwrap_or_else(|| panic!("demand line has no `{key}`: {line}"))
        + marker.len();
    let rest = &line[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e'))
        .unwrap_or(rest.len());
    rest[..end]
        .parse()
        .unwrap_or_else(|_| panic!("demand `{key}` is not a number: {line}"))
}

fn parse_demand(block: &str) -> Prog {
    let marker = "\n    demand: {";
    let start = block
        .find(marker)
        .unwrap_or_else(|| panic!("lesson block has no demand: {}", &block[..80]))
        + 1;
    let rest = &block[start..];
    let line = &rest[..rest.find('\n').unwrap_or(rest.len())];

    let mode_marker = "mode: \"";
    let ms = line.find(mode_marker).expect("demand has a mode") + mode_marker.len();
    let mode = &line[ms..][..line[ms..].find('"').expect("mode is quoted")];

    match mode {
        "Current" => Prog::Current(demand_num(line, "value")),
        "Rest" => Prog::Rest,
        "CcCv" => Prog::CcCv {
            i: demand_num(line, "value"),
            v_cell: demand_num(line, "v_cell"),
            taper: demand_num(line, "taper"),
        },
        "Pulse" => Prog::Pulse {
            i: demand_num(line, "value"),
            on_s: demand_num(line, "on_s"),
            off_s: demand_num(line, "off_s"),
        },
        other => panic!("web/app.js uses a demand mode this test does not mirror: {other}"),
    }
}

/// Everything between this lesson's `prose:` array and the end of its `expect:` string.
///
/// Deliberately a raw slice of the source rather than the parsed strings: the literal
/// check wants the sentence as authored, and JavaScript escapes (`\u00a0`, `\"`) would
/// have to be undone consistently by both sides to compare parsed text. Taking the
/// source means the claim literal is matched against exactly what an author typed.
fn lesson_text(block: &str, id: &str) -> String {
    // Deliberately a panic and not a fallback. An earlier draft used `unwrap_or(0)`,
    // which degrades toward *passing*: slicing from 0 still yields text containing the
    // prose, so the literal check keeps going green on a scraper that has stopped
    // knowing what it is reading. A silent pass is the failure mode this whole file
    // exists to prevent — see the five-green harness in docs/plans/surface-vs-bulk.md.
    let start = block.find("\n    prose: [").unwrap_or_else(|| {
        panic!(
            "lesson `{id}` has no `prose: [` in the shape this scraper expects. The \
             lesson formatting changed; fix the scraper rather than letting it fall \
             back to a slice that still happens to contain the sentence."
        )
    });
    block[start..].to_string()
}

/// The `bms` field: `true`/`false` to force, `null` to leave whatever the scenario has.
///
/// The three cases are distinguished explicitly, and an unrecognised one panics. A
/// missing field silently reading as `null` would flip a BMS-on lesson to the
/// scenario default and move its numbers rather than fail.
fn parse_bms(block: &str, id: &str) -> Option<bool> {
    for (literal, value) in [
        ("\n    bms: true", Some(true)),
        ("\n    bms: false", Some(false)),
        ("\n    bms: null", None),
    ] {
        if block.contains(literal) {
            return value;
        }
    }
    panic!(
        "lesson `{id}` has no `bms:` field this scraper recognises (expected true, \
         false or null). Treating that as null would quietly change which pack the \
         lesson builds."
    )
}

/// Split `const LESSONS` into one block per step.
fn lessons() -> Vec<Lesson> {
    let src = app_js();
    let array_start = src
        .find("const LESSONS = [")
        .expect("web/app.js still declares `const LESSONS = [`");
    // Bounded at the array's own close, and this is not tidiness. Blocks are split on the
    // next `id:` marker, so without this the LAST lesson's block ran to the end of
    // `web/app.js` — some 240 lines of `proseHtml`, `setWatch` and friends. Its eight
    // claims' literal check was therefore a substring test against the page's source
    // code as well as its own prose, and any ledger over that step's whole prose would
    // have been scanning the numbers in a function body. Fails toward green, which is
    // the shape this file exists to keep out.
    let close = src[array_start..]
        .find("\n];")
        .expect("`const LESSONS` is still closed by a `];` at the start of a line");
    let body = &src[array_start..array_start + close];

    let id_marker = "\n    id: \"";
    let starts: Vec<usize> = body.match_indices(id_marker).map(|(i, _)| i).collect();
    assert!(
        !starts.is_empty(),
        "no `id:` fields found inside const LESSONS — the lesson formatting changed and \
         this scraper needs updating"
    );

    let default = default_dt();
    let mut out = Vec::new();
    for (n, &s) in starts.iter().enumerate() {
        let e = starts.get(n + 1).copied().unwrap_or(body.len());
        let block = &body[s..e];
        let id = str_field(block, "id").expect("lesson has an id");
        out.push(Lesson {
            scenario: str_field(block, "scenario")
                .unwrap_or_else(|| panic!("lesson {id} has no scenario")),
            demand: parse_demand(block),
            ambient_c: num_field(block, "ambient_c")
                .unwrap_or_else(|| panic!("lesson {id} has no ambient_c")),
            bms: parse_bms(block, &id),
            dt: num_field(block, "dt").unwrap_or(default),
            until_s: num_field(block, "until_s")
                .unwrap_or_else(|| panic!("lesson {id} has no until_s")),
            speed_x: num_field(block, "speed_x"),
            // Absent means "reload only if the scenario changed", which this scraper
            // cannot evaluate without knowing which step the reader came from — so it
            // reads as false, the conservative direction: the fence it feeds refuses a
            // probe claim rather than admitting one on a pack that may be inherited.
            reload: block.contains("\n    reload: true"),
            text: lesson_text(block, &id),
            id,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Running a step the way the page runs it
// ---------------------------------------------------------------------------

fn load(scenario_file: &str) -> (Scenario, sim_core::ChemistryParams) {
    let root = repo_root();
    let text = read(&root.join("scenarios").join(scenario_file));
    let scenario = parse_scenario(&text).expect("scenario parses");
    let id = scenario
        .chemistry
        .clone()
        .expect("scenario names a chemistry");
    let chem_text = read(&root.join("chemistries").join(format!("{id}.toml")));
    let chem = parse_chemistry(&chem_text).expect("chemistry parses");
    (scenario, chem)
}

fn build(lesson: &Lesson) -> Pack {
    let (scenario, chem) = load(&lesson.scenario);
    let enabled = lesson.bms.unwrap_or(scenario.pack.bms.is_some());
    let (pack, _dropped) = scenario
        .build_pack_with_bms(chem, enabled)
        .expect("pack builds");
    pack
}

/// The pack the BMS checkbox builds — the same file, with the protection grown on or off.
///
/// `$("bms").onchange` clicks Reset, which calls `SimEngine::restart(enabled)`, which
/// rebuilds from the scenario exactly as the first load did. It is not a running pack with
/// its BMS switched off: the run restarts at t = 0, which is what the page's own note under
/// the checkbox says it does.
///
/// The fault queue comes back with it — that rebuild reads the same scenario — which is
/// what makes step 19's unprotected arm the *same experiment*, short and all, rather than
/// a different one.
fn build_with_bms(lesson: &Lesson, enabled: bool) -> Pack {
    let (scenario, chem) = load(&lesson.scenario);
    let (pack, _dropped) = scenario
        .build_pack_with_bms(chem, enabled)
        .expect("pack builds");
    pack
}

/// The pack an author builds to ask what a mechanism was worth: the same scenario, the same
/// controls, one chemistry coefficient overwritten.
///
/// Nothing on the page reaches this. It is the counterfactual half of step 21's attribution,
/// and it edits the *parsed* chemistry rather than a file on disk — so the shipped
/// `chemistries/*.toml` is what every other trajectory in this suite still reads, and no
/// ordering between tests can leak the override. See [`Arm::fade_per_ah`] for why an arm is
/// allowed to be unwalkable and what that costs it.
///
/// `enabled` is the BMS as the arm or the lesson decides it, resolved by the caller for the
/// same reason [`build`] resolves it: the scenario is loaded twice otherwise.
fn build_with_reversal_fade(lesson: &Lesson, enabled: bool, fade_per_ah: f64) -> Pack {
    let (scenario, mut chem) = load(&lesson.scenario);
    chem.reversal.fade_per_ah = fade_per_ah;
    let (pack, _dropped) = scenario
        .build_pack_with_bms(chem, enabled)
        .expect("pack builds");
    pack
}

/// `pulsePhase`: the on-leg iff `round(t/dt) % (kOn + kOff) < kOn`.
///
/// The phase is counted in **steps** off the pack's own `sim_time_s`, never off an
/// accumulator this test keeps. That quantisation is the page's defence against float
/// drift, and a mirror with its own clock puts pulse edges a step out somewhere past
/// t = 1000 — where the error looks like physics rather than like bookkeeping.
fn pulse_on(sim_time_s: f64, on_s: f64, off_s: f64, dt: f64) -> bool {
    let k_on = (on_s / dt).round() as i64;
    let k_off = (off_s / dt).round() as i64;
    let k_period = k_on + k_off;
    if k_period <= 0 {
        return false;
    }
    let index = (sim_time_s / dt).round() as i64;
    let phase = ((index % k_period) + k_period) % k_period;
    phase < k_on
}

/// The demand a program asks for at this instant.
fn demand_now(prog: Prog, pack: &Pack, dt: f64, last: Option<&Telemetry>) -> Demand {
    match prog {
        Prog::Current(i) => Demand::Current(i),
        Prog::Rest => Demand::Rest,
        Prog::Pulse { i, on_s, off_s } => {
            if pulse_on(pack.sim_time_s(), on_s, off_s, dt) {
                Demand::Current(i)
            } else {
                Demand::Rest
            }
        }
        // `ccCvDemand`: below the band a constant current, at or above it a voltage hold.
        Prog::CcCv { i, v_cell, .. } => {
            let series = f64::from(pack.series());
            let target = v_cell * series;
            let band = 0.001 * series;
            let v = last.map_or(0.0, |t| t.v_terminal);
            if v < target - band {
                Demand::Current(-i)
            } else {
                Demand::Voltage(target)
            }
        }
    }
}

/// One sampled engine step.
struct Row {
    /// Simulation time at the *end* of the step \[s\].
    t_s: f64,
    telemetry: Telemetry,
    /// The pack's largest per-cell `soc_deficit`, as a fraction of capacity.
    ///
    /// Ground truth, read straight off `Pack::cell` — **not** what the `past empty` row
    /// shows. That row samples the same quantity on a wall-clock throttle and so lags a
    /// running simulation by up to a quarter-second of real time; step 21's own prose
    /// records it reading 9.438 points at an instant the engine was at 9.704. Claims
    /// measured from this field are therefore value-only and may name no `display`.
    deficit_max: f64,
    /// The pack's *smallest* per-cell `soc_deficit`, as a fraction of capacity.
    ///
    /// The other end of a spread no readout row prints. `past empty` shows the worst cell
    /// alone, and step 7's sentence is about the range — "the eight cells sit between 23.5
    /// and 27.8 points of charge past empty" — which is the pack grid on `past empty`
    /// rather than the row. Value-only for the same reason [`Self::deficit_max`] is, and
    /// one reason more: the grid is per-cell, so no single string renders it.
    deficit_min: f64,
    /// **Which cell** [`Self::deficit_max`] belongs to — `(series, parallel)`, from zero,
    /// the two numbers the pack grid's tiles are addressed by.
    ///
    /// An address rather than a quantity, and it is a measurement all the same: at the
    /// instant the first cell crosses empty it is the only cell with a debt, so the worst
    /// cell *is* the first crosser. That is the reading step 7's *"(0,0) first at 345.0 s"*
    /// rests on, and it is a proxy rather than an identity — read at any later instant this
    /// field says which cell owes most, which is a different sentence.
    ///
    /// **Before anything crosses it is (0, 0) by tie**, since every deficit is zero and
    /// [`deficit_range`] keeps the first cell it walks. Nothing may read an address at such
    /// an instant; a claim that did would be asserting the walk order rather than the pack.
    deficit_max_cell: (usize, usize),
    /// Which cell [`Self::deficit_min`] belongs to, read the same way.
    ///
    /// The mirror of the note above: at the instant the *last* cell crosses, it is the only
    /// one whose debt is still near zero, so the cell that owes least is the one that has
    /// just arrived. Step 7's *"(1,1) last at 356.5"*.
    deficit_min_cell: (usize, usize),
    /// The `surface gap` row's two numbers, bulk minus surface on each electrode, as
    /// fractions — `None` on an equivalent circuit, which has no electrodes.
    ///
    /// Per-cell like [`Self::deficit_max`], read the same way — and **on the same throttle**,
    /// which is what an earlier version of this comment got wrong. It said `past empty` was
    /// sampled on a 250 ms wall clock "and this row is not"; both are formatted from
    /// `cells`, and `cells` is sampled on `CELLS_PERIOD_MS` (250 ms) rather than per frame.
    /// Step 17's own prose says so from the reader's side — *"these two numbers are sampled
    /// four times a second while everything else is redrawn every frame"* — so the page, the
    /// lesson and this file now agree.
    ///
    /// What that costs is narrower than the wrong version implied and worth stating exactly,
    /// because it is what decides whether a claim here may name a `display`. A throttled row
    /// is only behind while something is **moving**: paused, the next sample catches up and
    /// stays. So the mirror is sound at an instant the reader can stop on — the zero-length
    /// probe, and the mark — and it is NOT sound mid-run, where the row can be up to a
    /// quarter-second of wall clock behind the voltage beside it (a dozen seconds of
    /// simulation at step 17's 200x). Claims read mid-run on this quantity are value-only
    /// for that reason, and say so in their notes; `past empty` differs only in that it has
    /// no un-throttled instant to be claimed at, since [`render_row`] refuses to mirror it
    /// at all.
    ///
    /// Cell `(0, 0)` because the page's readout reads `cells[0]` — the packs that have this
    /// quantity are 1S1P, which is a fact the readout's own doc comment turns on.
    surface_gap: Option<(f64, f64)>,
    /// What the BMS had **measured** as of the end of this step — `None` on a pack with no
    /// BMS, which has no sensors at all.
    ///
    /// The first thing in this file that is not ground truth. Every other field here is
    /// read off the engine's own state; this one is read off the only thing the protection
    /// logic is allowed to see, which is CLAUDE.md's eighth principle made measurable.
    sensed: Option<Sensed>,
    /// A **zero-length `Rest` read** taken at this instant \[V\] — what the cell's terminal
    /// would say with the current switched off and nothing given any time to relax.
    ///
    /// `Some` only on the leg boundaries of a pulse train (and on the probe row of a pulse
    /// step), because that is the only place anything reads it and a `dt = 0` step on a DFN
    /// is a whole Newton solve. `None` everywhere else, and every pulse quantity in
    /// [`measure`] refuses rather than falling back to `v_terminal`.
    ///
    /// **Why the decomposition needs it.** Steps 12 and 13 break one tooth into the part
    /// that returns the instant the current stops and the part that climbs back slowly, and
    /// the boundary between those two is the voltage at the moment the current goes away —
    /// which no stepped row holds. The first rest sample is already one `dt` into the
    /// relaxation, so a harness reading it gets *every* figure in both decompositions
    /// slightly low: the circuit's rebound comes out 71.8 mV against the 74.8 the prose
    /// states, which looks exactly like plausible drift and is not. Read this way all ten
    /// figures across the two steps reproduce to the digit. `docs/plans/path-prose-ledger.md`
    /// found this by hand; this is it made standing.
    ///
    /// Sound because `Pack::step(0.0, ..)` mutates nothing — the contract
    /// [`a_zero_length_probe_moves_nothing`] asserts, and the same one the page's `readNow`
    /// turns on. What it is *not* is a reading the page takes by itself: see the module
    /// docs on what a reader would have to do to see this number.
    rest_v: Option<f64>,
    /// The cell's **total** overpotential \[V\] — `CellView::overpotential_v` off cell
    /// `(0, 0)`, which is the cell the pack grid's first tile is.
    ///
    /// Free, in the sense that [`deficit_range`] and [`surface_gap`] already build that
    /// view on every row. Cell `(0, 0)` for [`surface_gap`]'s reason: the steps whose prose
    /// speaks of *the* cell's overpotential are 1S1P, and on a pack that is not, a sentence
    /// naming one cell's internals is on the wrong step rather than reading the wrong cell.
    overpotential_v: f64,
    /// The **RC-pair half** of [`Self::overpotential_v`] \[V\] — `Σ V_rc` on an equivalent
    /// circuit, leaving the fitted diffusion term as the difference.
    ///
    /// `Some` only at the instants a claim asked for, which is what makes this affordable.
    /// There is **no public accessor**: `CellView` carries the total and `EcmState::v_rc` is
    /// reachable only by serialising `Pack::snapshot()`, whose JSON carries the whole
    /// chemistry — three kilobytes of tables per row, on a step that takes 139 241 of them.
    /// So [`run`] is told the instants up front and [`drive`] pays the cost on those rows
    /// alone. It is the same fence [`Self::rest_v`] keeps for the same kind of reason, one
    /// level stricter: that one is taken wherever it *could* be read, this one only where
    /// it *is*.
    ///
    /// Reading it out of the snapshot is what keeps this a test-side quantity: no `sim-core`
    /// change, no new `CellView` field, and so no `sim_server::API_VERSION` /
    /// `sim_wasm::WASM_API_VERSION` bump for a number only a claim reads.
    rc_overpotential_v: Option<f64>,
    /// Whether this step ran under a **voltage hold** — the second leg of a CC-CV charge.
    ///
    /// The demand the step was taken with, recorded rather than inferred from the current.
    /// `|i| < the box` is what the *reader* sees, and it is not the same statement: it is
    /// also true of a BMS derate, of a clamped pack, and of the last constant-current step
    /// before a leg change if anything ever softened one. Step 9's sentence is about which
    /// demand the controller was issuing, so this is that demand.
    ///
    /// `false` on every other program, which is what it means — a pulse train and a plain
    /// discharge never hold a voltage. The one quantity that reads it ([`measure`]'s
    /// `cccv_cc_ends_s`) refuses on a step whose demand is not the page's CC-CV policy
    /// rather than reading a run of `false`s as "the leg never changed".
    voltage_hold: bool,
}

/// The sensor channels this file reads, with the frame's own clock beside them.
///
/// A cut-down [`sim_core::bms::SensorFrame`] rather than a clone of one, on the rule the
/// rest of this file follows: a field nothing reads is a pinned constant with no consulting
/// code, so only the channel a claim measures is carried. The frame's **time** is carried
/// whether or not a claim reads it, because it is what tells a measurement from a stale
/// one — see the refusal in [`measure_row`].
struct Sensed {
    /// Highest measured probe temperature \[K\], or `None` on a BMS with no probes.
    ///
    /// The hottest *instrumented* cell, which is not the hottest cell. That difference is
    /// the whole subject of the sentence this was built for.
    probe_max_k: Option<f64>,
    /// **Lowest** measured group voltage \[V\], or `None` on a BMS with no groups.
    ///
    /// The channel the hard under-voltage rung is judged on, and the second thing here that
    /// is belief rather than truth. Step 18's two sensor sentences are what it is for —
    /// *"sensors sampled at the end of the previous step, which still read a resting
    /// 3.3142 V per group"* and *"the frame taken after the spike reads 1.3336 V"* — and
    /// both are statements about the frame, not about the pack.
    ///
    /// **Measured, and worth stating: on this pack the two coincide exactly.** The voltage
    /// sensor in `sim-core` carries no offset and no noise term — only the current sensor
    /// does — so a group's measured voltage is its true one to the last bit, and a claim
    /// pointed at `v_cell_min_at` would read the same digits today. That is a property of
    /// the model and not of this step, and it is precisely why the claims read the sensor:
    /// the day a voltage-noise term lands, a truth-side claim would go on passing about a
    /// number the sentence is not about. CLAUDE.md's eighth principle is that the gap is a
    /// feature; a check that cannot see the gap cannot see it close either.
    v_group_min: Option<f64>,
    /// Simulation time this frame was sampled at \[s\].
    ///
    /// Sampling is gated on `dt > 0` inside `Pack::step`, so a zero-length probe does not
    /// refresh it and a paused pack's frame is legitimately old. The page renders exactly
    /// this comparison — `lag = simTime - sensors.sampled_at_s` — and greys its own panel
    /// when it is non-zero.
    sampled_at_s: f64,
}

/// The sensor frame a pack's BMS is holding, cut down to what this file reads.
fn sensed(pack: &Pack) -> Option<Sensed> {
    let frame = pack.bms()?.sensors();
    Some(Sensed {
        probe_max_k: frame.max_probe_k(),
        v_group_min: frame.v_group.iter().copied().reduce(f64::min),
        sampled_at_s: frame.sampled_at_s,
    })
}

/// `Σ V_rc` on cell `(0, 0)` \[V\], read out of the pack's serialised snapshot.
///
/// The one thing in this file that reaches a private field, and it reaches it the only way
/// the public API allows: `Snapshot` is `Serialize`, so the state is readable even where no
/// accessor is. The path is walked explicitly — `pack.groups[0].cells[0].model.<variant>` —
/// rather than searched for a `v_rc` key anywhere in the tree, on the same terms as
/// [`Tie::Name`]'s prefix: a search would silently answer off some other cell the day the
/// layout changes, where a walk fails loudly.
///
/// Panics on a cell model that has no RC pairs. A porous-electrode cell's overpotential does
/// not decompose this way at all — it is kinetic plus concentration, not placeholder plus
/// fitted — so a claim asking for the split there is a claim on the wrong step.
fn rc_overpotential_v(pack: &Pack) -> f64 {
    let json = serde_json::to_value(pack.snapshot()).expect("a pack snapshot serialises");
    let model = &json["pack"]["groups"][0]["cells"][0]["model"];
    let inner = model
        .as_object()
        .and_then(|m| m.values().next())
        .unwrap_or_else(|| panic!("a serialised cell model is a one-variant object: {model}"));
    let v_rc = inner["v_rc"].as_array().unwrap_or_else(|| {
        panic!(
            "this cell model carries no `v_rc`, so its overpotential has no RC half to \
             separate: {inner}"
        )
    });
    v_rc.iter().filter_map(serde_json::Value::as_f64).sum()
}

/// The `surface gap` row's pair, off the cell the page's readout reads.
fn surface_gap(pack: &Pack) -> Option<(f64, f64)> {
    let cell = pack.cell(0, 0).expect("pack has a cell at 0S0P");
    Some((cell.surface_gap_neg?, cell.surface_gap_pos?))
}

/// The pack's largest and smallest per-cell deficit, over ground truth.
///
/// The maximum is `Math.max` over every cell, which is what the page's row does — the
/// pack's *worst* cell rather than a mean, because cells do not pass empty together. The
/// minimum is the other end of that same disagreement, and it is the number step 7's
/// sentence needs: a pack whose cells all cross empty together has a range of zero, and
/// the whole point of the sentence is that this one does not.
///
/// Both walk the same loop rather than being two functions, so the pair can never be read
/// off two different sets of cells.
///
/// **Each end carries the cell it belongs to**, which is what lets step 7 name the cells
/// that cross empty first and last. The addresses come off the same walk as the values for
/// the same reason the two values do: an argument about *which* cell is worst, read off a
/// second loop, could disagree with the number beside it.
fn deficit_range(pack: &Pack) -> (DeficitEnd, DeficitEnd) {
    let mut worst = DeficitEnd {
        pts: 0.0,
        series: 0,
        parallel: 0,
    };
    let mut best = DeficitEnd {
        pts: f64::INFINITY,
        series: 0,
        parallel: 0,
    };
    for s in 0..usize::from(pack.series()) {
        for p in 0..usize::from(pack.parallel()) {
            let cell = pack
                .cell(s, p)
                .unwrap_or_else(|| panic!("pack has no cell at {s}S{p}P"));
            // Strictly greater / strictly less, so a tie stays with the cell the walk
            // reached first. It matters before anything crosses, where every deficit is
            // zero and both ends are (0, 0) by tie rather than by physics — which is why
            // nothing reads an address at such an instant. See `Row::deficit_max_cell`.
            if cell.soc_deficit > worst.pts {
                worst = DeficitEnd {
                    pts: cell.soc_deficit,
                    series: s,
                    parallel: p,
                };
            }
            if cell.soc_deficit < best.pts {
                best = DeficitEnd {
                    pts: cell.soc_deficit,
                    series: s,
                    parallel: p,
                };
            }
        }
    }
    (best, worst)
}

/// One end of the pack's deficit spread: how far past empty, and which cell.
#[derive(Debug, Clone, Copy)]
struct DeficitEnd {
    /// The cell's `soc_deficit`, as a fraction of capacity.
    pts: f64,
    /// Its series index, counted from zero — the first number of the pack grid's `(s, p)`.
    series: usize,
    /// Its index within that parallel group, also from zero.
    parallel: usize,
}

/// One step's trajectory, sampled every engine step — including its charge leg, if the
/// step has one, and the zero-length probe the page takes before any of it.
struct Run {
    rows: Vec<Row>,
    /// What `readNow()`'s `dt = 0` read reports before the reader presses Run.
    ///
    /// **Deliberately not a row in [`Self::rows`]**, and the separation is the whole
    /// design. Two independent reasons, either one sufficient:
    ///
    /// * **Time cannot name it.** [`Self::row_at`] addresses rows by nearest time, and a
    ///   probe shares its instant with a stepped row exactly — at the start of a run with
    ///   the first step's `t = dt` nearby, and (were probes ever taken mid-run) with the
    ///   step that ends at the same instant. The two differ by precisely the thing a probe
    ///   is for: one step of relaxation. An addressing scheme that cannot tell them apart
    ///   would hand an author whichever was pushed first.
    /// * **It would move every reduction.** `first_flag`, `flags_arriving_at`,
    ///   `delivered_ah`, `deficit_zero_s` and `soc_gap_pts_min` all fold over `rows`, and
    ///   a prepended row changes each of them silently. Measured, not feared:
    ///   `belief-drifts`'s probe reads a gap of 3.0000 points against the run's minimum of
    ///   3.0182 — a probe has no step to lag the truth by — so it would have stolen that
    ///   claim's minimum and moved its instant from 0.5 s to 0.0 without any prose
    ///   changing. See `docs/plans/path-probe-row.md`.
    ///
    /// So a claim *declares* that it reads the probe, exactly as its `arm` is declared
    /// rather than inferred from `read_at_s`.
    probe: Row,
    /// The pack's serialised snapshot when the run ends.
    ///
    /// Carried for one assertion and no claim reads it: step 18 says that clearing the
    /// queue and clearing the latch in either order leaves *an identical pack*, which is a
    /// statement about state rather than about any quantity. Serialised rather than
    /// compared field by field so the comparison is over everything the snapshot carries —
    /// including the RNG, which is the half a hand-written equality would forget.
    end_snapshot: String,
    /// The demand program the buttons were pressed under, and the step length they were
    /// pressed at — an arm's overrides already applied.
    ///
    /// Carried because one quantity is about the *program* rather than about the rows:
    /// `cccv_taper_s` needs the taper current the page is comparing against, and the
    /// decision grid `CCCV_PERIOD_S / dt` sets, and neither is recoverable from telemetry.
    /// Everything else in [`measure`] reads the trajectory alone.
    prog: Prog,
    /// How many groups the pack has in series.
    ///
    /// Carried for the same reason `prog` is, and used with it: the CC-CV target a reader
    /// sees is `v_cell * series`, which is `ccCvNote`'s own arithmetic and not a field of
    /// any file. `v_below_cccv_target_mv_at` is the only quantity that needs it. Read off
    /// the built pack rather than off the scenario so an arm that walks next door for its
    /// pack cannot be measured against the declaring step's topology.
    series: f64,
    dt: f64,
}

impl Run {
    /// The row a claim reads: the zero-length probe if it declares one, else the stepped
    /// row nearest `at_s`.
    fn read(&self, at_s: f64, probe: bool) -> &Row {
        if probe {
            &self.probe
        } else {
            self.row_at(at_s)
        }
    }

    /// The row whose end-of-step time is closest to `t`.
    ///
    /// The row carries its own time because the panel's clock renders the frame the
    /// reader is looking at, not the instant a claim was authored against.
    ///
    /// **Appending a leg cannot move what this returns for a pre-mark claim.** Every leg
    /// row is at `t > until_s` and a pre-mark claim reads at `read_at_s <= until_s`, so
    /// the nearest leg row is at least as far away as the mark's own row; where the two
    /// distances tie exactly, `min_by` keeps the earlier. The same holds for `first_flag`
    /// and for `v_at_soc_below` in [`measure`], which take the *first* match and so can
    /// only be answered by a leg row for something that never happened before the mark.
    fn row_at(&self, t: f64) -> &Row {
        self.rows
            .iter()
            .min_by(|a, b| (a.t_s - t).abs().total_cmp(&(b.t_s - t).abs()))
            .expect("run produced at least one row")
    }

    /// The telemetry of the row whose end-of-step time is closest to `t`.
    fn at(&self, t: f64) -> &Telemetry {
        &self.row_at(t).telemetry
    }

    /// The zero-length `Rest` read taken at exactly `t` \[V\].
    ///
    /// Exactly, not nearest, and that is the difference between this and [`Self::at`]. A
    /// leg boundary is one specific row and the row beside it is a whole step of relaxation
    /// away — the very quantity the read exists to separate — so a nearest match here would
    /// answer with the wrong side of the boundary and look right. Two failures are
    /// distinguished because they mean different things: the run never reached this
    /// instant, or it did and took no read there.
    fn rest_read_at(&self, t: f64) -> f64 {
        let row = self.row_at(t);
        assert!(
            (row.t_s - t).abs() < 1e-9,
            "no row at t = {t} s: the nearest is {} s. The run does not reach this tooth, \
             or the leg boundaries do not fall on the step grid.",
            row.t_s
        );
        row.rest_v.unwrap_or_else(|| {
            panic!(
                "the row at t = {t} s carries no zero-length rest read. Those are taken \
                 only on a pulse train's leg boundaries, so this instant is not one."
            )
        })
    }

    /// First simulation time a flag was seen at.
    fn first_flag(&self, name: &str) -> Option<f64> {
        self.rows
            .iter()
            .find(|r| format!("{:?}", r.telemetry.flags).contains(name))
            .map(|r| r.t_s)
    }

    /// The flags the step ending nearest `t` raises that the step before it did not.
    ///
    /// This is what tells an *event* instant from an ordinary read instant, and it is the
    /// fence on [`Accounted::ReadAt`]: a sentence that names the moment something happens
    /// is claiming that moment, where a sentence that merely reads a row at 250 s is not.
    /// Diffed against the previous row rather than taken from [`Run::first_flag`] so it
    /// needs no list of flag names to ask about.
    fn flags_arriving_at(&self, t: f64) -> Vec<String> {
        let names = |r: &Row| -> Vec<String> {
            let s = format!("{:?}", r.telemetry.flags);
            let inner = s
                .split_once('(')
                .map_or(s.as_str(), |(_, rest)| rest.trim_end_matches(')'))
                .to_string();
            inner
                .split('|')
                .map(|t| t.trim().to_string())
                // `EventFlags(0x0)` is how an empty set prints; a name is upper snake.
                .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
                .collect()
        };
        let i = self
            .rows
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (a.t_s - t).abs().total_cmp(&(b.t_s - t).abs()))
            .map(|(i, _)| i)
            .expect("run produced at least one row");
        let before = if i == 0 {
            Vec::new()
        } else {
            names(&self.rows[i - 1])
        };
        names(&self.rows[i])
            .into_iter()
            .filter(|f| !before.contains(f))
            .collect()
    }
}

/// How many steps one CC-CV decision lasts.
///
/// `advance` does not ask [`demand_now`] per step for a CC-CV run: it chops each frame's
/// steps at multiples of `CCCV_PERIOD_S / dt` and holds **one** demand across the whole
/// window, so which step the legs change on is a property of the simulation rather than
/// of how the browser scheduled a frame.
///
/// **The window was once missing here, and the cost was measured on `two-legs`** — the step
/// where the CV leg actually engages: without it the mirror decided every step, which is the
/// page's *inner* function without the loop around it, and the leg changed a step EARLY. The
/// page's first voltage hold is the step ending at 5420.5 s; the mirror's was 5420.0. Nothing in `path-claims.toml` moved when it was fixed,
/// because the only claimed CC-CV step then was `leg-that-is-not-there`, whose LFP cell never
/// reaches the band at all and is therefore on a constant current under either rule. That is
/// why it was invisible, not why it was harmless: that step's own claims landed nine hours
/// after the fix. Two of them read the boundary now — `cccv_cc_ends_s` at half a step, and
/// `i_at:5420`, which measured −1.5552 A with the window forced back off — so the gap could
/// not come back unseen.
///
/// The period itself is read off the page by [`cccv_period_s`] rather than repeated here.
fn cccv_window_steps(dt: f64) -> u64 {
    // `Math.max(1, Math.round(CCCV_PERIOD_S / dt))`.
    let k = (cccv_period_s() / dt).round();
    if k < 1.0 {
        1
    } else {
        k as u64
    }
}

/// Step `pack` on one demand program until its own clock reaches `end_s`.
///
/// The clock is the pack's, never an accumulator this function keeps, for the reason
/// [`pulse_on`] gives — and, for a CC-CV run, for the reason [`cccv_window_steps`] gives:
/// the decision grid is anchored to `sim_time_s` on the page too, which is what makes a
/// restore land back on the same decision points.
///
/// **What this still does not model** is `ccCvDone`, the page's completion test, which
/// stops the run when the current falls under the taper. It is checked at the end of each
/// *chopped chunk* rather than each window, so how long a finished charge keeps running
/// depends on how the browser scheduled its frames — the one place the page's CC-CV
/// behaviour is not a function of the simulation alone. So a run here does not stop where
/// the page would; it runs to the step's mark and a claim about the taper reads the
/// crossing off the rows.
///
/// That is enough to claim *when the page stops* only in the case where the crossing lands
/// on a decision-window boundary, because a chunk never crosses one — see the invariant in
/// [`measure`]'s `cccv_taper_s`, which refuses to answer anywhere else rather than
/// returning the earliest of several instants the page could show.
/// What [`drive`] accumulates: the rows so far, and the telemetry the last step returned.
///
/// One value rather than two out-parameters, because the two are read together — a
/// client-side demand program decides the next step from the previous step's telemetry, so a
/// driver holding one without the other could not run a CC-CV leg at all. It also keeps
/// `drive` inside clippy's argument budget now that it carries a capture list.
#[derive(Default)]
struct Trace {
    rows: Vec<Row>,
    last: Option<Telemetry>,
}

fn drive(
    pack: &mut Pack,
    prog: Prog,
    dt: f64,
    end_s: f64,
    env: &Env,
    trace: &mut Trace,
    capture: &[f64],
) {
    let windowed = matches!(prog, Prog::CcCv { .. });
    let k = cccv_window_steps(dt);
    let mut held: Option<Demand> = None;
    // `<=` because a mark that lands exactly on a step boundary is a mark the page
    // stops *at*, not one step before it.
    while pack.sim_time_s() < end_s - dt * 0.5 {
        let d = if windowed {
            let index = (pack.sim_time_s() / dt).round() as u64;
            if index.is_multiple_of(k) || held.is_none() {
                held = Some(demand_now(prog, pack, dt, trace.last.as_ref()));
            }
            held.expect("a window's demand is decided before it is used")
        } else {
            demand_now(prog, pack, dt, trace.last.as_ref())
        };
        let t = pack.step(dt, d, env);
        trace.last = Some(t);
        let (deficit_min, deficit_max) = deficit_range(pack);
        // The zero-length `Rest` read, and only where something reads it: a pulse train's
        // leg boundaries. `Pack::step(0.0, ..)` mutates nothing, so this cannot disturb the
        // trajectory — but it is a full solve on a porous model, which is why it is not
        // taken on every row. See [`Row::rest_v`].
        let rest_v = match prog {
            Prog::Pulse { on_s, off_s, .. } => {
                let now = pack.sim_time_s();
                let boundary =
                    pulse_on(now, on_s, off_s, dt) != pulse_on(now - dt, on_s, off_s, dt);
                boundary.then(|| pack.step(0.0, Demand::Rest, env).v_terminal)
            }
            _ => None,
        };
        let now = pack.sim_time_s();
        // The instants a claim asked the RC half for, matched on the same nearest-row rule
        // [`Run::row_at`] uses — half a step either side, so an on-grid `read_at_s` lands on
        // exactly one row and an off-grid one lands on the row `row_at` would return.
        let wanted = capture.iter().any(|w| (now - w).abs() <= dt * 0.5 + 1e-9);
        trace.rows.push(Row {
            t_s: now,
            telemetry: t,
            deficit_max: deficit_max.pts,
            deficit_min: deficit_min.pts,
            deficit_max_cell: (deficit_max.series, deficit_max.parallel),
            deficit_min_cell: (deficit_min.series, deficit_min.parallel),
            surface_gap: surface_gap(pack),
            sensed: sensed(pack),
            rest_v,
            overpotential_v: pack
                .cell(0, 0)
                .expect("pack has a cell at 0S0P")
                .overpotential_v,
            rc_overpotential_v: wanted.then(|| rc_overpotential_v(pack)),
            voltage_hold: matches!(d, Demand::Voltage(_)),
        });
    }
}

/// Run a step the way the page runs it — and then, on an arm, the buttons the step's prose
/// tells the reader to press.
///
/// Three shapes, and which one this is comes off the arm rather than out of this function:
///
/// * **No arm.** The step as configured, run to its mark. What every claim without an
///   `arm` reads.
/// * **[`Start::Mark`].** The same trajectory to the mark, then the actions. The pack is
///   the same one continuing, not a second run: at the mark `pathArrived` sets
///   `path.until = null` and pauses, the demand box has no change handler at all —
///   `advance` reads it fresh on the next frame — so pressing Run after typing a new
///   current resumes with nothing rebuilt and `dt` unchanged.
/// * **[`Start::Restart`].** A fresh pack under this arm's controls, then the actions. The
///   page's Restart button rebuilds from the scenario and leaves the controls alone, which
///   is why `dt` and `bms` overrides belong to the pack from t = 0 here.
///
/// Every arm drives its own pack from scratch. It costs a re-run of the pre-mark
/// trajectory for each continuation arm, and it buys the thing step 18 needs: four arms
/// branching off one mark, none of them able to see another's buttons.
///
/// **`lesson` is the step the arm is declared on; the pack may come from a different one.**
/// [`Arm::pack_from`] names the lesson a reader walks back to, and everything the *pack* is
/// — scenario, timestep, ambient, BMS, standing demand — is read off that lesson instead,
/// with this arm's own overrides on top. `lesson` still decides nothing but where the
/// instruction was written, which is why it stays the first argument.
fn run(lesson: &Lesson, arm: Option<&Arm>, capture: &[f64], lessons: &[Lesson]) -> Run {
    // The lesson the pack belongs to. Identical to `lesson` for every arm but a twin, and
    // resolved by id rather than by index so a reordering of `const LESSONS` cannot quietly
    // repoint one. A name with no lesson behind it is caught in
    // `every_arm_is_instructed_by_its_own_step`; here it would be a silent fallback to the
    // wrong pack, so it panics.
    let lesson = pack_lesson_of(arm, lesson, lessons);
    // The picker, which is the one override that changes the FILE without naming a lesson.
    // A restart is implied ([`assert_picker`]), so this pack is built from the named file at
    // t = 0 under the step's own controls — `loadScenario` touches none of them — with the
    // arm's own overrides on top exactly as they go on top of a walk.
    let picked;
    let lesson = match arm.and_then(|a| a.scenario.as_deref()) {
        Some(file) => {
            picked = Lesson {
                scenario: file.to_string(),
                ..lesson.clone()
            };
            &picked
        }
        None => lesson,
    };
    let dt = arm.and_then(|a| a.dt).unwrap_or(lesson.dt);
    let mut pack = match (arm.and_then(|a| a.bms), arm.and_then(|a| a.fade_per_ah)) {
        // The counterfactual comes first because it has to resolve the BMS itself: an arm
        // that overrides both would otherwise get one branch or the other rather than both.
        // No arm does today, and this is written the correct way round rather than the
        // reachable way round — see `Arm::fade_per_ah`.
        (bms, Some(fade)) => {
            let (scenario, _) = load(&lesson.scenario);
            let enabled = bms.or(lesson.bms).unwrap_or(scenario.pack.bms.is_some());
            build_with_reversal_fade(lesson, enabled, fade)
        }
        (Some(bms), None) => build_with_bms(lesson, bms),
        (None, None) => build(lesson),
    };
    // TWO environments, split at the mark: the step's slider before it, the arm's after.
    //
    // This used to be one, which was sound only while an ambient override implied
    // [`Start::Restart`] — a restart arm has no pre-mark segment, so no stretch of its
    // trajectory ran under the step's own slider. Step 8 is the sentence that paid for the
    // split: it asks a reader to raise the slider *at* the mark and press Run, so the pack
    // that produces its second leg spent 200 000 s at 25 °C first, and a single 45 °C
    // environment would fade it further than the sentence says. See
    // `docs/plans/path-derived-arm.md`.
    //
    // A restart arm sees its own slider from t = 0. It skips the pre-mark drive, so the only
    // thing that branch reaches on such an arm is the **probe** — the readouts a reader sees
    // after clicking Restart with the slider already dragged, which is the arm's ambient and
    // not the lesson's.
    //
    // **Measured, and unobservable in principle rather than merely today.** Flipping that
    // branch to the lesson's ambient leaves the whole suite green, and the recorded reason was
    // that no claim reads a probe on a restart arm that overrides the ambient — with "the
    // claim that would reach it is a `probe = true` reading on one of them" written down as
    // the way to close it. That is false, and measuring it is what showed it: a zero-length
    // step cannot see an environment at all. Probed at 25 °C and at 45 °C on this step's own
    // fresh pack, the telemetry and the snapshot are byte-identical — nothing in `Env` reaches
    // telemetry except through a `dt` that is zero here. So on a restart arm this branch is
    // dead to every claim that could ever be written, and it is kept written the correct way
    // round as a statement of what the page does. See
    // `docs/plans/path-ledger-idle-step.md`, which is where that measurement is.
    let after = Env {
        t_ambient: arm.and_then(|a| a.ambient_c).unwrap_or(lesson.ambient_c) + K,
        t_coolant: None,
    };
    let before = Env {
        t_ambient: match arm.map(|a| a.start) {
            Some(Start::Restart) => after.t_ambient,
            _ => lesson.ambient_c + K,
        },
        t_coolant: None,
    };
    // The probe and the pre-mark drive take `before` — the probe because it is read before
    // the run is armed, which on a continuation is the step's own slider whatever the arm
    // does later. The actions take `after`.
    //
    // `applyStep`'s `await readNow()`, taken after the step's controls are dialled in and
    // before the run is armed: a `dt = 0` step under **this step's** demand, which for a
    // `Pulse` is the leg its own clock is on rather than `Rest`. It is what fills the
    // readouts a reader sees "before you press Run".
    //
    // Taken first and used as-is, because `Pack::step(0.0, ..)` mutates nothing: probed
    // twice on an SPM, a BMS pack and a circuit, the second telemetry is bit-identical to
    // the first and `snapshot()` is unchanged across both. That is what makes the page's
    // *two* probes on a reloading step — one under the stale demand box in `loadScenario`,
    // then this one — reproducible by modelling only the second.
    //
    // **The box as it stands when the probe is taken**, which is not always the step's.
    // `applyStep` dials the step's own demand in and probes, so a continuation arm's probe
    // is the step's — the reader has not typed anything yet. A **restart** arm is the other
    // order: the reader types a current and *then* presses Restart, and `loadScenario`'s
    // `readNow` answers for the box in front of them. Measured, and unobservable today —
    // flipping this back to the step's demand leaves the suite green, because no claim reads
    // a probe on a restart arm that types a current. Written the correct way round rather
    // than the reachable way round, on the same terms as the ambient split above.
    let probe_prog = match arm {
        Some(a) if a.start == Start::Restart => arm_prog(a, lesson.demand),
        _ => lesson.demand,
    };
    let probe_telemetry = pack.step(0.0, demand_now(probe_prog, &pack, dt, None), &before);
    let (probe_deficit_min, probe_deficit_max) = deficit_range(&pack);
    let probe = Row {
        t_s: pack.sim_time_s(),
        telemetry: probe_telemetry,
        deficit_max: probe_deficit_max.pts,
        deficit_min: probe_deficit_min.pts,
        deficit_max_cell: (probe_deficit_max.series, probe_deficit_max.parallel),
        deficit_min_cell: (probe_deficit_min.series, probe_deficit_min.parallel),
        surface_gap: surface_gap(&pack),
        sensed: sensed(&pack),
        // The open-circuit end of the first tooth's sag, and the only place to get it: at
        // t = 0 a pulse train is already on its first ON leg, so the probe above is taken
        // under load and there is no row before it. Pulse steps only, for the reason
        // [`Row::rest_v`] gives.
        rest_v: matches!(lesson.demand, Prog::Pulse { .. })
            .then(|| pack.step(0.0, Demand::Rest, &before).v_terminal),
        overpotential_v: pack
            .cell(0, 0)
            .expect("pack has a cell at 0S0P")
            .overpotential_v,
        // Never on the probe. A claim reading the RC half declares an instant of the run;
        // the probe is not one, and [`Run::read`] would hand it over to any claim that set
        // `probe = true` — which is the "whichever was pushed first" hazard the probe row
        // is kept out of `rows` to avoid.
        rc_overpotential_v: None,
        // A probe is taken before the run is armed, so it is nobody's leg. On a CC-CV step
        // the demand above is whatever `ccCvDemand` answers for a pack at rest, and no
        // quantity asks this of the probe row — `cccv_cc_ends_s` folds over `rows`.
        voltage_hold: false,
    };
    let mut trace = Trace::default();

    // A continuation arm, and a step with no arm at all, both run the step as configured
    // first. A restart arm does not: its pack is the rebuilt one and its clock starts at
    // zero, so the actions are the whole trajectory.
    if arm.is_none_or(|a| a.start == Start::Mark) {
        drive(
            &mut pack,
            lesson.demand,
            dt,
            lesson.until_s,
            &before,
            &mut trace,
            capture,
        );
    }

    if let Some(arm) = arm {
        // The demand box as it stands while the buttons are pressed: the arm's own current
        // if it typed one in, else whatever the step dialled in. Note that typing a current
        // replaces the step's *program* — a reader who types a number into the box on a
        // `Pulse` step has left the pulse train, which is what the box does.
        let prog = arm_prog(arm, lesson.demand);
        for action in &arm.actions {
            match action {
                // Neither button advances the clock, which is the whole of the step-18
                // sentence they exist for. Nothing is pushed to `rows`: there is no frame,
                // because the page renders no frame — "nothing advances while the page is
                // paused".
                Action::ClearQueued => {
                    pack.clear_faults();
                }
                Action::ClearLatched => {
                    pack.clear_bms_fault();
                }
                // One step, expressed as a `drive` to one `dt` away rather than as a
                // second stepping loop, so a `Step 1` on a windowed CC-CV program cannot
                // diverge from what a `Run` of the same length would do.
                Action::Step1 => {
                    let to_s = pack.sim_time_s() + dt;
                    drive(&mut pack, prog, dt, to_s, &after, &mut trace, capture);
                }
                Action::Run { to_s } => {
                    drive(&mut pack, prog, dt, *to_s, &after, &mut trace, capture);
                }
            }
        }
    }

    let end_snapshot = serde_json::to_string(&pack.snapshot()).expect("a pack snapshot serialises");
    Run {
        rows: trace.rows,
        probe,
        end_snapshot,
        // The program the trajectory's last stretch ran under, which for an arm that typed
        // a current into the box is that current and not the step's own. Same rule the
        // stepping loop above uses, because it is the same fact.
        prog: arm.map_or(lesson.demand, |a| arm_prog(a, lesson.demand)),
        series: f64::from(pack.series()),
        dt,
    }
}

// ---------------------------------------------------------------------------
// The claims
// ---------------------------------------------------------------------------

/// Which rule a claim's `tol` follows.
///
/// Before this existed, `tol` was a free number with its justification in prose, and the
/// justification drifted from the number twice in two commits (`04933c5`, `a1b0945`) —
/// both times a note citing "half a unit in the last printed place" beside a tolerance
/// that was a half-step instead. This turns the citation into a field the test can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum TolFrom {
    /// The prose spells this claim's quantity, and `tol` is exactly half a unit in that
    /// number's last printed place. The default shape: 239 of 283 claims.
    Spelled,
    /// Same, but `tol` is strictly *tighter* than that rule. Safe by construction — a
    /// smaller tolerance can only redden the test — so it needs no cap, only proof that
    /// the rule it beats is still computable. Used where a claim is pinned harder than
    /// the sentence needs (a chemistry constant, an exactly-1.0 starting point), where
    /// the prose hedges a round number the engine misses by more than its last place, for
    /// an **address** — step 7 names the cells that cross empty first and last, and an
    /// index is an integer the engine either reports or does not, so half a unit in its
    /// last place is slack with no meaning — and for four grid times whose prose *does*
    /// spell them: half a step is tighter than the whole second those sentences print, so
    /// the number was always right and only the declaration was wrong. 38 of 283.
    Tighter,
    /// The quantity is a time the engine can only report on the step grid, and the prose
    /// spells no number in it — it gives a consequence, or a rendering of the clock.
    /// `tol` is half a timestep, which for a grid time is the tightest meaningful bound:
    /// the engine either hits the claimed step or misses by a whole one. 6 of 283, every
    /// one of them a claim whose [`States`] is `nothing` or `displayed`: a claim that
    /// spells its own number takes that number's rule instead, however coarse the grid is.
    ///
    /// This is the variant that could have re-licensed the defect it was written after,
    /// so it is fenced three ways in [`every_tolerance_follows_its_declared_rule`]. In
    /// particular a literal that spells anything finer than a half-step is not eligible:
    /// `**383.0 s later**` returns a grid time too, and half a step is five times looser
    /// than the tenth it prints.
    Grid,
}

/// What the claim's own sentence says about the number the engine produces.
///
/// This runs prose → value, which is the direction nothing in this file used to run.
/// `literal` was a substring test against the page and `value` a comparison against the
/// engine, and the two never met: re-measure a drifted `value`, leave the sentence
/// alone, and both halves stay green while the prose is wrong. A drift small enough to
/// hide inside a readout row's rounding — 0.6387 V becoming 0.6392 — passed the display
/// check too.
///
/// The obvious closing move, "format `value` and require the result in the prose", is
/// the one this file has refused from the start: a formatter that has to agree with how
/// a human wrote each sentence generates false failures and then gets suppressed. What
/// makes the tie possible instead is that English states a number in only a few frames,
/// and a claim can name which one it is using. Each variant below is one frame, and each
/// is checked at the *sentence's* own precision — half a unit in the last printed place
/// of `spells`, the same rule [`TolFrom`] uses, because the question here is exactly
/// "does the engine's number round to the one the reader is shown".
///
/// This also closes the leverage `spells` used to have. It was required to be *a* number
/// in the claim's literal and never the one stating this claim's quantity, so on a
/// multi-number sentence an author could name the coarsest and take its tolerance —
/// pointing the voltage claim on `The cell empties at 4146.5 s at 1.9290 V` at `4146.5`
/// left every test green with the voltage pinned a thousand times looser than its
/// sentence licensed. Under `same` that mis-pointing compares 4146.5 against 1.9290 and
/// fails on sight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum States {
    /// The sentence prints the quantity itself. 260 of 283, and the shape to prefer: it is
    /// the only variant with no second reading available to an author.
    Same,
    /// The sentence prints the magnitude and puts the sign in a word — `refused 0.822 A`
    /// of a current of −0.82224 A. Fenced to negative values, because on a positive one
    /// it is a silent alias for `same`.
    Magnitude,
    /// The sentence prints how far *below one* the value is: `0.53 points are gone` of a
    /// `soh cap` of 0.9947106. Health is the only quantity here that is naturally read as
    /// a shortfall, and the sentences about it say so.
    Complement,
    /// The sentence prints a duration since this step's mark — `**383.0 s later**` of an
    /// absolute 983.0 s on a step whose `until_s` is 600. Fenced to claims read on an arm
    /// that *continues* the step's own run: before the mark there is nothing to be later
    /// than, and on an arm that restarts the pack the mark is not an origin at all.
    SinceMark,
    /// The sentence prints a duration *remaining* to the mark — `the last 53 seconds` of
    /// a flag at 4146.5 s on a step that ends at 4200. Fenced to pre-mark claims, which
    /// with the fence above stops an author trying both frames until one fits.
    UntilEnd,
    /// The sentence does not print the engine's number at all — it prints what the row
    /// prints, and the row is a formatter: `it goes from `10m` to `16m`` of 983.0 s. The
    /// tie is the display check rather than any arithmetic, so this variant asserts the
    /// chain is complete instead of measuring: the claim must name a row, must be
    /// `quoted`, and the string that row prints must sit inside *this* claim's literal.
    /// Then literal ⊇ shows == the rendering of the measured value, end to end. A
    /// `spells` is forbidden, or the two claims that could be read either way would be
    /// the author's pick rather than the sentence's.
    Displayed,
    /// The sentence names a value the quantity has *left*: the second half of ``soh cap`
    /// leaves 100.00 % at t = 10 s`, read on the step where it is no longer 100.00.
    /// Asserts the opposite of `same` — the engine must be further from the spelled
    /// number than the sentence's own precision — and is the one variant that could be
    /// satisfied by pointing at any unrelated figure, so it is not allowed to stand
    /// alone: see the sibling fence in [`every_claim_states_the_value_it_measures`].
    Departure,
    /// The sentence prints no number about this quantity — `An `OC` flag` says only that
    /// it arrived. Checkable rather than declared: the literal must contain no digits at
    /// all, which is what stops this being the escape hatch that re-opens the hole.
    Nothing,
}

#[derive(Debug, serde::Deserialize)]
struct Claim {
    step: String,
    literal: String,
    quantity: String,
    value: f64,
    tol: f64,
    /// Which rule `tol` follows. Required, with no serde default on purpose: a default
    /// would hand a new claim a justification nobody chose, which is the "looks like
    /// coverage" shape this file rejects everywhere else.
    tol_from: TolFrom,
    /// The number in `literal` that states this claim's quantity, written exactly as the
    /// sentence writes it — `"4.030"`, not `4.03`; `"-0.069"` for a prose minus. Required
    /// by `spelled` and `tighter`, forbidden by `grid`.
    ///
    /// "Exactly as the sentence writes it" includes writing it in letters: a sentence that
    /// says "about **three** points" spells `"three"`, resolved through [`WORD_NUMERALS`].
    /// A word commits to no decimal place, so the rule it licenses is half a unit of the
    /// unit it is written in — half a point, here.
    ///
    /// It is the *printed places* that matter, so the frame does not have to match: the
    /// prose may give a duration where the claim reads an absolute time, or a magnitude
    /// where the value is negative. A tolerance is a precision, and a precision does not
    /// care about sign or origin. What it does care about is the unit, which is
    /// `spells_pow10`.
    #[serde(default)]
    spells: Option<String>,
    /// How many powers of ten larger the unit `spells` is written in is than `value`'s.
    /// `2` for a prose percentage or points-of-a-hundred against a fraction; `0`
    /// otherwise, which is most of them.
    #[serde(default)]
    spells_pow10: i32,
    /// Which frame the sentence states this claim's quantity in — see [`States`].
    /// Required, and with no serde default for the reason `tol_from` has none: a default
    /// would hand a claim a reading of its own sentence that nobody chose.
    states: States,
    read_at_s: f64,
    /// The readout row this claim is about, if it is about one at all — a label from
    /// `READOUTS` in `web/app.js`.
    #[serde(default)]
    display: Option<String>,
    /// Exactly what that row prints at `read_at_s`. Recorded as a string for the same
    /// reason `literal` is: it is what a reader sees, not a rendering of `value`.
    #[serde(default)]
    shows: Option<String>,
    /// Does the prose quote that string verbatim? Opt-in, because a sentence is entitled
    /// to state a measured quantity without quoting the panel — but a sentence that tells
    /// the reader to go and *look* at the row is not.
    #[serde(default)]
    quoted: bool,
    /// Which of the step's `[[arm]]`s this claim is read on — the name of one — or absent
    /// for the step's own trajectory, run to its mark and no further.
    ///
    /// Explicit rather than inferred from `read_at_s`, and for a sharper reason than the
    /// one `after_mark` had: a restart arm covers the *same* instants as the step's own run
    /// (step 18's `dt = 5` arm ends at the mark, like the step) and reports different
    /// numbers there. Time cannot name the trajectory. Checked in both directions — a claim
    /// naming an arm that does not exist, or reading at an instant that arm never reaches,
    /// is a claim about a trajectory nobody ran.
    #[serde(default)]
    arm: Option<String>,
    /// Is this claim read on the **zero-length probe** — what the panel shows before the
    /// reader presses Run, rather than on any step of the trajectory?
    ///
    /// Declared and not inferred, for the reason `after_mark` is: `read_at_s = 0` cannot
    /// mean it, because [`Run::row_at`] answers 0 with the *first stepped row*, and the
    /// two readings differ by exactly one step of relaxation — which on the pulse steps is
    /// 2.9 mV out of 74.8. See [`Run::probe`] for what the probe is and why it is not a
    /// row, and [`every_probe_claim_is_taken_before_the_run`] for the three fences on it.
    #[serde(default)]
    probe: bool,
    #[allow(
        dead_code,
        reason = "authoring context for a human reader, not asserted"
    )]
    note: String,
}

impl Claim {
    /// `(row label, printed string)` if this claim makes a display claim at all.
    ///
    /// Half a display claim is rejected rather than ignored. A `display` with no `shows`
    /// asserts nothing while looking like coverage, and a `shows` with no `display` names
    /// no row to render — both are the "fails toward green" shape this file exists to
    /// keep out.
    fn display_claim(&self) -> Option<(&str, &str)> {
        match (&self.display, &self.shows) {
            (Some(d), Some(s)) => Some((d.as_str(), s.as_str())),
            (None, None) => {
                assert!(
                    !self.quoted,
                    "claim `{}` on step `{}` sets quoted = true but names no display row \
                     and no shown string — there is nothing for the prose to quote.",
                    self.literal, self.step
                );
                None
            }
            (Some(d), None) => panic!(
                "claim `{}` on step `{}` names the display row `{d}` but no `shows`. A \
                 display row without the string it prints asserts nothing.",
                self.literal, self.step
            ),
            (None, Some(s)) => panic!(
                "claim `{}` on step `{}` says the panel shows `{s}` but does not say \
                 which row shows it. Add `display`.",
                self.literal, self.step
            ),
        }
    }

    /// The tolerance this file's rule gives: half a unit in the last printed place of
    /// `spells`, brought into `value`'s unit by `spells_pow10`.
    ///
    /// Computed as one multiply — `5 × 10^e` — rather than as `half_unit / 10^pow10`,
    /// so a percentage claim's rule is the same float however it is spelled.
    fn spelled_rule_tol(&self) -> f64 {
        let spells = self.spells.as_deref().unwrap_or_else(|| {
            panic!(
                "claim `{}` on step `{}` is `{:?}` and names no `spells`. Both spelled \
                 rules are derived from the number the sentence prints; without it there \
                 is no rule, only a number.",
                self.literal, self.step, self.tol_from
            )
        });
        assert!(
            spells_as_number(spells).is_some(),
            "claim `{}` on step `{}` spells `{spells}`, which is neither digits nor a word \
             in WORD_NUMERALS.",
            self.literal,
            self.step
        );
        // A word carries its own unit and there is nothing to convert: "three points" is
        // three points, where `"0.53"` against a fraction is a percentage that has to be
        // brought down two decades. Left unfenced, `spells = "three"` with pow10 = 2 reads
        // the sentence as 0.03 and licenses a tolerance of 0.005 — a scale nobody wrote,
        // on the one kind of `spells` whose text cannot show it.
        assert!(
            !WORD_NUMERALS.iter().any(|(w, _)| *w == spells) || self.spells_pow10 == 0,
            "claim `{}` on step `{}` spells the word `{spells}` and sets spells_pow10 = {}. \
             A word is written in the unit the sentence uses and has no other reading; a \
             scale here would silently move both the number and its tolerance.",
            self.literal,
            self.step,
            self.spells_pow10
        );
        assert!(
            self.spells_pow10.abs() <= 12,
            "claim `{}` on step `{}` sets spells_pow10 = {}. That is a unit conversion of \
             a thousand billion; it is far likelier to be a typo.",
            self.literal,
            self.step,
            self.spells_pow10
        );
        5.0 * 10f64.powi(-(spelled_places(spells) + 1) - self.spells_pow10)
    }

    /// The number the sentence spells, brought into `value`'s own unit.
    ///
    /// The same `spells_pow10` that scales the tolerance scales the number, so a claim
    /// whose prose speaks percent against a fraction is compared in one place and in one
    /// unit. Panics through [`Claim::spelled_rule_tol`]'s message if there is no `spells`
    /// — every caller has already established that this variant needs one.
    fn spelled_value(&self) -> f64 {
        let spells = self.spells.as_deref().unwrap_or_else(|| {
            panic!(
                "claim `{}` on step `{}` states `{:?}` and names no `spells`.",
                self.literal, self.step, self.states
            )
        });
        let n: f64 = spells_as_number(spells).unwrap_or_else(|| {
            panic!(
                "claim `{}` on step `{}` spells `{spells}`, which is neither digits nor a \
                 word in WORD_NUMERALS.",
                self.literal, self.step
            )
        });
        n * 10f64.powi(-self.spells_pow10)
    }
}

/// How many digits `s` prints after its decimal point.
fn decimals_of(s: &str) -> i32 {
    match s.find('.') {
        Some(i) => (s.len() - i - 1) as i32,
        None => 0,
    }
}

/// The numbers this path's prose spells in letters rather than in digits.
///
/// A sentence is entitled to write a round quantity as a word — "a gap of about **three
/// points**" — and until this table existed such a sentence could not be claimed at all:
/// `spells` is the number as the sentence writes it, and every reader of it parsed the
/// string as a float. The choice was then between a claim that lies about its own wording
/// and no claim, which is how that sentence went four slices unchecked.
///
/// **Every entry is required to be used.** A word nothing reads is the `CCCV_PERIOD_S`
/// shape this file rejects everywhere else, so [`every_word_numeral_is_read_by_something`]
/// fails on a table entry nothing consults — the same guard
/// [`every_ledger_rule_is_a_phrase_and_is_used`] keeps over the ledger's vocabulary. Add the
/// next word when the next claim or rule needs it, not before.
///
/// **Two readers, and neither is a scanner.** `spells` is the claim side. `six` is the
/// ledger side, read as the operand of one derivation ([`Operand::Word`]) — step 22's "six
/// of these in series is the 12 V battery". Both are places an author *names* a word.
/// This table is **not** the scanner's alphabet, and the difference is load-bearing:
/// [`CARDINALS`] is what [`spelled_numbers`] reads, consulted for every word of a
/// word-scanned step whether or not an author named it. Were the scanner to read this table
/// instead, every entry would be "used" by construction and the guard above would stop
/// discriminating — a live check disarmed while looking like coverage. Where the two
/// overlap they are held equal, by [`the_two_word_tables_cannot_disagree`].
const WORD_NUMERALS: &[(&str, f64)] = &[
    ("six", 6.0),
    // Read by neither a claim nor a rule but by an ARM: step 21's counterfactual sets the
    // over-discharge coefficient "to zero", and `Arm::fade_per_ah` is held to the same
    // "the value must be spelled in the instruction" rule as the demand box and the `dt`
    // box. `contains_number` already bounds a word match the way it bounds a digit one.
    ("zero", 0.0),
];

/// The number `spells` names, in the unit the sentence writes it in, or `None` if the
/// string is neither digits nor a word this file knows.
///
/// The one place words are resolved. `spelled_rule_tol` and `spelled_value` both used to
/// call `parse` themselves, and a second resolution site is how a word could come to be a
/// number for the tolerance and not for the value.
fn spells_as_number(spells: &str) -> Option<f64> {
    number_of(spells).or_else(|| {
        WORD_NUMERALS
            .iter()
            .find(|(w, _)| *w == spells)
            .map(|(_, v)| *v)
    })
}

/// How many places `spells` prints after its decimal point — the precision the sentence
/// commits to, which is what both spelled tolerance rules are half a unit of.
///
/// A word prints none: "three points" is a claim to the point, and half a unit of it is
/// half a point. Written as its own branch rather than left to [`decimals_of`] finding no
/// `.` in `three` and returning 0 by accident.
fn spelled_places(spells: &str) -> i32 {
    if WORD_NUMERALS.iter().any(|(w, _)| *w == spells) {
        0
    } else {
        decimals_of(spells)
    }
}

/// One number as the prose writes it: where it starts, what it reads as, and how many
/// bytes of source the token covers.
///
/// **The paragraph here used to say the opposite, and it was the reason the field was
/// wrong.** It read: "The length is not `token.len()`: the scanner below truncates
/// `at 5769.` and `1.2.3`, so the token is shorter than the run it came from. The ledger's
/// phrase matcher needs the run, because what follows a number in the sentence begins after
/// the characters that were actually there." The second sentence is true and the conclusion
/// drawn from it is backwards: what follows `5769` in the source is the full stop, so a
/// phrase written as `"at {n}. The"` needs the matcher to stop *before* it. Both readers of
/// this field wanted the token and neither said so.
#[derive(Debug, Clone)]
struct Written {
    /// Byte offset of the first digit, in the text this was scanned from.
    at: usize,
    /// The number as written, sign excluded — `"4146.5"`.
    token: String,
    /// Bytes of source the **token** covers, from `at`.
    ///
    /// The token and never the raw digit run, and the difference is a defect this file
    /// shipped: `at 5769.` scans as a run of five bytes and a token of four, and while
    /// this field carried the five, [`claimed_accounting`] read `at + len` as the number's
    /// extent and decided a token sitting at the very end of a claimed literal was
    /// *outside* it. Every such number was therefore invisible to check 6's accounting
    /// when the ledger asked, which fails toward red on the number itself — a rule is
    /// demanded for something a claim already spells — and toward **green** on the
    /// double-accounting panic beside it, which is the hazard the whole taxonomy is
    /// arranged against. See `docs/plans/path-ledger-one-step-that-got-through.md`.
    ///
    /// [`join_thousands`] already measured from the trimmed end for its own reason, so it
    /// is unaffected; the joined token keeps its space and covers the source exactly.
    len: usize,
    /// What **one** of the unit `token` is written in comes to, in the unit a tie reads.
    ///
    /// `1.0` for every number [`written_numbers`] finds, and for a spelled quantity whose
    /// unit is the tie's own. `60.0` where the sentence writes minutes and the file holds
    /// seconds; `3600.0` for an hour.
    ///
    /// This exists so that two things can be true at once, and both are needed on the first
    /// step scanned this way. *"at three minutes"* names an instant a claim is read at
    /// **exactly** — 3 × 60 is 180 and `read_at_s` is 180 — so the arms that compare an
    /// instant must see seconds. *"twenty-four minutes"* is a rounded reading of 1464 s, so
    /// the arm that compares a tie against the prose must work in **minutes**, where the
    /// sentence commits to one and the rounding is half of it. A scan that canonicalised to
    /// seconds would tighten the second to half a second and redden a true sentence; one
    /// that stayed in minutes would loosen the first to half a minute and stop noticing
    /// which of six instants a list item names.
    ///
    /// Not a `pow10` because minutes are not a decade. `pow10` is still the claim's and the
    /// rule's own field and means what it always did; this is per **token**, because it is
    /// the token's own unit that decides it.
    scale: f64,
    /// The source text this token was read from — `"twenty-four minutes"`.
    ///
    /// Equal to `token` for a number written in digits, and different for every spelled
    /// one: `token` is the numeral as a decimal string in the unit the sentence writes
    /// (`"24"`), because that is what every arm downstream compares, and this is what a
    /// reader actually sees. Error messages print this. Nothing compares it, which is the
    /// point — a message that said `24` about a sentence reading *twenty-four* would send
    /// an author looking for digits that are not there.
    phrase: String,
    /// The **unit noun** the words state, as [`UNIT_NOUNS`] spells it, or `""` for a number
    /// the sentence writes in digits.
    ///
    /// [`Written::scale`] cannot answer *"is this a length of time"*: a second, a point, a
    /// percent and a multiplier all scale by one. The two arms in
    /// [`accounting_without_arithmetic`] that compare **numbers** — the step length a claim
    /// was measured under, and the instant it was read at — are both in seconds, and they
    /// are offered every spelled quantity because a number needs no digits to be compared.
    ///
    /// Without this field they were offered quantities that are not durations at all.
    /// *"against the twin's half a percent"* is `0.5`, `nothing-to-clamp` holds its step
    /// length at `0.5` s, and check 6 tied a percentage to a duration and reported green —
    /// the same coincidence, one layer up, that the overlap skip in [`spelled_numbers`]
    /// exists for, and found the same way: by opening the scan on a lesson whose numbers
    /// happened to collide.
    ///
    /// It only ever **narrows**. A number written in digits carries its unit in the prose
    /// around it, has always been offered these arms, and is untouched.
    unit: &'static str,
}

/// Every number written in `text`, in order, with the offset each one starts at.
///
/// The digit scanner: [`numeric_tokens`] is this with the positions dropped. It is one of
/// two — [`spelled_numbers`] reads the quantities a sentence writes in English — and they
/// stay apart on purpose, so that a step can have its digits tied without anything claiming
/// its words are. [`ledger_numbers`] is where the two meet, and it is the only place they
/// do. Signs are not
/// collected, because both readers of this want a magnitude — a tolerance is symmetric,
/// and a ledger arm compares against a file constant whose sign is in the field name.
fn written_numbers(text: &str) -> Vec<Written> {
    let mut runs: Vec<Written> = Vec::new();
    let mut cur = String::new();
    let mut at = 0usize;
    for (i, c) in text.char_indices() {
        if c.is_ascii_digit() || (c == '.' && !cur.is_empty()) {
            if cur.is_empty() {
                at = i;
            }
            cur.push(c);
        } else if !cur.is_empty() {
            let token = std::mem::take(&mut cur);
            let len = token.len();
            runs.push(Written {
                at,
                phrase: token.clone(),
                token,
                len,
                scale: 1.0,
                unit: "",
            });
        }
    }
    if !cur.is_empty() {
        let len = cur.len();
        runs.push(Written {
            at,
            phrase: cur.clone(),
            token: cur,
            len,
            scale: 1.0,
            unit: "",
        });
    }
    // `at 5769.` and `1.2.3` both come out of the loop above; keep the leading number.
    let runs: Vec<Written> = runs
        .into_iter()
        .filter_map(|w| {
            let token = match w.token.match_indices('.').nth(1) {
                Some((i, _)) => w.token[..i].to_string(),
                None => w.token.trim_end_matches('.').to_string(),
            };
            // `len` follows the trimmed token, never the run it was cut from. The token is
            // a prefix of the source at `at` by construction, so this is still an extent
            // into the text and not a length in the abstract.
            (!token.is_empty()).then_some(Written {
                at: w.at,
                len: token.len(),
                phrase: token.clone(),
                token,
                scale: 1.0,
                unit: "",
            })
        })
        .collect();
    join_thousands(text, runs)
}

/// Join digit runs a **space is separating into thousands groups**: `11 880` is one number.
///
/// The path's prose writes large numbers this way and the scanner did not know it, so
/// `11 880` came out as `11` and `880` — two numbers, neither of which any claim could
/// spell and neither of which any accounting arm could tie to anything. The effect was
/// silent and it shaped seven slices of authoring: of the four such numbers in the lesson
/// prose (`10 000`, `11 280`, `11 880`, `200 000`), **not one appeared in any claimed
/// literal**. Sentences containing them could not be claimed, so they were not, and nothing
/// said why.
///
/// The rule is deliberately narrow, because the failure direction matters: joining two
/// numbers that are not one would make check 6 demand an accounting for a number the
/// sentence never printed, and no author could satisfy it. A group joins only when the
/// separator is exactly one ASCII space, the group is exactly three digits, and neither
/// side carries a decimal point — so `at 2 s, 464 s` (a space and a comma between) and
/// `0.5 s, 5 s` are untouched. Chains join left to right, which is what makes `200 000`
/// and a hypothetical `1 234 567` both come out whole.
///
/// The joined token **keeps its space**, so `spells` can still be "written exactly as the
/// sentence writes it". Everything that turns a token into a number goes through
/// [`number_of`], which strips it.
fn join_thousands(text: &str, runs: Vec<Written>) -> Vec<Written> {
    let mut out: Vec<Written> = Vec::with_capacity(runs.len());
    for w in runs {
        let joins = out.last().is_some_and(|prev: &Written| {
            // From the TRIMMED token's end. `len` agrees with it now and did not always:
            // it carried the untrimmed run, so on `at 5769. 880 s` it covered the full stop
            // — and the gap then measured from after it, landed on the space, and joined
            // `5769` to `880` into a figure the sentence never printed. Caught by the unit
            // test's own case, and only because the case was written; the suite was green
            // with it live. Reading the token is what kept this correct through that, and
            // it stays because the token is what this means.
            let gap = prev.at + prev.token.len();
            w.token.len() == 3
                && !w.token.contains('.')
                && !prev.token.contains('.')
                && gap < w.at
                && text.get(gap..w.at) == Some(" ")
        });
        if joins {
            let prev = out.last_mut().expect("checked above");
            prev.token.push(' ');
            prev.token.push_str(&w.token);
            prev.len = w.at + w.token.len() - prev.at;
            // The phrase follows the token here, because for a number in digits they are
            // the same string and a joined group is still digits: `11 880` is what the
            // sentence writes and what an error message should say.
            prev.phrase.clone_from(&prev.token);
        } else {
            out.push(w);
        }
    }
    out
}

/// A prose token as a number, with any thousands separators removed.
///
/// The one place a token becomes an `f64`. Every caller had its own `parse` before
/// [`join_thousands`] existed, and a second conversion site is how `11 880` could be one
/// number to the scanner and two to the accounting.
fn number_of(token: &str) -> Option<f64> {
    token.replace(' ', "").parse::<f64>().ok()
}

/// Every number written in `text`, as it is written — `["4146.5", "1.9290"]`.
///
/// Used to fence the `grid` variant, where the question is not what a number means but
/// how finely it is printed, and to scan a claimed sentence for the figures check 6 has
/// to account for. Neither parses the digits, so this deliberately keeps them as
/// characters.
fn numeric_tokens(text: &str) -> Vec<String> {
    written_numbers(text).into_iter().map(|w| w.token).collect()
}

/// The cardinal words a sentence may spell a whole number with.
///
/// Separate from [`WORD_NUMERALS`], which is a list of words an *author* names in a claim's
/// `spells` or a rule's operand — four entries, each required to be used. This is the
/// scanner's alphabet: it is consulted for every word of a word-ledgered step whether or not
/// anyone named it, so "every entry is used" is not a rule it could keep. Keeping them apart
/// is what stops this table quietly satisfying
/// [`every_word_numeral_is_read_by_something`], which would disarm that guard while looking
/// like coverage.
const CARDINALS: &[(&str, f64)] = &[
    ("zero", 0.0),
    ("one", 1.0),
    ("two", 2.0),
    ("three", 3.0),
    ("four", 4.0),
    ("five", 5.0),
    ("six", 6.0),
    ("seven", 7.0),
    ("eight", 8.0),
    ("nine", 9.0),
    ("ten", 10.0),
    ("eleven", 11.0),
    ("twelve", 12.0),
    ("thirteen", 13.0),
    ("fourteen", 14.0),
    ("fifteen", 15.0),
    ("sixteen", 16.0),
    ("seventeen", 17.0),
    ("eighteen", 18.0),
    ("nineteen", 19.0),
    ("twenty", 20.0),
    ("thirty", 30.0),
    ("forty", 40.0),
    ("fifty", 50.0),
    ("sixty", 60.0),
    ("seventy", 70.0),
    ("eighty", 80.0),
    ("ninety", 90.0),
];

/// The fraction words, which stand where a cardinal stands — *"half an hour"*.
///
/// `third` is deliberately absent. In this path's prose it is an ordinal every time it
/// appears (*"Third, hover the single tile"*), and it is the one fraction that has no exact
/// decimal, so a token formatted from it would carry seventeen places the sentence never
/// committed to.
const FRACTIONS: &[(&str, f64)] = &[("half", 0.5), ("quarter", 0.25)];

/// Words that scale the numeral before it reaches its unit — *"three **thousandths** of a
/// point"*.
///
/// These are not units and reading them as units is a defect with a direction: it makes
/// *"three thousandths of a point"* scan as the number 3, which is 1000 times the quantity
/// the sentence states. It then fails red against anything that knows the real figure and
/// **green** against anything that happens to equal 3, which is the wrong-arm hazard the
/// whole taxonomy is arranged against.
const SCALE_WORDS: &[(&str, f64)] = &[
    ("tenth", 0.1),
    ("tenths", 0.1),
    ("hundredth", 0.01),
    ("hundredths", 0.01),
    ("thousandth", 0.001),
    ("thousandths", 0.001),
];

/// The unit nouns v1 admits, and what one of each is in the unit a tie reads.
///
/// **Measures only, and the exclusion is deliberate rather than an oversight.** The target
/// step also counts its own furniture — *"two numbers"*, *"two electrodes"*, *"Four
/// footnotes"*, *"six gap figures"*, *"three decimals"* — and those are a different
/// problem: they are the prose stating the length of one of its own lists, which needs an
/// arm that reads a list rather than a file, and which this repo has recorded as its own
/// open item since `docs/plans/path-ledger-the-gradient.md`. Admitting them here would
/// swallow that project inside this one and answer it with whatever arm happened to equal
/// the count.
///
/// So the scanner does not see them, and that is said out loud in the module header rather
/// than left to be inferred from a green run — the same way the digit scanner's blindness to
/// words has been declared since it was built. A limit that is written down is not a waiver:
/// nothing here claims those numbers are tied to anything.
const UNIT_NOUNS: &[(&str, f64)] = &[
    ("second", 1.0),
    ("seconds", 1.0),
    ("minute", 60.0),
    ("minutes", 60.0),
    ("hour", 3600.0),
    ("hours", 3600.0),
    ("day", 86400.0),
    ("days", 86400.0),
    // A point of state of charge — a hundredth, and the unit the surface-gap row prints in.
    // The conversion to the fraction the engine holds is the reading rule's `pow10`, exactly
    // as it is for a number in digits; it is not this scale, which is about the *word's*
    // unit and not about the file's.
    ("point", 1.0),
    ("points", 1.0),
    // The multiplier — "more than six times the negative's". Unitless, so it scales by one.
    ("time", 1.0),
    ("times", 1.0),
    // A percentage, spelled. The word and not the sign: `%` is a digit's unit and the digit
    // scanner has always found it. This is here because *"against the twin's half a
    // percent"* was unread, and unread is the direction that matters — the sentence quotes
    // a neighbouring lesson's measurement and nothing was asked to check it. The
    // fraction-to-percent conversion is a rule's `pow10`, exactly as it is for a number in
    // digits; this scale is about the *word's* unit and there is one percent to a percent.
    ("percent", 1.0),
    ("percentage", 1.0),
];

/// Words allowed to sit between the numeral and its unit without breaking the phrase.
///
/// Closed on purpose. Anything not in here ends the phrase, so the scanner refuses
/// *"three points of the eight seconds"* rather than reading across a clause and tying a
/// numeral to a noun from the next idea along.
const FILLERS: &[&str] = &[
    "of",
    "a",
    "an",
    "the",
    "simulated",
    "whole",
    "further",
    "other",
    "such",
    "new",
    "single",
    "real",
    "full",
    "gap",
];

/// One word of `text`, with the byte range it covers.
fn words_of(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        let wordy = c.is_ascii_alphanumeric() || c == '-';
        match (wordy, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i, text[s..i].to_lowercase()));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, text.len(), text[s..].to_lowercase()));
    }
    out
}

/// A cardinal, a fraction, or a hyphenated compound of two cardinals — `"twenty-four"`.
fn numeral_word(word: &str) -> Option<f64> {
    let look = |w: &str| {
        CARDINALS
            .iter()
            .chain(FRACTIONS.iter())
            .find(|(k, _)| *k == w)
            .map(|(_, v)| *v)
    };
    if let Some(v) = look(word) {
        return Some(v);
    }
    let (a, b) = word.split_once('-')?;
    match (look(a), look(b)) {
        // "twenty-four" and never "half-hour": a compound adds two cardinals, and a
        // fraction is not one. `numeral_word` is asked about the whole hyphenated word by
        // the caller, so "eighteen-minute" reaches here too — its tail is a unit, not a
        // numeral, and it correctly finds nothing. The caller splits that case itself.
        (Some(x), Some(y)) if x >= 20.0 && y < 10.0 => Some(x + y),
        _ => None,
    }
}

fn lookup(table: &[(&str, f64)], word: &str) -> Option<f64> {
    table.iter().find(|(k, _)| *k == word).map(|(_, v)| *v)
}

/// The [`UNIT_NOUNS`] entry `word` is, **noun and scale together**.
///
/// [`lookup`] answers with the scale alone, which is all three shapes needed until
/// [`Written::unit`] existed. They are different questions: the scale says how to convert
/// the number, the noun says what kind of quantity it is, and a second and a percent have
/// the same scale.
fn unit_noun(word: &str) -> Option<(&'static str, f64)> {
    UNIT_NOUNS.iter().find(|(k, _)| *k == word).copied()
}

/// Whether a spelled quantity's unit is a **length of time**.
///
/// The gate on the two arms of [`accounting_without_arithmetic`] that compare numbers, both
/// of which are in seconds. Empty means the sentence wrote digits, which those arms have
/// always been asked about and which this does not change.
fn unit_is_time(unit: &str) -> bool {
    matches!(
        unit,
        "" | "second" | "seconds" | "minute" | "minutes" | "hour" | "hours" | "day" | "days"
    )
}

/// The numeral, in the unit the sentence writes it, as the shortest decimal string that
/// says what the words commit to.
///
/// The string is what every arm downstream reads, and its **decimal places are the
/// precision** — `"0.003"` for three thousandths is a claim to the thousandth, `"24"` for
/// twenty-four minutes a claim to the minute. So it is formatted from the value rather than
/// carried as text: a word commits to no more places than it spells, and `{}` on an `f64`
/// prints exactly the places the arithmetic produced.
fn spelled_token(value: f64) -> String {
    let s = format!("{value}");
    // `1e-3` is what Rust prints for 0.001, and a token with an exponent in it has no
    // decimal places to read. Nothing this scanner can build reaches that magnitude — the
    // smallest is a thousandth of a unit — but the scanner is what would go quiet if one
    // did, so it says so instead.
    assert!(
        !s.contains('e'),
        "a spelled quantity came to {value}, which formats as `{s}` — an exponent, whose \
         decimal places are not the precision the words commit to. Every arm downstream \
         reads that string as a number and its places as a tolerance, so this has to be a \
         plain decimal or the scan is measuring something else."
    );
    s
}

/// Every quantity `text` spells in **English**, in order, as [`Written`]s the ledger reads
/// beside the ones written in digits.
///
/// The ledger has scanned digits since it was built, and the module header has declared the
/// blindness this closes for as long: a sentence writing *"three thousandths of a point"* or
/// *"twenty-four minutes"* stated a measurement no check in this repo could see. A green
/// ledger said a step's digits were tied. On the step this was built for, nine such
/// quantities were invisible, and **five of them name the instants at which that step's own
/// claims are read** — so the digits were tied to the engine while the words saying *when*
/// each one was measured were tied to nothing.
///
/// Four shapes, and only the first is the obvious one:
///
/// 1. **numeral, unit** — *"three minutes"*, *"fifty simulated seconds"*, *"half an hour"*,
///    and the hyphenated attributive *"an eighteen-minute discharge"*. A scale word may sit
///    between them (*"three thousandths of a point"*).
/// 2. **unit, "and a half"** — *"the first minute and a half"*, which is 90 s and has the
///    numeral *after* its unit. Its own shape because English puts it there.
/// 3. **list ellipsis** — *"5.71 at three minutes, 5.80 at six, and …"*. The unit is stated
///    once and carried by every item after it. This is the shape that matters most and the
///    one a naive scanner misses entirely: on the first step scanned, four of the six
///    instants its gap claims are read at are written this way, so a scan without it would
///    have certified the step while seeing one instant in six.
/// 4. **numeral, "and a half", unit** — *"four and a half seconds"*, *"two and a half
///    days"*. Shape 2 the other way round, and the two are one quantity written in the two
///    orders English allows, so reading one and not the other is reading a word order rather
///    than a shape. Six phrases across five lessons, and until it existed the closed-list
///    guard below refused every one of them by name — which is the direction that shape was
///    supposed to fail in, and did.
///
/// A numeral this cannot value **panics** rather than being skipped, through
/// [`no_spelled_quantity_is_silently_skipped`]. A scanner that quietly passes over what it
/// cannot parse reports a green that means "I found nothing I understood", which is the
/// five-green-harness shape `docs/plans/surface-vs-bulk.md` cost this repo a slice to learn.
/// The numbers a ledgered step's prose prints - digits always, and English quantities where
/// the step is in the ledger's `spelled` list.
///
/// One function, read by the scan and by the rule-usage guard alike, on the terms
/// [`claimed_accounting`] reuses [`accounting_for`]: two scans of the same prose that could
/// answer differently is the defect this whole file is arranged against. It was that shape
/// for exactly one compile - the rule written for *"left to rest for half an hour"* covered
/// its number in the ledger and matched nothing in the guard, which then reported a live
/// rule as dead.
fn ledger_numbers(text: &str, step: &str, ledger: &Ledger) -> Vec<Written> {
    let mut numbers = written_numbers(text);
    if ledger.spelled.iter().any(|s| s == step) {
        numbers.extend(spelled_numbers(text));
        numbers.sort_by_key(|w| w.at);
    }
    numbers
}

fn spelled_numbers(text: &str) -> Vec<Written> {
    let words = words_of(text);
    let mut out: Vec<Written> = Vec::new();

    for (i, (at, end, word)) in words.iter().enumerate() {
        // A word already inside a phrase an earlier shape read whole is not the head of a
        // second one.
        //
        // **This is not tidiness, and it was measured.** Before shape 4 existed, *"40.33 A
        // four and a half seconds later"* was read by shape 1 as the two words `half
        // seconds` — 0.5 s — and `nothing-to-clamp` holds its step length at exactly 0.5 s,
        // so the ledger tied that quantity to the step-length control and reported GREEN on
        // a sentence stating 4.5. A wrong arm that happens to hold the right number is the
        // hazard the whole taxonomy is arranged against, and here the scanner was handing it
        // over. Shape 4 reads the phrase whole; this skip is what stops its own tail being
        // read a second time behind it.
        if out.iter().any(|w| *at > w.at && *at < w.at + w.len) {
            continue;
        }
        // Shape 1, hyphenated: "eighteen-minute". The unit is fused onto the numeral, so
        // neither half is a word of its own to walk from.
        if let Some((head, tail)) = word.split_once('-') {
            if let (Some(v), Some((unit, scale))) = (numeral_word(head), unit_noun(tail)) {
                out.push(Written {
                    at: *at,
                    len: end - at,
                    token: spelled_token(v),
                    phrase: text[*at..*end].to_string(),
                    scale,
                    unit,
                });
                continue;
            }
        }
        // Shape 2: "<unit> and a <fraction>", where the numeral follows its unit.
        //
        // Any fraction and not only a half, which is what it read when it was written. A
        // perturbation of *"the first minute and a half"* into *"a minute and a quarter"*
        // went GREEN: the scanner saw no quantity there at all, so nothing was asked to
        // account for one, and the sentence was free to say 75 s where the claim beside it
        // is read at 90. Narrow-by-accident is the shape this file's own guards keep
        // finding, and it fails toward silence every time.
        if let Some((unit, scale)) = unit_noun(word) {
            let tail: Vec<&str> = words[i + 1..]
                .iter()
                .take(3)
                .map(|(_, _, w)| w.as_str())
                .collect();
            if let (["and", "a", frac], Some(f)) =
                (&tail[..], tail.get(2).and_then(|w| lookup(FRACTIONS, w)))
            {
                let _ = frac;
                let stop = words[i + 3].1;
                out.push(Written {
                    at: *at,
                    len: stop - at,
                    token: spelled_token(1.0 + f),
                    phrase: text[*at..stop].to_string(),
                    scale,
                    unit,
                });
                continue;
            }
        }
        // Shape 4: "<numeral> and a <fraction> <unit>" — the numeral first, its fraction
        // next, and the unit behind both.
        //
        // Shape 2's mirror image, and the one [`no_spelled_quantity_is_silently_skipped`]
        // was written pointing at: *"four and a half seconds"*, *"two and a half days"*,
        // *"eight and a half seconds"*. English writes the same quantity both ways round and
        // the position of the numeral is the only difference, so a scanner reading one and
        // not the other is not reading a shape — it is reading a word order.
        //
        // It has to be tried BEFORE shape 1, because shape 1 can match this phrase's tail on
        // its own and come out with the fraction alone. See the skip at the top of the loop
        // for what that cost.
        if let Some(v) = numeral_word(word) {
            let tail: Vec<&str> = words[i + 1..]
                .iter()
                .take(3)
                .map(|(_, _, w)| w.as_str())
                .collect();
            if let (["and", "a", _], Some(f)) =
                (&tail[..], tail.get(2).and_then(|w| lookup(FRACTIONS, w)))
            {
                let mut j = i + 4;
                while words
                    .get(j)
                    .is_some_and(|(_, _, w)| FILLERS.contains(&w.as_str()))
                {
                    j += 1;
                }
                if let Some((_, stop, word)) = words.get(j) {
                    if let Some((unit, scale)) = unit_noun(word) {
                        out.push(Written {
                            at: *at,
                            len: stop - at,
                            token: spelled_token(v + f),
                            phrase: text[*at..*stop].to_string(),
                            scale,
                            unit,
                        });
                        continue;
                    }
                }
            }
        }
        // Shape 1: a numeral, an optional scale word, fillers, then the unit.
        let Some(mut value) = numeral_word(word) else {
            continue;
        };
        let mut j = i + 1;
        if let Some(s) = words.get(j).and_then(|(_, _, w)| lookup(SCALE_WORDS, w)) {
            value *= s;
            j += 1;
        }
        while words
            .get(j)
            .is_some_and(|(_, _, w)| FILLERS.contains(&w.as_str()))
        {
            j += 1;
        }
        let Some((_, stop, noun)) = words.get(j) else {
            continue;
        };
        let Some((unit, scale)) = unit_noun(noun) else {
            continue;
        };
        out.push(Written {
            at: *at,
            len: stop - at,
            token: spelled_token(value),
            phrase: text[*at..*stop].to_string(),
            scale,
            unit,
        });
    }

    // Shape 3, last, because it reads what the shapes above found: an item of a list whose
    // unit was stated by an earlier item. Restricted to `at <numeral>` closing on a comma,
    // a full stop or an "and" — the shape of a list item and not of a phrase — and to a
    // unit carried from the same sentence, so nothing inherits across an idea.
    let mut carried: Vec<Written> = Vec::new();
    for (i, (at, end, word)) in words.iter().enumerate() {
        if word != "at" {
            continue;
        }
        let Some((n_at, n_end, n_word)) = words.get(i + 1) else {
            continue;
        };
        let Some(value) = numeral_word(n_word) else {
            continue;
        };
        // Already the head of a phrase shape 1 read whole — "at three minutes".
        if out.iter().any(|w| w.at == *n_at) {
            continue;
        }
        let closes = text[*n_end..].starts_with(',')
            || text[*n_end..].starts_with('.')
            || text[*n_end..].starts_with(" and");
        if !closes {
            continue;
        }
        let (from, _) = sentence_span(text, *at);
        let Some(prior) = out.iter().rfind(|w| w.at >= from && w.at + w.len <= *at) else {
            continue;
        };
        carried.push(Written {
            at: *at,
            len: n_end - at,
            token: spelled_token(value),
            phrase: text[*at..*n_end].to_string(),
            scale: prior.scale,
            // The unit is inherited with the scale, because that is the whole shape: the
            // item states a numeral and nothing else, and what it means comes from the
            // item ahead of it.
            unit: prior.unit,
        });
        let _ = end;
    }
    out.extend(carried);
    out.sort_by_key(|w| w.at);
    out
}

/// Two tolerances agree.
///
/// Relative rather than `==` because the rule is computed and the file's is parsed:
/// `5.0 * 10f64.powi(-5)` and TOML's `5.0e-5` are the same number to fourteen places and
/// need not be the same bits. The window is far tighter than any authoring mistake and
/// far looser than the arithmetic.
fn tol_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

/// The prose types a typographic minus (U+2212); every formatter on the page emits the
/// ASCII one, because that is what `toFixed` returns.
///
/// This is the one difference the `quoted` check forgives, and it is forgiven in exactly
/// one direction: the comparison is done with both sides normalised, so a sentence may
/// spell `−0.069 V` and still be quoting a row that prints `-0.069 V`. Nothing else is
/// normalised — a digit, a unit, a space or a decimal place that differs is a difference
/// the reader would see.
fn ascii_minus(s: &str) -> String {
    s.replace('\u{2212}', "-")
}

/// Where an arm's pack comes from — the two things a reader can be holding when they
/// start pressing buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Start {
    /// The step's own pack, run to its mark and then continued. `pathArrived` sets
    /// `path.until = null` and pauses without rebuilding anything, so this is literally
    /// what is in front of a reader when the run stops on its own.
    ///
    /// Step 18 reaches it a second way and the prose says so — "press **Back** then
    /// **Next** to put the pack back at the mark" re-applies the whole step and runs it
    /// again, which lands on this same trajectory because nothing before the mark was
    /// touched. Each arm drives its own copy, so two arms branching off one mark cannot
    /// see each other's buttons.
    Mark,
    /// A fresh pack at t = 0 — the page's **Restart**, which rebuilds from the same
    /// scenario file, fault queue and all, *without re-applying the step's controls*.
    ///
    /// That second half is the whole of step 18's instruction rather than a detail:
    /// Back-then-Next would put `dt` back to 0.5, and Restart holds the 5 the reader just
    /// typed. So an arm's `dt` and `bms` overrides survive here, and `SimEngine::restart`
    /// is the same shape — it rebuilds the pack and then `$("reset").onclick` pushes the
    /// ambient slider back through `applyEnv`.
    Restart,
}

/// One thing the reader does on an arm, in order.
///
/// Every variant is a control on the page rather than an operation on the engine, and the
/// names are the button captions. What separates them is the one thing step 18 is about:
/// **whether the pack advances**. `ClearQueued` and `ClearLatched` change the pack without
/// stepping it; `Step1` and `Run` step it. "The move you cannot take back is the Run" is a
/// statement about exactly this list.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
enum Action {
    /// **Clear queued** — `Pack::clear_faults`. Despite the caption it removes faults that
    /// have already fired as well as queued ones, which is why step 18 calls it a repair
    /// and why its note counts the short it took out.
    ClearQueued,
    /// **Clear latched BMS fault** — `Pack::clear_bms_fault`. Unlatches and nothing else;
    /// the contactor closes again only if the pack is fit for it on the next step.
    ClearLatched,
    /// **Step 1** — exactly one engine step under this arm's standing demand.
    ///
    /// Spelled `step_1` in the file rather than taking the `snake_case` rule's `step1`,
    /// so it reads as the button's caption like every other action here.
    #[serde(rename = "step_1")]
    Step1,
    /// **Run** until the pack's own clock reaches `to_s` \[s\].
    ///
    /// **`to_s` is a choice, not a measurement**, and it is the weak joint in the whole
    /// mechanism: past a step's mark the page stops for nothing, so nothing outside this
    /// file bounds how far a run goes. Set it from what the step's prose asks the reader
    /// to watch for and say so in the arm's `note`; the module docs record what it costs
    /// the reachability check.
    Run { to_s: f64 },
}

/// An instructed control change: what the reader is told to do next, and the trajectory
/// that follows from doing it.
///
/// This began as `[[leg]]`, which could express exactly one thing — a second demand typed
/// into the box at the mark. That covered steps 20 and 21, where both of the defects the
/// display check exists for lived, and covered nothing else. Four steps' worth of prose
/// instructs a reader to uncheck the BMS, put `dt` up to 5 and press **Restart**, clear a
/// latched fault, clear the fault queue, press **Step 1**, or simply press **Run** again —
/// and every number that follows those sentences was unclaimable while a leg was all there
/// was. `docs/plans/path-prose-ledger.md` named this as one of the two capabilities the
/// harness was missing; the other was the zero-length probe.
///
/// Declared here rather than in `web/app.js` because the page does not do any of it: the
/// reader does. A `then:` field in the lesson block would be a field the page never reads,
/// sitting where every other field is one the page acts on.
///
/// **The tie back to the page is [`every_arm_is_instructed_by_its_own_step`], and it is
/// stricter than the leg's was.** A leg only had to cite a sentence in the step's prose and
/// spell its current inside it. An arm carries three controls, so *each* one has to be
/// anchored in the sentence it claims to be following: the current spelled, the timestep
/// spelled, the BMS named — and every override has to be a genuine change from what the
/// step configures. Without that an arm whose instruction says "uncheck the BMS" could
/// quietly also run at `dt = 5`, and its claims would be true of a trajectory nobody is
/// told to produce.
#[derive(Debug, serde::Deserialize)]
struct Arm {
    /// `id` of the lesson in `const LESSONS`.
    step: String,
    /// What a claim writes in its own `arm` field. Unique within the step.
    name: String,
    /// The lesson whose **pack** this arm runs, if it is not this step's own.
    ///
    /// Step 16's closing instruction is *"run both files again"*: it asks the reader to put
    /// one current into two different scenarios, and the second of them is the lesson next
    /// door. Before this field an arm could only ever rebuild the step it was declared on,
    /// so the sentence comparing the two models at 1 C had no trajectory behind it and its
    /// three numbers were deleted from the page. `docs/plans/path-ledger-dfn-step.md` named
    /// that as the gap this closes.
    ///
    /// **What it models is navigation, not a control.** Every other override here is a box
    /// or a checkbox; this one is the reader pressing **Back**, landing on another lesson —
    /// which rebuilds from *that* lesson's scenario under *that* lesson's controls, because
    /// `applyStep` reloads and re-dials — and then typing this arm's current and pressing
    /// **Restart**. So the named lesson supplies the scenario, the timestep, the ambient and
    /// the BMS, and this arm supplies what the reader types on top.
    ///
    /// Three fences, in [`assert_walkable`]: it may not name its own step, the lesson it
    /// names must be on a *different* scenario file, and it implies [`Start::Restart`]. Not
    /// one of them is reachable from the claims file, so each has a `should_panic` test of
    /// its own rather than a paragraph. The instruction still has to be in **this** step's
    /// prose, which is the whole point — the sentence that sends a reader next door is
    /// written here.
    ///
    /// **Half of what this resolves is checked by its own user and half is not.** The
    /// *scenario* half is: the twin arm reads a single-particle file where its step reads a
    /// porous-electrode one, so an implementation that built this step's pack would land on
    /// 3484 s where the claim says 3496 and fail loudly. The *block* half — timestep,
    /// ambient, BMS — is not, because the two lessons agree on all three. It is written the
    /// way the page behaves rather than the way a test could tell, the same position
    /// [`Tie::Elsewhere`] states for its own named-lesson read.
    #[serde(default)]
    pack_from: Option<String>,
    /// The scenario file this arm **loads from the picker**, if it is not the one the step
    /// is standing on.
    ///
    /// [`Arm::pack_from`] models a walk to another *lesson*; this models the picker itself.
    /// From the reader's side the two are one gesture — `loadScenario` rebuilds from the
    /// named file at t = 0 and leaves every control alone, which is the reading
    /// [`assert_walkable`]'s fourth fence already settled for a walk. They are two fields
    /// because a scenario the path never teaches has no lesson to name, and step 2's third
    /// cell is exactly that: `cc_discharge_lgm50.toml` is in the `<option>` list of
    /// `web/index.html` and in no lesson block. Before this field the sentence that sends a
    /// reader to it had no trajectory behind it, and every number after it — the cell's
    /// nameplate, the rate, both falls, the time one of them takes and the charge left at
    /// the mark — was tied to nothing at all.
    ///
    /// Five fences, in [`assert_picker`]: the file must differ from the step's own, it must
    /// be one the picker actually offers, its name must be spelled in the instruction, it
    /// implies [`Start::Restart`] because loading a file *is* a rebuild, and it may not be
    /// combined with `pack_from` — which would be two navigations under one sentence asking
    /// for either.
    ///
    /// **What it changed for a tie.** [`Tie::OnArm`] used to refuse a file field outright,
    /// on the grounds that "an arm overrides controls, not files: asking a scenario field
    /// under an arm's name would resolve to the same number and claim it came from somewhere
    /// else". That argument is exactly right for every arm that does not carry this field,
    /// and false for one that does — the picked file really is the arm's. Step 2's third
    /// cell prints its nameplate (`5.153198 Ah`), the provenance of its curve (`Chen2020`)
    /// and its own name (`cc_discharge_lgm50`), and none of those could be tied to anything
    /// while the refusal was flat. The extension waited for the slice that ledgers step 2
    /// because a vocabulary rule is only counted against a *ledgered* step
    /// ([`every_ledger_rule_is_a_phrase_and_is_used`]), so it would have landed with no user
    /// at all. The name is [`Tie::Picker`]'s job and the two fields are the file half of
    /// `OnArm`; see `docs/plans/path-ledger-third-cell-step.md`.
    #[serde(default)]
    scenario: Option<String>,
    /// The sentence in that step's prose that tells the reader to make this change,
    /// verbatim. Not a paraphrase: it is checked as a substring, like a claim's `literal`.
    instruction: String,
    /// Whether the reader is continuing the run or rebuilding it. See [`Start`].
    start: Start,
    /// The current the reader types into the demand box \[A\], discharge-positive, if this
    /// arm changes it. Must be spelled inside `instruction`.
    #[serde(default)]
    demand_a: Option<f64>,
    /// The **charge current** the reader types into the CC-CV group \[A\], if this arm
    /// changes it. Positive charges, which is the opposite sign convention to `demand_a`
    /// and is the page's, not this file's.
    ///
    /// A second field rather than a mode-aware `demand_a`, because the page has two boxes
    /// and shows one at a time. `applyDemandMode`'s own comment says why: *"The single
    /// `value` box cannot serve CC-CV: the mode needs three numbers, and one of them is
    /// entered with the opposite sign convention to everything else on this page."* Typing
    /// into the simple box on a CC-CV step is not a thing a reader can do — the box is
    /// hidden — so an arm that set `demand_a` there would be describing a trajectory the
    /// page cannot reach, and `demand_a`'s own doc already says that typing into it
    /// *replaces the program*. Keeping them separate is what stops step 11's "ask for 6 A"
    /// silently becoming a 6 A discharge.
    ///
    /// Same contract as `demand_a` otherwise: must be spelled inside `instruction`, must
    /// differ from the step's own, and only the two of them together decide
    /// [`Control::DemandValue`] on an arm.
    #[serde(default)]
    cc_cv_a: Option<f64>,
    /// The **pulse current** the reader types into the `Pulse` group \[A\],
    /// discharge-positive, if this arm changes it.
    ///
    /// The third current box, and it is a third field for exactly the reason `cc_cv_a` is a
    /// second one: `applyDemandMode` hides `demand-simple` on a `Pulse` step just as it does
    /// on a CC-CV one, because the mode needs three numbers of its own. So an arm that typed
    /// into the simple box here would be describing a trajectory the page cannot reach — and
    /// worse than unreachable, [`arm_prog`] reads that box as *replacing the program*, so the
    /// arm would end the pulse train and run a flat discharge under a sentence about teeth.
    ///
    /// Step 14 is what needed it. Its closing instruction sends the reader to the circuit
    /// with the same train, and every number that follows is per-tooth. Without this field
    /// the sentence had no trajectory behind it and its five figures were tied to nothing.
    ///
    /// Same contract as its two siblings: it must be spelled inside `instruction`, it may not
    /// be combined with either of the other boxes, the lesson its pack comes from must
    /// actually be on the `Pulse` mode, and it must differ from that lesson's own current.
    /// **The legs are not overridable and deliberately so** — `on_s` and `off_s` stay the
    /// pack lesson's, which is what [`arm_control_value`] already says of them.
    #[serde(default)]
    pulse_a: Option<f64>,
    /// The step length the reader types into the `dt` box \[s\], if this arm changes it.
    /// Must be spelled inside `instruction`, and must differ from the step's own.
    #[serde(default)]
    dt: Option<f64>,
    /// The BMS checkbox, if this arm changes it. `instruction` must name the BMS, and the
    /// value must differ from what the step configures — an override that changes nothing
    /// is a declaration with no fact under it.
    #[serde(default)]
    bms: Option<bool>,
    /// The chemistry's `[reversal] fade_per_ah` this arm's pack is **built with**, if it is
    /// not the file's own \[capacity fraction per Ah past empty\].
    ///
    /// **The one arm in this file no reader can walk, and it says so rather than pretending
    /// otherwise.** Every other override here is a box, a checkbox or a slider — something on
    /// the page. This one edits the parsed chemistry between `parse_chemistry` and
    /// `Pack::new`, which is a thing an author does and a reader cannot. It exists because
    /// step 21's whole argument is an attribution, and an attribution needs a control:
    ///
    /// > Measured rather than assumed: the identical run with the over-discharge coefficient
    /// > set to zero and nothing else touched ends the same ten minutes at **99.96 %**.
    ///
    /// Without it the step's headline — 4.80 of the 4.82 points are the reversal — is an
    /// inference, and the two numbers that carry it are tied to nothing. With it the
    /// subtraction is between two measured trajectories and the ledger can check both ends.
    /// `docs/plans/reversal-damage-ui.md` measured this control out of tree when the step was
    /// written; this field is that measurement moved into the suite.
    ///
    /// **`instruction` is read as the sentence that REPORTS the change, not one that asks for
    /// it.** The substring check is unchanged — the sentence still has to be in the step's own
    /// prose, so a reword still reddens — but a reader is being told what was measured rather
    /// than told to go and measure it. That is the honest reading of the sentence above, and
    /// the distinction is worth keeping visible: an arm nobody can walk may only ever assert
    /// what the prose *claims about* a counterfactual.
    ///
    /// Four fences, all in [`every_arm_is_instructed_by_its_own_step`]: the value must be
    /// spelled in the instruction (in digits or in letters, through [`WORD_NUMERALS`] — this
    /// sentence writes "zero"), it must differ from what the chemistry file already says, it
    /// implies [`Start::Restart`] because a pack cannot be rebuilt halfway through its own
    /// run, and it may not be combined with `pack_from`, which would be two exotic overrides
    /// composed with no sentence asking for either.
    #[serde(default)]
    fade_per_ah: Option<f64>,
    /// The ambient slider \[°C\], if this arm drags it. Must be spelled inside
    /// `instruction`, and must differ from the step's own.
    ///
    /// Celsius, because that is what the slider, the lesson block and the prose all speak;
    /// it becomes kelvin at the one place it reaches [`Env`], like every other temperature
    /// this file reads out of the page.
    ///
    /// **Draggable at the mark, which it was not always.** This said "Restart only, and that
    /// is a scoping refusal rather than a fidelity one" for as long as [`run`] kept one
    /// environment for a whole trajectory. `run` keeps two now, split at the mark, and the
    /// fence came down with the slice that built the split — so the paragraph outlived the
    /// rule it described, and step 8's own continuation arm had been contradicting it since.
    /// The comment in [`every_arm_is_instructed_by_its_own_step`] where the assertion used to
    /// be records the same history from the other side.
    #[serde(default)]
    ambient_c: Option<f64>,
    /// The buttons, in the order the reader presses them. Never empty: an arm that does
    /// nothing is a second copy of the step wearing a name.
    actions: Vec<Action>,
    /// Another arm on the same step whose **end state this one must equal, bit for bit**.
    ///
    /// The one thing in step 18's prose that is not a number: "press those same two
    /// buttons in the other order, still without running, and you get an identical pack".
    /// A claim states a quantity, and no quantity says that — so it is asserted here, over
    /// the serialised snapshot, by [`every_identical_arm_really_is_identical`].
    #[serde(default)]
    identical_to: Option<String>,
    #[allow(
        dead_code,
        reason = "authoring context for a human reader, not asserted"
    )]
    note: String,
}

impl Arm {
    /// A control value as the prose would spell it — `-2`, not `-2.0`; `5`, not `5.0`.
    fn spelled(v: f64) -> String {
        if v.fract() == 0.0 {
            format!("{v:.0}")
        } else {
            format!("{v}")
        }
    }

    /// The simulation time this arm's trajectory ends at \[s\], without running anything.
    ///
    /// Pure arithmetic over the actions, so [`every_claim_is_reachable_in_its_own_step`]
    /// stays engine-free — a prose defect should not have to fail from behind a long run.
    fn end_s(&self, lesson: &Lesson) -> f64 {
        let dt = self.dt.unwrap_or(lesson.dt);
        let mut clock = match self.start {
            Start::Mark => lesson.until_s,
            Start::Restart => 0.0,
        };
        for action in &self.actions {
            match action {
                Action::Step1 => clock += dt,
                Action::Run { to_s } => clock = clock.max(*to_s),
                Action::ClearQueued | Action::ClearLatched => {}
            }
        }
        clock
    }

    /// The earliest time a claim on this arm may read at \[s\].
    ///
    /// On a continuation that is the mark — a claim reading at or before it is measured on
    /// the trajectory every other claim on the step already reads, not on the change this
    /// arm is about. On a restart the arm has its own timeline from zero.
    fn earliest_s(&self, lesson: &Lesson) -> f64 {
        match self.start {
            Start::Mark => lesson.until_s,
            Start::Restart => 0.0,
        }
    }
}

/// Does `text` contain `number` as a number, rather than as a run of characters inside a
/// different one?
///
/// The sign is why this is not `contains`. Every leg so far is a *reversal*, and a leg
/// written as `+2` where the prose says `−2` would find its "2" inside the prose's own
/// `-2` and pass — a leg run in the wrong direction, tied to a sentence that says the
/// other one. The leg's claims would then all be measured on that wrong trajectory. So a
/// match may not be flanked by anything that would make it part of another number: a
/// digit or a decimal point on either side, or a minus in front.
///
/// A number spelled in letters ([`WORD_NUMERALS`]) is bounded the same way and by the
/// same argument, one alphabet over: `three` inside `threefold` is not this number, so a
/// word match may not be flanked by a letter or a digit either.
fn contains_number(text: &str, number: &str) -> bool {
    let word = number.starts_with(|c: char| c.is_ascii_alphabetic());
    let flanker = move |c: char| {
        if word {
            c.is_alphanumeric()
        } else {
            c.is_ascii_digit() || c == '.'
        }
    };
    text.match_indices(number).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + number.len()..].chars().next();
        !before.is_some_and(|c| flanker(c) || (!word && c == '-')) && !after.is_some_and(flanker)
    })
}

#[derive(Debug, serde::Deserialize)]
struct Claims {
    claim: Vec<Claim>,
    #[serde(default)]
    arm: Vec<Arm>,
    /// The sentences that do arithmetic on their own numbers. See [`Derivation`].
    #[serde(default)]
    derived: Vec<Derivation>,
    /// Which steps have their whole prose scanned. Required, with no serde default: an
    /// absent `[ledger]` would read as "no step is ledgered and none needs listing", which
    /// is the state this contract exists to end.
    ledger: Ledger,
    /// The English-spelled quantities the digits rule has not reached yet. See
    /// [`no_lesson_spells_a_quantity_in_english`].
    ///
    /// `default` here is the safe direction, unlike `ledger`'s: an absent section means the
    /// ban allows nothing, so every phrase still in the prose fails by name. A missing list
    /// cannot read as permission.
    #[serde(default)]
    english: Vec<EnglishPhrase>,
}

/// One quantity a lesson still spells in English, and why it is still there.
///
/// A backlog entry, not a waiver, and the difference is enforced from both sides: the phrase
/// must actually appear in that step's prose, so an entry cannot outlive the sentence it
/// describes, and the list is the only thing standing between the prose and a red.
#[derive(Debug, serde::Deserialize)]
struct EnglishPhrase {
    /// The lesson's id, as `const LESSONS` writes it.
    step: String,
    /// The phrase exactly as [`english_quantities`] reports it.
    ///
    /// Two fields and no third. A `note` field was drafted here and taken out again: nothing
    /// could read it, and a declared string beside a derived fact is how a file comes to
    /// describe itself wrongly. What each phrase would take is written as a comment above
    /// its step's block, where it is prose for a person and cannot be mistaken for a check.
    phrase: String,
}

/// The quantities that can be read off **one row**, and are therefore the quantities a
/// zero-length probe can answer.
///
/// Split out from [`measure`] rather than duplicated into it: a probe is a row and has no
/// history, so this is precisely the boundary between "a claim may declare `probe`" and
/// "it may not". A quantity added here becomes probe-readable automatically; one added to
/// the reductions below stays refused, with a message naming itself.
fn measure_row(quantity: &str, row: &Row) -> Option<f64> {
    let t = &row.telemetry;
    Some(match quantity {
        "v_at_mark" | "v_at" => t.v_terminal,
        // The two ends of the `cell v` row, which is one string printing both. A group in
        // parallel has one node voltage and every cell in it sits at that voltage, so
        // these are per-*group* readings and a sentence naming "the weakest group" and one
        // naming "every cell" are asking for the same number. See `Pack::step`, where both
        // are folded from `v_g`.
        //
        // Separate quantities rather than a pair, on the same terms as the surface gap
        // below: a claim states one number, and step 7's sentence about a pack driven
        // backwards is about the *spread* between them.
        "v_cell_min_at" => t.v_cell_min,
        "v_cell_max_at" => t.v_cell_max,
        // The distance between those two ends, in millivolts — the unit the sentence that
        // needs it speaks ("spread over 11 mV", "the 130 mV gap you see *after* the trip").
        //
        // A separate quantity rather than two claims on the ends, and the paragraph above
        // is not an argument against it: that one says a claim states ONE number, which is
        // exactly what this is. Step 11's prose never prints either end — it prints the
        // distance and nothing else — so claiming `v_cell_min_at` and `v_cell_max_at`
        // there would be stating two numbers the sentence does not contain. This is the
        // same shape as `soc_gap_pts_at` and `t_gap_k_at` below: a gap the page shows and
        // never prints, with the reader doing the subtraction off the `cell v` row.
        //
        // No `display` may be named on a claim measured here, for `t_gap_k_at`'s narrower
        // reason: `cell v` IS in the [`render_row`] mirror, but it prints the two ends and
        // not their difference, so there is no asserted string a claim could quote.
        //
        // Millivolts because the sentence is: a pack whose groups sit 0.011 V apart is a
        // pack nobody would describe that way, and a claim that stated 0.011 could not be
        // checked against a prose that spells 11.
        "v_cell_spread_mv_at" => (t.v_cell_max - t.v_cell_min) * 1000.0,
        "soc_at" => t.soc_true,
        "i_at" => t.i_actual,
        "t_max_at" => t.t_max,
        "soh_cap_at" => t.soh_capacity,
        "soh_res_at" => t.soh_resistance,
        // Heat, and the charge the pack would not take. Both exist because the `heat`
        // and `clamp` rows are the two the overcharge step tells a reader to read, and a
        // display claim with no value behind it would be pinning a rendering of a number
        // nothing checks.
        "q_gen_at" => t.q_gen_w,
        "i_rejected_at" => t.i_rejected_a,
        // The `overpotential` figure and the two mechanisms under it, in millivolts —
        // the unit both the pack grid and the prose use, so a claim reads as the sentence
        // does. Three quantities and not one with a selector, on the same terms as the
        // surface gap: a claim states one number, and the whole subject of steps 23 and 24
        // is that these two do not move together.
        //
        // `rc` is the placeholder RC pair and `diffusion` is what is left — the fitted term
        // — so the pair is a decomposition of the first by construction rather than two
        // measurements that ought to add up. The RC half is `None` on every row a claim did
        // not ask for; see [`Row::rc_overpotential_v`].
        "overpotential_mv_at" => row.overpotential_v * 1000.0,
        "rc_overpotential_mv_at" => rc_half(row) * 1000.0,
        "diffusion_overpotential_mv_at" => (row.overpotential_v - rc_half(row)) * 1000.0,
        // The debt below empty, in points of charge — the units the prose and the `past
        // empty` row both speak, so a claim reads as the sentence does. Ground truth, and
        // value-only: that row is sampled on a wall clock, so no claim measured here may
        // name a display. See `Row::deficit_max`.
        "deficit_pts_at" => row.deficit_max * 100.0,
        // The shallow end of the same spread. No row prints it — see `Row::deficit_min` —
        // so a claim measured here may name no `display` either.
        "deficit_pts_min_at" => row.deficit_min * 100.0,
        // WHICH cell each end of that spread is, as the pack grid addresses it. Four
        // quantities and not two pairs, because a claim states one number and the prose
        // prints two: `(0,0)` is a series index beside a parallel one, and a claim that
        // carried both could not be spelled by either digit.
        //
        // Value-only, and no `display` may be named: the grid renders a tile per cell and
        // labels none of them with its address — the reader is told which tile they are
        // looking at by where it sits, which is not a string this file can mirror.
        "deficit_worst_cell_series_at" => row.deficit_max_cell.0 as f64,
        "deficit_worst_cell_parallel_at" => row.deficit_max_cell.1 as f64,
        "deficit_best_cell_series_at" => row.deficit_min_cell.0 as f64,
        "deficit_best_cell_parallel_at" => row.deficit_min_cell.1 as f64,
        // The two halves of the `surface gap` row, each in points of charge — the units
        // the row prints and the prose speaks. Separate quantities and not a pair, because
        // a claim states one number: step 17's whole argument is that these two do
        // *different things*, and a claim that averaged them would be about neither.
        //
        // A circuit panics rather than reading zero, for the reason the row prints its
        // reason instead of a number: "no electrodes" and "flat" are not the same fact,
        // and this file's own `isPorous` mirror exists to keep them apart.
        "surface_gap_neg_pts" | "surface_gap_pos_pts" => {
            let (neg, pos) = row.surface_gap.unwrap_or_else(|| {
                panic!(
                    "a claim reads `{quantity}` at t = {} s on a step whose cell model has \
                     no surface. The `surface gap` row there says `circuit — no \
                     electrodes` rather than printing a zero, so the quantity does not \
                     exist — the claim is on the wrong step.",
                    row.t_s
                )
            });
            100.0
                * if quantity.ends_with("neg_pts") {
                    neg
                } else {
                    pos
                }
        }
        // What the BMS believes minus what is true, in points of charge — the gap
        // CLAUDE.md's eighth principle exists to expose, and the subject of step 4's whole
        // closing paragraph.
        //
        // No `display` may be named on a claim measured here, and the reason is different
        // from `deficit_pts_at`'s: the panel has no gap row at all. It prints `soc (true)`
        // and `soc (bms)` a tenth of a point each and the reader does the subtraction, so
        // the gap is a quantity the page shows without ever printing. That is also what
        // sets the tolerance the two claims on it take — see their notes.
        //
        // The row's own time, not the `at_s` an author asked for: `row_at` returns the
        // nearest row, and a message naming the instant an author wanted rather than the
        // one that was read sends them to the wrong place.
        "soc_gap_pts_at" => gap_pts(t, row.t_s),
        // The temperature channel of that same panel: what the hottest **probe** reads
        // minus what the hottest **cell** is, in kelvin. Belief minus truth, the same
        // subtraction and the same order as `soc_gap_pts_at` one channel up, and the same
        // string the page builds — `fmtSigned(probeMax - t.t_max, 2, "K")`.
        //
        // Negative on a pack whose hot cell is not instrumented, which is the interesting
        // case and the only one this path has a sentence about: "protection is late by
        // 1.3 K of somebody else's temperature" prints the magnitude and puts the sign in
        // the word *late*, so a claim on it states `magnitude` — the variant that exists
        // for exactly this and is fenced to negative values.
        //
        // No `display` may be named on a claim measured here. The number is on the screen,
        // in the BMS panel's `temperature` row, but that panel is not in this file's
        // [`render_row`] mirror — `READOUTS` is — so there is no asserted string to quote.
        // That is a narrower reason than `soc_gap_pts_at`'s, whose panel prints no gap at
        // all, and it is the one that would change first if the mirror grew.
        "t_gap_k_at" => {
            let sensed = row.sensed.as_ref().unwrap_or_else(|| {
                panic!(
                    "a claim reads the BMS's temperature sensors at t = {} s on a step \
                     that runs with no BMS. Such a pack has no sensors to read — the \
                     page's own panel says so instead of printing a number — so the \
                     quantity does not exist there rather than being zero.",
                    row.t_s
                )
            });
            // The page's two gates on this comparison, in its own order, and neither is a
            // detail: `booted` first, then `lag`. A pack that has never advanced has never
            // *sampled*, so the frame it carries is the construction-time open-circuit
            // read — every probe at the initial temperature, on a pack that is uniform
            // anyway. Reading a gap off it would report an exact zero and call it a
            // measurement, which is the false-agreement half of the false-accusation
            // `web/app.js` documents at the same gate.
            assert!(
                row.t_s > 0.0,
                "a claim reads `t_gap_k_at` off the zero-length probe. A `dt = 0` step \
                 samples no sensors, so the frame there is the boot read the page labels \
                 `the sensors have not sampled yet` — every probe at the pack's initial \
                 temperature. The gap it would report is an artefact of construction, not \
                 a measurement. Give the claim an instant on the run."
            );
            // A FORWARD GUARD, AND MEASURED TO BE ONE. Deleting this assertion reddens
            // nothing in the file today: sampling is gated on `dt > 0`, so every *stepped*
            // row carries a frame sampled at its own instant, and the one row that does not
            // — the probe — is already refused above. It is kept, and labelled, because a
            // stale frame is the one way this quantity stops being a subtraction of two
            // readings of one instant, and the page carries the same comparison for the
            // same reason. Said plainly rather than left to be assumed, because an
            // unreachable assertion under a green test reads as a covering one.
            assert!(
                (sensed.sampled_at_s - row.t_s).abs() <= 1e-9,
                "the sensor frame at t = {} s was sampled at {} s — {} s behind. The page \
                 greys its own panel on exactly this comparison rather than showing the \
                 two side by side, so a claim measured here would be subtracting readings \
                 of two different instants.",
                row.t_s,
                sensed.sampled_at_s,
                row.t_s - sensed.sampled_at_s
            );
            let probe_max = sensed.probe_max_k.unwrap_or_else(|| {
                panic!(
                    "a claim reads `t_gap_k_at` at t = {} s on a BMS with no temperature \
                     probes. The panel prints `no probes` there rather than a temperature.",
                    row.t_s
                )
            });
            probe_max - t.t_max
        }
        // The **lowest group voltage the BMS measured** at this instant \[V\] — the channel
        // the hard under-voltage rung is judged on, and the one the contactor latches from.
        //
        // Belief and not truth, like `t_gap_k_at` above and unlike everything else here.
        // Its two fences are that one's, for that one's reasons: a zero-length probe samples
        // no sensors at all, and a frame sampled at some other instant is not a reading of
        // this one. What it does *not* carry is the truth-side subtraction — the sentences
        // this exists for quote the sensor's own number rather than a gap, and a gap here
        // would be zero on every pack in the path (see [`Sensed::v_group_min`]).
        //
        // No `display` may be named on a claim measured here, on `t_gap_k_at`'s terms: the
        // BMS panel prints it, [`render_row`] does not mirror that panel.
        "v_group_min_at" => {
            let sensed = row.sensed.as_ref().unwrap_or_else(|| {
                panic!(
                    "a claim reads the BMS's voltage sensors at t = {} s on a step that                      runs with no BMS. Such a pack has no sensors to read.",
                    row.t_s
                )
            });
            assert!(
                row.t_s > 0.0,
                "a claim reads `v_group_min_at` off the zero-length probe. A `dt = 0` step                  samples no sensors, so the frame there is the boot read rather than a                  measurement of anything the reader is watching."
            );
            assert!(
                (sensed.sampled_at_s - row.t_s).abs() <= 1e-9,
                "the sensor frame at t = {} s was sampled at {} s — {} s behind, so a claim                  measured here would be quoting a reading of some other instant.",
                row.t_s,
                sensed.sampled_at_s,
                row.t_s - sensed.sampled_at_s
            );
            sensed.v_group_min.unwrap_or_else(|| {
                panic!(
                    "a claim reads `v_group_min_at` at t = {} s on a BMS whose frame                      carries no group voltages at all.",
                    row.t_s
                )
            })
        }
        _ => return None,
    })
}

/// The quantities a claim may name, and how each is read off a run.
///
/// Kept as an explicit match rather than a registry so an unknown quantity is a
/// compile-time-shaped failure with a list in the message, not a silent skip.
/// One tooth of a pulse train, located from the program and read off the trajectory.
///
/// The three instants are arithmetic — `on_s` and `off_s` are the page's, and the leg a
/// pulse is on is a pure function of `sim_time_s`, which is the whole reason that demand
/// mode exists — and the five voltages are looked up, never interpolated. A tooth the run
/// does not reach panics rather than returning a partial one.
struct Tooth {
    /// When the current stops \[s\]: the last row of the loaded leg.
    stop_s: f64,
    /// When the rest ends \[s\]: the last row before the next leg starts.
    rest_end_s: f64,
    /// Open-circuit voltage at the instant the tooth begins \[V\].
    rest_at_start: f64,
    /// Terminal voltage at the bottom of the loaded leg \[V\].
    v_at_stop: f64,
    /// Open-circuit voltage at that same instant \[V\] — the zero-length read, and the
    /// boundary between the part that returns at once and the part that climbs back.
    rest_at_stop: f64,
    /// Terminal voltage at the end of the rest \[V\]. The current is already zero here, so
    /// this needs no zero-length read of its own.
    v_at_rest_end: f64,
}

impl Tooth {
    fn of(run: &Run, n: usize) -> Self {
        let Prog::Pulse { on_s, off_s, .. } = run.prog else {
            panic!(
                "a claim reads a pulse quantity on a step whose demand is not the page's \
                 `Pulse` program. There are no teeth to count."
            )
        };
        assert!(
            n >= 1,
            "teeth are counted from one; this claim asks for {n}"
        );
        let period = on_s + off_s;
        let start_s = (n - 1) as f64 * period;
        let stop_s = start_s + on_s;
        let rest_end_s = start_s + period;
        // The tooth's opening open-circuit voltage. For the first tooth there is no row
        // before it at all — a pulse train is already on its loaded leg at t = 0 — so the
        // only reading is the probe's, which is why [`run`] takes one there.
        let rest_at_start = if n == 1 {
            run.probe.rest_v.expect(
                "the probe row carries no zero-length rest read, so this is not a pulse step",
            )
        } else {
            run.rest_read_at(start_s)
        };
        Self {
            stop_s,
            rest_end_s,
            rest_at_start,
            v_at_stop: run.at(stop_s).v_terminal,
            rest_at_stop: run.rest_read_at(stop_s),
            v_at_rest_end: run.at(rest_end_s).v_terminal,
        }
    }
}

fn measure(quantity: &str, run: &Run, at_s: f64, probe: bool, mark_s: f64) -> f64 {
    if let Some(v) = measure_row(quantity, run.read(at_s, probe)) {
        return v;
    }
    // A row quantity may **name the instant it is read at** — `v_at:400` — which is what
    // makes two readings of one quantity on one step individually addressable. Nothing
    // measures differently for it: the tag is stripped and the same row is read.
    //
    // Why a name and not just `read_at_s`: `Tie::Quoted` borrows another step's
    // measurement by naming it, and a step that files eight voltages under `v_at` gives a
    // quoting sentence no way to say *which*. The fence there refuses the ambiguity rather
    // than letting file order settle it, so the tag is how a step opts out of being
    // unquotable. See [`Tie::Quoted`].
    //
    // **The tag is asserted, not decorative**, on the same terms as the pulse family's
    // `want()` above: it must be the instant the claim already declares, so it can never
    // become a second, disagreeing address for the same reading. That is also what keeps
    // it from being a free-form label — `v_at:first` does not parse and does not resolve.
    //
    // Only row quantities take one. `pulse_rebound_mv:1` counts teeth and
    // `t_at_v_below:2.5` names a threshold, and neither prefix is a row quantity, so
    // neither is reachable from here.
    if let Some((name, tag)) = quantity
        .split_once(':')
        .and_then(|(name, tag)| tag.parse::<f64>().ok().map(|tag| (name, tag)))
    {
        if let Some(v) = measure_row(name, run.read(at_s, probe)) {
            assert!(
                (tag - at_s).abs() < run.dt * 0.5,
                "a claim reads `{quantity}`, whose tag names t = {tag} s, and declares \
                 `read_at_s = {at_s}`. The tag is the instant the reading is taken at and \
                 not a label, so the two cannot disagree: one of them is pointing at a \
                 moment the claim is not about."
            );
            return v;
        }
    }
    // How much capacity the pack has lost **since this step's mark**, as a fraction.
    //
    // Not a value the pack has at any instant, which is why it is here rather than in
    // [`measure_row`]: it is the difference between two rows of one trajectory. Step 8's
    // second leg is the sentence it exists for — *"20 K buys 2.84 points over the next
    // 200 000 s"* — where `complement` gives the 3.90 points lost since the pack was new
    // and `since_mark` is a frame for instants rather than for quantities.
    //
    // The origin is the step's own mark and never a declared time, so a claim cannot point
    // it at whichever instant makes the arithmetic work. A continuation arm carries the
    // pre-mark rows, which is what makes the reading available at all.
    if quantity == "soh_cap_fade_since_mark" {
        assert!(
            at_s > mark_s,
            "a claim reads `{quantity}` at t = {at_s} s on a step whose mark is at \
             {mark_s} s. Before the mark this quantity folds over a stretch that has not \
             happened, and at the mark it is zero for every sentence that could print it — \
             so it is only ever a reading on a continuation arm."
        );
        return measure_row("soh_cap_at", run.read(mark_s, false))
            .expect("`soh_cap_at` is a row quantity")
            - measure_row("soh_cap_at", run.read(at_s, probe))
                .expect("`soh_cap_at` is a row quantity");
    }
    // Everything past here folds over the whole trajectory, and a probe is one row taken
    // before any of it exists. Refused rather than answered from `rows` behind the claim's
    // back: a claim that says `probe = true` and gets the stepped run's minimum would be
    // green about a measurement nobody made.
    assert!(
        !probe,
        "a claim reads `{quantity}` off the zero-length probe. That quantity is a \
         reduction over the whole run — a flag's arrival, a charge total, a minimum — and \
         the probe is a single read taken before the first step, with no history behind \
         it. Drop `probe` and give the claim the instant it is really read at."
    );
    // The two **cost** quantities take an instant tag too, on the same terms as a row
    // quantity's above — and it is stripped *here*, past the `!probe` assert, rather than
    // up there with the rest. That position is load-bearing: both are differences from the
    // zero-length probe, so a tagged one resolved in front of that assert would answer zero
    // by construction and go green about a measurement nobody made.
    //
    // Step 18 is what needed it. Its base run states its temperature rise twice — the
    // 0.96 K the tooth leaves at 60.5 s and the 0.92 K still there at the 90 s mark — and
    // the step next door quotes one of them, which [`Tie::Quoted`] cannot do while two
    // claims answer to one name.
    let quantity = match quantity
        .split_once(':')
        .and_then(|(name, tag)| tag.parse::<f64>().ok().map(|tag| (name, tag)))
    {
        Some((name, tag))
            if matches!(
                name,
                "soc_lost_pts_at" | "t_rise_k_at" | "v_below_cccv_target_mv_at"
            ) =>
        {
            assert!(
                (tag - at_s).abs() < run.dt * 0.5,
                "a claim reads `{quantity}`, whose tag names t = {tag} s, and declares                  `read_at_s = {at_s}`. The tag is the instant the reading is taken at and                  not a label, so the two cannot disagree."
            );
            name
        }
        _ => quantity,
    };
    // Three families take an argument after a colon, because the thing being read is not
    // a fixed time: a flag's arrival, the voltage at a charge level, and the instant a
    // voltage threshold is crossed.
    if let Some(flag) = quantity.strip_prefix("flag_first_s:") {
        return run.first_flag(flag).unwrap_or_else(|| {
            panic!("the run never raised `{flag}` — the claim is about a flag that no longer fires")
        });
    }
    if let Some(frac) = quantity.strip_prefix("v_at_soc_below:") {
        let frac: f64 = frac
            .parse()
            .unwrap_or_else(|_| panic!("`{frac}` is not a charge fraction"));
        return run
            .rows
            .iter()
            .find(|r| r.telemetry.soc_true <= frac)
            .unwrap_or_else(|| panic!("the run never fell to soc <= {frac}"))
            .telemetry
            .v_terminal;
    }
    // The fall between two charge levels \[mV\]: the voltage at the first row at or below
    // the higher, less the voltage at the first row at or below the lower.
    //
    // **A quantity for a sentence that prints a drop and neither of its ends.** Step 2's own
    // 481 mV is two claims sharing one literal, because the sentence beside it gives both
    // voltages ("4.030 V to 3.549"). The two cells it sends the reader to are quoted as
    // "620 mV" and "618 mV" with no voltage anywhere near either, and the reason is the
    // lesson: what a reader is meant to carry away is the SHAPE of the curve — how far it
    // falls across the middle — and not where this particular cell happens to sit. Two
    // claims on the ends would pin numbers the prose does not print and leave the number it
    // does print tied to a derivation over them.
    //
    // Read on the same first-row-at-or-below rule as `v_at_soc_below`, so the two agree by
    // construction where a step states an end and the fall in the same paragraph.
    if let Some(rest) = quantity.strip_prefix("v_fall_mv_soc:") {
        let (from, to) = rest.split_once(':').unwrap_or_else(|| {
            panic!("`{rest}` is not a pair of charge fractions separated by a colon")
        });
        let at = |frac: &str| -> f64 {
            let frac: f64 = frac
                .parse()
                .unwrap_or_else(|_| panic!("`{frac}` is not a charge fraction"));
            run.rows
                .iter()
                .find(|r| r.telemetry.soc_true <= frac)
                .unwrap_or_else(|| panic!("the run never fell to soc <= {frac}"))
                .telemetry
                .v_terminal
        };
        return (at(from) - at(to)) * 1000.0;
    }
    // When the terminal first falls to or below a threshold \[s\] — the mirror image of
    // `v_at_soc_below:`, which asks what the voltage is at a charge level.
    //
    // It exists for the sentence step 16 is built around: *"it is finished: **2.422 V at
    // t = 464 s, past the 2.50 V cut-off**"*. That instant is a crossing rather than a
    // reading, and until this quantity existed the only number in the file that could have
    // stood for it was `flag_first_s:OPERATING_POINT_OUT_OF_WINDOW`, which happens to
    // arrive on the same step. Happens to is the objection: a flag saying the demand left
    // the servable window is not the sentence's claim that the cell reached its cut-off,
    // and two quantities printing one digit is the mis-pointing [`States`] exists to
    // refuse. See `docs/plans/path-untouched-steps.md`.
    //
    // **The threshold is an argument rather than a lookup**, on the same terms as
    // `v_at_soc_below:`'s charge fraction: `sim-data` can read `v_min` out of the
    // chemistry file, and a claim that took it from there would be asserting the crossing
    // of whatever that field says rather than of the number its own sentence prints. The
    // day the `Chemistry` accounting arm lands, tying the two together is that arm's job.
    // When the debt falls back through a stated depth, **on the way down**.
    //
    // The threshold is the sentence's own number and not a field, on `t_at_v_below`'s terms:
    // step 20 says the voltage starts climbing "when the deficit re-enters the 2-point ramp",
    // and the 2 there is what the ledger ties to the chemistry separately. A quantity that
    // read `[reversal]` itself would be asserting the crossing of whatever that file says.
    //
    // **After the peak, which is what makes it the way down.** A charge leg's rows include
    // everything before the mark, so the debt crosses every depth twice — once while it is
    // being run up and once while it is being repaid — and a first-match search would answer
    // with the outward crossing and look right. The peak is read rather than assumed to be
    // the mark: an arm that kept discharging past the mark would move it, and this should
    // follow the run.
    if let Some(pts) = quantity.strip_prefix("deficit_falls_below_pts:") {
        let pts: f64 = pts
            .parse()
            .unwrap_or_else(|_| panic!("`{pts}` is not a depth in points"));
        let peak = run
            .rows
            .iter()
            .position(|r| {
                r.deficit_max
                    == run
                        .rows
                        .iter()
                        .map(|r| r.deficit_max)
                        .fold(f64::MIN, f64::max)
            })
            .expect("a run has rows, so it has a deepest one");
        return run.rows[peak..]
            .iter()
            .find(|r| r.deficit_max * 100.0 <= pts)
            .unwrap_or_else(|| {
                panic!(
                    "the debt peaks at {:.4} points at t = {} s and is still {:.4} at                      t = {} s, so it never comes back through {pts}. The leg is too short,                      or nothing is repaying it.",
                    run.rows[peak].deficit_max * 100.0,
                    run.rows[peak].t_s,
                    run.rows.last().expect("rows").deficit_max * 100.0,
                    run.rows.last().expect("rows").t_s,
                )
            })
            .t_s;
    }
    // When the charge first falls to or below a level \[s\] — `t_at_v_below`'s sibling, one
    // axis over. The voltage crossing is what a reader watches on the plot; this is what a
    // sentence means when it says how long a cell takes to get somewhere on the SOC axis,
    // and step 2's third cell prints exactly that: the same fall, 5708.5 s to reach it.
    if let Some(frac) = quantity.strip_prefix("t_at_soc_below:") {
        let frac: f64 = frac
            .parse()
            .unwrap_or_else(|_| panic!("`{frac}` is not a charge fraction"));
        return run
            .rows
            .iter()
            .find(|r| r.telemetry.soc_true <= frac)
            .unwrap_or_else(|| {
                panic!(
                    "the run never fell to soc <= {frac} — its lowest charge is {:.6}. The \
                     claim is about a crossing that no longer happens.",
                    run.rows
                        .iter()
                        .map(|r| r.telemetry.soc_true)
                        .fold(f64::MAX, f64::min)
                )
            })
            .t_s;
    }
    if let Some(volts) = quantity.strip_prefix("t_at_v_below:") {
        let volts: f64 = volts
            .parse()
            .unwrap_or_else(|_| panic!("`{volts}` is not a voltage"));
        return run
            .rows
            .iter()
            .find(|r| r.telemetry.v_terminal <= volts)
            .unwrap_or_else(|| {
                panic!(
                    "the run never fell to v <= {volts} — its lowest terminal voltage is \
                     {:.6} V. The claim is about a crossing that no longer happens.",
                    run.rows
                        .iter()
                        .map(|r| r.telemetry.v_terminal)
                        .fold(f64::MAX, f64::min)
                )
            })
            .t_s;
    }
    // When the `surface gap` row's **negative half first prints `0.00`** [s].
    //
    // `t_at_v_below`'s shape with the threshold taken out of the author's hands, and that is
    // the whole reason it is a quantity rather than a claim at an instant an author picked.
    // Step 17's sentence is about what the ROW says - *"by the time the negative gap first
    // reads 0.00"* - so the crossing is the display's own rule, `fmt_gap_pts(neg, 2)`, which
    // is the same mirror the display check runs on and carries `gapPts`'s negative-zero
    // guard with it. A `gap_neg_below_pts:<x>` would have let the sentence be true of
    // whatever `x` made it true.
    //
    // **The zero has to be final**, and that is asserted rather than assumed. A first-match
    // search answers a flicker as readily as an arrival, and "first reads 0.00" is a
    // sentence about a gradient that has finished draining. On this trajectory the gap falls
    // monotonically through the rest, so the two readings coincide; the assert is what says
    // so out loud if the trajectory ever stops being monotone there.
    if quantity == "gap_neg_zero_s" {
        let zero = |row: &Row| {
            row.surface_gap
                .is_some_and(|(neg, _)| fmt_gap_pts(neg, 2) == "0.00")
        };
        let first = run.rows.iter().position(zero).unwrap_or_else(|| {
            panic!(
                "the `surface gap` row's negative half never prints `0.00` on this run -                  its smallest reading is {:.6} points. The claim is about a crossing that                  no longer happens, or this step's cell model has no surface at all.",
                run.rows
                    .iter()
                    .filter_map(|r| r.surface_gap.map(|(neg, _)| neg * 100.0))
                    .fold(f64::MAX, f64::min)
            )
        });
        assert!(
            run.rows[first..].iter().all(zero),
            "the `surface gap` row's negative half prints `0.00` from t = {} s and then              leaves zero again. `first reads 0.00` is a sentence about a gradient that has              finished draining, and a first match on a flicker would answer it with an              instant the reader would not recognise.",
            run.rows[first].t_s,
        );
        return run.rows[first].t_s;
    }
    // The same crossing, timed from the start of the pulse leg `at_s` is on — step 24's
    // *"the run stops at the same `1.750 V` again — after **237.5 s**"*.
    //
    // A separate quantity rather than a frame on the one above, because neither existing
    // frame reaches it and both would be wrong in a way that looks right. `since_mark` is
    // **zero** here: the crossing IS this step's mark. And `t_at_v_below` answers 737 s,
    // because leg one crossed the same threshold first — the very fact the step is about.
    // What the sentence prints is how long the *second* discharge lasted, which is a
    // duration inside one leg and has no other name.
    if let Some(volts) = quantity.strip_prefix("leg_s_at_v_below:") {
        let volts: f64 = volts
            .parse()
            .unwrap_or_else(|_| panic!("`{volts}` is not a voltage"));
        let from = leg_start_s(run, at_s, quantity);
        return run
            .rows
            .iter()
            .find(|r| r.t_s >= from && r.telemetry.v_terminal <= volts)
            .unwrap_or_else(|| {
                panic!(
                    "the leg beginning at t = {from} s never fell to v <= {volts}. The \
                     claim is about a crossing that no longer happens on this leg."
                )
            })
            .t_s
            - from;
    }
    // The pulse-train family. Five quantities, all `<name>:<tooth>` with the tooth counted
    // from one, and all of them differences between two instants the `Pulse` program
    // defines rather than readings at `read_at_s`. Steps 12, 13 and 14 are built on the
    // decomposition of one tooth and nothing in this file could express it before.
    if let Some((name, n)) = quantity
        .strip_prefix("pulse_")
        .and_then(|rest| rest.split_once(':'))
    {
        let n: usize = n
            .parse()
            .unwrap_or_else(|_| panic!("`{n}` is not a tooth number"));
        let legs = Tooth::of(run, n);
        // `read_at_s` is asserted rather than decorative, on the same terms as
        // `soc_gap_pts_min`: these quantities have their own instants and a claim carrying
        // a different one would be pointing at a moment nothing measured.
        let want = |expect: f64, what: &str| {
            assert!(
                (at_s - expect).abs() < run.dt * 0.5,
                "a claim reads `{quantity}` at t = {at_s} s. That quantity is measured {what}, \
                 which for tooth {n} of this train is t = {expect} s. A whole-tooth \
                 measurement has to name the instant it belongs to, or `read_at_s` says \
                 nothing."
            );
        };
        let mv = 1000.0;
        return match name {
            // The whole drop from the open-circuit voltage the tooth starts at to the
            // bottom of the loaded leg.
            "sag_mv" => {
                want(legs.stop_s, "at the instant the current stops");
                (legs.rest_at_start - legs.v_at_stop) * mv
            }
            // The part that comes back the instant the current goes away: `I·R0` on a
            // circuit, charge-transfer kinetics on a particle. This is the one that needs
            // the zero-length read — no stepped row sits at this instant with no current.
            "jump_mv" => {
                want(legs.stop_s, "at the instant the current stops");
                (legs.rest_at_stop - legs.v_at_stop) * mv
            }
            // The part that climbs back slowly over the rest: the RC pairs on a circuit, a
            // concentration profile levelling out on a particle. The number both steps tell
            // the reader to watch.
            "rebound_mv" => {
                want(legs.rest_end_s, "at the end of the tooth's rest");
                (legs.v_at_rest_end - legs.rest_at_stop) * mv
            }
            // The part that never comes back — charge that has actually left, and the
            // open-circuit voltage itself stepping down.
            "lost_mv" => {
                want(legs.rest_end_s, "at the end of the tooth's rest");
                (legs.rest_at_start - legs.v_at_rest_end) * mv
            }
            // How much of that rest's rebound has arrived by `read_at_s`, as a fraction.
            //
            // A fraction and not a percentage so `states = complement` reads naturally: the
            // sentence that says 8 % arrives in the last five minutes is the same
            // measurement as the one that says 92 % had already arrived, and `complement`
            // is how this file already spells "the sentence prints the other side of it".
            "rebound_arrived" => {
                assert!(
                    at_s > legs.stop_s && at_s <= legs.rest_end_s,
                    "a claim reads `{quantity}` at t = {at_s} s, which is not inside tooth \
                     {n}'s rest ({} s to {} s). A fraction of a rebound has no meaning \
                     outside the rest it is a fraction of.",
                    legs.stop_s,
                    legs.rest_end_s
                );
                let full = legs.v_at_rest_end - legs.rest_at_stop;
                (run.at(at_s).v_terminal - legs.rest_at_stop) / full
            }
            other => panic!(
                "path-claims.toml names a pulse quantity this test cannot measure: \
                 `pulse_{other}`. Known: pulse_sag_mv, pulse_jump_mv, pulse_rebound_mv, \
                 pulse_lost_mv, pulse_rebound_arrived — each `:<tooth>`, counted from one."
            ),
        };
    }
    match quantity {
        // Amp-hours out of the terminals by `at_s`, `Σ i·dt / 3600`.
        //
        // The one quantity in this file that no readout row shows, and it is here because
        // the lead-acid steps are a comparison OF it: the same cell delivers 6.96 A·h taken
        // slowly and 4.42 A·h taken hard, which is Peukert's law and is the reason that
        // chemistry's rating carries a rate. `soc (true)` is the page-visible half of the
        // same fact — this engine coulomb-counts, so charge out and charge left are one
        // measurement seen twice — but the sentence a reader is given is in amp-hours,
        // because that is the unit a battery is sold in. A claim measured here may name no
        // `display`, for the same reason `deficit_pts_at` may not: there is no row.
        //
        // **The row at `at_s` is excluded, and that is not an off-by-one.** Every lead-acid
        // mark sits on the first step whose terminal has fallen BELOW the cutoff, so that
        // step is the one that went too far. A real cutoff happens partway through it, and
        // `crates/sim-data/tests/lead_acid_rate.rs::delivered_bracket` takes the same
        // conservative bound for the same reason and says so at length: including it
        // overcounts, excluding it undercounts by at most one step's charge, and the truth
        // is between. This file quotes the same end of the bracket that test does, so the
        // two instruments cannot disagree by a step's worth and call it a drift.
        "delivered_ah" => {
            let dt = run
                .rows
                .windows(2)
                .next()
                .map_or(0.0, |w| w[1].t_s - w[0].t_s);
            run.rows
                .iter()
                .filter(|r| r.t_s < at_s - dt * 0.5)
                .map(|r| r.telemetry.i_actual * dt / 3600.0)
                .sum()
        }
        // The same charge, counted from the start of the pulse leg `at_s` is on rather than
        // from t = 0 — step 24's `1.4220 A·h`, which is what the second discharge got out of
        // a cell that had already declared itself empty.
        //
        // The leg's origin is read off the run ([`leg_start_s`]) and not off the program, so
        // it needs nothing declared twice. The same end of the bracket is excluded at both
        // ends, so this is exactly `delivered_ah(at_s) − delivered_ah(leg start)` and cannot
        // disagree with the whole-run quantity by a step's charge.
        "leg_delivered_ah" => {
            let dt = run
                .rows
                .windows(2)
                .next()
                .map_or(0.0, |w| w[1].t_s - w[0].t_s);
            let from = leg_start_s(run, at_s, quantity);
            run.rows
                .iter()
                .filter(|r| r.t_s > from - dt * 0.5 && r.t_s < at_s - dt * 0.5)
                .map(|r| r.telemetry.i_actual * dt / 3600.0)
                .sum()
        }
        // When the debt STARTS: the first step at which any cell is past empty, which is
        // what the `past empty` readout coming off zero is.
        //
        // **Equal to `flag_first_s:SOC_CLAMPED_LOW` by construction, not by coincidence** —
        // the flag is raised on the step the coulomb counter clamps, which is the step the
        // deficit it carries becomes non-zero. So this quantity is not independent evidence
        // for that instant and is not here as a cross-check: it exists because step 7's
        // sentence is about the readout rather than about the flag, and a sentence has to be
        // tied to the thing it names. Two quantities agreeing by construction is the
        // structural blindness this file has been caught by before; it is stated here rather
        // than discovered later.
        "deficit_leaves_zero_s" => run
            .rows
            .iter()
            .find(|r| r.deficit_max > 0.0)
            .unwrap_or_else(|| {
                panic!(
                    "the run never went past empty: the worst cell still owes nothing \
                     at t = {} s, so no cell ever crossed and the claim is about an \
                     instant that does not exist.",
                    run.rows.last().expect("rows").t_s
                )
            })
            .t_s,
        // And when the LAST cell crosses: the first step at which every cell owes
        // something. `deficit_min` is the shallowest debt in the pack, so it leaves zero on
        // the step the final cell arrives.
        "deficit_all_owed_s" => run
            .rows
            .iter()
            .find(|r| r.deficit_min > 0.0)
            .unwrap_or_else(|| {
                panic!(
                    "some cell in this pack never went past empty — the shallowest debt \
                     is still {:.6} at t = {} s, and \"the eight cells cross\" is a \
                     sentence about all of them.",
                    run.rows.last().expect("rows").deficit_min,
                    run.rows.last().expect("rows").t_s
                )
            })
            .t_s,
        // How long the pack takes to cross empty end to end. A measurement rather than the
        // sentence's own arithmetic over the two instants beside it: those two sit in
        // literals of their own, so a derivation could not reach them, and a spread is in
        // any case the thing the sentence is about — it moves when the scatter moves, which
        // is the failure a reader would care about.
        //
        // **`read_at_s` is asserted rather than ignored**, on `soc_gap_pts_min`'s terms and
        // for its reason. A duration has no instant of its own, so this arm ignores `at_s`
        // entirely — which makes a claim's declared instant free, and a free field beside a
        // number is the shape this file keeps being caught by. Measured, not feared: with
        // no assert, moving this claim's `read_at_s` to 200 s left all 35 tests green while
        // its own note said the reading was taken "where the spread is complete".
        //
        // The instant it has to name is the LATER crossing, which is the first moment the
        // spread is a whole number rather than a lower bound. The two grid quantities
        // either side of this one need no such fence: each returns the instant it is read
        // at, exactly as `flag_first_s:*` and `deficit_zero_s` do.
        "deficit_crossing_spread_s" => {
            let ends = measure("deficit_all_owed_s", run, at_s, probe, mark_s);
            assert!(
                (at_s - ends).abs() < f64::EPSILON,
                "the pack finishes crossing empty at t = {ends} s and this claim reads at \
                 t = {at_s} s. A spread has no instant of its own, so it has to be read \
                 where it is complete — otherwise `read_at_s` says nothing and the shape \
                 of the run can change under a green claim."
            );
            ends - measure("deficit_leaves_zero_s", run, at_s, probe, mark_s)
        }
        // The first instant the trace reaches the floor it never leaves — where the
        // collapse below empty *stops*, which is what step 20's "the fall simply stops"
        // names and what the 83 seconds that sentence's neighbour prints is measured to.
        //
        // Defined off the run's own minimum rather than off `[reversal].floor_v`, for the
        // reason `t_at_v_below` gives about taking its threshold from the chemistry file: a
        // quantity that read the declared floor would be asserting the crossing of whatever
        // that field says rather than the flattening a reader is watching. The terminal
        // voltage is the open-circuit floor minus `I·R`, so the two are not the same number
        // anyway — 0 V declared, −0.064 V on the trace.
        //
        // **Two fences, because a minimum has neither an instant nor a duration of its own.**
        // The row must not be the last one, or "stays there for as long as you care to run"
        // would be a claim about a run that ended; and the voltage must still be within a
        // microvolt of it at the end, which is what makes "the fall stopped" different from
        // "the fall paused here".
        "v_floor_s" => {
            let floor = run
                .rows
                .iter()
                .map(|r| r.telemetry.v_terminal)
                .fold(f64::MAX, f64::min);
            let last = run.rows.last().expect("rows");
            let at = run
                .rows
                .iter()
                .find(|r| (r.telemetry.v_terminal - floor).abs() < 1e-9)
                .expect("the minimum is attained by the row it was taken from");
            assert!(
                at.t_s < last.t_s,
                "the run's lowest terminal voltage is on its final row, at t = {} s. A                  floor is a voltage the trace stays at, and a run that stops on it cannot                  say whether it would have.",
                at.t_s,
            );
            assert!(
                (last.telemetry.v_terminal - floor).abs() < 1e-6,
                "the trace reaches {floor:.6} V at t = {} s and is at {:.6} V by t = {} s,                  so it did not stay there. This quantity is the instant the fall STOPS; a                  run that turns round again has a minimum but no floor.",
                at.t_s,
                last.telemetry.v_terminal,
                last.t_s,
            );
            at.t_s
        }
        // When the debt is paid off: the first step at zero deficit after a step that had
        // one. Measured rather than assumed because it is the instant two of this path's
        // claims are read at, and a hardcoded time would go quietly stale if the
        // trajectory moved — the failure mode `read_at_s` already has for a crossing.
        "deficit_zero_s" => {
            let owed = run.rows.iter().position(|r| r.deficit_max > 0.0).expect(
                "the run never went past empty — the claim is about a debt that \
                         is no longer incurred",
            );
            run.rows[owed..]
                .iter()
                .find(|r| r.deficit_max == 0.0)
                .unwrap_or_else(|| {
                    panic!(
                        "the run went past empty and never came back: the deficit is \
                         still {:.6} at t = {} s. The charge leg is too short, or the \
                         demand does not repay it.",
                        run.rows.last().expect("rows").deficit_max,
                        run.rows.last().expect("rows").t_s
                    )
                })
                .t_s
        }
        // The smallest that gap ever gets over the whole run, which is how "simply never
        // closes" is made checkable. The mark reading alone cannot say it: an estimator
        // that closed the gap at 300 s and re-opened it by 600 would pass the mark claim
        // and fail this one.
        //
        // **`read_at_s` is asserted rather than ignored.** A reduction over every row has
        // no natural instant, and a claim carrying a decorative one is the shape this file
        // rejects — so the claim has to name where the minimum is, and it moving is a
        // change in the trajectory's shape that an author should look at. Measured, not
        // assumed: the gap on this step is NOT monotone (the current sensor's noise wobbles
        // it), so "the minimum is the first row" is a fact about the run rather than an
        // arithmetic consequence of the gap growing.
        "soc_gap_pts_min" => {
            let (best, at) = run
                .rows
                .iter()
                .map(|r| (gap_pts(&r.telemetry, r.t_s), r.t_s))
                .fold((f64::MAX, f64::NAN), |a, b| if b.0 < a.0 { b } else { a });
            let claimed = run.row_at(at_s);
            assert!(
                (claimed.t_s - at).abs() < f64::EPSILON,
                "the smallest BMS gap on this run is {best} points at t = {at} s, and the \
                 claim reads at t = {at_s} s (the row at {}). A whole-run minimum has to \
                 name the instant it happens at, or `read_at_s` says nothing and the shape \
                 of the run can change under a green claim.",
                claimed.t_s
            );
            best
        }
        // What the run has *cost* by `at_s`, against what the panel showed before the
        // reader pressed Run: points of charge gone, and kelvin gained by the hottest cell.
        //
        // Both are differences from the zero-length probe, and that reference is the whole
        // reason they are here rather than in `measure_row`. Two of this path's sentences
        // state a cost rather than a level — "0.56 points at 0.5 s, 5.57 at 5 s, 11.14 at
        // 10 s, where the cell ends 19 K hotter instead of 1", and "this fault costs fifty
        // points" — and neither is any single reading. Claiming the level instead (89.44 %,
        // 316.85 K) would be claiming a number the sentence does not print, which is the
        // mis-pointing `States` exists to refuse.
        //
        // The probe is the right origin and not merely a convenient one: it is what
        // `applyStep` reads after the step's controls are dialled in and before the run is
        // armed, so it is literally the panel the reader is looking at when they press Run.
        // A claim may not read either quantity *on* the probe — `measure` has already
        // refused that above — because the difference there is zero by construction.
        //
        // **A subtraction of two engine readings is still not a `Derived` arm.** Nothing
        // here reads the prose: the origin comes off the run, not off another number in the
        // sentence. See the accounting arms in `docs/plans/path-prose-ledger.md` for the
        // distinction, which is the one that keeps this from being a declaration.
        "soc_lost_pts_at" => (run.probe.telemetry.soc_true - run.at(at_s).soc_true) * 100.0,
        "t_rise_k_at" => run.at(at_s).t_max - run.probe.telemetry.t_max,
        // How long the **constant-current leg** lasts \[s\]: the last instant the controller
        // was still asking for a current, which is the step before the first voltage hold.
        //
        // A different question from `cccv_taper_s` below, and a better-behaved one. That one
        // is about when the page *stops*, which `ccCvDone` decides at the end of a chopped
        // chunk; this is about when the page changes *leg*, which `ccCvDemand` decides on the
        // decision grid and nowhere else. Every demand this run took was chosen at a window
        // boundary — [`drive`] holds one across the whole window, as the page does — so the
        // leg boundary is a function of the simulation whatever the frame rate, with no
        // invariant needed to say so.
        //
        // **Read off the demand, never off the current.** [`Row::voltage_hold`] records what
        // the controller asked for. Inferring the leg from `|i| < the box` would be a
        // different statement that happens to agree here: a derate, a clamp or a protection
        // trip all soften a current without changing the leg.
        //
        // Two refusals rather than a plausible answer:
        //
        // * a step whose demand is not the page's CC-CV policy has no legs to divide;
        // * a run that never holds a voltage, or that goes *back* to constant current after
        //   holding one, has no single leg boundary — the sentence this exists for says the
        //   current "stays at −1.5 A for 5420 s", which is a claim that there is exactly one.
        "cccv_cc_ends_s" => {
            if !matches!(run.prog, Prog::CcCv { .. }) {
                panic!(
                    "a claim reads `cccv_cc_ends_s` on a step whose demand is not the page's \
                     CC-CV policy. There are no two legs to find the boundary of."
                )
            }
            let first_hold = run
                .rows
                .iter()
                .position(|r| r.voltage_hold)
                .unwrap_or_else(|| {
                    panic!(
                        "the run never left constant current — the controller asked for a \
                         voltage hold on none of its {} steps, so this charge has one leg \
                         and not two. `leg-that-is-not-there` is the step that is about, \
                         and it has no boundary to claim.",
                        run.rows.len()
                    )
                });
            assert!(
                run.rows[first_hold..].iter().all(|r| r.voltage_hold),
                "the controller went back to constant current after holding a voltage, so \
                 this run has more than one leg boundary and `cccv_cc_ends_s` is not a \
                 number. Either the band is chattering — see docs/plans/cc-cv.md, which \
                 sized it against the solver's residual for exactly this — or the sentence \
                 claiming a single leg change is about a trajectory that no longer has one."
            );
            assert!(
                first_hold > 0,
                "the first step of this run was already a voltage hold, so its \
                 constant-current leg is empty and the boundary is not a duration. A pack \
                 that starts inside the band is a pack with nothing to charge."
            );
            run.rows[first_hold - 1].t_s
        }
        // How far the pack is **below the CC-CV target** \[mV\] — the target being
        // `v_cell * series`, which is `ccCvNote`'s own arithmetic and no file's field.
        //
        // Step 11 prints this quantity twice and prints the target once: *"only 25 mV short
        // of the 16.80 V it is aiming for"* one step before the over-voltage trip, and
        // *"the 130 mV gap you see after the trip"* one step after it. The page shows both
        // sides of the subtraction in one string — `terminal 16.775 V of 16.800 V` — and
        // never the difference, so this is a gap the reader does the arithmetic for, on the
        // same terms as `soc_gap_pts_at` and `t_gap_k_at`. No `display` may be named on a
        // claim measured here.
        //
        // **Not in [`measure_row`], and the reason is a fact rather than a fence**: a row
        // does not know what it was being charged toward. This needs `run.prog` and
        // `run.series`, which is also what makes it unreadable from a probe — the right
        // answer, since neither of this step's two readings exists before the run.
        //
        // Millivolts, for `v_cell_spread_mv_at`'s reason: the sentence speaks them.
        //
        // **The target moves with an arm that retypes the charge current, and no claim has
        // exercised that.** `run.prog` is the arm's program where one is set, so an arm
        // with `cc_cv_a` shifts what the pack is aiming at — correct, and unobserved: both
        // of step 11's readings are on the step's own run. Worth knowing before a step with
        // no constant-voltage leg claims this on a retyped arm.
        //
        // Refuses on any program but CC-CV rather than falling back to some other target. A
        // pack on a `Current` demand is being charged toward nothing, so the quantity does
        // not exist there — it is not zero.
        "v_below_cccv_target_mv_at" => {
            let Prog::CcCv { v_cell, .. } = run.prog else {
                panic!(
                    "a claim reads how far the pack is below its CC-CV target on a step \
                     whose demand program is {:?}. Only a CC-CV charge has a target; on \
                     anything else the quantity does not exist rather than being zero.",
                    run.prog
                );
            };
            (v_cell * run.series - run.at(at_s).v_terminal) * 1000.0
        }
        // **By when** a CC-CV charge has finished \[s\]: the first decision-window boundary
        // at or after the taper crossing.
        //
        // `cccv_taper_s` below answers "where the page stops" and refuses unless the
        // crossing is itself a boundary, because off a boundary the stop instant is a fact
        // about the browser's frame schedule rather than about the simulation. Step 11 is
        // the case that refusal leaves with nothing to say: its unprotected charge crosses
        // the taper at 4817.5 s, which is step 9635 at dt = 0.5 and not a multiple of the
        // 20-step window. The reader is told somewhere in (crossing, next boundary], and
        // only the far end of that interval is a function of the simulation.
        //
        // So this is a **bound and is worded as one wherever it is claimed** — "the charge
        // ends by 4820 s" is true whatever the frame rate; "at 4820 s" would not be. A
        // sentence that wants the instant itself needs a trajectory whose crossing lands on
        // a boundary, which is what step 9 has and says so.
        //
        // The crossing is found the same way `cccv_taper_s` finds it, and the window is
        // `CCCV_PERIOD_S` read out of the page rather than restated here.
        "cccv_window_close_s" => {
            let Prog::CcCv { taper, .. } = run.prog else {
                panic!(
                    "a claim reads a CC-CV decision boundary on a step whose demand \
                     program is {:?}. Only a CC-CV charge has decision windows.",
                    run.prog
                );
            };
            let crossed = run
                .rows
                .iter()
                .position(|r| r.telemetry.i_actual.abs() < taper)
                .unwrap_or_else(|| {
                    panic!(
                        "the current never falls under the {taper} A taper on this \
                         trajectory, so there is no window for the charge to end in. \
                         Either the run is too short or the claim is on the wrong arm."
                    )
                });
            let period = cccv_period_s();
            let t_s = run.rows[crossed].t_s;
            (t_s / period).ceil() * period
        }
        // When a CC-CV charge finishes \[s\]: the first row whose current has fallen to the
        // taper the page is comparing against.
        //
        // `ccCvDone` is the one piece of the page's CC-CV behaviour that `drive` does not
        // model, and its own doc says why — the test is evaluated at the end of each
        // *chopped chunk*, and a chunk ends either at a decision-window boundary or wherever
        // the frame's step budget ran out. So the instant the page STOPS is bounded below by
        // the crossing and above by the next window boundary, and where inside that it lands
        // is a fact about how the browser scheduled the run.
        //
        // **Unless the crossing is itself a window boundary, and then there is no window.**
        // A chunk never crosses a boundary, so every boundary the run reaches is a chunk
        // end and `ccCvDone` is evaluated there; if the current is already under the taper
        // at that step and was not at the one before, the page stops exactly there whatever
        // the frame rate. That is the case on step 9 — 6210 s is step 12420 at dt = 0.5,
        // and 12420 is a multiple of the 20-step window — so "it stops at 6210 s" is a
        // statement about the simulation after all.
        //
        // The assertion below is what keeps that true rather than assumed. If the trajectory
        // ever moves the crossing off a boundary, this quantity refuses to answer instead of
        // returning a number that is only the earliest of several the page could show. An
        // invariant rather than a tolerance: the alternative is a claim that is right about
        // one frame schedule and silent about the rest.
        "cccv_taper_s" => {
            let Prog::CcCv { taper, .. } = run.prog else {
                panic!(
                    "a claim reads `cccv_taper_s` on a step whose demand is not the page's \
                     CC-CV policy. There is no taper to cross."
                )
            };
            let k = cccv_window_steps(run.dt);
            let crossed = run
                .rows
                .iter()
                .position(|r| r.telemetry.i_actual.abs() <= taper)
                .unwrap_or_else(|| {
                    panic!(
                        "the run never fell to |i| <= {taper} A — its smallest current is \
                         {:.6} A. The claim is about a charge that no longer terminates.",
                        run.rows
                            .iter()
                            .map(|r| r.telemetry.i_actual.abs())
                            .fold(f64::MAX, f64::min)
                    )
                });
            let t_s = run.rows[crossed].t_s;
            let index = (t_s / run.dt).round() as u64;
            assert!(
                index.is_multiple_of(k),
                "the current first falls under the {taper} A taper at t = {t_s} s, which is \
                 step {index} and NOT a multiple of the {k}-step decision window. \
                 `ccCvDone` is evaluated at the end of each chopped chunk, so on this \
                 trajectory the instant the page stops depends on how the browser scheduled \
                 the frames: anywhere from here to the next window boundary. There is no \
                 single number to claim. Either the trajectory moved, or a claim reading \
                 this quantity was never entitled to."
            );
            t_s
        }
        // The coupling CLAUDE.md refuses to let a chemistry model one half of: points
        // of resistance growth per point of capacity lost.
        "soh_ratio_at" => {
            let t = run.at(at_s);
            (t.soh_resistance - 1.0) / (1.0 - t.soh_capacity)
        }
        other => panic!(
            "path-claims.toml names a quantity this test cannot measure: `{other}`. \
             Known: v_at_mark, v_at, v_cell_min_at, v_cell_max_at, v_cell_spread_mv_at, \
             soc_at, i_at, \
             t_max_at, soh_cap_at, soh_res_at, soh_ratio_at, q_gen_at, i_rejected_at, \
             deficit_pts_at, deficit_pts_min_at, deficit_zero_s, deficit_leaves_zero_s, \n             deficit_all_owed_s, deficit_crossing_spread_s, deficit_worst_cell_series_at, \n             deficit_worst_cell_parallel_at, deficit_best_cell_series_at, \n             deficit_best_cell_parallel_at, delivered_ah, \n             v_below_cccv_target_mv_at, cccv_taper_s, cccv_window_close_s, \
             cccv_cc_ends_s, pulse_sag_mv:<tooth>, pulse_jump_mv:<tooth>, \
             pulse_rebound_mv:<tooth>, pulse_lost_mv:<tooth>, pulse_rebound_arrived:<tooth>, \
             soc_lost_pts_at, t_rise_k_at and 
             v_below_cccv_target_mv_at (all three of which take an instant tag too), 
             soc_gap_pts_at, soc_gap_pts_min, t_gap_k_at, v_group_min_at, \
             surface_gap_neg_pts, surface_gap_pos_pts, gap_neg_zero_s, flag_first_s:<FLAG>, \
             v_at_soc_below:<fraction>, t_at_v_below:<volts>, overpotential_mv_at, \
             rc_overpotential_mv_at, diffusion_overpotential_mv_at, leg_delivered_ah, \
             leg_s_at_v_below:<volts>.\n\
             Any of the row quantities above may also carry the instant it is read at — \
             `v_at:400` — which is how a step files several readings under separately \
             quotable names. The tag must be a number and must equal the claim's own \
             `read_at_s`; `v_at:first` is what lands here."
        ),
    }
}

/// When the pulse leg containing `at_s` began \[s\] — the last leg boundary at or before it.
///
/// Read off [`Row::rest_v`], which [`drive`] fills on exactly the rows where the program
/// changes leg and nowhere else. That is the point: the origin comes from the same place the
/// tooth quantities take theirs, so a leg-relative number and a tooth cannot disagree about
/// where a leg starts. Nothing is declared twice and no program is re-derived here.
///
/// The boundary row is the **first step of the new leg**, so a duration measured from it is
/// a duration of full steps under the new demand — which is what both callers want and what
/// step 24's `237.5 s` is.
fn leg_start_s(run: &Run, at_s: f64, quantity: &str) -> f64 {
    run.rows
        .iter()
        .rfind(|r| r.t_s <= at_s + 1e-9 && r.rest_v.is_some())
        .unwrap_or_else(|| {
            panic!(
                "`{quantity}` is measured from the start of a pulse leg, and this run \
                 reaches no leg boundary at or before t = {at_s} s. The step is not a \
                 pulse train, or the claim reads inside its first leg — which starts at \
                 t = 0 and has `delivered_ah` and `t_at_v_below` already."
            )
        })
        .t_s
}

/// The RC-pair half of a row's overpotential \[V\], refusing rather than falling back.
///
/// A row carries it only if a claim named this instant before the run started, so a `None`
/// here means the claim and the capture list have parted — the `read_at_s` moved, or the
/// quantity was renamed on one side. Falling back to zero would read as "the RC pair is
/// spent", which is a real state this quantity is used to distinguish.
fn rc_half(row: &Row) -> f64 {
    row.rc_overpotential_v.unwrap_or_else(|| {
        panic!(
            "no RC-pair reading was taken at t = {} s. That read is expensive, so it is \
             taken only at the instants the claims on this trajectory ask for — see \
             `Row::rc_overpotential_v`. A claim's `read_at_s` moved without the run being \
             told, or a quantity name was changed on one side only.",
            row.t_s
        )
    })
}

/// `soc_bms − soc_true` at one row, in points of charge.
///
/// The BMS's estimate is an `Option` because a step may run with no BMS at all, and on
/// such a step this quantity does not exist rather than being zero — a claim naming it
/// there is a claim about a panel row the reader is not shown.
fn gap_pts(telemetry: &Telemetry, t_s: f64) -> f64 {
    let bms = telemetry.soc_bms.unwrap_or_else(|| {
        panic!(
            "a claim reads the BMS's charge estimate at t = {t_s} s on a step that runs \
             with no BMS. The `soc (bms)` row is blank there and the gap is not a \
             quantity; the claim is on the wrong step, or the step's `bms:` was turned off."
        )
    });
    (bms - telemetry.soc_true) * 100.0
}

fn parse_claims_file() -> Claims {
    let text = read(&repo_root().join("web").join("path-claims.toml"));
    let parsed: Claims = toml::from_str(&text).expect("web/path-claims.toml parses");
    assert!(
        !parsed.claim.is_empty(),
        "web/path-claims.toml has no claims — an empty claims file passes every check \
         and proves nothing"
    );
    parsed
}

fn claims() -> Vec<Claim> {
    parse_claims_file().claim
}

/// The English-spelled quantities still in the prose, as `(step, phrase)` pairs.
///
/// Flattened to a pair because that is the whole of what the ban compares: the note beside
/// each entry is for a person, and a check that read it would be checking an author's
/// description of a phrase rather than the phrase.
fn english_exceptions() -> Vec<(String, String)> {
    parse_claims_file()
        .english
        .into_iter()
        .map(|e| (e.step, e.phrase))
        .collect()
}

fn ledger() -> Ledger {
    parse_claims_file().ledger
}

/// The declared derivations. See [`Derivation`].
fn derivations() -> Vec<Derivation> {
    parse_claims_file().derived
}

/// The declared arms. A step may have several; their names must differ.
///
/// A step used to be allowed one leg and no more, because a claim pointed at it with a
/// bare `after_mark = true` and a second leg would have been silently ignored. An arm is
/// named and a claim names it, so several per step is now the normal case — step 18 has
/// six. What must stay unique is the name: two arms sharing one would hand every claim
/// on that step whichever was parsed first, which is the same silent-wrong-trajectory
/// failure one level down.
fn arms() -> Vec<Arm> {
    let arms = parse_claims_file().arm;
    for (i, arm) in arms.iter().enumerate() {
        assert!(
            !arms[..i]
                .iter()
                .any(|other| other.step == arm.step && other.name == arm.name),
            "step `{}` declares two arms called `{}`. A claim names its arm, so the second \
             would be unreachable and its claims measured on the first.",
            arm.step,
            arm.name
        );
        assert!(
            !arm.actions.is_empty(),
            "arm `{}` on step `{}` lists no actions. An arm that presses nothing is the \
             step's own trajectory wearing a name — and any claim on it would be checked \
             against a run no reader has to do anything to reach.",
            arm.name,
            arm.step
        );
    }
    arms
}

/// The arm a claim reads, or `None` for the step's own trajectory to its mark.
fn arm_of<'a>(all: &'a [Arm], claim: &Claim) -> Option<&'a Arm> {
    let name = claim.arm.as_deref()?;
    Some(
        all.iter()
            .find(|a| a.step == claim.step && a.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "claim `{}` on step `{}` reads arm `{name}`, and that step declares no \
                     such arm.",
                    claim.literal, claim.step
                )
            }),
    )
}

/// The lesson whose **pack** an arm runs — its own step's, unless [`Arm::pack_from`] sends
/// the reader next door.
///
/// One function rather than three copies of the same `find`, because three places have to
/// agree about it and two of them are checks: what [`run`] builds, what the override fences
/// in [`every_arm_is_instructed_by_its_own_step`] compare against, and what
/// [`every_claim_is_reachable_in_its_own_step`] measures a timeline with. A step that
/// resolved this differently from the runner would check one trajectory and describe another.
fn pack_lesson_of<'a>(arm: Option<&Arm>, lesson: &'a Lesson, lessons: &'a [Lesson]) -> &'a Lesson {
    match arm.and_then(|a| a.pack_from.as_deref()) {
        Some(id) => lessons.iter().find(|l| l.id == id).unwrap_or_else(|| {
            panic!(
                "an arm on step `{}` runs the pack of the lesson `{id}`, and there is no \
                 such lesson. A step id that no longer exists means the prose it points at \
                 was renamed and this arm was left behind.",
                lesson.id
            )
        }),
        None => lesson,
    }
}

/// Can a reader actually walk to the pack this arm runs?
///
/// Three refusals on [`Arm::pack_from`], lifted out of
/// [`every_arm_is_instructed_by_its_own_step`] so each can be asked directly. Not one of
/// them is reachable by any arm in the file — the only twin arm satisfies all three — and a
/// fence no perturbation can enter is the shape this file has been caught by before, so the
/// three `should_panic` tests below are what price them.
/// The scenario files the page's own picker offers, as `web/index.html` writes them.
///
/// Read out of the markup rather than off the `scenarios/` directory, and the difference is
/// the whole point of the fence that uses it: a file on disk is not a file a reader can
/// reach. `loadScenarioList` replaces these options from `GET /scenarios` at run time, so
/// the served list is the wider one — this is the narrower, static list the page ships with,
/// and an arm that sends a reader to a file outside it is describing a click that may not be
/// there. Failing toward red on the smaller list is the safe direction.
fn picker_files() -> Vec<String> {
    let markup = index_html();
    let marker = "<option value=\"";
    markup
        .match_indices(marker)
        .filter_map(|(at, _)| {
            let rest = &markup[at + marker.len()..];
            let end = rest.find('"')?;
            let value = &rest[..end];
            value.ends_with(".toml").then(|| value.to_string())
        })
        .collect()
}

/// The fences on [`Arm::scenario`] — the picker arm, which changes the file under the
/// reader's controls.
///
/// Separate from [`assert_walkable`] because the two fields model two gestures and only one
/// of them names a lesson. What they share is the argument: an arm that changes the file has
/// to change it to something a reader can actually get to, and has to say so in the prose of
/// the step it is declared on.
fn assert_picker(arm: &Arm, lesson: &Lesson) {
    let Some(file) = arm.scenario.as_deref() else {
        return;
    };
    assert_ne!(
        file, lesson.scenario,
        "arm `{}` on step `{}` loads `{file}` from the picker, which is the file the step is \
         already on. Then nothing is loaded and the arm is an ordinary restart wearing a \
         second name — the same refusal `assert_walkable` makes of a walk to one's own \
         scenario.",
        arm.name, arm.step
    );
    assert!(
        picker_files().iter().any(|f| f == file),
        "arm `{}` on step `{}` loads `{file}`, and `web/index.html`'s picker does not offer \
         it. The instruction tells a reader to pick a file out of a list; a file that is not \
         in the list is a click nobody can make, whatever `scenarios/` holds.",
        arm.name,
        arm.step
    );
    let stem = file.trim_end_matches(".toml");
    assert!(
        ascii_minus(&arm.instruction).contains(stem),
        "arm `{}` on step `{}` loads `{file}`, and `{stem}` does not appear in the \
         instruction it claims to be following:\n  {}\n\
         The file and the sentence that sends a reader to it are two statements of one \
         fact, on the same terms as a typed current — this is the check that keeps \
         them one.",
        arm.name,
        arm.step,
        arm.instruction
    );
    assert!(
        arm.start == Start::Restart,
        "arm `{}` on step `{}` loads `{file}` on a continuation. There is no such position: \
         `loadScenario` closes the backend, rebuilds from the chosen file and starts the \
         clock at zero, so a picked file is a run from t = 0 whatever the reader had \
         reached.",
        arm.name,
        arm.step
    );
    assert!(
        arm.pack_from.is_none(),
        "arm `{}` on step `{}` both walks to `{}` and picks `{file}`. Those are two \
         navigations composed under one sentence, and the second would silently decide \
         which pack the first was about — the same refusal `fade_per_ah` makes of its own \
         pairing with a walk.",
        arm.name,
        arm.step,
        arm.pack_from.as_deref().unwrap_or_default()
    );
}

fn assert_walkable(arm: &Arm, lesson: &Lesson, pack_lesson: &Lesson) {
    assert_picker(arm, lesson);
    let Some(id) = arm.pack_from.as_deref() else {
        return;
    };
    assert_ne!(
        pack_lesson.id, lesson.id,
        "arm `{}` on step `{}` names its own step in `pack_from`. That is the step's own \
         pack with extra words — the same refusal `Tie::Elsewhere` makes, and for the same \
         reason: it would be green without the sentence being about anything.",
        arm.name, arm.step
    );
    assert_ne!(
        pack_lesson.scenario, lesson.scenario,
        "arm `{}` on step `{}` runs the pack of `{id}`, which is on the same scenario file \
         (`{}`). Then it is not another pack at all, only another lesson block pointing at \
         this one, and every number on the arm would be reachable without walking anywhere. \
         If the point really is a different control rather than a different file, this arm \
         is an ordinary restart on its own step.",
        arm.name, arm.step, lesson.scenario
    );
    assert!(
        arm.start == Start::Restart,
        "arm `{}` on step `{}` runs another lesson's pack from `{:?}`. There is no such \
         position: this step's mark is a state of THIS scenario, and a reader who walks to \
         `{id}` has left it. `applyStep` reloads that lesson and re-dials its controls, so \
         the only thing a typed current can do there is precede a **Restart**.",
        arm.name,
        arm.step,
        arm.start
    );

    // The fourth fence, and it is the one that settles which of TWO READINGS of a walk this
    // field models. `pack_from`'s own docs read it as the reader pressing **Back** — landing
    // on the named lesson, which re-dials *that* lesson's controls. Step 14's sentence reads
    // it the other way: *load the other file from the picker*, which changes the scenario and
    // leaves the controls of the step you are standing on. `docs/plans/path-ledger-what-it-cost.md`
    // closed by naming that as this slice's first question.
    //
    // The two readings differ only where the two lessons' controls differ — and on both arms
    // in the file they do not, which is what makes the question answerable by a rule instead
    // of by a choice. So: **a walk may change the file and nothing else.** Where the controls
    // agree, both readings produce the same trajectory and the sentence is unambiguous; where
    // they part, the arm would be measuring something the prose does not say, and this refuses
    // it rather than picking a reading on the author's behalf.
    //
    // Everything the arm itself overrides is exempt, because that is the reader typing. What
    // is compared is what neither the arm nor the sentence touches.
    let (pack_scenario, _) = load(&pack_lesson.scenario);
    let (here_scenario, _) = load(&lesson.scenario);
    let bms_there = pack_lesson.bms.unwrap_or(pack_scenario.pack.bms.is_some());
    let bms_here = lesson.bms.unwrap_or(here_scenario.pack.bms.is_some());
    let differences: Vec<String> = [
        (arm.dt.is_none() && (pack_lesson.dt - lesson.dt).abs() > f64::EPSILON)
            .then(|| format!("`dt` is {} there and {} here", pack_lesson.dt, lesson.dt)),
        (arm.ambient_c.is_none()
            && (pack_lesson.ambient_c - lesson.ambient_c).abs() > f64::EPSILON)
            .then(|| {
                format!(
                    "the ambient is {} °C there and {} °C here",
                    pack_lesson.ambient_c, lesson.ambient_c
                )
            }),
        (arm.bms.is_none() && bms_there != bms_here)
            .then(|| format!("the BMS is {bms_there} there and {bms_here} here")),
        // The demand's *shape*, not its current: which mode the box is in, and — on a pulse
        // train — how long the legs are. An arm types a current and never a mode or a leg
        // (`arm_control_value` says so of both legs), so a difference in either is a
        // difference the reader cannot have made.
        (!same_shape(pack_lesson.demand, lesson.demand)).then(|| {
            format!(
                "the demand program is {:?} there and {:?} here",
                pack_lesson.demand, lesson.demand
            )
        }),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert!(
        differences.is_empty(),
        "arm `{}` on step `{}` walks to `{id}`, and the two lessons disagree about controls \
         this arm does not type: {}.\n\
         A walk may change the FILE and nothing else. With the controls in step, pressing \
         **Back** to `{id}` and loading `{}` from the picker produce the same trajectory, so \
         the sentence means one thing; where they part, the two readings measure different \
         runs and this arm would be silently picking one. Say which in the prose and give the \
         arm the override, or leave the controls alone.",
        arm.name,
        arm.step,
        differences.join(", "),
        pack_lesson.scenario
    );
}

/// Do these two demand programs differ only in the current a reader could type?
///
/// Mode, and the pulse legs. Everything else a `Prog` carries is a current, and a current is
/// what an arm's three boxes are for. See the fourth fence in [`assert_walkable`].
fn same_shape(a: Prog, b: Prog) -> bool {
    match (a, b) {
        (Prog::Current(_), Prog::Current(_)) | (Prog::Rest, Prog::Rest) => true,
        // The CC-CV target and taper are two more numbers a reader types, and `cc_cv_a`
        // reaches only the current — so they are compared here rather than exempted.
        (
            Prog::CcCv {
                v_cell: va,
                taper: ta,
                ..
            },
            Prog::CcCv {
                v_cell: vb,
                taper: tb,
                ..
            },
        ) => (va - vb).abs() < f64::EPSILON && (ta - tb).abs() < f64::EPSILON,
        (
            Prog::Pulse {
                on_s: ona,
                off_s: offa,
                ..
            },
            Prog::Pulse {
                on_s: onb,
                off_s: offb,
                ..
            },
        ) => (ona - onb).abs() < f64::EPSILON && (offa - offb).abs() < f64::EPSILON,
        _ => false,
    }
}

/// Does this claim read a continuation of the step's own run, past its mark?
///
/// The question `after_mark` used to answer directly. It is now derived, because an arm
/// that *restarts* has its own timeline from zero and "after the mark" is not a thing that
/// can be said about it — a claim on step 18's `dt = 5` arm reads at t = 90 s, which is the
/// mark, on a trajectory that is nonetheless not the step's.
fn reads_past_the_mark(all: &[Arm], claim: &Claim) -> bool {
    arm_of(all, claim).is_some_and(|a| a.start == Start::Mark)
}

/// A claim may not name a step that no longer exists.
///
/// This is the only completeness guard here, and it runs one way only: it catches a
/// renamed or deleted lesson, not an unclaimed one. See the module docs.
#[test]
fn every_covered_step_exists() {
    let lessons = lessons();
    for c in claims() {
        assert!(
            lessons.iter().any(|l| l.id == c.step),
            "path-claims.toml claims about step `{}`, which is not an id in \
             const LESSONS. The lesson was renamed or removed and the claim was left \
             behind.",
            c.step
        );
    }
    for arm in arms() {
        assert!(
            lessons.iter().any(|l| l.id == arm.step),
            "path-claims.toml declares the arm `{}` on step `{}`, which is not an id in \
             const LESSONS.",
            arm.name,
            arm.step
        );
    }
}

/// An arm is a change the *reader* makes, so the step must be the thing that asks for it.
///
/// This is the only non-circular content the whole mechanism has. How far an arm runs is
/// this file's own choice (see [`Action::Run`] and the module docs), so the value and
/// display checks on an arm claim are worth exactly as much as the trajectory being one a
/// reader would actually produce.
///
/// The leg this grew out of asserted three things: the sentence is in the prose, the
/// current is in the sentence, and the leg is not simulated for nothing. An arm carries
/// three controls rather than one, and **that is where a leg's fence would have gone
/// slack**: the instruction only had to exist, so an arm citing "uncheck the BMS" could
/// also have moved `dt` and run at a different current with nothing to say so. Every
/// override therefore needs its own anchor in the sentence, and every override has to be a
/// real change from what the step configures — a `bms = true` beside a step that already
/// has one is a declaration with no fact under it, which is the shape `tol_from` exists to
/// catch one level down.
#[test]
fn every_arm_is_instructed_by_its_own_step() {
    let lessons = lessons();
    let all = claims();
    let arms = arms();
    for arm in &arms {
        let lesson = lessons
            .iter()
            .find(|l| l.id == arm.step)
            .unwrap_or_else(|| panic!("no lesson `{}`", arm.step));

        assert!(
            ascii_minus(&lesson.text).contains(&ascii_minus(&arm.instruction)),
            "step `{}` declares the arm `{}`, instructed by:\n  {}\nand that sentence is \
             not in the step's own prose.\n\
             Either the prose was reworded and the arm was left behind, or this harness \
             is now running an arm no reader is told to run — which would make every \
             claim on it true of a trajectory nobody sees.",
            arm.step,
            arm.name,
            arm.instruction
        );

        let instruction = ascii_minus(&arm.instruction);

        // The pack's lesson, which is this step's unless the arm walks next door. Every
        // "an override that changes nothing" fence below compares against THIS lesson and
        // not against the declaring step: what the reader is holding when they type is the
        // other lesson's controls, so that is what an override has to differ from. Written
        // the correct way round rather than the reachable way round — the one twin arm in
        // the file overrides only the demand box, so no other branch can tell the two
        // lessons apart today.
        let pack_lesson = pack_lesson_of(Some(arm), lesson, &lessons);
        assert_walkable(arm, lesson, pack_lesson);

        if let Some(demand_a) = arm.demand_a {
            let spelled = Arm::spelled(demand_a);
            assert!(
                contains_number(&instruction, &spelled),
                "arm `{}` on step `{}` types {demand_a} A into the demand box, but \
                 `{spelled}` does not appear as a number in the instruction it claims to \
                 be following:\n  {}\n\
                 The current and the sentence that tells the reader to type it are two \
                 statements of one fact; this is the check that keeps them one. Note that \
                 a sign difference fails here too — see `contains_number`.",
                arm.name,
                arm.step,
                arm.instruction
            );
        }

        check_cc_cv_current(arm, pack_lesson, &instruction);
        check_pulse_current(arm, pack_lesson, &instruction);

        if let Some(dt) = arm.dt {
            let spelled = Arm::spelled(dt);
            assert!(
                contains_number(&instruction, &spelled),
                "arm `{}` on step `{}` runs at dt = {dt} s, and `{spelled}` does not \
                 appear as a number in its instruction:\n  {}\n\
                 A step length is not a detail here — it is the quantity step 18's own \
                 headline is about, and every number on this arm scales with it.",
                arm.name,
                arm.step,
                arm.instruction
            );
            assert!(
                (dt - pack_lesson.dt).abs() > f64::EPSILON,
                "arm `{}` on step `{}` declares dt = {dt} s, which is what the lesson its \
                 pack comes from (`{}`) already sets. An override that changes nothing is \
                 a control the reader was never asked to touch.",
                arm.name,
                arm.step,
                pack_lesson.id
            );
            assert!(
                arm.start == Start::Restart,
                "arm `{}` on step `{}` changes dt on a continuation. **This is a scoping \
                 refusal, not a fidelity one**, and the distinction matters to whoever \
                 reads it next: the page's `dt` box is read fresh on every frame, so a \
                 mid-run change is perfectly reachable and this harness could model it. \
                 What it would cost is that every number on the arm becomes a function of \
                 how far the reader had already got when they typed — and no step in the \
                 path instructs one. Step 18, the only step that changes `dt` at all, \
                 tells the reader to press **Restart**. Relaxing this is a decision to \
                 support a control change nobody is asked to make, not a bug fix.",
                arm.name,
                arm.step
            );
        }

        if let Some(bms) = arm.bms {
            assert!(
                arm.instruction.contains("BMS"),
                "arm `{}` on step `{}` sets the BMS checkbox to {bms}, and its instruction \
                 does not mention the BMS:\n  {}",
                arm.name,
                arm.step,
                arm.instruction
            );
            let (scenario, _) = load(&pack_lesson.scenario);
            let as_configured = pack_lesson.bms.unwrap_or(scenario.pack.bms.is_some());
            assert!(
                bms != as_configured,
                "arm `{}` on step `{}` sets the BMS to {bms}, which is what `{}` is already \
                 configured with. The arm is then that lesson's own run under a second \
                 name, and its claims say nothing about unchecking anything.",
                arm.name,
                arm.step,
                pack_lesson.id
            );
            assert!(
                arm.start == Start::Restart,
                "arm `{}` on step `{}` toggles the BMS on a continuation. The page cannot \
                 do that: `$(\"bms\").onchange` clicks Reset, so the pack is rebuilt and \
                 the run goes back to t = 0. See `build_with_bms`.",
                arm.name,
                arm.step
            );
        }

        if let Some(fade) = arm.fade_per_ah {
            let spelled = Arm::spelled(fade);
            let word = WORD_NUMERALS
                .iter()
                .find(|(_, v)| (v - fade).abs() < f64::EPSILON)
                .map(|(w, _)| *w);
            assert!(
                contains_number(&instruction, &spelled)
                    || word.is_some_and(|w| contains_number(&instruction, w)),
                "arm `{}` on step `{}` rebuilds the pack with `fade_per_ah = {fade}`, and \
                 neither `{spelled}` nor any word for it appears as a number in the \
                 sentence it claims to be reporting:\n  {}\n\
                 The coefficient and the sentence that tells a reader what it was set to \
                 are two statements of one fact. A counterfactual whose value is not in \
                 the prose is the worst case of all - nothing a reader sees would move if \
                 it changed.",
                arm.name,
                arm.step,
                arm.instruction
            );
            let (_, chem) = load(&pack_lesson.scenario);
            assert!(
                (fade - chem.reversal.fade_per_ah).abs() > f64::EPSILON,
                "arm `{}` on step `{}` declares `fade_per_ah = {fade}`, which is what \
                 `{}`'s own chemistry file already says. The arm is then that lesson's run \
                 under a second name, and the subtraction its claims are for comes to zero \
                 while every check stays green.",
                arm.name,
                arm.step,
                pack_lesson.id
            );
            assert!(
                arm.start == Start::Restart,
                "arm `{}` on step `{}` changes a chemistry coefficient on a continuation. \
                 There is no such trajectory: the coefficient is read when the pack is \
                 BUILT, so a pack that reached the mark under the shipped value cannot \
                 have been running under this one. Unlike the `dt` fence next door this is \
                 a fidelity refusal and not a scoping one - the page cannot do it and \
                 neither can the engine.",
                arm.name,
                arm.step
            );
            assert!(
                arm.pack_from.is_none(),
                "arm `{}` on step `{}` combines a chemistry override with `pack_from`. \
                 Each of those is an unusual thing for one arm to do and no sentence in \
                 the path asks for both at once; composed, what the reader is told they \
                 are looking at would be two removes from the step they are on.",
                arm.name,
                arm.step
            );
        }

        if let Some(ambient_c) = arm.ambient_c {
            let spelled = Arm::spelled(ambient_c);
            assert!(
                contains_number(&instruction, &spelled),
                "arm `{}` on step `{}` drags the ambient slider to {ambient_c} °C, and \
                 `{spelled}` does not appear as a number in its instruction:\n  {}\n\
                 Note that the prose writes a negative temperature with a typographic \
                 minus; `instruction` is normalised through `ascii_minus` before this \
                 test, so the arm spells `-5` and the sentence may say `−5`.",
                arm.name,
                arm.step,
                arm.instruction
            );
            assert!(
                (ambient_c - pack_lesson.ambient_c).abs() > f64::EPSILON,
                "arm `{}` on step `{}` declares an ambient of {ambient_c} °C, which is \
                 where `{}` already leaves the slider. An override that changes nothing is \
                 a control the reader was never asked to touch.",
                arm.name,
                arm.step,
                pack_lesson.id
            );
            // No `Start::Restart` fence here, and its removal is this slice's. It stood as
            // a scoping refusal and said so — `$("ambient").oninput` calls `applyEnv` and
            // rebuilds nothing, so unlike the BMS checkbox the page really can drag this
            // mid-run — and what it was scoping around was `run` keeping ONE environment
            // for the whole trajectory. `run` now keeps two, split at the mark, so step 8's
            // "raise the ambient slider to 45 °C and press Run" is a trajectory this file
            // can produce. See `docs/plans/path-derived-arm.md`.
        }

        // An arm has to assert something. A claim is the usual way; being one half of an
        // `identical_to` pair is the other, and it is not a loophole — that pair asserts
        // the sentence no quantity can state, and both halves are needed to state it. What
        // this still refuses is the arm that neither carries a claim nor is compared to
        // anything, which is a longer simulation that looks like coverage.
        let read_by_a_claim = all
            .iter()
            .any(|c| c.step == arm.step && c.arm.as_deref() == Some(arm.name.as_str()));
        let in_an_identity_pair = arm.identical_to.is_some()
            || arms
                .iter()
                .any(|a| a.step == arm.step && a.identical_to.as_deref() == Some(&arm.name));
        assert!(
            read_by_a_claim || in_an_identity_pair,
            "step `{}` declares the arm `{}`, and nothing reads it: no claim names it and \
             no other arm is compared against it.",
            arm.step,
            arm.name
        );

        if let Some(twin) = arm.identical_to.as_deref() {
            assert!(
                arms.iter().any(|a| a.step == arm.step && a.name == twin),
                "arm `{}` on step `{}` says it ends identical to `{twin}`, and that step \
                 declares no such arm.",
                arm.name,
                arm.step
            );
            assert_ne!(
                twin, arm.name,
                "arm `{}` on step `{}` says it ends identical to itself.",
                arm.name, arm.step
            );
        }
    }
}

/// The one thing step 18 claims that is not a quantity: two button orders, one pack.
///
/// > Press those same two buttons in the other order, still without running, and you get an
/// > identical pack: the move you cannot take back is the Run.
///
/// No `quantity` states that. Reading it as "the voltage matches" would be the weaker
/// sentence and a picked one — the reader is told the *pack* is the same, and the way to
/// assert that is over everything the snapshot carries, RNG included.
///
/// It is also the assertion that would fail loudest if the two clear buttons ever stopped
/// commuting, which is a property of `sim-core` rather than of the page: `clear_faults` and
/// `clear_bms_fault` touch different state today, and nothing but this says they must.
#[test]
fn every_identical_arm_really_is_identical() {
    let lessons = lessons();
    let arms = arms();
    for arm in &arms {
        let Some(twin_name) = arm.identical_to.as_deref() else {
            continue;
        };
        let lesson = lessons
            .iter()
            .find(|l| l.id == arm.step)
            .unwrap_or_else(|| panic!("no lesson `{}`", arm.step));
        let twin = arms
            .iter()
            .find(|a| a.step == arm.step && a.name == twin_name)
            .unwrap_or_else(|| panic!("no arm `{twin_name}` on step `{}`", arm.step));

        let mine = run(lesson, Some(arm), &[], &lessons);
        let theirs = run(lesson, Some(twin), &[], &lessons);
        assert_eq!(
            mine.end_snapshot, theirs.end_snapshot,
            "step `{}`: arms `{}` and `{twin_name}` are declared to end on an identical \
             pack, and their snapshots differ.\n\
             The prose says the order of these buttons does not matter. Either it now \
             does — in which case the sentence is wrong and so is the teaching point about \
             what you cannot take back — or the two arms differ in something other than \
             the order, which makes this assertion say nothing.",
            arm.step, arm.name
        );
    }
}

/// The tolerance rule, enforced rather than written down.
///
/// `tol` is what decides how much of a claim is actually claimed, and until this test it
/// was the one field in the file nothing checked: a careless value makes the value check
/// pass on anything, and the only thing standing behind it was a sentence in the claim's
/// own `note`. That sentence went wrong twice in two commits — `04933c5` and `a1b0945`
/// both fixed a tolerance whose note cited "half a unit in the last printed place" while
/// holding a half-step — and the second time it happened on the very slice fixing the
/// first. A rule that is re-derived by hand for each claim is a rule that is sometimes
/// not derived.
///
/// The three variants are [`TolFrom`]. What each one asserts:
///
/// * `spelled` — `tol` is exactly the rule. Nothing else to say.
/// * `tighter` — `tol` is strictly under the rule. No upper bound is needed and none is
///   given: a tolerance smaller than the rule can only make this test redder, never
///   greener, so the hazard this whole test exists for does not live on that side.
/// * `grid` — three fences, because this is the variant that could have re-blessed the
///   defect it was written after. The quantity must be one the engine can only report on
///   the step grid; the claim must spell nothing (a half-declaration is rejected the way
///   [`Claim::display_claim`] rejects half a display claim); and **no number in the
///   literal may be printed more finely than a half-step**. That last one is the fence
///   that matters. `**383.0 s later**` is a grid time by quantity and would sail through
///   the first two, and a half-step there is five times looser than the tenth the
///   sentence prints — which is precisely the shape `a1b0945` corrected. Under this
///   check it is not eligible for `grid` at all.
///
/// Unit-blind on that last fence, and deliberately so: `The cell empties at 4146.5 s at
/// 1.9290 V` spells a voltage as well as a time, and comparing 0.25 s against 5e-5 V is
/// not a comparison. It errs toward making the author name the number — which is the
/// right error, and on that claim the number it forces them to name is the binding one.
#[test]
fn every_tolerance_follows_its_declared_rule() {
    let lessons = lessons();
    for c in claims() {
        assert!(
            c.tol > 0.0,
            "claim `{}` on step `{}` has tol = {}. A non-positive tolerance is a claim \
             that can only pass by exact float equality, or one that cannot pass at all.",
            c.literal,
            c.step,
            c.tol
        );

        match c.tol_from {
            TolFrom::Spelled => {
                let rule = c.spelled_rule_tol();
                let spells = c.spells.as_deref().expect("checked in spelled_rule_tol");
                assert!(
                    contains_number(&ascii_minus(&c.literal), &ascii_minus(spells)),
                    "claim `{}` on step `{}` says it spells `{spells}`, and that number is \
                     not in its own literal.\n\
                     The sentence was reworded, or the claim was copied from a sibling and \
                     its `spells` came with it. Either way the tolerance below is derived \
                     from a number this claim is not about.",
                    c.literal,
                    c.step
                );
                assert!(
                    tol_eq(c.tol, rule),
                    "claim `{}` on step `{}`:\n  spells `{spells}` (pow10 {})\n  the rule \
                     gives {rule:.3e} — half a unit in that number's last printed place\n  \
                     the file says {:.3e}  ({:.3}x)\n\
                     A tolerance and a rule that disagree is the defect 04933c5 and \
                     a1b0945 both fixed. Set `tol` to the rule; if the claim needs to be \
                     pinned harder than the sentence, say `tol_from = \"tighter\"`; if it \
                     needs to be looser, the sentence is printing more precision than the \
                     engine has and the sentence is what should change.",
                    c.literal,
                    c.step,
                    c.spells_pow10,
                    c.tol,
                    c.tol / rule
                );
            }
            TolFrom::Tighter => {
                let rule = c.spelled_rule_tol();
                let spells = c.spells.as_deref().expect("checked in spelled_rule_tol");
                assert!(
                    contains_number(&ascii_minus(&c.literal), &ascii_minus(spells)),
                    "claim `{}` on step `{}` says it spells `{spells}`, and that number is \
                     not in its own literal.",
                    c.literal,
                    c.step
                );
                assert!(
                    c.tol < rule && !tol_eq(c.tol, rule),
                    "claim `{}` on step `{}` is marked `tighter` and is not: it spells \
                     `{spells}`, whose rule is {rule:.3e}, and holds {:.3e}.\n\
                     `tighter` is the variant that needs no upper bound because it can \
                     only redden this test. A tolerance at or above the rule is the \
                     ordinary case — mark it `spelled` and set it to the rule.",
                    c.literal,
                    c.step,
                    c.tol
                );
            }
            TolFrom::Grid => {
                let lesson = lessons
                    .iter()
                    .find(|l| l.id == c.step)
                    .unwrap_or_else(|| panic!("no lesson `{}`", c.step));
                assert!(
                    c.spells.is_none() && c.spells_pow10 == 0,
                    "claim `{}` on step `{}` is marked `grid` and also spells a number \
                     ({:?}, pow10 {}).\n\
                     `grid` means the prose gives no number in this quantity. If it does \
                     give one, that number sets the tolerance — the whole point of the \
                     split is that a spelled number outranks the step grid.",
                    c.literal,
                    c.step,
                    c.spells,
                    c.spells_pow10
                );
                // The four names the engine answers on the step grid, and the list is
                // deliberately a list: every one of them returns the `t_s` of a ROW, so
                // half a step is the tightest bound that means anything, and a quantity
                // whose value is interpolated or continuous has no business here.
                let grid_quantity = c.quantity.starts_with("flag_first_s:")
                    || c.quantity.starts_with("deficit_falls_below_pts:")
                    || c.quantity == "deficit_zero_s"
                    || c.quantity == "v_floor_s";
                assert!(
                    grid_quantity,
                    "claim `{}` on step `{}` is marked `grid`, and `{}` is not a quantity \
                     the engine reports on the step grid.\n\
                     Half a timestep is only a meaningful bound for a time the engine can \
                     land on exactly. For anything continuous it is a number with no \
                     justification at all.",
                    c.literal, c.step, c.quantity
                );
                let half_step = lesson.dt / 2.0;
                assert!(
                    tol_eq(c.tol, half_step),
                    "claim `{}` on step `{}` is marked `grid` and holds {:.3e}; the step \
                     runs at dt = {} s, so half a step is {half_step:.3e}.",
                    c.literal,
                    c.step,
                    c.tol,
                    lesson.dt
                );
                for token in numeric_tokens(&c.literal) {
                    let implied = 5.0 * 10f64.powi(-(decimals_of(&token) + 1));
                    assert!(
                        implied > c.tol || tol_eq(implied, c.tol),
                        "claim `{}` on step `{}` is marked `grid` and holds a half-step of \
                         {:.3e}, but its own literal prints `{token}` — half a unit in \
                         that number's last place is {implied:.3e}, which is tighter.\n\
                         A sentence printing that finely is a sentence making a finer \
                         claim than a half-step, and taking the half-step throws the \
                         difference away. This is the fence that keeps `grid` from \
                         re-blessing the `383.0 s later` defect: name the number with \
                         `spells` and take its rule.",
                        c.literal,
                        c.step,
                        c.tol
                    );
                }
            }
        }
    }
}

/// Every word in [`WORD_NUMERALS`] is read by something — a claim that spells it, or a
/// ledger rule that derives a number from it.
///
/// The table is a translation from English to a number, and a translation nothing consults
/// is coverage-shaped: it reads as "this file understands written numbers" while the one
/// word it was added for could have been deleted from the claim beside it. Same guard, same
/// argument, as [`every_ledger_rule_is_a_phrase_and_is_used`] keeps over the ledger's
/// vocabulary — and the same history behind it, `CCCV_PERIOD_S` sitting pinned and unread
/// for six slices while the mirror it was meant to guard was wrong.
///
/// **Two readers now, and deliberately one table.** `spells` reads a word because a claim's
/// sentence writes its quantity in letters; [`Operand::Word`] reads one because a
/// derivation's operand is written that way. One table is what stops a word meaning
/// different numbers on the two sides — the same argument the header's own vocabulary is
/// held to in [`every_count_these_files_state_about_themselves_is_derived`].
#[test]
fn every_word_numeral_is_read_by_something() {
    let all = claims();
    for (word, value) in WORD_NUMERALS {
        let spelled = all.iter().any(|c| c.spells.as_deref() == Some(*word));
        let derived = LEDGER_VOCABULARY.iter().any(|rule| {
            rule.ties.iter().any(|tie| match tie {
                Tie::Derived { operands, .. } => operands
                    .iter()
                    .any(|op| matches!(op, Operand::Word(w) if w == word)),
                _ => false,
            })
        });
        // The third reader, and the one that made this a list rather than a pair: an ARM
        // whose override is spelled in letters. `Arm::fade_per_ah` is held to the same
        // rule as the demand box - the value must be in the instruction - and step 21s
        // sentence writes its value as "zero". Read here through the same table for the
        // same reason the other two are: a word means one number in this file however it
        // is reached.
        let instructed = arms().iter().any(|a| {
            a.fade_per_ah
                .is_some_and(|f| (f - *value).abs() < f64::EPSILON)
                && contains_number(&ascii_minus(&a.instruction), word)
        });
        assert!(
            spelled || derived || instructed,
            "WORD_NUMERALS translates `{word}` to {value} and nothing in \
             web/path-claims.toml spells it, no ledger rule derives from it and no \
             arm's instruction spells an override with it. Either the \
             sentence it was added for was reworded — in which case its claim is failing \
             elsewhere and this entry is why that is hard to see — or the word was never \
             used. Add words when a claim or a rule needs them; a table read by nothing is \
             the `CCCV_PERIOD_S` shape."
        );
    }
}

/// Check 5 — the number the sentence prints is the number the engine produced.
///
/// The join between the literal check and the value check, which until this existed were
/// two green halves with nothing between them. See [`States`] for what each frame means
/// and why a formatter could not have done this job.
///
/// Three things are worth being explicit about.
///
/// **The tolerance here is the sentence's, not the claim's.** `tol` bounds engine against
/// `value`; this bounds `value` against the prose, and the right bound for that is half a
/// unit in the last printed place of what the prose printed — [`Claim::spelled_rule_tol`],
/// the same rule [`every_tolerance_follows_its_declared_rule`] enforces from the other
/// side. A `tighter` claim is deliberately pinned harder than its sentence; asking the
/// sentence to meet that would fail every hedge in the file (`just under 14 A` is 13.82).
///
/// **The comparison forgives the last bit, and today that arm decides nothing.** Two
/// claims land exactly on their rounding boundary: `3.6357` against a measured 3.63565,
/// where the subtraction comes out 4.99999999998e-5 against a rule of 5e-5, and `the last
/// 53 seconds` against a flag 53.5 s before a mark at 4200, where the difference is 0.5
/// against a rule of 0.5. Both are inside `diff <= rule` as the bits actually fell — the
/// first just under, the second by exact equality — so `||` short-circuits and the
/// `tol_eq` arm is never reached on any of the 49. None of the perturbations written
/// against this check reaches it either. It is there because which side of `<=` a
/// boundary case lands on is a fact about binary and not about the prose, and a claim
/// should not go red on a rounding of its own rule; a green suite is not evidence that it
/// works. Said plainly rather than left to be inferred, on the same terms as `quoted`
/// above: a check that catches nothing today reads as a covering one.
///
/// **No engine is run.** Every frame here is arithmetic on `value` and, for the two
/// duration frames, on the step's scraped `until_s`. A prose defect should not fail behind
/// step 8's 400 000 engine steps.
#[test]
fn every_claim_states_the_value_it_measures() {
    let lessons = lessons();
    let all = claims();
    let arms = arms();
    for c in &all {
        let lesson = lessons
            .iter()
            .find(|l| l.id == c.step)
            .unwrap_or_else(|| panic!("no lesson `{}`", c.step));

        // The two variants that state no number of their own forbid `spells` outright.
        // Without that, the claims which could be read either way — `0.1 %` is both the
        // sentence's figure and the row's rendering — would be the author's pick rather
        // than the sentence's, which is the freedom this whole check exists to remove.
        if matches!(c.states, States::Displayed | States::Nothing) {
            assert!(
                c.spells.is_none() && c.spells_pow10 == 0,
                "claim `{}` on step `{}` states `{:?}` and also spells a number ({:?}, \
                 pow10 {}).\n\
                 Those two variants mean the sentence prints no figure of its own in this \
                 quantity. If it does print one, that figure is what this claim is about \
                 — use the frame it is printed in.",
                c.literal,
                c.step,
                c.states,
                c.spells,
                c.spells_pow10
            );
        }

        match c.states {
            States::Nothing => {
                let tokens = numeric_tokens(&c.literal);
                assert!(
                    tokens.is_empty(),
                    "claim `{}` on step `{}` states `nothing`, and its own literal prints \
                     {tokens:?}.\n\
                     `nothing` is the only variant that ties the prose to the engine by \
                     saying there is nothing to tie, so it is the one place a careless \
                     claim could re-open the hole this check closes. A literal with a \
                     digit in it has a figure a reader will read as this quantity — name \
                     its frame.",
                    c.literal,
                    c.step
                );
            }
            States::Displayed => {
                let (row, shows) = c.display_claim().unwrap_or_else(|| {
                    panic!(
                        "claim `{}` on step `{}` states `displayed` and names no readout \
                         row. The row *is* the tie: there is no arithmetic here, only the \
                         chain from the sentence to the formatter.",
                        c.literal, c.step
                    )
                });
                assert!(
                    c.quoted,
                    "claim `{}` on step `{}` states `displayed` and is not `quoted`. The \
                     chain this variant asserts runs literal ⊇ shows == the rendering of \
                     the measured value; without `quoted` the middle link is missing and \
                     the sentence is tied to nothing.",
                    c.literal, c.step
                );
                assert!(
                    ascii_minus(&c.literal).contains(&ascii_minus(shows)),
                    "claim `{}` on step `{}` states `displayed`, and the `{row}` row's \
                     string `{shows}` is not inside that literal.\n\
                     `quoted` only asks for it somewhere in the step's prose, which on a \
                     step with several rows can be a different sentence entirely. This \
                     variant needs it in *this* claim's own sentence, because that \
                     sentence is what it is standing in for.",
                    c.literal,
                    c.step
                );
            }
            // Everything else compares a number to a number, in `value`'s unit.
            _ => {
                let stated = c.spelled_value();
                let rule = c.spelled_rule_tol();
                let mapped = match c.states {
                    States::Same | States::Departure => c.value,
                    States::Magnitude => {
                        assert!(
                            c.value < 0.0,
                            "claim `{}` on step `{}` states a `magnitude` of {}, which is \
                             not negative. On a positive value this variant is a silent \
                             alias for `same` — say `same`.",
                            c.literal,
                            c.step,
                            c.value
                        );
                        c.value.abs()
                    }
                    States::Complement => 1.0 - c.value,
                    States::SinceMark => {
                        assert!(
                            reads_past_the_mark(&arms, c),
                            "claim `{}` on step `{}` states a duration `since_mark` and is \
                             not read on an arm that continues past the mark. Before the \
                             mark there is nothing to be later than; on an arm that \
                             restarts the pack the mark is not an origin at all. The two \
                             duration frames are only distinguishable because each is \
                             fenced to one side of it.",
                            c.literal,
                            c.step
                        );
                        c.value - lesson.until_s
                    }
                    States::UntilEnd => {
                        assert!(
                            !reads_past_the_mark(&arms, c),
                            "claim `{}` on step `{}` states a duration `until_end` and is \
                             read past the mark. Past the mark the step has no end to \
                             count down to — `until_s` is where the page stopped, not \
                             where the continuation does.",
                            c.literal,
                            c.step
                        );
                        lesson.until_s - c.value
                    }
                    States::Displayed | States::Nothing => unreachable!("handled above"),
                };

                let diff = (stated - mapped).abs();
                let within = diff <= rule || tol_eq(diff, rule);

                if c.states == States::Departure {
                    // A claim that asserts a *difference* is satisfied by pointing at any
                    // unrelated figure in the sentence — `t = 10 s` would do — so it is
                    // not allowed to stand alone. The sentence "leaves 100.00 %" is two
                    // statements, and this variant is only the second of them.
                    assert!(
                        !within,
                        "claim `{}` on step `{}` states a `departure` from {stated} and \
                         the value {} is {diff:.3e} away, inside the sentence's own \
                         precision of {rule:.3e}.\n\
                         The sentence says the quantity has left that number and at this \
                         instant it has not — which is a claim about what a reader sees \
                         change, so it failing means the change moved, not that the \
                         tolerance is wrong.",
                        c.literal, c.step, c.value
                    );
                    let vouched = all.iter().any(|o| {
                        o.states == States::Same
                            && o.step == c.step
                            && o.literal == c.literal
                            && o.quantity == c.quantity
                            && o.spells == c.spells
                            && o.spells_pow10 == c.spells_pow10
                            && o.read_at_s < c.read_at_s
                    });
                    assert!(
                        vouched,
                        "claim `{}` on step `{}` states a `departure` from {stated} and no \
                         earlier claim on the same sentence and quantity states `same` \
                         with that number.\n\
                         Alone, this variant asserts only that the engine is *not* some \
                         figure, which any unrelated number in the sentence satisfies. It \
                         is half of a pair: one claim shows the quantity holding the \
                         value, the next shows it gone. The sibling is what makes the \
                         number the sentence's rather than the author's.",
                        c.literal, c.step
                    );
                } else {
                    assert!(
                        within,
                        "step `{}`, claim `{}`:\n  the sentence spells {stated} (as \
                         `{}`, pow10 {})\n  read as `{:?}` of the claimed value {}, that \
                         is {mapped}\n  difference {diff:.3e}, and the sentence's own \
                         precision allows {rule:.3e}\n\
                         The prose and the stored value have come apart. This is the check \
                         that stops a re-measured `value` sliding under an unchanged \
                         sentence: if the engine moved, re-word the sentence in web/app.js \
                         and update `literal`, `spells` and `value` together. If the \
                         sentence states this quantity in a different frame than the one \
                         named here, the frame is what is wrong.",
                        c.step,
                        c.literal,
                        c.spells.as_deref().unwrap_or(""),
                        c.spells_pow10,
                        c.states,
                        c.value
                    );
                }
            }
        }
    }
}

/// A sentence doing arithmetic on numbers it prints itself.
///
/// The declaration behind [`Accounted::Derived`], and the last arm of the taxonomy
/// `docs/plans/path-prose-ledger.md` laid out. Step 8's is the file's first and today its
/// only one:
///
/// > 20 K buys 2.84 points over the next 200 000 s against the 1.06 the first leg cost,
/// > about 2.7×.
///
/// **What is declared is the operands and the operation, never the value.** The quotient is
/// recomputed from the tokens the sentence prints and compared at the precision the prose
/// commits to, exactly as [`Tie::Product`] compares `4.61 Ah`, so a row pointed at the wrong
/// operand fails on sight. That is what separates this from the "list of declared
/// identities" the plan refuses: a row cannot supply a number, only name where in the
/// sentence to read one.
///
/// **And every operand must itself be accounted by one of the other arms**
/// ([`every_derivation_is_a_sentence_doing_arithmetic`]). Without that clause the arm says
/// only that two unpinned numbers divide into a third, which is a circle rather than a tie.
#[derive(Debug, serde::Deserialize)]
struct Derivation {
    /// The lesson whose prose this sentence is in.
    step: String,
    /// The sentence, quoted exactly as [`Claim::literal`] quotes one — and it must be a
    /// sentence some claim on this step quotes, or the operands would have no accounting to
    /// inherit.
    literal: String,
    /// The number this row accounts for, written as the sentence writes it.
    spells: String,
    /// What the sentence does with the operands.
    op: Op,
    /// The operands, in order, each written as the sentence writes it. Every one must be a
    /// number the sentence prints.
    from: Vec<String>,
    #[allow(
        dead_code,
        reason = "authoring context for a human reader, not asserted"
    )]
    note: String,
}

/// The arithmetic a [`Derivation`] performs.
///
/// One variant, because one sentence in the path prints one. A sum or a difference gets
/// built the day a sentence needs it — an operation nothing performs would be the
/// `CCCV_PERIOD_S` shape this file has already been caught by once: pinned, and for six
/// slices consulted by nothing. (It has a reader now — see [`cccv_period_s`] — which does
/// not retire the lesson, only the example's present tense.)
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Op {
    /// The first operand divided by the second — `2.84 / 1.06`, printed `2.7×`.
    Ratio,
}

impl Op {
    /// The value, or `None` if the operands make it undefined.
    fn apply(self, from: &[f64]) -> Option<f64> {
        match self {
            Op::Ratio => match from {
                [a, b] if *b != 0.0 => Some(a / b),
                _ => None,
            },
        }
    }

    /// How many operands it takes.
    fn arity(self) -> usize {
        match self {
            Op::Ratio => 2,
        }
    }
}

/// How a number printed inside a claimed literal is accounted for.
///
/// Derived, never declared, and that is the whole of the design. The three other fields
/// this file added to close a hole — `tol_from`, `states`, `spells` — are declarations,
/// because each encodes something a machine cannot decide: which rule an author meant,
/// which frame a sentence uses. Every variant here is an exact numeric fact about the
/// claim beside the token, so a declared `accounts = "read_at"` sitting beside a token
/// that is really something else would be a fresh instance of the defect `tol_from` was
/// introduced to kill — a claim citing a rule it does not follow. The test tries all
/// four and names the ones that failed.
///
/// **The fourth arm is [`Self::Setting`], and it arrived the way this doc said it would.**
/// What stood here was that there were three arms, that no literal in the file needed a
/// fourth, and that "a future literal printing a chemistry constant or a control setting
/// will fail here loudly — and the right answer will be to give it an arm that checks it
/// against whatever decides it, not a waiver." Step 18's headline is that literal: it
/// prints three step lengths beside three measurements, and `docs/plans/path-arms.md`
/// established that this check, not the harness, was what stopped the sentence being
/// claimed. `Setting` is the arm, and like the other three it decides nothing by
/// declaration — it ties the token to the step length of a trajectory a claim on the same
/// sentence is really measured on.
///
/// There is still deliberately no variant meaning "this number is not a measurement". An
/// escape hatch is exactly what re-opens the hole this check closes: a number a reader is
/// shown, inside a sentence this file already claims, must be tied to *something*. A
/// literal printing a chemistry constant still fails here loudly, and the answer is still
/// an arm that checks it against the chemistry file — `docs/plans/path-prose-ledger.md`
/// sizes that one and the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Accounted {
    /// A claim on this sentence names it in `spells`, so checks 5 and 6 already tie it to
    /// the engine. The sign is ignored on both sides: `numeric_tokens` does not collect
    /// one, and `**−0.0640 V**` spells `-0.0640` of a negative value.
    Spelled,
    /// It is the instant a claim on this sentence is *read at*, written in that claim's
    /// own frame — absolute before the mark, since the mark on a leg. `99.45 % at 250 s`
    /// prints the health and the moment, and the moment is the claim's `read_at_s`. The
    /// payload is that instant made absolute again, for the fence below.
    ///
    /// **Both readings of one instant are accepted on a continuation, and that is a
    /// widening.** It was the since-mark reading alone, on the reasoning that the two
    /// duration frames in [`States`] are fenced to opposite sides of the mark — but that
    /// fence is about the *quantity*, where the two frames give different numbers, and
    /// this is about the moment, where they are one moment written two ways. Step 15's
    /// continuation falsified the assumption behind the narrow version: it writes "2.502 V
    /// at 1058 s", on the clock, because the clock is what a reader watches keep running.
    /// The two readings differ by exactly the mark, so no token can match both, and
    /// [`accounting_for`] asserts the mark is positive rather than assuming it.
    ///
    /// **And fenced against events, which is the arm's real hazard.** This was written
    /// believing that giving `207.5 s` a claim of its own closed the sentence; the
    /// perturbation that deletes that claim came back *green*, because the two other
    /// claims on the sentence are read at 207.5 and this arm accounted the number as
    /// their read instant. "We measured then" is a far weaker statement than "the cell
    /// empties then", and the second is what the sentence tells a reader. So a
    /// `ReadAt` accounting is refused at any instant the run raises a flag it did not
    /// have on the step before — see [`Run::flags_arriving_at`] and the fence in
    /// [`every_claim_matches_the_engine`], which is the only place a run exists.
    ReadAt(f64),
    /// It is inside a string this file already asserts the panel prints — a claim's
    /// `shows`, which the display check compares against the mirrored formatter.
    ///
    /// One extra instant is available to a `displayed` claim read on a leg: the clock's
    /// rendering of the mark, which is where the leg begins and therefore the one other
    /// moment such a sentence can be speaking about. ``it goes from `10m` to `16m``
    /// prints both ends. Only the `sim time` row qualifies, because it is the only row
    /// that is a function of time alone — every other one would need telemetry, and this
    /// check runs no engine for the reason [`every_claim_states_the_value_it_measures`]
    /// runs none: a prose defect should not fail from behind step 8's 400 000 steps.
    Shown,
    /// It is a **control the reader dials in**, and the trajectory it produces is one that
    /// a claim on this sentence is read on. Step 18's headline is the sentence this was
    /// written for and today the only one that needs it:
    ///
    /// > 0.56 points at 0.5 s, **5.57 at 5 s**, 11.14 at 10 s, where the cell ends 19 K
    /// > hotter instead of 1.
    ///
    /// Five of those eight numbers are measurements and three are `dt` — the step length
    /// the reader types into the box. A step length is not read at, not shown by any row
    /// and not spelled by any claim, so before this arm existed the sentence could not be
    /// claimed at all without leaving three of its numbers tied to nothing. That is the
    /// blocker `docs/plans/path-arms.md` found had moved onto this check once both
    /// trajectories existed, and it is what this closes.
    ///
    /// **The derivation is what makes it an arm rather than a waiver, and the generous
    /// version is a real trap.** Step 18's lesson block also carries `speed_x: 10`. An arm
    /// that accounted a token against *any* numeric field of the block would tie this
    /// sentence's `10` to the speed multiplier — the right answer off the wrong field,
    /// green, and still green the day one of the two moves. It is the same defect
    /// [`LedgerRule`] refuses when it insists a rule name its field rather than search
    /// the file.
    ///
    /// **That trap is reasoned rather than measured**, and the distinction is this file's own.
    /// It used to rest on an absence — "[`Lesson`] does not scrape `speed_x` at all, so the
    /// generous version cannot be built, or perturbed into existence, without adding the field
    /// first" — and that absence is gone: the ledger's [`Control::Speed`] reads `speed_x` now,
    /// because step 8's prose prints it. What holds the trap shut is the design rather than the
    /// missing field: the tie below is to a **trajectory**, and no trajectory has a speed. A
    /// generous version is now buildable and would be a rewrite of this arm rather than a
    /// slip. What *was* measured is the weaker half of the same property: tying this arm to the
    /// step's own `dt` instead of to each claim's trajectory leaves the headline's `5` and `10`
    /// unaccounted and reddens check 6 by name.
    ///
    /// So the tie is to a **trajectory**: the token must equal the step length of a run
    /// that a claim in this sentence group actually reads — the step's own `dt` for a
    /// claim with no arm, that arm's `dt` for a claim with one. `speed_x` is then
    /// unreachable by construction, and an author cannot declare a setting without
    /// building the arm whose numbers it produces. The payload is that step length \[s\].
    ///
    /// **A token that is also spelled, read at, or shown is refused rather than resolved.**
    /// See [`accounting_for`]: two readings of one number is the hazard the `ReadAt` fence
    /// and `cover_by_rule`'s double-cover panic both exist for, and a new arm is exactly
    /// where it would come back.
    ///
    /// **Demand and the mark are deliberately not here**, and the ambient arrived the way
    /// this doc said it would. What stood here was that `dt` was the only control an arm can
    /// override that a sentence in this path also prints, and that "the next sentence that
    /// prints an ambient temperature is where that arm gets built". Step 8's is that
    /// sentence — *"20 K buys 2.84 points"* — and it prints the **step** rather than the
    /// level: the 45 °C the reader dials in against the 25 the slider was already on. So
    /// that is what the arm reads, and a sentence printing an ambient *level* still fails
    /// here loudly, because nothing in the path prints one and an arm for it would be what
    /// `CCCV_PERIOD_S` was for six slices: pinned, and consulted by nothing.
    ///
    /// The two readings are fenced against each other rather than ordered: a step whose `dt`
    /// happened to equal its arm's ambient step would hand the token whichever was tried
    /// first. See the assert in [`accounting_without_arithmetic`].
    Setting(f64),
    /// It is the sentence's **own arithmetic over numbers it prints itself** — `2.7×` from
    /// the `2.84` and the `1.06` beside it. See [`Derivation`], which is where the operands
    /// and the operation are declared, and where the value never is.
    ///
    /// The payload is the value recomputed from the operands, which is what the caller
    /// compares against the token.
    Derived(f64),
}

/// Every claim on one sentence — a `(step, literal)` pair — in file order.
fn sentence_group<'a>(all: &'a [Claim], step: &str, literal: &str) -> Vec<&'a Claim> {
    all.iter()
        .filter(|c| c.step == step && c.literal == literal)
        .collect()
}

/// Every distinct claimed sentence in the file.
fn sentences(all: &[Claim]) -> Vec<(&str, &str)> {
    let mut out: Vec<(&str, &str)> = all
        .iter()
        .map(|c| (c.step.as_str(), c.literal.as_str()))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// How this sentence's claims account for one number printed in it, without the sentence's
/// own arithmetic — the four arms that need no other token to be settled first.
///
/// Split from [`accounting_for`] because [`Accounted::Derived`] rests on its operands having
/// an accounting of their own, and a `Derived` operand would have no floor. Calling this on
/// the operands is what makes the recursion one level deep by construction rather than by a
/// depth counter.
/// One number **as the sentence writes it**, which is what every arm of check 6 reads.
///
/// Four fields that had been four parameters, and they travel together because they are one
/// thing: the characters, the unit the words put it in, the scale that unit is in seconds,
/// and whether the sentence spelled it in letters at all. Three of the four are consulted by
/// a *different* arm, and the fourth parameter arrived the day one of those arms turned out
/// to be answering about a quantity that was not a duration.
struct Reading<'a> {
    /// The number as a decimal string — the source characters for a digit, and what the
    /// words come to for a spelled one. See [`Written::token`].
    token: &'a str,
    /// See [`Written::scale`].
    scale: f64,
    /// See [`Written::unit`].
    unit: &'static str,
    /// Whether the sentence writes it in letters. The arms that compare *characters* are
    /// asked only about digits; see [`accounting_without_arithmetic`].
    in_words: bool,
}

impl<'a> Reading<'a> {
    /// A number the sentence writes in digits — no unit of its own, nothing to convert.
    fn digits(token: &'a str) -> Self {
        Self {
            token,
            scale: 1.0,
            unit: "",
            in_words: false,
        }
    }
}

impl<'a> From<&'a Written> for Reading<'a> {
    fn from(w: &'a Written) -> Self {
        Self {
            token: &w.token,
            scale: w.scale,
            unit: w.unit,
            // A spelled quantity's token is derived from its words, so it differs from the
            // source text; a digit's is the source text.
            in_words: w.token != w.phrase,
        }
    }
}

fn accounting_without_arithmetic(
    read: &Reading,
    group: &[&Claim],
    lesson: &Lesson,
    arms: &[Arm],
) -> Option<Accounted> {
    let (token, unit, unit_name, in_words) = (read.token, read.scale, read.unit, read.in_words);
    // The two arms below that compare *numbers* — the step length, and the instant a claim
    // is read at — are both in **seconds**, and a spelled quantity reaches them whatever
    // noun it was read off, because a number needs no digits to be compared. So a sentence
    // saying *"half a percent"* offered them `0.5`, and on a lesson whose step length is
    // 0.5 s one of them took it. See [`Written::unit`]; a number written in digits has an
    // empty unit here and is unaffected.
    let timed = unit_is_time(unit_name);
    // A control one of this sentence's own claims is measured under. Compared as a number
    // rather than as text, so a sentence writing `5.0` where the box takes 5 is the same
    // setting — the token is the reader's, the tie is to the run.
    //
    // Two of them now: the step length, and the **ambient step** an arm dials in against the
    // slider the lesson left. The second is a difference and not a level, because that is
    // what step 8's sentence prints, and °C and K differ by an offset that cancels in one.
    //
    // Taken first although it is tried last, because the three arms below return early and
    // this one has to be able to refuse them. See the fence under `shown`.
    let setting = timed
        .then(|| {
            group.iter().find_map(|c| {
                let arm = arm_of(arms, c);
                let dt = arm.and_then(|a| a.dt).unwrap_or(lesson.dt);
                let ambient_step = arm
                    .and_then(|a| a.ambient_c)
                    .map(|c| c - lesson.ambient_c)
                    .filter(|d| d.abs() > f64::EPSILON);
                let n = number_of(token)? * unit;
                // A step whose two controls landed on one number would hand the token whichever
                // reading was tried first, which is the hazard this whole taxonomy is arranged
                // against. Asserted rather than assumed, because nothing else keeps them apart.
                if let Some(step) = ambient_step {
                    assert!(
                (step.abs() - dt).abs() > 1e-9,
                "step `{}` reads a trajectory whose step length is {dt} s and whose ambient \
                 moves by {step} K. Those are the same number, so a token spelling it has \
                 two readings and this function decides which — not the sentence.",
                lesson.id,
            );
                }
                if (n - dt).abs() < 1e-9 {
                    return Some(dt);
                }
                ambient_step.filter(|step| (n - step.abs()).abs() < 1e-9)
            })
        })
        .flatten();
    // Two readings of one number, which is the hazard this whole taxonomy is arranged
    // against. Refused rather than resolved by trial order: with an order, an author who
    // meant a measurement and wrote a step length gets whichever arm happens to be tried
    // first, and the check becomes a fact about this function.
    let clash = |other: &str| {
        assert!(
            setting.is_none(),
            "step `{}`, sentence `{}`:\n  it prints `{token}`, which is both {other} and \
             a control of a trajectory this sentence's claims read ({}).\n\
             Two readings of one number means the accounting is decided by which arm was \
             tried first rather than by the sentence. Reword the sentence, or split the \
             literal so the two readings sit in different groups.",
            group.first().map_or("", |c| c.step.as_str()),
            group.first().map_or("", |c| c.literal.as_str()),
            setting.unwrap_or(f64::NAN),
        );
    };

    // The arms that compare *characters* — what a row shows, and the clock at the mark —
    // are asked only about a number the sentence writes in digits. A spelled quantity's
    // `token` is a decimal string this file derived from the words (`"24"` for
    // *twenty-four*), and a row printing `24` would then account for it by a coincidence of
    // formatting rather than by anything a reader can see. The arms that compare *numbers*
    // have no such problem and are asked normally.
    //
    // **What a claim `spells` is the exception, and it is an exception because `spells` can
    // itself be a word.** `nothing-to-clamp` carries a claim on the pack's charge loss whose
    // `spells` is `"fifty"` and whose value is 50.3811, written for the sentence *"this
    // fault costs **fifty points**"* — the author named the word, and check 5 holds the
    // measurement to it. Under the blanket refusal above the ledger then reported that same
    // sentence as tied to nothing, and the repair on offer would have been a vocabulary rule
    // re-deriving a number the claim beside it already measures. So a word is compared to a
    // word: the claim's `spells` must be a [`WORD_NUMERALS`] entry whose value is what the
    // scanner read. No formatting coincidence is available, because neither side is a
    // rendering — `"fifty"` matches `fifty` and nothing else.
    //
    // Compared **without** the unit scale, because `spells` is the number as the sentence
    // writes it: a claim spelling `"three"` beside *"three minutes"* is naming the three,
    // not the hundred and eighty.
    let spelled = group.iter().any(|c| {
        let Some(spells) = c
            .spells
            .as_deref()
            .map(|s| ascii_minus(s).trim_start_matches('-').to_string())
        else {
            return false;
        };
        if !in_words {
            return spells == token;
        }
        WORD_NUMERALS.iter().any(|(word, value)| {
            *word == spells && number_of(token).is_some_and(|n| (n - value).abs() < 1e-9)
        })
    });
    if spelled {
        clash("spelled by a claim on it");
        return Some(Accounted::Spelled);
    }

    let read_at = timed
        .then(|| {
            group.iter().find_map(|c| {
                // Both renderings of the one instant. A continuation's are often written since the
                // mark — `383.0 s later` — and step 15's are written on the clock, because the
                // clock is what keeps running while the reader watches: "2.502 V at 1058 s". A
                // restart arm and the step's own run have only the absolute reading.
                //
                // This used to be `since the mark` alone for a continuation, which was an
                // assumption about how prose is written rather than a fact, and step 15's sentence
                // falsifies it. Widening cannot let an author try both until one fits, which is
                // what the single reading was for: the two differ by exactly `until_s`, so no
                // token can match both — asserted below rather than assumed, because the whole
                // widening rests on it. What an author gains is the ability to account a number
                // that is genuinely one of the two true readings of the instant their claim is
                // measured at, which is the arm's whole statement.
                let mut instants = vec![c.read_at_s];
                // And a third reading, on one quantity: **since the current stopped**.
                //
                // A rest is a leg with an origin of its own, exactly as a continuation is, and a
                // sentence about what happens *during* one counts from where it began — step 12's
                // "99.5 % of it has arrived within the first 300 s" is the claim's own 360 s read
                // against a tooth whose current went off at 60.
                //
                // **Restricted to `pulse_rebound_arrived`, and the restriction is the fence.**
                // Every other pulse quantity is read at a leg boundary, where this reading would
                // return the leg length itself — a number the prose writes as the demand program
                // and that has a vocabulary rule of its own. Two readings of one number is what
                // this taxonomy is arranged against, so the frame is given only to the one quantity
                // that is measured at an arbitrary instant inside the rest, which is also the only
                // one whose sentence has any reason to say how far into it we are.
                if let (Some(n), Prog::Pulse { on_s, off_s, .. }) = (
                    c.quantity
                        .strip_prefix("pulse_rebound_arrived:")
                        .and_then(|n| n.parse::<usize>().ok()),
                    lesson.demand,
                ) {
                    let stopped_s = (n.max(1) - 1) as f64 * (on_s + off_s) + on_s;
                    // The same fence the mark carries below: the two readings differ by exactly
                    // this, so no token can match both — asserted, not assumed.
                    assert!(
                        on_s > 0.0,
                        "step `{}` runs a pulse program whose loaded leg is {on_s} s long, so \
                 \"since the current stopped\" and \"on the clock\" are the same number.",
                        lesson.id,
                    );
                    instants.push(c.read_at_s - stopped_s);
                }
                if reads_past_the_mark(arms, c) {
                    assert!(
                lesson.until_s > 0.0,
                "step `{}` has a mark at t = {} s, so a continuation's two readings of one \
                 instant — on the clock, and since the mark — are the same number. The \
                 fence that lets both be accounted is that they cannot be.",
                lesson.id,
                lesson.until_s
            );
                    instants.push(c.read_at_s - lesson.until_s);
                }
                number_of(token)
                    .map(|n| n * unit)
                    .filter(|n| instants.iter().any(|i| (n - i).abs() < 1e-9))
                    .map(|_| c.read_at_s)
            })
        })
        .flatten();
    if let Some(absolute_s) = read_at {
        clash("an instant a claim on it is read at");
        return Some(Accounted::ReadAt(absolute_s));
    }

    let shown = !in_words
        && group.iter().any(|c| {
            let own = c
                .shows
                .as_deref()
                .is_some_and(|s| numeric_tokens(s).iter().any(|t| *t == token));
            // The clock at the mark: a continuation's own origin, for a sentence that quotes
            // the row at both ends of it.
            let at_mark = c.states == States::Displayed
                && reads_past_the_mark(arms, c)
                && c.display.as_deref() == Some("sim time")
                && numeric_tokens(&fmt_time(lesson.until_s))
                    .iter()
                    .any(|t| *t == token);
            own || at_mark
        });
    if shown {
        clash("a number a row this sentence's claims assert prints");
        return Some(Accounted::Shown);
    }

    setting.map(Accounted::Setting)
}

/// How this sentence accounts for one number printed in it, or `None`.
///
/// Shared by check 6, which runs the whole scan without an engine, and by the event fence
/// in [`every_claim_matches_the_engine`], which re-derives it for the one arm that needs a
/// trajectory to be checked. Derived rather than declared: see [`Accounted`].
///
/// The four arms above first, then the sentence's own arithmetic. **A token both arms answer
/// is refused rather than resolved**, which is the same hazard `cover_by_rule`'s
/// double-cover panic exists for: with a trial order, a number that is genuinely a
/// measurement and also happens to be the ratio of two others gets whichever this function
/// tried first, and the check becomes a fact about this function.
fn accounting_for(
    read: &Reading,
    group: &[&Claim],
    lesson: &Lesson,
    arms: &[Arm],
    derived: &[Derivation],
) -> Option<Accounted> {
    let token = read.token;
    let base = accounting_without_arithmetic(read, group, lesson, arms);
    let Some((literal, row)) = group
        .first()
        .map(|c| c.literal.as_str())
        .and_then(|literal| {
            derived
                .iter()
                .find(|d| d.step == group[0].step && d.literal == literal && d.spells == token)
                .map(|row| (literal, row))
        })
    else {
        return base;
    };
    assert!(
        base.is_none(),
        "step `{}`, sentence `{literal}`:\n  it prints `{token}`, which a `[[derived]]` row \
         accounts for as the sentence's own arithmetic — and which {} already accounts for.\n\
         Two readings of one number means the accounting is decided by which arm was tried \
         first rather than by the sentence. Drop the `[[derived]]` row: a number a claim \
         measures is measured, whatever else it also happens to equal.",
        group[0].step,
        match base {
            Some(Accounted::Spelled) => "a claim on it spells it",
            Some(Accounted::ReadAt(_)) => "it is an instant a claim on it is read at",
            Some(Accounted::Shown) => "a row this sentence's claims assert prints it",
            _ => "it is a control of a trajectory this sentence's claims read",
        },
    );
    // Granted only when the arithmetic reproduces the digit, so a row pointed at the wrong
    // operand accounts for nothing rather than accounting for it wrongly. The message a
    // reader gets for that comes from
    // [`every_derivation_is_a_sentence_doing_arithmetic`], which says which operands were
    // read and what they came to.
    derived_value(row, literal)
        .filter(|v| to_fixed(*v, decimals_of(token).max(0) as usize) == token)
        .map(Accounted::Derived)
}

/// What a [`Derivation`] comes to, or `None` if an operand is not a number the sentence
/// prints.
///
/// The operands are read out of the **sentence** rather than out of the row: `from` says
/// where to look and the prose says what the number is. That is the difference between this
/// and the list of declared identities `docs/plans/path-prose-ledger.md` refuses — a row
/// cannot supply a value, only name where one is written.
fn derived_value(row: &Derivation, literal: &str) -> Option<f64> {
    let printed = numeric_tokens(&ascii_minus(literal));
    let operands: Vec<f64> = row
        .from
        .iter()
        .filter(|t| printed.iter().any(|p| p == *t))
        .filter_map(|t| number_of(t))
        .collect();
    (operands.len() == row.from.len())
        .then(|| row.op.apply(&operands))
        .flatten()
}

/// Check 6 — every number a claimed sentence prints is tied to something.
///
/// Checks 1–5 tie the number a claim *spells* to the value it measures and say nothing
/// about the other figures in the same sentence. `**99.98 %** when the cell empties at
/// **207.5 s and 1.9306 V**` carries three numbers and used to carry two claims: the
/// percentage and the voltage were each pinned, and the `207.5` was checked only as
/// characters that had to still be there. Prose and literal could drift to `210 s`
/// together and every check stayed green — which is the *original* hole this file was
/// built to close, surviving on the one number in the sentence nobody claimed.
///
/// So: scan each claimed literal for numbers, and require every one of them to be
/// accounted for by [`Accounted`]. That found the `207.5`, which now has a claim of its
/// own against the flag the sentence says arrives there.
///
/// Two limits worth stating rather than leaving to be found:
///
/// * **A sentence is grouped by `(step, literal)`.** Two claims quoting *different*
///   substrings of one sentence are two groups, so a number spelled only by the sibling
///   group goes unaccounted here. That is the fail-toward-red direction and no claim in
///   the file is written that way today, but the next author to split a sentence will
///   meet it, and the fix is to give both claims the same literal.
/// * **This says which numbers are claimed, not which sentences are.** A step with no
///   claims has no literals to scan and is untouched by this check; fourteen of the
///   twenty-four still have none. Step-level completeness needs a different instrument —
///   a ledger over each step's whole prose — and that one does need a taxonomy for the
///   numbers that are settings, chemistry constants and ordinals rather than
///   measurements. See the module docs.
#[test]
fn every_number_in_a_claimed_literal_is_accounted_for() {
    let lessons = lessons();
    let all = claims();
    let arms = arms();
    let derived = derivations();

    for (step, literal) in sentences(&all) {
        let lesson = lessons
            .iter()
            .find(|l| l.id == step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));
        let group = sentence_group(&all, step, literal);

        for token in numeric_tokens(&ascii_minus(literal)) {
            assert!(
                accounting_for(&Reading::digits(&token), &group, lesson, &arms, &derived).is_some(),
                "step `{step}`, sentence `{literal}`:\n  it prints `{token}`, and none of \
                 the {} claim(s) on it accounts for that number.\n\
                 Tried, in order:\n  \
                 - spelled: no claim here names `{token}` in `spells`\n  \
                 - read at: no claim here is read at that instant \
                 ({:?} in its own frame)\n  \
                 - shown:   it is in no `shows` string this sentence's claims assert\n  \
                 - setting: it is not the step length or the ambient step of any \
                 trajectory this sentence's claims read\n  \
                 - derived: no `[[derived]]` row on this sentence spells it, or the one \
                 that does no longer comes to it\n\
                 A number inside a sentence this file already claims, tied to nothing, is \
                 the hole checks 1-5 leave open: the prose and the literal can drift \
                 together and every one of them stays green. Give it a claim of its own \
                 — that is what `207.5 s` got — or, if it really is not a measurement, \
                 add an arm to `Accounted` that checks it against whatever does decide \
                 it. There is no waiver, on purpose.",
                group.len(),
                group
                    .iter()
                    .map(|c| if reads_past_the_mark(&arms, c) {
                        c.read_at_s - lesson.until_s
                    } else {
                        c.read_at_s
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }
}

/// Every `[[derived]]` row is a sentence doing arithmetic, and the arithmetic works.
///
/// Check 6 grants [`Accounted::Derived`] only when a row's operands are numbers the sentence
/// prints and the operation reproduces the digit. That silence is the right behaviour there
/// — a row that no longer works accounts for nothing — but it is a poor message, so the
/// fences live here, where each can say which operand was read and what it came to.
///
/// Six of them, and every one is a way the arm could have degenerated into the "list of
/// declared identities" `docs/plans/path-prose-ledger.md` refuses:
///
/// 1. **The sentence is one some claim quotes.** A row on unclaimed prose would be an
///    accounting arm reaching past the claims, which is the ledger's job and a different
///    contract.
/// 2. **The number it accounts for is printed in that sentence** — otherwise the row
///    accounts for nothing and looks like coverage.
/// 3. **Every operand is printed in that sentence too.** This is the load-bearing one: an
///    operand the prose does not print is a number the author supplied, and then the row is
///    an identity rather than a reading.
/// 4. **Every operand has an accounting of its own, and it is not `derived`.** Without it
///    the arm says two unpinned numbers divide into a third. Chains are refused rather than
///    followed: a derivation of a derivation has no floor.
/// 5. **The operands differ from each other and from the result**, or the arithmetic is
///    trivially satisfiable.
/// 6. **The arithmetic reproduces the printed digit**, at the precision the prose commits
///    to, through the page's own rounding rule.
#[test]
fn every_derivation_is_a_sentence_doing_arithmetic() {
    let lessons = lessons();
    let all = claims();
    let arms = arms();
    let derived = derivations();

    for row in &derived {
        let lesson = lessons
            .iter()
            .find(|l| l.id == row.step)
            .unwrap_or_else(|| panic!("`[[derived]]` names `{}`, which is not a lesson", row.step));
        let group = sentence_group(&all, &row.step, &row.literal);
        assert!(
            !group.is_empty(),
            "the `[[derived]]` row on step `{}` quotes\n  {}\nand no claim on that step \
             quotes the same sentence. A derivation inherits its operands' accountings from \
             the claims on the sentence, so a row on prose nobody claims can inherit \
             nothing.",
            row.step,
            row.literal,
        );
        let printed = numeric_tokens(&ascii_minus(&row.literal));
        assert!(
            printed.contains(&row.spells),
            "the `[[derived]]` row on step `{}` accounts for `{}`, which is not a number the \
             sentence it quotes prints:\n  {}\n  printed: {printed:?}\n\
             A row that accounts for nothing is the shape this file rejects everywhere \
             else — it looks like coverage. If the sentence was reworded, reword the row.",
            row.step,
            row.spells,
            row.literal,
        );
        assert_eq!(
            row.from.len(),
            row.op.arity(),
            "the `[[derived]]` row for `{}` on step `{}` is `{:?}`, which takes {} \
             operand(s), and lists {}.",
            row.spells,
            row.step,
            row.op,
            row.op.arity(),
            row.from.len(),
        );
        for operand in &row.from {
            assert!(
                printed.contains(operand),
                "the `[[derived]]` row for `{}` on step `{}` reads an operand `{operand}` \
                 that the sentence does not print:\n  {}\n  printed: {printed:?}\n\
                 **This is the fence that keeps the arm a reading rather than an \
                 identity.** An operand the prose does not carry is a number the author \
                 supplied, and then the row asserts its own arithmetic instead of the \
                 sentence's.",
                row.spells,
                row.step,
                row.literal,
            );
            assert_ne!(
                operand, &row.spells,
                "the `[[derived]]` row for `{}` on step `{}` lists the number it accounts \
                 for as one of its own operands.",
                row.spells, row.step,
            );
            let accounted =
                accounting_without_arithmetic(&Reading::digits(operand), &group, lesson, &arms);
            assert!(
                accounted.is_some(),
                "the `[[derived]]` row for `{}` on step `{}` reads `{operand}`, and nothing \
                 accounts for that operand:\n  {}\n\
                 An operand tied to nothing makes the derivation a circle — it would say \
                 only that two numbers nobody pinned divide into a third. Give the operand \
                 a claim of its own, or one of check 6's other arms.",
                row.spells,
                row.step,
                row.literal,
            );
        }
        let mut seen = row.from.clone();
        seen.sort();
        let before = seen.len();
        seen.dedup();
        assert_eq!(
            before,
            seen.len(),
            "the `[[derived]]` row for `{}` on step `{}` reads one operand twice.",
            row.spells,
            row.step,
        );
        let value = derived_value(row, &row.literal).unwrap_or_else(|| {
            panic!(
                "the `[[derived]]` row for `{}` on step `{}` is `{:?}` over {:?}, and that \
                 has no value — a division by zero, or an operand that is not a number.",
                row.spells, row.step, row.op, row.from,
            )
        });
        let places = decimals_of(&row.spells).max(0) as usize;
        assert_eq!(
            to_fixed(value, places),
            row.spells,
            "step `{}`, sentence\n  {}\nsays `{}`, and its own arithmetic says {value}: \
             `{:?}` over {:?}.\n\
             The sentence and the numbers it is doing arithmetic on have parted. Nothing \
             here is a measurement — check the operands' own claims first, because if one \
             of them moved this is the sentence telling you the conclusion moved with it.",
            row.step,
            row.literal,
            row.spells,
            row.op,
            row.from,
        );
    }
}

// ---------------------------------------------------------------------------
// The ledger: every number in a step's WHOLE prose
// ---------------------------------------------------------------------------

/// Which steps have their whole prose scanned, and which are admitted not to.
///
/// Check 6 scans the sentences a claim already quotes; this list is the other axis. A
/// ledgered step's prose is scanned end to end and every number in it has to be tied to
/// something, whether a claim mentions the sentence or not.
///
/// **Both lists are required, and every lesson must be in exactly one of them.** An
/// opt-in list on its own would let a new lesson join the unchecked majority in silence,
/// which is how the fourteen unclaimed steps came to exist. Naming the exclusions makes
/// adding a step a decision someone has to write down —
/// [`every_lesson_is_ledgered_or_named_as_not`] is the same contract
/// [`every_covered_step_exists`] keeps for claims, kept in both directions.
#[derive(Debug, serde::Deserialize)]
struct Ledger {
    /// Steps scanned whole.
    steps: Vec<String>,
    /// Steps not scanned, listed on purpose. This is a statement of the gap, not a waiver
    /// for any particular number.
    unledgered: Vec<String>,
    /// Steps whose prose is additionally scanned for quantities spelled in **English**.
    ///
    /// A third axis, and opt-in for the same reason `steps` is: `spelled_numbers` finds
    /// figures no arm has ever had to answer for, so turning it on for a step is a piece of
    /// authoring rather than a flag. Every entry must also be in `steps` — a step whose
    /// digits are not tied has no business claiming its words are.
    ///
    /// Every step not here must be in [`Ledger::word_blind`], which is what stops the gap
    /// this leaves from being a silence.
    #[serde(default)]
    spelled: Vec<String>,
    /// Steps whose prose is **not** word-scanned, listed on purpose.
    ///
    /// `spelled`'s counterpart, on exactly the terms [`Ledger::unledgered`] is `steps`'s,
    /// and it exists for the reason that pairing does: at one step in twenty-four the
    /// omission spoke for itself, and at six it stops speaking. Without this list the file
    /// can say which steps *are* word-scanned and cannot say which are not — so a lesson
    /// added tomorrow would be word-blind by default and nothing would ever say so.
    /// [`every_lesson_is_word_scanned_or_named_as_not`] requires each lesson to be in
    /// exactly one of the two.
    #[serde(default)]
    word_blind: Vec<String>,
}

/// One phrase the lesson prose uses to print a number some file already decides.
///
/// This table is the ledger's honest cost. A generous arm — "the number appears somewhere
/// in the scenario file" — accounts for a third of the path's numbers and means nothing: a
/// scenario has enough integers in it that a `2` finds `series = 2` by accident. So the
/// arm has to name the field, and something has to say which field a sentence is talking
/// about. That something is the phrase around the number.
///
/// What is declared here is only the *vocabulary* — that "`{n}` in series" is the page's
/// way of saying `pack.series`. The number itself is never declared: it is read out of the
/// file and compared, so a rule pointed at the wrong field fails on sight, the same way a
/// mis-pointed `spells` does. Keep the phrases specific for that reason. A bare
/// `"{n} mV"` would match any millivolt figure in any ledgered step and account it against
/// the sensor offset, which passes silently the day one of them happens to be 120.
struct LedgerRule {
    /// The sentence shape, with `{n}` where each number sits. Matched literally against
    /// the prose, so it is written the way the prose writes it.
    phrase: &'static str,
    /// What decides each `{n}`, in order — one [`Tie`] per placeholder.
    ties: &'static [Tie],
    /// How many powers of ten larger the unit the *prose* writes is than the unit the
    /// *file* writes — 2 for a percentage against a fraction, 3 for mA against A. Same
    /// convention as a claim's `spells_pow10`.
    pow10: i32,
}

/// What decides one number a ledgered step prints.
///
/// Every variant is a *derived numeric fact*: a value read out of a file in the tree, or
/// computed from values read out of files in the tree. None of them is a declaration, which
/// is the design constraint carried over from [`Accounted`] — a declared "this number is a
/// setting" beside a token that is really a measurement is a fresh instance of the defect
/// `tol_from` exists to catch.
///
/// The taxonomy is `docs/plans/path-prose-ledger.md`'s, built one arm at a time as a
/// sentence needs it, and **all six of its kinds now exist**: the last, the general
/// `Derived` over a sentence's own siblings, waited until a *ledgered* step printed one —
/// step 22's "six of these in series is the 12 V battery" — because an arm with nothing to
/// account is the `CCCV_PERIOD_S` shape this file has already been caught by once. Several
/// variants below are finer distinctions the plan's six did not separate (`Ratio` beside
/// `Product`, `Span` beside `Member`, `Clock`, which reads a rendering rather than a file,
/// and `Page`, which reads a constant of the client rather than of a scenario), each built
/// the same way: when one sentence needed it. Check 6 has a `Derived` of
/// its own ([`Accounted::Derived`]); that one is over a claimed sentence and says nothing
/// about this scan, the same way `setting` sits in both.
enum Tie {
    /// A named field of the step's scenario file — a dotted key path, with `*` walking an
    /// array so `faults.*.at_s` does not care what order the file lists its faults in.
    ///
    /// **A wildcard is read strictly: *every* value it reaches must be the number.** It was
    /// existential first — some fault at 600 — and that is a fail-toward-green on the one
    /// relational thing step 5's prose says. `lying-sensor` claims the short and the sensor
    /// lie land "in the same instant"; measured, with the second fault moved to 700 s, the
    /// existential arm stayed green and the sentence was false. Strict makes the arm say
    /// what the sentence says.
    ///
    /// The cost is stated rather than discovered: a *third* fault scheduled at some other
    /// time would fail this rule even though the sentence — which names two — would still
    /// be true. That is the fail-toward-red direction, and the answer then is a path that
    /// selects the fault the sentence names, which this walker cannot express.
    Scenario(&'static str),
    /// A named field of the *chemistry* that scenario names — same path syntax, read out
    /// of `chemistries/<id>.toml`.
    ///
    /// A separate arm rather than a second path root, because the two files answer
    /// different questions: a scenario is what this pack is, a chemistry is what this cell
    /// can do. Step 6's "rated 3 C continuous" is a property of every LFP cell in the repo
    /// and would still be 3 C if the pack were one cell.
    Chemistry(&'static str),
    /// A control on the lesson's own block — a number the *page acts on*. See [`Control`].
    Setting(Control),
    /// The sentence's own arithmetic: the product of the ties below it.
    ///
    /// **The one arm that rounds.** Every other tie compares exactly, because a constant
    /// printed in prose is either the file's number or a defect. A product is a *computed*
    /// quantity and its exactness is spurious — `2.303451 Ah × 2` is `4.606902`, and no
    /// sentence would print that — so it is compared at the precision the prose itself
    /// commits to: the token's own decimal places, through the page's rounding rule
    /// ([`to_fixed`], ties away from zero, which is also the schoolbook rule a human
    /// author uses).
    ///
    /// Each factor must resolve to exactly one number. A `*` wildcard under a product would
    /// make "which of them" the author's pick, which is the hazard the strict wildcard above
    /// exists to close.
    Product(&'static [Tie]),
    /// A digit that is part of a **name**, read out of a named *string* field of the
    /// chemistry — `meta.name` and `meta.provenance`.
    ///
    /// Step 12 calls its cell "the LG M50", and the `50` in it is not a quantity at all: it
    /// is four fifths of a part number. Nothing measures it and no numeric field holds it,
    /// so before this arm the sentence could not be ledgered — the same position the `0` of
    /// `R0` was in, and that one had no field to point at and was reworded away instead.
    ///
    /// **That last clause stopped being true the day this arm was built, and step 1 is where
    /// it showed.** Its second paragraph says the voltage "drops instantly by `I·R0`", and
    /// the LFP chemistry's own provenance names `R0` as one of the sections that are still
    /// placeholders — so the digit has a field after all, and a stronger one than a reword:
    /// the string that ties it is the string that *declares what it is*. The lesson is not
    /// about this arm. It is that a note recording why something could not be done outlives
    /// the reason, and the ledger's own steps are where such notes are read as fact.
    ///
    /// **The `prefix` is what makes it exact rather than a search.** The tie collects only
    /// the digit runs that follow `prefix` in the field's value, so `M{n}` against
    /// `"LG M50 21700 (NMC811/graphite)"` resolves to `[50]` and not to the format code or
    /// the cathode ratio sitting beside it. Without it, a rule would account any token that
    /// appeared anywhere in the string, which is the generous match the whole table refuses
    /// — and that string is long enough to hold three of them.
    ///
    /// Swap the scenario's chemistry and the field says something else, so the sentence
    /// fails on sight. That is the property that makes this a tie and not a waiver.
    Name {
        /// Dotted key path to a string field of the chemistry file.
        field: &'static str,
        /// The characters the digits follow in that string.
        prefix: &'static str,
    },
    /// The **position of another lesson** in the path, counted from one.
    ///
    /// "The cell is the LG M50 from step 2's aside" prints a number that no file holds and
    /// no engine produces: it is where a *different* lesson sits in `const LESSONS`. The arm
    /// names that lesson by id and derives the ordinal from the array, so inserting a step
    /// ahead of it turns every cross-reference to it red — which is the whole failure this
    /// is for. A path this long acquires steps, and a sentence pointing at "step 2" is one
    /// insertion away from pointing at the wrong one, silently.
    ///
    /// The id is declared and the number never is, the same contract [`LedgerRule`] keeps
    /// for a field name: an author says *which step they mean*, and the file says where it
    /// is.
    Ordinal(&'static str),
    /// The number is **a node of a table** the chemistry declares — a value in the array at
    /// this path, not a particular one.
    ///
    /// **The one existential tie, and it is existential because the sentence is.** Step 12
    /// says the depth of its teeth steps "because the `[ocv]` table's node at 85 % charge
    /// passed under the pulse", which is a claim that 0.85 is *a* node — not that it is the
    /// twenty-first, which is a fact about the table's layout that no reader is shown and
    /// that a re-fit would change without making the sentence wrong. [`Tie::Scenario`]'s
    /// wildcard is strict for the opposite reason: `faults.*.at_s` carries a sentence
    /// saying two faults land together, and there "some fault" was a fail-toward-green.
    ///
    /// The cost is stated rather than discovered: on a table this dense — thirty-four nodes
    /// between 0 and 1 — a mistyped node has a real chance of being some *other* node and
    /// passing. What it cannot do is pass when the table no longer has a node there at all,
    /// which is what a re-fit does and what the sentence is really resting on.
    Member(&'static str),
    /// The sentence's own arithmetic again, **divided**: the first tie below it over the
    /// second, and exactly two of them.
    ///
    /// Step 22 says `0.36 A is C/20`, and the `20` is neither a file's number nor a
    /// measurement — it is the hours the cell's own rating is quoted over, which is the
    /// capacity divided by the current in the demand box. Tying it to the pair is what
    /// makes the sentence fail when either half moves: change the box to C/5 and the rate
    /// this sentence names is wrong, which is precisely what a reader would be misled by.
    ///
    /// Compared at the prose's own precision for [`Tie::Product`]'s reason — 7.2 / 0.36 is
    /// 20.000000000000004 in binary floating point, and no sentence would print that.
    Ratio(&'static [Tie]),
    /// The **difference of two ties**: the first less the second, and exactly two of them.
    ///
    /// [`Tie::Ratio`]'s sibling, and built for the same reason — a sentence stating what two
    /// numbers come to when you take one from the other, where neither the sentence's own
    /// tokens nor any single file holds the answer. Step 16 prints three of them and not one
    /// is expressible any other way: *"it drops **535 mV**"* is that step's own readings at
    /// two instants, *"the single-particle arm falls 34 mV"* is the twin's at the same two,
    /// and *"has 596 seconds still to run"* is the twin's cut-off less this step's.
    ///
    /// [`Tie::Derived`]'s `Difference` is the same arithmetic over a sentence's **own
    /// printed tokens**, and none of those three sentences prints both operands: they print
    /// the answer and nothing else. That is the whole distinction between the two families —
    /// this side reads files and claims, that side reads the prose beside the number.
    ///
    /// **Order is the claim.** Reversed, step 16's `535 mV` drop becomes a −535 mV climb and
    /// the sentence is about a cell recovering. Compared at the prose's own precision for
    /// [`Tie::Product`]'s reason: a difference of two measurements lands on
    /// `0.5354644...`, and no sentence would print that.
    Difference(&'static [Tie]),
    /// The **sum of two ties**, and the third of the arithmetic family beside
    /// [`Tie::Ratio`] and [`Tie::Difference`].
    ///
    /// Step 19 is what built it, and it prints two numbers that no file holds and no engine
    /// reports. *"The trip is a probe crossing 343.15 K"* is a **threshold assembled from two
    /// files**: the chemistry's ceiling for the cell (`cell.t_max_k`, 333.15) plus the
    /// scenario's hard margin (`pack.bms.protection.t_hard_margin_k`, 10). Neither half is
    /// the number, and the sum is what the protection layer actually compares a probe
    /// against — so moving the ceiling in the chemistry or the margin in the scenario turns
    /// the sentence red, which is the property that makes this a tie. *"A twin whose run ends
    /// at 299.1 K"* is the same shape across a lesson boundary: the twin's ambient plus the
    /// rise the twin's own claim pins.
    ///
    /// **Order is irrelevant here and is the reason this is not a fourth [`LedgerOp`].**
    /// Its two neighbours both say "order is the claim" — reversed, a difference changes
    /// sign and a ratio inverts. A sum reversed is the same number, so nothing about the
    /// sentence is encoded in which tie is written first, and a fence about order would be
    /// a fence about nothing.
    ///
    /// **Exactly two, and each must resolve to exactly one number**, on [`Tie::Product`]'s
    /// terms: with several, which value the sentence meant would be the author's pick rather
    /// than the file's. **Compared at the prose's own precision**, like every computed tie.
    Sum(&'static [Tie]),
    /// The tie below it, **read in hours where the file reads seconds**.
    ///
    /// One wrapper for one job: an amp-hour figure worked out from a current and a duration.
    /// Step 16 says *"15.459594 A for 464 s is 1.99 A·h"* — the current is the demand box,
    /// the duration is a claim on this step, and the product of the two is in ampere-seconds
    /// while the sentence writes ampere-hours. A rule's `pow10` cannot carry that: 3600 is
    /// not a power of ten. Nothing else in this file can either, which is why the conversion
    /// is a tie rather than a per-rule field.
    ///
    /// **It is a unit, not a number, and that is what keeps it out of the declaration
    /// business.** An hour is 3600 seconds by definition, the same way [`to_c`] converts a
    /// temperature and `pow10` converts a percentage; no value is being supplied by an
    /// author.
    ///
    /// **Compared like a computed tie** rather than delegating to what it wraps, which is the
    /// opposite choice from [`Tie::Elsewhere`] and deliberately so: `Elsewhere` changes which
    /// lesson answers and leaves the answer's exactness alone, where a conversion by a
    /// non-decimal factor always lands off a round number. A prose figure in hours is
    /// rounded by construction.
    ///
    /// **That choice is unreachable through the vocabulary today, and it has a test of its
    /// own because of that.** Its one user sits inside a [`Tie::Product`], and [`tie_agrees`]
    /// is asked about a rule's *outermost* tie — so the product's own rounding is what decides
    /// `1.99`, and moving this variant out of the rounding group leaves the whole suite green
    /// (measured, not assumed). A comparison arm nothing reaches is the `CCCV_PERIOD_S` shape
    /// this file has been caught by once, so [`an_hours_tie_rounds_the_way_a_computed_tie_does`]
    /// asks the question directly instead of leaving the paragraph above to stand on nothing.
    Hours(&'static Tie),
    /// The **span of a table** the chemistry declares: its largest value minus its
    /// smallest, at this path.
    ///
    /// "Lead-acid spans only 180 mV of open-circuit voltage end to end" is a statement
    /// about the whole `[ocv]` table rather than about any node of it, so neither
    /// [`Tie::Chemistry`] nor [`Tie::Member`] can carry it: the first wants one field and
    /// the second asks whether some node *is* the number.
    ///
    /// **Fenced to two values or more.** The span of a one-node table is zero, and a
    /// sentence saying a chemistry spans nothing would then be accounted by a table that
    /// had been emptied — a fail-toward-green on exactly the restructuring this arm is
    /// supposed to notice. Rounded like a product, and for the same reason: 2.130 - 1.950
    /// is 0.17999999999999994.
    Span(&'static str),
    /// The **panel's clock at the step's mark** — `fmtTime(until_s)`, as the `sim time` row
    /// renders it.
    ///
    /// Step 22 says "the panel reads `19.3h`, not twenty", and `19.3` is not the mark
    /// (69620.5) nor any field of any file: it is what one row prints when the run stops.
    /// A [`Tie::Setting`]-shaped arm reading `until_s` could not account for it, and the
    /// sentence is quoting the row, so the tie is to the row's own formatter — the same
    /// [`fmt_time`] mirror the display check runs on, and the same reasoning
    /// [`Accounted::Shown`] gives for granting a claim the clock at the mark: `sim time` is
    /// the only row that is a function of time alone, so it is the only one this scan can
    /// render without an engine.
    ///
    /// Move the mark and the sentence goes red, which is the property that makes it a tie.
    Clock,
    /// A **constant of the page's own policy**, parsed out of `web/app.js` by name.
    ///
    /// Step 9 tells the reader how often the charge controller is allowed to change its
    /// mind — *"the rule is checked every 10 s of simulation time, never once per frame"* —
    /// and that number is in no scenario, no chemistry and no lesson block. It is
    /// `CCCV_PERIOD_S`, a constant of the client-side policy `CLAUDE.md` puts there, and the
    /// sentence's whole point is that the page keeps a grid of its own.
    ///
    /// **Parsed, never declared**, which is what makes it a tie: [`cccv_period_s`] reads the
    /// literal out of the page, so widening the window to 30 s turns this sentence red on
    /// sight. It is the same instrument [`Tie::Clock`] uses one level down — that one reads a
    /// *rendering* the page performs, this one reads a number the page holds — and the same
    /// one `default_dt` has read out of the markup since this file was written.
    ///
    /// **The name is declared and the value never is.** Every other arm here names a field
    /// and lets the file answer; this names a constant and does the same. What it does *not*
    /// do is take a path into the source: a tie able to read any expression in `app.js` would
    /// find a number for almost any token, which is the generous match the whole table
    /// refuses. One constant, one parser, and a new one is a new function.
    Page(&'static str),
    /// A [`Tie::Setting`] read off **one of this step's arms** — the value the reader dials
    /// in, rather than the one the step arrives with — or, on an arm that loads another file
    /// from the picker, a field of **that** file.
    ///
    /// Step 8 prints both in two sentences: the slider sits at 25 °C for the first leg, and
    /// *"raise the ambient slider to 45 °C and press Run"* is the second. `Setting(Ambient)`
    /// answers 25 for that step whatever the sentence says, so before this arm the 45 was
    /// tied to nothing — and the wrong fix was available and worse: pointing a rule at the
    /// scenario's `initial_temp_k` would have found a 298.15 that is not this number and
    /// then, at some other lesson, one that accidentally is.
    ///
    /// **A wrapper, on [`Tie::Elsewhere`]'s terms**, and the parallel is exact: that one
    /// changes *which lesson* answers a question and leaves the question alone, this one
    /// changes *which trajectory's controls* answer it.
    ///
    /// **It wrapped a `Setting` and nothing else until step 2 was ledgered, and the argument
    /// for that is still in the tree because it is still right — about the arms it was made
    /// about.** It read: "a file read does not become a different fact for being asked under
    /// an arm's name, so wrapping one would be an arm that means nothing". True of every arm
    /// that changes only controls, and false of one that carries [`Arm::scenario`]: the
    /// picked file really is the arm's, and step 2's third cell prints its nameplate and the
    /// provenance of its own curve. So the refusal is narrowed rather than dropped — a file
    /// tie is read off the picked scenario where there is one, and panics with the original
    /// wording where there is not.
    ///
    /// **The override must be real.** An arm that leaves the control alone resolves to
    /// nothing and panics by name rather than falling back to the step's own value. That
    /// fallback is the whole hazard: it would account the sentence's 45 against the step's 25
    /// and go green on a number that is not in any file. None of the three fences is
    /// reachable from the claims file — a rule is code — so each has a `should_panic` test of
    /// its own: [`an_on_arm_may_not_read_a_file_off_an_arm_that_picks_none`],
    /// [`an_on_arm_may_not_wrap_anything_else`] and
    /// [`an_on_arm_reads_a_control_the_arm_overrides`].
    OnArm {
        /// The arm's `name`, as `[[arm]]` writes it and a claim's `arm` field reads it.
        arm: &'static str,
        /// What to read off it. A [`Tie::Setting`], or — on an arm that picks a file — a
        /// [`Tie::Chemistry`], [`Tie::Scenario`] or [`Tie::Name`] read against that file.
        /// The fences above refuse everything else, and refuse all three of those on an arm
        /// that picks nothing.
        tie: &'static Tie,
    },
    /// The taxonomy's sixth arm: a number the sentence works out **from its own siblings**,
    /// as their product.
    ///
    /// `docs/plans/path-prose-ledger.md` reserved this slot and named the sentence that
    /// would fill it — step 22's *"six of these in series is the 12 V battery"*. The `12`
    /// is in no file: it is six times the `2 V` the same sentence prints two clauses
    /// earlier. Every other arm checks a token against a file; this one checks it against
    /// other tokens beside it, which is why it was left until a ledgered step printed one.
    ///
    /// The operands are declared and the value never is (see [`Operand`]), and each one has
    /// to be accounted for by an arm that is *not* this one — a derivation whose operands
    /// are themselves derived has no floor. That fence is what makes it a tie rather than
    /// the declared identity the plan refuses.
    ///
    /// Check 6 has an arm of this name over claimed literals ([`Accounted::Derived`]); the
    /// two scans are separate, as they are for `setting`.
    ///
    /// **The operation is declared.** This shipped product-only, with the price stated:
    /// "the day a ledgered step derives one by another, this grows an operation the way
    /// check 6's `[[derived]]` carries `op`". Steps 23 and 24 bring both of the others in
    /// one slice — a difference (`6.9620 − 4.4190` amp-hours left in the cell) and three
    /// quotients (two heats, and leg two against leg one) — so the operation is now a field
    /// rather than an assumption. See [`Op`], and note that order is load-bearing under two
    /// of the three.
    Derived {
        /// What the sentence does to its operands.
        op: LedgerOp,
        /// What it does it to, in the order the operation takes them.
        operands: &'static [Operand],
    },
    /// A digit that is part of a **control's label**, read out of `web/index.html` by the
    /// control's `id`.
    ///
    /// [`Tie::Name`]'s twin one file over. Step 18 tells the reader to *"press **Step 1**
    /// twice"*, and that `1` is no more a quantity than the `50` of an LG M50 is: it is
    /// half the name of a button. Nothing measures it, no scenario or chemistry field holds
    /// it, and the sentence would be *wrong* without it, because a reader has to find that
    /// button on the page.
    ///
    /// **Anchored on the `id`, and the `prefix` makes it exact**, both for [`Tie::Name`]'s
    /// reasons: the markup is long enough that an unanchored search would account almost any
    /// token, which is the generous match the whole table refuses. The label is read from the
    /// `id` to the next tag, so `<button id="stepone">Step 1</button>` with `prefix = "Step "`
    /// resolves to `[1]` and nothing else on the page can answer.
    ///
    /// Rename the button and the sentence goes red, which is the property that makes it a
    /// tie. That is not hypothetical bookkeeping — the path's instructions name eight
    /// controls by their printed labels, and a label is the one thing about a control that a
    /// reader cannot infer.
    Label {
        /// The control's `id` attribute, as `web/index.html` writes it.
        id: &'static str,
        /// The characters the digits follow inside that control's label.
        prefix: &'static str,
    },
    /// Digits inside the **name of the file an arm picks out of the picker**.
    ///
    /// [`Tie::Name`]'s third sibling, after [`Tie::Label`]: `Name` reads a chemistry's own
    /// name, `Label` reads a control's, and this reads a scenario file's. Step 2 is what
    /// needed it — its closing paragraph sends the reader to `cc_discharge_lgm50` twice, and
    /// the `50` in that name is no more a quantity than the `1` of **Step 1** is. It is half
    /// the name of a file, and a sentence that got it wrong would be sending a reader to a
    /// file that is not in the list.
    ///
    /// **It is not a new guarantee, and saying so is the honest part.** [`assert_picker`]'s
    /// third fence already requires the picked file's stem to appear inside the arm's
    /// instruction, and an instruction is a verbatim substring of the step's prose — so the
    /// day that file is renamed, the arm fails there whether or not this exists. What this
    /// adds is that the *ledger* names the right decider for those digits instead of leaving
    /// them unaccounted, which is the difference between a green scan and a covering one.
    ///
    /// **Anchored on a prefix**, for [`Tie::Name`]'s reason exactly: `cc_discharge_lgm` is
    /// what makes it reach the `50` and nothing else. Compared exactly — a file name is a
    /// constant.
    Picker {
        /// The arm whose `scenario` field holds the file. It must carry one.
        arm: &'static str,
        /// The characters the digits follow inside that file's name.
        prefix: &'static str,
    },
    /// **How many entries** the array at this scenario path has.
    ///
    /// Every other arm here reads a value out of a file; this one reads the file's *shape*.
    /// Step 18 says **Clear queued** *"counts the short it removed as **1 fault**"*, and the
    /// number is neither a resistance nor an instant nor anything the engine reports — it is
    /// how many faults the scenario declares, rendered by a note on the page. Put a second
    /// `[[faults]]` block in the file and the sentence is wrong, which is exactly what this
    /// makes fail.
    ///
    /// **Refuses anything that is not an array.** A count of a table or of a scalar would be
    /// a number this arm invented rather than one the file states, and one is the answer it
    /// would most often invent — the value that makes a wrong sentence look right here.
    Count(&'static str),
    /// The chemistry's **open-circuit voltage at a charge the inner tie names** \[V\], per
    /// cell, interpolated off the `[ocv]` table exactly as `sim-core` reads it.
    ///
    /// A wrapper on [`Tie::Elsewhere`]'s and [`Tie::Hours`]'s terms, and the only one that
    /// answers a question about the *curve* rather than about a point of it. Step 18's last
    /// aside is what needed it: *"on this plateau half a percent of charge moves the
    /// open-circuit voltage by 0.4 mV"*. That figure is in no file. It is the difference
    /// between the curve at the charge the pack started from and the curve at the charge the
    /// tooth left it with, times the four cells in series — and LFP's plateau is the whole
    /// point of the sentence, so a tie that could not read the curve could not carry it.
    ///
    /// **Linear interpolation, clamped at both ends**, which is what
    /// `sim_core`'s lookup does and what the chemistry file's own comment promises. This is
    /// a second implementation of that lookup and so a second source of truth, on the same
    /// terms as the page's demand programs: the reason it is acceptable here is that it
    /// reads the same table the engine reads, so a re-fit moves both together.
    ///
    /// **What WOULD part them, named rather than left to "the interpolation rule might
    /// change".** `CLAUDE.md` reserves two optional additions to this path: a `dOCV/dT` table
    /// for temperature correction, and a per-chemistry hysteresis term. Either lands inside
    /// the engine's open-circuit read and neither would touch the `[ocv]` table, so this
    /// mirror would go on interpolating the bare curve for a pack that is a kelvin above
    /// ambient and the rule would stay green. That is the same standing hazard [`MIRRORED`]
    /// exists for on the page's side, and it is written down here for the same reason.
    ///
    /// **Compared like a computed tie.** An interpolated value lands between two nodes by
    /// construction, so exactness would be spurious — [`Tie::Product`]'s reasoning. Its one
    /// user today sits two levels inside a `Product`, which is where `Hours` was when its
    /// rounding was found to be unreachable, so [`an_ocv_tie_rounds_the_way_a_computed_tie_does`]
    /// asks the question directly rather than leaving this paragraph to stand on nothing.
    ///
    /// **What it does NOT settle, said here rather than left to a green rule to imply**: the
    /// step's sentence says "half a percent", and both a round 0.5 points and the measured
    /// 0.557 print `0.4 mV` through this rule. It cannot tell those two readings apart. What
    /// it catches is a re-fit of the plateau, a change of series count, or a different
    /// starting charge — which is what the sentence actually rests on.
    Ocv(&'static Tie),
    /// The **magnitude** of the thing it wraps, for a sentence that puts the sign in its own
    /// words instead of in the number.
    ///
    /// [`written_numbers`] finds digits and never a leading minus, so every token this scan
    /// is handed is unsigned. That is harmless while the files answer with positive numbers,
    /// and step 11 is the first ledgered sentence where one does not: *"drag the ambient to
    /// −5 °C"* is an arm whose `ambient_c` is `-5`, and comparing 5 against it fails on the
    /// sign alone.
    ///
    /// The claim side has carried the same variant since it was written — `states =
    /// "magnitude"`, for a sentence that "prints the magnitude and puts the sign in the word
    /// *late*" — and this is its ledger twin, with the same fence: **refused on a value that
    /// is not negative**, because on a positive one it is the tie it wraps with extra words
    /// and would go green for the wrong reason. Unreachable from the claims file, so the
    /// fence is priced by [`a_magnitude_refuses_a_value_that_is_not_negative`].
    ///
    /// **The sign has to be in the phrase.** A rule reading a magnitude off a sentence that
    /// does not write the minus would account a `5` that means five degrees above freezing
    /// against a slider dragged five below, so the phrase this wraps carries the `-`
    /// itself and the placeholder picks up the digits after it.
    ///
    /// Compared the way the thing it wraps compares, on [`Tie::Elsewhere`]'s terms: taking
    /// an absolute value does not turn a file read into a computed quantity.
    Magnitude(&'static Tie),
    /// The same question asked of **a different lesson** — the inner tie, resolved against
    /// the named step's block, scenario and chemistry.
    ///
    /// Step 23 is the third lesson on one scenario file and its opening sentences say so by
    /// comparison: *"21.6 A instead of 0.36"*, *"reached in `12m` instead of `19.3h`"*. The
    /// `0.36` is step 22's demand box and the `19.3` is step 22's clock — facts
    /// [`Tie::Setting`] and [`Tie::Clock`] already read, about the wrong lesson. A wrapper
    /// rather than two more flat arms, because "what does the step next door say" is one
    /// question and the sentence asks it twice with different inner ties.
    ///
    /// Three refusals, all in [`tie_values`]:
    ///
    /// * **No nesting.** An `Elsewhere` inside an `Elsewhere` has no floor and no sentence
    ///   needs one.
    /// * **Not its own step.** A wrapper naming the lesson it sits in is [`Tie::Setting`]
    ///   with extra words — green, and for the wrong reason.
    /// * **No inner [`Tie::Derived`]**. That arm reads *this* sentence's siblings, and
    ///   "the sibling of a token in another lesson" is not a thing.
    ///
    /// **Its blind spot is stated because its two users cannot close it.** Both name a step
    /// on step 23's own scenario file, so "resolves against the *named* lesson's files" is
    /// untested by construction: an implementation that read this step's files would pass
    /// both sentences. [`an_elsewhere_reads_the_named_lessons_own_files`] is that test,
    /// pointed at an LFP step on purpose.
    Elsewhere {
        /// The lesson's id, as `const LESSONS` writes it.
        step: &'static str,
        /// What to read there.
        tie: &'static Tie,
    },
    /// A number **another step measured** — the value of that step's claim on a named
    /// quantity.
    ///
    /// The first arm here that is not a fact about a file, and it exists because step 23
    /// quotes step 22's cell: *"4.4190 A·h came out against the last step's 6.9620"*,
    /// *"`0.07 W` where the last step stopped"*. A constant can be tied to a file; a
    /// measurement cannot, and step 23's prose is not the place to re-measure step 22's
    /// trajectory. So the tie is to the **claim** — which check 7 checks against the engine
    /// where it lives, so a quotation here inherits that check instead of duplicating it.
    ///
    /// **Named by `(step, quantity)`, never by step alone.** "Some claim on step 22 spells
    /// 0.07" is the search-the-file match [`Tie::Name`]'s prefix and [`Accounted::Setting`]'s
    /// trajectory tie both exist to refuse: it would account any token that happened to
    /// equal any number that step measures.
    ///
    /// **One measurement, not one claim.** This used to demand that exactly one claim named
    /// the quantity, and step 12 cannot satisfy it: its first rebound is stated in two
    /// sentences, so `pulse_rebound_mv:1` carries two claims at one instant with one value.
    /// What the fence is really about is a quantity whose claims answer *differently* —
    /// step 20's `v_at` is ten readings at ten instants — so agreement is what it checks
    /// now, and a pair that has drifted apart fails here rather than passing unseen.
    ///
    /// **A step opts out of being unquotable by tagging its instants.** Step 15 was the
    /// case this fence was first written against and is no longer one: its eight voltages
    /// are `v_at:0` … `v_at:1060`, each tag asserted against the claim's own `read_at_s`.
    /// See [`measure`]. That is the move any step owes a sentence that wants to quote it,
    /// and step 16 paid it for itself when it was ledgered.
    ///
    /// **A step may quote ITSELF, and there is no fence against it because there is nothing
    /// to fence.** [`Tie::Elsewhere`] refuses naming its own lesson — a wrapper that changes
    /// which lesson answers, pointed at this one, is the arm it wraps with extra words. This
    /// is not that: what it names is a *measurement*, and [`claimed_accounting`] reaches a
    /// measurement only when the number sits inside the literal of the sentence whose claims
    /// decide it. Step 16 prints its own crossing instant in two further sentences and its
    /// own 535 mV collapse in a third, none of which any claim quotes, so the alternative to
    /// quoting itself is four numbers tied to nothing. The claim still answers to the engine
    /// where it lives, which is the whole of what this arm rests on wherever it points.
    ///
    /// **Compared at the prose's own precision**, like every computed tie, which is what
    /// lets one arm carry both `0.07` (two places) and `0.0746` (four) off one claim value.
    /// The quoted claim's own `spells_pow10` is not consulted; see [`tie_values`] for the
    /// fence that used to refuse one and why it could not fire correctly.
    ///
    /// **This is what would have caught the defect this slice found.** Step 23 said its
    /// heat was 87 times step 22's "at the same state of charge", and step 22's own claim
    /// note said it was there "so that step 23's 6.09 W has something to be 87 times". Two
    /// files, three assertions, one suite, all green: nothing compared the two steps'
    /// numbers to each other. See `docs/plans/path-ledger-last-two-steps.md`.
    ///
    /// **The address is `(step, arm, quantity)`, and the arm half is this slice's.** A step
    /// and a quantity name a *number* only while the step runs one trajectory. Step 16 now
    /// runs three — its own 3 C discharge and two 1 C reruns — and `t_at_v_below:2.5`
    /// answers 464 on one of them and 3484 on another. Before the arm was part of the
    /// address that pair was simply unquotable, and the four sentences that quote either of
    /// them would have failed together on the agreement assert below. `None` is *the step's
    /// own run*, not "any of them": an address that matches whatever happens to be lying
    /// around is the search-the-file match this whole taxonomy refuses.
    Quoted {
        /// The lesson whose claim decides it.
        step: &'static str,
        /// Which of that lesson's `[[arm]]`s the claim is read on, or `None` for its own
        /// run to its mark — the same field a claim writes, read the same way.
        arm: Option<&'static str>,
        /// That claim's `quantity`. Every claim on that step and arm naming it must agree.
        quantity: &'static str,
        /// Which side of that value this sentence prints.
        states: QuotedAs,
    },
    /// **When** a claim was read — its `read_at_s`, not its value.
    ///
    /// [`Tie::Quoted`] answers "what did that measurement come to"; this answers "at what
    /// instant was it taken", and the two are different sentences. The gap it closes was
    /// named by an earlier slice and left open: `docs/plans/path-ledger-weaker-short.md`
    /// records an instruction to run "to about 400 s" leaving the page because no file
    /// decides it, and says in as many words that "a tie reading a claim's own `read_at_s`
    /// would have declared from both sides".
    ///
    /// **It is not free, and that is the whole argument for it.** An instant an author
    /// picks looks like a number the author also supplies — the circularity that fence was
    /// guarding against. But a claim's `read_at_s` is pinned by that claim's own **value**:
    /// `surface_gap_neg_pts:360` asserts the row reads 5.80 *at 360 s*, so moving the
    /// instant reddens the value check. Tying a sentence to it is therefore tying it to a
    /// trajectory, one step removed, exactly as [`Tie::Quoted`] is.
    ///
    /// This is the arm the elliptical list items reach through [`Accounted::ReadAt`] when
    /// they sit inside a claim's own literal. It exists for the sentence that names the same
    /// instant somewhere else — *"Across that last stretch from six minutes on"* — where the
    /// positional fence in [`claimed_accounting`] correctly refuses to help.
    Instant {
        /// The lesson whose claim was read at it.
        step: &'static str,
        /// Which of that lesson's `[[arm]]`s, or `None` for its own run.
        arm: Option<&'static str>,
        /// That claim's `quantity`. Every claim on that step and arm naming it must have
        /// been read at the same instant, on [`Tie::Quoted`]'s terms and for its reason.
        quantity: &'static str,
    },
    /// A rate the prose states **per second** where the page holds a period in
    /// milliseconds — `1000 / ms`.
    ///
    /// A unit conversion and nothing more, on exactly [`Tie::Hours`]'s terms: that one turns
    /// the seconds a file holds into the hours a sentence writes, and this one turns a
    /// period into the frequency a sentence writes. *"These two numbers are sampled four
    /// times a second"* is `CELLS_PERIOD_MS` said the other way up, and a reciprocal is not
    /// something `pow10` can express — which is why it is a variant rather than a field.
    ///
    /// Rounds like a computed tie, for the reason every conversion here does: the sentence
    /// commits to the places it prints and a division rarely lands on them.
    PerSecond(&'static Tie),
}

/// Which side of a quoted claim's value the quoting sentence prints.
///
/// The claims file has had this distinction since it was written — [`States::Complement`],
/// "the sentence prints how far *below one* the value is" — and step 13's own 8 % claim
/// uses it on the very quantity step 13 then quotes step 12 for. So the tie takes the
/// claims file's word rather than growing a second vocabulary for the same idea.
///
/// The complement is taken **before** the rule's `pow10`, which is what makes step 12's
/// `0.995238` come out as the `0.5 %` step 13 prints: `1 − 0.995238`, then a hundred, then
/// rounded to the one place the sentence commits to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuotedAs {
    /// The sentence prints the value itself.
    Same,
    /// The sentence prints `1 − value`. Fenced by nothing but the comparison: a complement
    /// pointed at a quantity that is not a fraction cannot round to the token.
    Complement,
}

/// What a [`Tie::Derived`] does to its operands.
///
/// Declared per rule rather than inferred, on the same terms as everything else in this
/// taxonomy: an author says what the sentence does, and the file says what the numbers are.
///
/// A separate type from [`Op`], which is check 6's and comes out of the claims file, on the
/// same terms the two `Derived` arms are separate: one scan reads a claimed sentence and the
/// other reads a whole step, and an operation shared between them would be one more place
/// the two could quietly answer differently. It uses check 6's word for division so that a
/// reader meeting both meets one vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LedgerOp {
    /// Every operand multiplied. Order is irrelevant, which is why the arm shipped without
    /// this field at all.
    Product,
    /// The first operand less every one after it. **Order is the claim**: reversed, step
    /// 23's `2.5` becomes −2.5 and the sentence is about a cell that has been overfilled.
    Difference,
    /// The first operand divided by every one after it. Order is the claim here too, and a
    /// zero divisor resolves to nothing rather than to an infinity the comparison would
    /// then round.
    Ratio,
}

/// One operand of a [`Tie::Derived`] — something the *sentence itself* supplies.
///
/// Never a value: an operand names where in the sentence to read the number, and the number
/// comes from the prose. Declaring `6.0` here would be the declared identity the ledger's
/// design refuses, because the author would then be supplying both sides of the arithmetic.
#[derive(Debug)]
enum Operand {
    /// A numeral this sentence prints, written exactly as it writes it.
    ///
    /// Resolved by scanning the sentence around the number being accounted, so an operand
    /// in a *different* sentence of the step is not reachable — the arm is about a figure
    /// worked out from what a reader can see in one breath. It must resolve to exactly one
    /// token, must not be the number being accounted, and must itself be accounted for by
    /// some other arm.
    Sibling(&'static str),
    /// A numeral this sentence spells **in letters** — "six of these in series".
    ///
    /// Resolved through [`WORD_NUMERALS`], the same table `spells` reads, so a word means
    /// one number in this file rather than two. The word must appear in the rule's own
    /// phrase ([`every_ledger_rule_is_a_phrase_and_is_used`]), which is what pins it to the
    /// sentence: the phrase match already requires those exact words around the number.
    ///
    /// **This is not the word scanner, and the difference still matters.** [`spelled_numbers`]
    /// reads a step's English quantities wherever the ledger's `spelled` list says to, and
    /// this reads one word an author names in one rule. On a step that is not word-scanned —
    /// which is every step but one — an English quantity is as invisible as it ever was, "all
    /// but three points" in this very step among them.
    Word(&'static str),
}

/// A number the lesson block sets and the page acts on.
///
/// Deliberately an enum of *named controls* rather than "any numeric field of the block",
/// and the reason is the same one [`Accounted::Setting`] gives for tying itself to a
/// trajectory: step 18's prose prints a `10` that is a step length, and the block beside it
/// carries `speed_x: 10000` and `dt: 0.5`. An arm that accounted a token against whatever
/// field of the block happened to hold that number would be right off the wrong field —
/// green, and still green the day one of the two moves.
///
/// One variant per control a ledgered step has so far printed, and the next one a ledgered
/// step prints is where the next gets added. The count is deliberately not written here: it
/// was "four" for three slices after the enum had six, which is the self-description defect
/// `docs/plans/path-self-description-sweep.md` swept nine instances of — and a count spelled
/// in a word is invisible to every scanner in this file.
#[derive(Debug, Clone, Copy)]
enum Control {
    /// The demand box, in the unit the box takes — amps, discharge-positive.
    DemandValue,
    /// The ambient slider, in the unit the slider takes — °C.
    ///
    /// **A level, where [`Accounted::Setting`] reads a step.** That arm's docs say a
    /// sentence printing an ambient *level* "still fails here loudly, because nothing in
    /// the path prints one" — true of the claims scan, and step 23 is the sentence that
    /// makes it false of this one: *"same chemistry, same 1S1P, same 25 °C, same
    /// half-second step"*. A level is what a held control is; a step is what a reader dials
    /// in at the mark. The two scans read different sentences, so they read different
    /// things, and neither is the other's fallback.
    ///
    /// Read in °C rather than K because that is the unit the slider and the sentence both
    /// speak. The offset does not cancel here the way it does in a difference.
    Ambient,
    /// How long the pulse program holds the current on \[s\].
    ///
    /// Not a demand the engine has: the page runs the train on top of it, which is why
    /// step 12's own prose introduces the leg lengths as the program rather than as
    /// anything the scenario file decides. `pulse_train_ecm.toml` is an initial condition
    /// and says nothing about legs — read its header, which says so at length.
    PulseOn,
    /// How long it then rests \[s\].
    PulseOff,
    /// The step's **mark** \[s\] — how far the page runs before it stops on its own.
    ///
    /// A control in the sense every other variant is: `pathArrived` stops the run there, and
    /// step 8's prose states it as the thing the reader is about to watch ("This runs to
    /// 200 000 s of simulation"). [`Tie::Clock`] reads the same field *rendered*, which is a
    /// different sentence — step 22 quotes the panel's `19.3h` and this one quotes the
    /// seconds.
    Until,
    /// The speed slider \[×\] — steps per frame.
    ///
    /// The one control here that the trajectory cannot see: it changes how fast a reader
    /// watches and never `dt`, which is what step 8's own lesson comment says and why its
    /// two-and-a-half days are twenty seconds of watching. Read off [`Lesson::speed_x`];
    /// a step that leaves the slider alone resolves to nothing rather than to the page's
    /// default, because a sentence printing a multiplier is printing one this block set.
    Speed,
    /// The **`dt` box** \[s\] — the step length the page hands the engine.
    ///
    /// The last of the boxes to get a variant here, and step 18 is why: it is the first
    /// ledgered step whose subject *is* the step length, so it is the first whose prose
    /// prints one as a setting rather than assuming it. Three sentences do — *"puts it
    /// back to 0.5"*, *"Put it back to 0.5 before moving on"*, and, through
    /// [`Tie::OnArm`], *"Put `dt` up to 5 s"*.
    ///
    /// [`Accounted::Setting`] has read the same box since it was built, off the claim's own
    /// trajectory; this is the ledger's side of it, read off the lesson block or off a named
    /// arm. The two scans answer different sentences, exactly as they do for the ambient.
    Dt,
    /// The CC-CV taper \[A\] — the current the page's completion test compares against.
    ///
    /// A control in the same sense [`Self::DemandValue`] is: it is a field of the demand box
    /// the reader can type in, and `ccCvDone` reads it. Step 9's prose is careful about whose
    /// number it is — *"the only thing that ends this charge is the current falling below the
    /// 0.15 A cutoff"* — and the lesson block's own comment says the same: a scenario is an
    /// initial condition and never a demand program, so the cutoff is the page's and not the
    /// file's.
    ///
    /// Reads as nothing on any program but CC-CV, which is what keeps it off the other two
    /// currents in the path: a `Pulse` step has no taper, and a rule asking for one there is
    /// a rule on the wrong step.
    Taper,
    /// The CC-CV **per-cell target** \[V\] — the voltage each cell is being charged toward.
    ///
    /// The third of the CC-CV group's three fields to get a variant, after the charge
    /// current ([`Self::DemandValue`]) and [`Self::Taper`]. Step 11 is why: it is the first
    /// ledgered step whose prose prints the target the page is aiming at rather than the
    /// current it is aiming with — *"only 25 mV short of the 16.80 V it is aiming for"* —
    /// and 16.80 is this box times the series count, which is `ccCvNote`'s own arithmetic
    /// (`const target = cfg.v_cell * series`).
    ///
    /// **Not the chemistry's ceiling, though the two spell the same digits here.** This
    /// step also says the top cell *"has already crossed 4.20"*, and that 4.20 is
    /// `cell.v_max` — the limit the over-voltage rung trips on. A page whose CC-CV box was
    /// retyped to 4.15 would move the first number and leave the second where it is, which
    /// is the whole reason they are read off different files rather than off whichever one
    /// was reached first. See `docs/plans/path-ledger-what-protection-costs.md`.
    ///
    /// Reads as nothing on any program but CC-CV, exactly as the taper does.
    CcCvVoltage,
}

/// The vocabulary, one entry per way a ledgered step names a number some file decides.
///
/// Every rule is required to match something ([`every_ledger_rule_is_a_phrase_and_is_used`]),
/// so a rule left behind by a prose edit fails here instead of sitting in the list looking
/// like coverage.
const LEDGER_VOCABULARY: &[LedgerRule] = &[
    // Step 1 — the first thing anyone reads. Eight of its twelve unaccounted numerals are
    // constants: the charge it starts at, the demand box twice, the rate that box works out
    // to, the cell's nameplate, the floor the chemistry declares, and two ordinals pointing
    // at the far end of the path. The ninth is the `0` of `R0`, which is a section of the
    // chemistry file and not a quantity at all.
    LedgerRule {
        phrase: "at {n} % charge, isothermal",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    LedgerRule {
        // The demand box and the rate it works out to, in the sentence that introduces both.
        // The nameplate is spelled whole here — `2.303451`, not the `2.303` this sentence
        // used to print — for the reason step 16's `5.15` and `15.46` were: a constant is
        // compared exactly, and a rounded restatement of one is neither the file's number
        // nor a computed quantity. See `docs/plans/path-ledger-dfn-step.md`.
        phrase: "{n} A out of this cell is {n} C \u{2014} it holds {n} Ah",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Chemistry("cell.capacity_ah"),
            ]),
            Tie::Chemistry("cell.capacity_ah"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The `0` of `R0` — a section of the chemistry file, in the same position as the `0`
        // of `[r0]` on step 13 and the `50` of `M50` on step 12. `Tie::Name`'s own docs
        // record this token as the one that "had no field to point at and was reworded away
        // instead", and that has been out of date since the arm was built: this chemistry's
        // provenance is where `R0` is DECLARED a placeholder, which is a stronger tie than
        // the reword it replaces. `RC` sits two characters later in the same string and the
        // digit run after it is empty, so the prefix reaches one number and not two.
        phrase: "drops instantly by `I\u{b7}R{n}`",
        ties: &[Tie::Name {
            field: "meta.provenance",
            prefix: "R",
        }],
        pow10: 0,
    },
    LedgerRule {
        // The floor the flag is about. Not the page's choice and not the scenario's — every
        // LFP cell in the repo stops here.
        phrase: "gone under the {n} V this chemistry file declares",
        ties: &[Tie::Chemistry("cell.v_min")],
        pow10: 0,
    },
    LedgerRule {
        // The same box again, in the sentence about what the flag does NOT do. Two rules
        // rather than one loose one, on the same terms as step 6's pair.
        phrase: "the demand box still says {n} A",
        ties: &[Tie::Setting(Control::DemandValue)],
        pow10: 0,
    },
    LedgerRule {
        // **Both ends of the phrase are load-bearing, and the short version was a trap.**
        // `step {n} of this path, and step {n}` also matches step 2 — *"That fall is step 20
        // of this path, and step 1's cell does the same thing"* — where the second slot is a
        // back-reference and not the ordinal of `what-it-cost` at all. Nothing would have
        // said so until step 2 was ledgered, because a rule does not know which step it was
        // written for and the scan only reads ledgered ones. `the subject of` and `is about`
        // are what keep it here: step 2 has neither.
        phrase: "the subject of step {n} of this path, and step {n} is about",
        ties: &[Tie::Ordinal("past-empty"), Tie::Ordinal("what-it-cost")],
        pow10: 0,
    },
    // Step 2 — the second cell, the third cell, and the two runs next door it compares
    // itself with. Sixteen of its numerals are constants or arithmetic over them; the rest
    // are claims, four of them on arms that leave this step's own file behind.
    LedgerRule {
        // The rate, twice, and step 1's rate beside it. Three ratios of a demand box to a
        // nameplate, printed to two places and then to three — which is why a ratio is
        // compared at the prose's own precision and a constant is not.
        //
        // The last of the four is read through `Elsewhere` rather than spelled: 0.868 is
        // 2 A of a 2.303451 A·h cell, and both of those are step 1's. A rule that read this
        // step's own box and cell would answer 0.867 and the sentence would be wrong about
        // the only thing it is comparing.
        phrase: "discharged at the same {n} C \u{2014} {n} here against step {n}'s {n}, because",
        ties: &[
            Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Chemistry("cell.capacity_ah"),
            ]),
            Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Chemistry("cell.capacity_ah"),
            ]),
            Tie::Ordinal("bare-curve"),
            Tie::Elsewhere {
                step: "bare-curve",
                tie: &Tie::Ratio(&[
                    Tie::Setting(Control::DemandValue),
                    Tie::Chemistry("cell.capacity_ah"),
                ]),
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        // The two nameplates and the two demand boxes, in the sentence that puts them side
        // by side. `2.303451` and not the `2.303` this sentence used to print: a constant is
        // compared exactly, and a rounded restatement of one is neither the file's number nor
        // a computed quantity. Step 1's own prose has spelled it whole since it was ledgered
        // — see `docs/plans/path-ledger-dfn-step.md` for where that rule came from.
        phrase: "This cell holds {n} Ah against the LFP cell's {n}, so the same C-rate is {n} A rather than {n}.",
        ties: &[
            Tie::Chemistry("cell.capacity_ah"),
            Tie::Elsewhere {
                step: "bare-curve",
                tie: &Tie::Chemistry("cell.capacity_ah"),
            },
            Tie::Setting(Control::DemandValue),
            Tie::Elsewhere {
                step: "bare-curve",
                tie: &Tie::Setting(Control::DemandValue),
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        // Both empty-times and the gap between them. The two instants are quoted off the two
        // steps' own claims — this one quotes ITSELF, which is what `Tie::Quoted` exists to
        // allow and what keeps the pair from being two free figures — and the gap is this
        // sentence's own subtraction over them.
        //
        // **The gap used to be a word.** It said "within eight seconds of each other", which
        // is true (the gap is 7.5 s) and invisible: `written_numbers` finds digits only, so
        // a hedge in letters is outside every scan in this file. Spelling it makes the
        // sentence exact and puts it on the two numbers it is a subtraction of.
        phrase: "Both empty within {n} s of each other \u{2014} {n} s and {n} s \u{2014}",
        ties: &[
            Tie::Derived {
                op: LedgerOp::Difference,
                operands: &[Operand::Sibling("4154.0"), Operand::Sibling("4146.5")],
            },
            Tie::Quoted {
                step: "bare-curve",
                arm: None,
                quantity: "flag_first_s:SOC_CLAMPED_LOW",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "same-discharge-other-chemistry",
                arm: None,
                quantity: "flag_first_s:SOC_CLAMPED_LOW",
                states: QuotedAs::Same,
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        // The third cell's file, named twice — once where the reader loads it and once where
        // they are told to leave it loaded. Two rules rather than one loose one, each naming
        // the arm whose instruction that sentence is, on step 6's precedent.
        phrase: "Load `cc_discharge_lgm{n}` from the picker",
        ties: &[Tie::Picker {
            arm: "the third cell",
            prefix: "cc_discharge_lgm",
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "Leave `cc_discharge_lgm{n}` loaded",
        ties: &[Tie::Picker {
            arm: "the third cell at this step's own current",
            prefix: "cc_discharge_lgm",
        }],
        pow10: 0,
    },
    LedgerRule {
        // The third cell's nameplate and the parameter set its curve was fitted from — both
        // read off the file the arm PICKS, which is the half of `Tie::OnArm` this step built.
        // Neither is in this step's own chemistry: `cc_discharge_nmc` holds 3.0 Ah and has no
        // PyBaMM source at all, which its own provenance says at length.
        phrase: "a {n} Ah cell fitted from PyBaMM's Chen{n}",
        ties: &[
            Tie::OnArm {
                arm: "the third cell",
                tie: &Tie::Chemistry("cell.capacity_ah"),
            },
            Tie::OnArm {
                arm: "the third cell",
                tie: &Tie::Name {
                    field: "meta.provenance",
                    prefix: "Chen",
                },
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        // The current the reader types on that arm, and the rate it works out to on that
        // cell. A ratio of two `OnArm` reads — the box the arm sets over the nameplate of the
        // file it picked — which is the first tie in this table whose BOTH sides belong to an
        // arm rather than to the step.
        //
        // The rate moved with the arm that measured it: this printed 0.868, which is step 1's
        // figure. See `docs/plans/path-third-cell.md`.
        phrase: "put the demand box up to {n} A, which is the same {n} C on the bigger cell",
        ties: &[
            Tie::OnArm {
                arm: "the third cell",
                tie: &Tie::Setting(Control::DemandValue),
            },
            Tie::Ratio(&[
                Tie::OnArm {
                    arm: "the third cell",
                    tie: &Tie::Setting(Control::DemandValue),
                },
                Tie::OnArm {
                    arm: "the third cell",
                    tie: &Tie::Chemistry("cell.capacity_ah"),
                },
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The step's own box again, in the sentence that asks the reader to put it back. The
        // second picker arm overrides nothing, so this is the lesson's control and not an
        // arm's — which is exactly what the arm's own note says makes it an arm.
        phrase: "put the box back to {n} A",
        ties: &[Tie::Setting(Control::DemandValue)],
        pow10: 0,
    },
    LedgerRule {
        phrase: "the same pair of flags step {n} ended on",
        ties: &[Tie::Ordinal("bare-curve")],
        pow10: 0,
    },
    LedgerRule {
        // The floor the operating-point flag is about, on this chemistry. Its twin two
        // sentences later reads the same field under a different phrase, on step 6's terms:
        // two rules rather than one loose one.
        phrase: "This cell's declared floor is {n} V",
        ties: &[Tie::Chemistry("cell.v_min")],
        pow10: 0,
    },
    LedgerRule {
        // How far each cell's voltage flag precedes its coulomb counter — this step's 42.5 s
        // and step 1's 8.5 s, each the difference between two flag times that step's own
        // claims already pin. The sentence is the comparison, so both sides are read the same
        // way and a change to either trajectory reddens it rather than quietly rescaling it.
        //
        // Step 1's figure used to be spelled "eight and a half", which is the same
        // invisible-to-every-scanner shape the 7.5 above was in.
        phrase: "a full {n} seconds ahead of the counter, where step {n}'s flat cell managed {n} s",
        ties: &[
            Tie::Difference(&[
                Tie::Quoted {
                    step: "same-discharge-other-chemistry",
                    arm: None,
                    quantity: "flag_first_s:SOC_CLAMPED_LOW",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "same-discharge-other-chemistry",
                    arm: None,
                    quantity: "flag_first_s:OPERATING_POINT_OUT_OF_WINDOW",
                    states: QuotedAs::Same,
                },
            ]),
            Tie::Ordinal("bare-curve"),
            Tie::Difference(&[
                Tie::Quoted {
                    step: "bare-curve",
                    arm: None,
                    quantity: "flag_first_s:SOC_CLAMPED_LOW",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "bare-curve",
                    arm: None,
                    quantity: "flag_first_s:OPERATING_POINT_OUT_OF_WINDOW",
                    states: QuotedAs::Same,
                },
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "the mark is {n},",
        ties: &[Tie::Setting(Control::Until)],
        pow10: 0,
    },
    LedgerRule {
        // How long the run goes on after the cell is empty — the mark less the instant it
        // emptied, both printed in this same sentence. The operands are the sentence's own,
        // which is what `Tie::Derived` is for, and each is accounted by something that is not
        // a derivation: the mark by the rule above, the crossing by this step's own claim.
        phrase: "it spends the last {n} seconds",
        ties: &[Tie::Derived {
            op: LedgerOp::Difference,
            operands: &[Operand::Sibling("4200"), Operand::Sibling("4154")],
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "under the {n} V a datasheet would call the cutoff",
        ties: &[Tie::Chemistry("cell.v_min")],
        pow10: 0,
    },
    LedgerRule {
        // How long step 1's cell goes on past its own knee — that step's mark less the
        // instant it emptied. Read as step 1's mark through `Elsewhere` and not as
        // `Setting(Until)`, though the two steps mark at the same 4200 s: the sentence is
        // about the run next door, and a rule that read this step's own field would be the
        // right number off the wrong file.
        //
        // **The two marks being equal makes this wrapper's own arm unfalsifiable here, and it
        // is not unfalsifiable in the file.** No perturbation of this rule can tell
        // `Elsewhere` from a bare `Setting`, because both answer 4200. What proves the
        // wrapper swaps the lesson for a `Setting` read is the rule two above, whose
        // `Elsewhere{bare-curve, Setting(DemandValue)}` answers 2 where this step's own box
        // says 2.6 — the same line of `tie_values`, asked where the two lessons disagree.
        //
        // **The figure moved to make this possible, and it is the honest one.** The sentence
        // said "53 seconds", where the subtraction is 53.5 — step 1's own claim note admits
        // its `53` is a truncation with zero margin that "could equally say 54". A computed
        // tie is compared at the prose's own precision, so 53.5 printed as `53` fails: the
        // arithmetic rounds to 54. Spelling the half is what a sentence quoting a subtraction
        // owes it.
        phrase: "does the same thing for the {n} seconds after its own knee",
        ties: &[Tie::Difference(&[
            Tie::Elsewhere {
                step: "bare-curve",
                tie: &Tie::Setting(Control::Until),
            },
            Tie::Quoted {
                step: "bare-curve",
                arm: None,
                quantity: "flag_first_s:SOC_CLAMPED_LOW",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // Two back-references, and the reason the step-1 rule above had to grow its second
        // half: `step {n} of this path, and step {n}` matches this sentence too, where the
        // second slot is step 1 and not `what-it-cost`. Nothing said so until this step was
        // scanned, because a rule does not know which step it was written for.
        phrase: "That fall is step {n} of this path, and step {n}'s cell",
        ties: &[Tie::Ordinal("past-empty"), Tie::Ordinal("bare-curve")],
        pow10: 0,
    },
    // Step 3 — the pack's topology and its manufacturing spread.
    LedgerRule {
        phrase: "{n} in series",
        ties: &[Tie::Scenario("pack.series")],
        pow10: 0,
    },
    LedgerRule {
        phrase: "{n} in parallel",
        ties: &[Tie::Scenario("pack.parallel")],
        pow10: 0,
    },
    LedgerRule {
        phrase: "with {n} % capacity",
        ties: &[Tie::Scenario("pack.scatter.capacity_sigma")],
        pow10: 2,
    },
    LedgerRule {
        phrase: "and {n} % resistance scatter",
        ties: &[Tie::Scenario("pack.scatter.r0_sigma")],
        pow10: 2,
    },
    // Step 4 — what the BMS's own instrument is wrong by. The whole lesson is that these
    // are the errors it cannot know about, so they are the scenario's numbers and not
    // anything the engine produces.
    LedgerRule {
        phrase: "current sensor reads {n} mA high",
        ties: &[Tie::Scenario("pack.bms.current_offset_a")],
        pow10: 3,
    },
    LedgerRule {
        phrase: "with {n} mA of noise",
        ties: &[Tie::Scenario("pack.bms.current_noise_sigma_a")],
        pow10: 3,
    },
    LedgerRule {
        phrase: "started {n} % wrong",
        ties: &[Tie::Scenario("pack.bms.initial_soc_error")],
        pow10: 2,
    },
    // Step 5 — the two scheduled faults. The prose's whole first paragraph is a reading of
    // the `[[faults]]` tables, which is what makes this step free to ledger.
    LedgerRule {
        phrase: "At t = {n} s this scenario springs",
        ties: &[Tie::Scenario("faults.*.at_s")],
        pow10: 0,
    },
    LedgerRule {
        phrase: "a {n} \u{3a9} internal short",
        ties: &[Tie::Scenario("faults.*.fault.SoftInternalShort.ohms")],
        pow10: 0,
    },
    LedgerRule {
        phrase: "on cell ({n},{n})",
        ties: &[
            Tie::Scenario("faults.*.fault.SoftInternalShort.s"),
            Tie::Scenario("faults.*.fault.SoftInternalShort.p"),
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "a +{n} mV offset",
        ties: &[Tie::Scenario("faults.*.fault.SensorOffset.offset")],
        pow10: 3,
    },
    LedgerRule {
        phrase: "Group {n}'s sensed voltage",
        ties: &[Tie::Scenario(
            "faults.*.fault.SensorOffset.sensor.GroupVoltage",
        )],
        pow10: 0,
    },
    LedgerRule {
        phrase: "by {n} mV and stays there",
        ties: &[Tie::Scenario("faults.*.fault.SensorOffset.offset")],
        pow10: 3,
    },
    // Step 6 — the first ledgered step whose numbers are not all scenario constants. Its
    // opening sentence prints the pack, the pack's capacity, the cell's rating and the
    // demand in one breath, which is why it took three arms to close.
    LedgerRule {
        // Not `"{n}S{n}P"`: a phrase has to carry words (see the rule check below), and
        // this is the sentence those two digits sit in. Step 3 writes the same topology as
        // "4 in series, 2 in parallel" and is covered by the two rules at the top — one
        // topology, two vocabularies, because two sentences say it two ways.
        phrase: "cells in {n}S{n}P is",
        ties: &[Tie::Scenario("pack.series"), Tie::Scenario("pack.parallel")],
        pow10: 0,
    },
    LedgerRule {
        // The sentence's own arithmetic, and the whole reason `Product` rounds: cells in
        // parallel add capacity, so pack Ah is the cell's `capacity_ah` times `parallel` —
        // 4.606902, which the prose prints as 4.61.
        phrase: "is {n} Ah at pack level",
        ties: &[Tie::Product(&[
            Tie::Chemistry("cell.capacity_ah"),
            Tie::Scenario("pack.parallel"),
        ])],
        pow10: 0,
    },
    LedgerRule {
        phrase: "the chemistry is rated {n} C continuous",
        ties: &[Tie::Chemistry("cell.max_discharge_c")],
        pow10: 0,
    },
    // The demand, stated twice: once as what the reader asks for and once as what the box
    // still says while the BMS refuses it. Two rules rather than one loose one, on the
    // same terms as the two topology vocabularies above.
    LedgerRule {
        // No full stop after the `{n}`, though the sentence ends there. A number's `len`
        // covers the run the scanner *trimmed* — `40.` is scanned as `40` and still spans
        // three bytes — so a phrase can never put a literal part immediately after a number
        // that ends a sentence. The prefix is what makes this rule specific anyway.
        phrase: "We are asking for {n}",
        ties: &[Tie::Setting(Control::DemandValue)],
        pow10: 0,
    },
    LedgerRule {
        phrase: "the demand box still reads {n}",
        ties: &[Tie::Setting(Control::DemandValue)],
        pow10: 0,
    },
    // Step 7 — the same demand with the BMS taken out. Five of its fifteen unaccounted
    // numerals are constants, and four of the five are the two controls this step is about:
    // the demand box, printed in three separate sentences, and the mark. The fifth is the
    // floor the chemistry declares. Everything else in the step is a measurement.
    LedgerRule {
        // The demand box, in the sentence that opens the step by saying nothing about it
        // has changed. Its own rule rather than a share of step 6's, because a phrase is
        // matched against whichever step is being scanned and step 6's two say "asking for"
        // and "the demand box still reads" — neither of which this sentence carries.
        phrase: "Same pack, same {n} A, BMS removed",
        ties: &[Tie::Setting(Control::DemandValue)],
        pow10: 0,
    },
    LedgerRule {
        // The floor the datasheet would print, which on this page is the chemistry file's
        // own `v_min` and not a number the step chose. Step 1 states the same constant in
        // its own words ("gone under the {n} V this chemistry file declares"), and the two
        // phrases share nothing but the number — which is the point of keeping them apart.
        phrase: "dives well under the {n} V the datasheet allows",
        ties: &[Tie::Chemistry("cell.v_min")],
        pow10: 0,
    },
    LedgerRule {
        // The demand box beside the number step 6 MEASURED under it. This is the sentence
        // `path-claims.toml`'s note on `pinned near 14 A` was written for: the ledger's
        // claimed arm is positional, so a `14` here is not accounted by step 6's claim on
        // its own prose — it has to be quoted, and quoting is what makes this sentence go
        // red if step 6's clamp ever moves.
        //
        // Both of step 6's `i_at` claims answer 13.8207, which is what `Tie::Quoted`'s
        // agreement fence requires, and `about 14` is that value at the precision this
        // sentence prints.
        phrase: "the BMS clamped your {n} A down to about {n} and",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Quoted {
                step: "protection-on",
                arm: None,
                quantity: "i_at",
                states: QuotedAs::Same,
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        // The two numbers the rest of the step follows from: how hard the pack is pulled,
        // and for how long. The mark is a control in `Control::Until`'s sense — the page
        // stops there — and this sentence is the one that tells the reader so.
        phrase: "{n} A for {n} s is more charge than this pack holds",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Setting(Control::Until),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The back-reference at the end of the step: the spread the eight cells cross over
        // is the manufacturing scatter step 3 introduced. An ordinal, so inserting a step
        // ahead of `pack-disagrees` turns this sentence red rather than leaving it quietly
        // pointing at the wrong lesson.
        //
        // The phrase carries `capacity` and `of step` on purpose. Last slice's sweep found
        // three rules in this table that reach a step they were not written for, two of
        // them step 3's own scatter rules; a bare `step {n}` here would have been a fourth
        // and would have matched every cross-reference in the path.
        phrase: "the capacity scatter of step {n}",
        ties: &[Tie::Ordinal("pack-disagrees")],
        pow10: 0,
    },
    // Step 12 — the pulse train. Its two leg lengths are the demand program the PAGE runs,
    // its cell is named rather than measured, its starting charge is the scenario's, and
    // the node it steps over is the chemistry's. Nothing in the first sentence is a number
    // the engine produces, and every one of them decides what the reader sees.
    LedgerRule {
        // Both legs in one rule, because the sentence introduces the program in one breath
        // and neither number means anything without the other.
        phrase: "for {n} s, `Rest` for {n}, repeat",
        ties: &[
            Tie::Setting(Control::PulseOn),
            Tie::Setting(Control::PulseOff),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The part number. See `Tie::Name` for why the `M` is carried in the tie as well as
        // in the phrase: here it keeps the arm off the `21700` and the `811` beside it.
        phrase: "the LG M{n} from",
        ties: &[Tie::Name {
            field: "meta.name",
            prefix: "M",
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "from step {n}'s aside",
        ties: &[Tie::Ordinal("same-discharge-other-chemistry")],
        pow10: 0,
    },
    LedgerRule {
        // `initial_soc`, and the scenario's own comment says why it is 0.90 rather than
        // 1.00 — a node of the same [ocv] table the rule below is about.
        phrase: "at {n} % charge, running",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    LedgerRule {
        phrase: "table's node at {n} % charge",
        ties: &[Tie::Member("ocv.soc.*")],
        pow10: 2,
    },
    // Step 13 — the same train on the single-particle model. Ten of its twenty numerals are
    // claimed measurements; not one of the other ten is a measurement at all. Two are the
    // scenario's, two are digit runs inside the chemistry's own provenance string, three are
    // numbers the step NEXT DOOR measured, and two are that chemistry's RC time constants,
    // which the file states as a resistance and a capacitance and never as a time.
    LedgerRule {
        // The charge all three pulse steps start from. A different sentence from step 12's
        // "at 90 % charge, running", and deliberately a different rule: a phrase generous
        // enough to cover both would be a phrase generous enough to cover a third.
        phrase: "the same {n} %, the same train",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    LedgerRule {
        // The one field that makes this step's file differ from step 12's, and the scenario
        // header says why it is written out rather than defaulted: it is part of the
        // snapshot layout, so it is a number the file has to carry.
        phrase: "{n} shells deep",
        ties: &[Tie::Scenario("pack.cell_model.Spm.shells")],
        pow10: 0,
    },
    LedgerRule {
        // The parameter set's year, which is four fifths of its name. Same shape as step
        // 12's `M{n}`, off the provenance rather than the display name: `Chen` also occurs
        // there as an author's surname, and the digit run after THAT is empty and dropped.
        phrase: "PyBaMM's Chen{n} rather than fitted",
        ties: &[Tie::Name {
            field: "meta.provenance",
            prefix: "Chen",
        }],
        pow10: 0,
    },
    LedgerRule {
        // The `0` of `[r0]` — a section name, not a quantity, and the position the same
        // digit was in when `Tie::Name`'s docs recorded it as reworded away. This one has a
        // field to point at: the chemistry's provenance is where those blocks are CALLED
        // placeholders, which is exactly what the sentence is telling the reader. `[[rc]]`
        // in the same string contains `[r` too, and the digit run after it is empty.
        phrase: "file's `[r{n}]` and `[[rc]]` are labelled placeholders",
        ties: &[Tie::Name {
            field: "meta.provenance",
            prefix: "[r",
        }],
        pow10: 0,
    },
    LedgerRule {
        // The circuit's first rebound, measured on step 12 and quoted here so the reader
        // has something to be four times smaller than.
        phrase: "where the circuit's was {n}",
        ties: &[Tie::Quoted {
            step: "circuit-repeats-itself",
            arm: None,
            quantity: "pulse_rebound_mv:1",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The same figure again, and the sentence is about the FIFTH this time — "was the
        // same five times" — so the tie is to step 12's fifth rebound and not to its first.
        // The two claims are 74.767 and 74.770, which is the sentence's own point.
        phrase: "its {n} mV was the same five times",
        ties: &[Tie::Quoted {
            step: "circuit-repeats-itself",
            arm: None,
            quantity: "pulse_rebound_mv:5",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The circuit's tail, which step 12 prints from the other end: it claims 99.5 % has
        // ARRIVED by the half-way point of the rest, and this sentence prints the 0.5 % that
        // has not. Hence the complement.
        //
        // THE FRAME GAP IS STATED RATHER THAN HIDDEN: step 12's claim is on its FIRST rest
        // and this sentence is about the fifth. They are the same number only because the
        // circuit is linear and time-invariant — step 12's whole lesson, and separately
        // claimed there as five rebounds identical to four decimal places. A re-fit that
        // made the circuit's late rests differ from its early ones would leave this green.
        // The alternative, a claim on step 12's fifth rest, has no admissible `tol_from`:
        // `spelled` and `tighter` both need a spelled number and step 12 prints none for it,
        // and `grid` is fenced to step-grid times.
        phrase: "against the circuit's {n} %",
        ties: &[Tie::Quoted {
            step: "circuit-repeats-itself",
            arm: None,
            quantity: "pulse_rebound_arrived:1",
            states: QuotedAs::Complement,
        }],
        pow10: 2,
    },
    LedgerRule {
        // The two RC time constants, which the chemistry file never writes: it writes a
        // resistance and a capacitance per pair, and tau is their product. One rule and two
        // ties, because the sentence names both pairs in one breath and the indexed path is
        // what keeps each product on its own pair — `rc.*.r_ohms` reaches both resistances,
        // and a factor reaching two values resolves to nothing by design.
        phrase: "the RC pairs are {n} s and {n} s",
        ties: &[
            Tie::Product(&[
                Tie::Chemistry("rc.0.r_ohms"),
                Tie::Chemistry("rc.0.c_farad"),
            ]),
            Tie::Product(&[
                Tie::Chemistry("rc.1.r_ohms"),
                Tie::Chemistry("rc.1.c_farad"),
            ]),
        ],
        pow10: 0,
    },
    // Step 14 — three times the current, and the last of the three pulse steps. Nineteen of
    // its thirty-one unaccounted numerals are here; the other twelve are claims, eight of
    // them on a new arm that walks to the circuit's file. What is striking about this step's
    // vocabulary is how much of it QUOTES: eight of these placeholders resolve to a claim on
    // step 12 or step 13 rather than to any field, because a step whose subject is "what does
    // three times the current buy" prints one measurement of its own for every one it is
    // comparing against.
    LedgerRule {
        // The charge all three pulse steps start from, in this step's own words. A third
        // phrase for one field, on the terms step 13's rule already states: a phrase generous
        // enough to cover two of them would be generous enough to cover a third.
        phrase: "back at {n} %",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    LedgerRule {
        // The rate this step's box works out to. The box is 15.459 A and the cell holds
        // 5.153198 Ah, so the quotient is 2.99989 and the sentence's `3` is that rounded to
        // the place it prints — a computed tie, on `Tie::Product`'s terms.
        phrase: "pulse current tripled to {n} C",
        ties: &[Tie::Ratio(&[
            Tie::Setting(Control::DemandValue),
            Tie::Chemistry("cell.capacity_ah"),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The question the step is about, and the two rates in it are two different lessons'
        // boxes over one cell's nameplate. `Elsewhere` for the first because 1 C is step 13's
        // run and this step never drives it.
        phrase: "does at {n} C, do I know what it does at {n} C?",
        ties: &[
            Tie::Elsewhere {
                step: "particle-remembers",
                tie: &Tie::Ratio(&[
                    Tie::Setting(Control::DemandValue),
                    Tie::Chemistry("cell.capacity_ah"),
                ]),
            },
            Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Chemistry("cell.capacity_ah"),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "For the circuit of step {n} the answer",
        ties: &[Tie::Ordinal("circuit-repeats-itself")],
        pow10: 0,
    },
    LedgerRule {
        // The circuit's 1 C figures, quoted from the lesson that measured them. The phrase
        // stops at the arrow in both cases, because what follows it is a measurement THIS
        // step makes and a claim on the new arm accounts for it — two readings of one number
        // is what `cover_by_rule`'s double-cover panic exists to refuse.
        phrase: "Its jump goes {n} \u{2192} ",
        ties: &[Tie::Quoted {
            step: "circuit-repeats-itself",
            arm: None,
            quantity: "pulse_jump_mv:1",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "its slow climb {n} \u{2192} ",
        ties: &[Tie::Quoted {
            step: "circuit-repeats-itself",
            arm: None,
            quantity: "pulse_rebound_mv:1",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The circuit's two ratios, each the arm's own claim over step 12's. **This is the
        // first rule in the file whose `Ratio` divides two QUOTED measurements**, and it is
        // what makes "triple the current and every term triples" an assertion rather than a
        // remark: move either lesson's box and the quotient moves off the printed number.
        phrase: "mV: **\u{d7}{n} and \u{d7}{n}**",
        ties: &[
            Tie::Ratio(&[
                Tie::Quoted {
                    step: "three-times-the-current",
                    arm: Some("the circuit at 3 C"),
                    quantity: "pulse_jump_mv:1",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "circuit-repeats-itself",
                    arm: None,
                    quantity: "pulse_jump_mv:1",
                    states: QuotedAs::Same,
                },
            ]),
            Tie::Ratio(&[
                Tie::Quoted {
                    step: "three-times-the-current",
                    arm: Some("the circuit at 3 C"),
                    quantity: "pulse_rebound_mv:1",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "circuit-repeats-itself",
                    arm: None,
                    quantity: "pulse_rebound_mv:1",
                    states: QuotedAs::Same,
                },
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The particle's 1 C figures, the same shape one model over.
        phrase: "instantaneous jump goes {n} \u{2192} ",
        ties: &[Tie::Quoted {
            step: "particle-remembers",
            arm: None,
            quantity: "pulse_jump_mv:1",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The saturating half — x1.87 — and the x3 it is NOT. That second number is the only
        // one in the sentence that is a ratio of two demand boxes rather than of two
        // measurements, which is exactly what the sentence is saying: the current tripled and
        // the answer did not.
        phrase: "mV \u{2014} **\u{d7}{n}**, not \u{d7}{n} \u{2014}",
        ties: &[
            Tie::Ratio(&[
                Tie::Quoted {
                    step: "three-times-the-current",
                    arm: None,
                    quantity: "pulse_jump_mv:1",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "particle-remembers",
                    arm: None,
                    quantity: "pulse_jump_mv:1",
                    states: QuotedAs::Same,
                },
            ]),
            Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Elsewhere {
                    step: "particle-remembers",
                    tie: &Tie::Setting(Control::DemandValue),
                },
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "Its slow climb goes {n} \u{2192} ",
        ties: &[Tie::Quoted {
            step: "particle-remembers",
            arm: None,
            quantity: "pulse_rebound_mv:1",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The accelerating half, and the number this slice MOVED. The prose said x6.01, which
        // is 103.9 divided by 17.3 — the two already-rounded millivolt figures beside it. The
        // measurements are 103.905970 and 17.268344 and their quotient is 6.0171, so the
        // sentence prints 6.02 now. It is the same defect `pulse_train_spm.toml`'s own header
        // records for its 4.5 mV ("subtracted from the three ROUNDED parts instead of
        // measured"), and this rule is what stops it recurring: nothing here divides a token.
        phrase: "which is **\u{d7}{n}** \u{2014} accelerating",
        ties: &[Tie::Ratio(&[
            Tie::Quoted {
                step: "three-times-the-current",
                arm: None,
                quantity: "pulse_rebound_mv:1",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "particle-remembers",
                arm: None,
                quantity: "pulse_rebound_mv:1",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        phrase: "without pausing, goes {n} \u{2192} ",
        ties: &[Tie::Quoted {
            step: "particle-remembers",
            arm: None,
            quantity: "pulse_sag_mv:1",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The sag's ratio — the one of the three a reader can take off the plot without
        // pausing, and the one that hides both effects under a mild-looking number.
        phrase: "mV: \u{d7}{n} \u{2014} a number that looks like",
        ties: &[Tie::Ratio(&[
            Tie::Quoted {
                step: "three-times-the-current",
                arm: None,
                quantity: "pulse_sag_mv:1",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "particle-remembers",
                arm: None,
                quantity: "pulse_sag_mv:1",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The step's headline, restating its own two ratios. Tied to the same pairs rather
        // than to the tokens two sentences up, so a sentence that drifted from its own
        // argument fails here instead of agreeing with a stale neighbour.
        phrase: "resistance can be {n} and {n} at once",
        ties: &[
            Tie::Ratio(&[
                Tie::Quoted {
                    step: "three-times-the-current",
                    arm: None,
                    quantity: "pulse_jump_mv:1",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "particle-remembers",
                    arm: None,
                    quantity: "pulse_jump_mv:1",
                    states: QuotedAs::Same,
                },
            ]),
            Tie::Ratio(&[
                Tie::Quoted {
                    step: "three-times-the-current",
                    arm: None,
                    quantity: "pulse_rebound_mv:1",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "particle-remembers",
                    arm: None,
                    quantity: "pulse_rebound_mv:1",
                    states: QuotedAs::Same,
                },
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // What the cost sentence is a cost OF: the shell count and the topology, all three
        // out of this step's own scenario file.
        phrase: "per step at {n} shells on a {n}S{n}P pack",
        ties: &[
            Tie::Scenario("pack.cell_model.Spm.shells"),
            Tie::Scenario("pack.series"),
            Tie::Scenario("pack.parallel"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The next step's rate, over the same cell's nameplate. `Elsewhere` because the box
        // is step 15's — 15.459594 A, four digits finer than this step's own.
        phrase: "same model from full at {n} C and stops at",
        ties: &[Tie::Elsewhere {
            step: "looks-fine-from-outside",
            tie: &Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Chemistry("cell.capacity_ah"),
            ]),
        }],
        pow10: 0,
    },
    LedgerRule {
        // The cut-off the next step stops at. The chemistry's own lower limit, and this
        // step's cell is that cell — no `Elsewhere` needed, and one would say less: the
        // number is a property of the LG M50 rather than of either lesson.
        phrase: "and stops at {n} V with",
        ties: &[Tie::Chemistry("cell.v_min")],
        pow10: 0,
    },
    LedgerRule {
        // And what is left on the readout when it does — step 15's claim, quoted to two
        // places where that step's sentence prints one. The address needs the ARM half: step
        // 15's mark is at 500 s and the cut-off is 560 s past it, so that reading lives on
        // its `carries on` continuation and `None` — the step's own run — answers nothing.
        // The trailing words are what keep this
        // phrase off step 15's `with 58.3 % still showing`, which is a different quantity in
        // a step this vocabulary is also scanned against.
        phrase: "V with {n} % still showing, nowhere near it",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: Some("carries on"),
            quantity: "soc_at:1060",
            states: QuotedAs::Same,
        }],
        pow10: 2,
    },
    LedgerRule {
        // The current the reader carries next door, which is this step's own box.
        phrase: "give it the same {n} A on the same train",
        ties: &[Tie::Setting(Control::DemandValue)],
        pow10: 0,
    },
    LedgerRule {
        // The two steps this one hands off to.
        phrase: "Step {n} of this path is about that choice, and step {n} is about",
        ties: &[Tie::Ordinal("past-empty"), Tie::Ordinal("what-it-cost")],
        pow10: 0,
    },
    // Step 15 — the single-particle half of the pair, scanned whole a slice after its twin
    // and for the opposite reason: step 16 was expensive because it quotes its neighbour
    // constantly, and this one is cheap because its neighbour already made it quotable.
    // Thirty-eight numerals, nineteen of them inside the three sentences its own claims
    // quote. Of the nineteen left, eleven of the rules below carry them — nine constants and
    // controls, three ordinals, three pieces of arithmetic and four quotations. Only one arm
    // needed anything of another file: `v_at` on step 14 was three readings under one name,
    // so quoting the floor from here meant tagging them. See
    // `docs/plans/path-ledger-spm-step.md`.
    LedgerRule {
        // The demand box and the rate it works out to, in one breath. The rate is arithmetic
        // and not a constant — this cell's nameplate is 5.153198 Ah and three of them is the
        // box exactly — which is what makes the sentence fail if either half moves.
        phrase: "`Current` at {n} A \u{2014} {n} C for this cell",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Chemistry("cell.capacity_ah"),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // Four numbers in one sentence and one rule for all four, because the sentence is a
        // list of things that have NOT changed and a phrase covering part of it would be
        // generous exactly where the sentence is specific. The part number is read the way
        // step 12 reads it; the two ordinals are where those lessons sit in `const LESSONS`,
        // so inserting a step ahead of either turns this sentence red.
        phrase: "Same LG M{n}, same single-particle model as steps {n} and {n}, same {n} \
                 shells",
        ties: &[
            Tie::Name {
                field: "meta.name",
                prefix: "M",
            },
            Tie::Ordinal("particle-remembers"),
            Tie::Ordinal("three-times-the-current"),
            Tie::Scenario("pack.cell_model.Spm.shells"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // What this pack starts at against what the two steps before it start at. Steps 13
        // and 14 are both `pulse_train_spm.toml`, so the `Elsewhere` could name either and
        // names the first — the sentence's own "steps 13 and 14" is the reason both are
        // right, and if that file's charge ever moved the sentence would be wrong about both.
        phrase: "the starting charge ({n} % rather than {n} %)",
        ties: &[
            Tie::Scenario("pack.initial_soc"),
            Tie::Elsewhere {
                step: "particle-remembers",
                tie: &Tie::Scenario("pack.initial_soc"),
            },
        ],
        pow10: 2,
    },
    LedgerRule {
        // The cut-off, which is the chemistry's own lower limit rather than anything this
        // scenario chooses — no BMS is built here, so nothing enforces it and the number is
        // the reader's mark rather than the engine's.
        phrase: "it crosses the {n} V cut-off",
        ties: &[Tie::Chemistry("cell.v_min")],
        pow10: 0,
    },
    LedgerRule {
        // The same shape as step 16's amp-hour sentence and deliberately the same ties: the
        // box, this step's own crossing claim, and the product of the two in hours. The
        // sentence used to write the box as `15.46`, which a constant tie compares exactly
        // and would have refused; it prints the number whole now, for the reason step 16's
        // `5.15` and `15.46` do. An arm that accepts rounded constants still does not exist.
        phrase: "{n} A for {n} s is **{n} A\u{b7}h**",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Quoted {
                step: "looks-fine-from-outside",
                arm: Some("carries on"),
                quantity: "t_at_v_below:2.5",
                states: QuotedAs::Same,
            },
            Tie::Product(&[
                Tie::Setting(Control::DemandValue),
                Tie::Hours(&Tie::Quoted {
                    step: "looks-fine-from-outside",
                    arm: Some("carries on"),
                    quantity: "t_at_v_below:2.5",
                    states: QuotedAs::Same,
                }),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // What fraction of the nameplate that is — the same product again, over the capacity.
        // A separate rule from the sentence above it because a percentage and a voltage
        // cannot share a `pow10`, which is the seam this table splits on throughout.
        phrase: "\u{2014} {n} % of this cell's",
        ties: &[Tie::Ratio(&[
            Tie::Product(&[
                Tie::Setting(Control::DemandValue),
                Tie::Hours(&Tie::Quoted {
                    step: "looks-fine-from-outside",
                    arm: Some("carries on"),
                    quantity: "t_at_v_below:2.5",
                    states: QuotedAs::Same,
                }),
            ]),
            Tie::Chemistry("cell.capacity_ah"),
        ])],
        pow10: 2,
    },
    LedgerRule {
        // The nameplate itself, the third number of that same clause.
        phrase: "of this cell's {n}, in the eighteen minutes",
        ties: &[Tie::Chemistry("cell.capacity_ah")],
        pow10: 0,
    },
    LedgerRule {
        // The floor, quoted from the step that measured it rather than re-measured here —
        // reaching it costs eight more marks of simulation on an arm that belongs to step 14.
        // The sentence gives one decimal where that step gives four, which is what the
        // quotation arm's rounding is for; and the ordinal beside it is the same step, named
        // twice in one sentence for two different reasons.
        phrase: "it eventually pins near {n} V, which is the floor step {n} mentions",
        ties: &[
            Tie::Quoted {
                step: "three-times-the-current",
                arm: Some("past the clamp"),
                quantity: "v_at:12600",
                states: QuotedAs::Same,
            },
            Tie::Ordinal("three-times-the-current"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The mark, in the closing instruction to hold onto what it reads.
        phrase: "Hold onto the {n} s reading",
        ties: &[Tie::Setting(Control::Until)],
        pow10: 0,
    },
    LedgerRule {
        // And the two readings themselves, both of them this step's own claims restated in a
        // sentence no claim quotes — the case `Tie::Quoted` allows a step to make of itself.
        phrase: "s reading \u{2014} {n} V,",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: None,
            quantity: "v_at:500",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "V, {n} % \u{2014} because the next step",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: None,
            quantity: "soc_at:500",
            states: QuotedAs::Same,
        }],
        pow10: 2,
    },
    // Step 16 — the Doyle-Fuller-Newman half of the pair, and the densest step in the path:
    // 38 numerals, of which 15 are claimed on its own pack. Nine of the rest are readings the
    // step NEXT DOOR measured, quoted rather than re-measured, because a claim is checked by
    // running its own step's scenario and this step's whole argument is about the other one.
    // Five more are readings of its OWN pack that a claim elsewhere in the step decides —
    // `Tie::Quoted` naming this same lesson, which is the only arm that reaches a measurement
    // in a sentence no claim quotes — and a sixth is worked out from two of those by
    // `Tie::Derived`. The last eight are constants and controls, which is what the entry in
    // `[ledger]` means by "six more … and the last eight": that entry's four parts are
    // 15 + 9 + 6 + 8, and they close on 38 only when both descriptions count the same way.
    // See `docs/plans/path-ledger-dfn-step.md` and `path-ledger-spm-step.md`.
    LedgerRule {
        // The charge it starts from, which is the one number in the opening sentence that is
        // not a control. A different phrase from step 13's "the same {n} %, the same train"
        // for that rule's own reason: a phrase generous enough to cover both would be
        // generous enough to cover a third.
        phrase: "Same cell, same {n} %",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    LedgerRule {
        // The two boxes the sentence says have not changed, in one breath.
        phrase: "same {n} A, same {n} \u{b0}C",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Setting(Control::Ambient),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The twin's zero-length probe, which is the disagreement this step opens with.
        phrase: "where the twin read {n}",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: None,
            quantity: "v_at:0",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The same probe again, in the parenthetical listing what the models answer. Two
        // rules rather than one loose one, on step 6's terms: two sentences say it two ways.
        phrase: "({n} for the particle",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: None,
            quantity: "v_at:0",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "against the twin's {n} on the first step",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: None,
            quantity: "v_at:2",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "where the other reads {n}",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: None,
            quantity: "v_at:400",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The instant the cell finishes, quoted off this step's OWN claim on the crossing —
        // the claim that spells it two sentences later. Not `Tie::Setting`: 464 s is nothing
        // the reader dials in and nothing any file holds; it is where the trajectory goes.
        phrase: "the minute that ends at {n} s",
        ties: &[Tie::Quoted {
            step: "the-electrolyte-starves",
            arm: None,
            quantity: "t_at_v_below:2.5",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The collapse, as the difference between two of this step's own claimed readings.
        // The sentence prints neither operand — it prints the drop — so `Tie::Derived`, which
        // reads a sentence's own tokens, cannot carry it.
        phrase: "it drops **{n} mV**",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "the-electrolyte-starves",
                arm: None,
                quantity: "v_at:400",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "the-electrolyte-starves",
                arm: None,
                quantity: "v_at:464",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 3,
    },
    LedgerRule {
        // The same difference on the twin, over the same two instants — the contrast the
        // sentence is for. Both operands are claims on step 15, and the second of them
        // (`v_at:464`) is why that step's reading list now prints `3.437 at 464`: a claim can
        // only be pinned to a number its own sentence spells.
        phrase: "arm falls {n} mV",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "looks-fine-from-outside",
                arm: None,
                quantity: "v_at:400",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "looks-fine-from-outside",
                arm: None,
                quantity: "v_at:464",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 3,
    },
    LedgerRule {
        // The chemistry's own end of discharge, which the claim beside it deliberately stops
        // short of: its note says the cut-off "is a chemistry constant and no arm accounts
        // for one", which was true of the claims scan and is what this arm is.
        phrase: "past the {n} V cut-off",
        ties: &[Tie::Chemistry("cell.v_min")],
        pow10: 0,
    },
    LedgerRule {
        // The whole amp-hour sentence, which is four facts in one breath: the box, the
        // instant, the arithmetic over the two of them, and the nameplate it is a fraction
        // of. The last two used to read `15.46` and `5.15`, and the prose now prints both
        // constants whole — the shorter forms could not be tied, because a constant is
        // compared exactly and a rounded restatement of one is neither the file's number nor
        // a computed quantity. See `docs/plans/path-ledger-dfn-step.md`.
        phrase: "{n} A for {n} s is {n} A\u{b7}h of this cell's {n}",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Quoted {
                step: "the-electrolyte-starves",
                arm: None,
                quantity: "t_at_v_below:2.5",
                states: QuotedAs::Same,
            },
            Tie::Product(&[
                Tie::Setting(Control::DemandValue),
                Tie::Hours(&Tie::Quoted {
                    step: "the-electrolyte-starves",
                    arm: None,
                    quantity: "t_at_v_below:2.5",
                    states: QuotedAs::Same,
                }),
            ]),
            Tie::Chemistry("cell.capacity_ah"),
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "same instant, reads {n} V",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: None,
            quantity: "v_at:464",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // How much longer the twin has: its cut-off less this step's, one claim from each
        // pack. Neither number is on the page.
        phrase: "has {n} seconds still to run",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "looks-fine-from-outside",
                // The twin's cut-off is 560 s past its own mark, so the claim on it is read
                // on the arm the reader presses Run again for — not on step 15's own run,
                // which stops at 500 s and never crosses anything.
                arm: Some("carries on"),
                quantity: "t_at_v_below:2.5",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "the-electrolyte-starves",
                arm: None,
                quantity: "t_at_v_below:2.5",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        phrase: "against the twin's {n} W",
        ties: &[Tie::Quoted {
            step: "looks-fine-from-outside",
            arm: None,
            quantity: "q_gen_at",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The current the last paragraph tells the reader to type, which is this cell's
        // capacity read as an ampere — the current that empties it in an hour. The sentence
        // used to gloss it "1 C", and that gloss had no arm: `Tie::Ratio` divides two file
        // reads, and here both halves would be this same field, so the arm would resolve to 1
        // whatever the file said. See `docs/plans/path-ledger-dfn-step.md`.
        phrase: "set the current to {n} A",
        ties: &[Tie::Chemistry("cell.capacity_ah")],
        pow10: 0,
    },
    LedgerRule {
        // How far apart the two models land at 1 C: the twin's crossing less this file's,
        // one claim from each of the two arms this step's closing instruction produces.
        // Neither operand is anywhere near this token in the sentence — they are its two
        // neighbours — but the tie side is still the right family, because what separates
        // them is WHICH TRAJECTORY each was read on and only an address can say that. A
        // `Tie::Derived` reading the printed `3484` and `3496` would be right about the
        // arithmetic and silent about the packs.
        phrase: "{n} s apart, which is",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "the-electrolyte-starves",
                arm: Some("the twin at one c"),
                quantity: "t_at_v_below:2.5",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "the-electrolyte-starves",
                arm: Some("one c"),
                quantity: "t_at_v_below:2.5",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The same gap as a fraction of this file's own run, and the one number in the
        // sentence that IS worked out from what a reader can see: the `12` two words back
        // over the `3484` two clauses back.
        //
        // Admissible only because the `12` is tie-side. `operand_value` refuses an operand
        // whose only accounting is another derivation, so if that rule were ever re-spelled
        // as a `Tie::Derived` over the printed instants, this one goes red rather than
        // quietly resting on a chain with no floor.
        phrase: "which is {n} %",
        ties: &[Tie::Derived {
            op: LedgerOp::Ratio,
            operands: &[Operand::Sibling("12"), Operand::Sibling("3484")],
        }],
        pow10: 2,
    },
    LedgerRule {
        // What the cheap model gets wrong about capacity, which at one current is the ratio
        // of the two cut-off instants: the same amps for 2.28 times as long.
        phrase: "wrong by a factor of {n} on how much charge",
        ties: &[Tie::Ratio(&[
            Tie::Quoted {
                step: "looks-fine-from-outside",
                // The twin's cut-off is 560 s past its own mark, so the claim on it is read
                // on the arm the reader presses Run again for — not on step 15's own run,
                // which stops at 500 s and never crosses anything.
                arm: Some("carries on"),
                quantity: "t_at_v_below:2.5",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "the-electrolyte-starves",
                arm: None,
                quantity: "t_at_v_below:2.5",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The rate this step runs at, said as a C-rate for the first time in the step: the
        // box over the nameplate. Step 22's `C/{n}` is the same division the other way up.
        phrase: "(At {n} C the cost gap",
        ties: &[Tie::Ratio(&[
            Tie::Setting(Control::DemandValue),
            Tie::Chemistry("cell.capacity_ah"),
        ])],
        pow10: 0,
    },
    // Step 22 — the first lead-acid step, and the first whose numbers need arithmetic that
    // is not a product. Its opening paragraph is the cell's nameplate: what one cell is,
    // what six of them make, what it is rated and the condition that rating carries.
    LedgerRule {
        // The nominal voltage, and no numeric field of the chemistry holds one — the file
        // has `v_max`, `v_min` and an OCV table, none of which is 2 V. What does hold it is
        // the cell's own name, which is what `Tie::Name` is for. The prefix keeps it off
        // the `12` sitting in `meta.provenance` two lines below.
        phrase: "a {n} V lead-acid cell",
        ties: &[Tie::Name {
            field: "meta.name",
            prefix: "lead-acid ",
        }],
        pow10: 0,
    },
    LedgerRule {
        // The sentence the ledger's sixth arm was reserved for. `12` is in no file: it is
        // the `2` this same sentence prints, six times over, and `six` is a word the phrase
        // itself pins.
        phrase: "six of these in series is the {n} V battery",
        ties: &[Tie::Derived {
            op: LedgerOp::Product,
            operands: &[Operand::Word("six"), Operand::Sibling("2")],
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "It is rated {n} Ah",
        ties: &[Tie::Chemistry("cell.capacity_ah")],
        pow10: 0,
    },
    LedgerRule {
        // The same rating, stated again as the condition a datasheet attaches to it. Two
        // rules rather than one loose one, on step 6's terms: two sentences say it two ways.
        phrase: "*{n} Ah if you take 20 hours",
        ties: &[Tie::Chemistry("cell.capacity_ah")],
        pow10: 0,
    },
    LedgerRule {
        // The demand box and what it is a fraction of, in one breath — which is what the
        // sentence says. The `20` is hours: the cell's rating over the current asked for.
        phrase: "{n} A is C/{n}",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Ratio(&[
                Tie::Chemistry("cell.capacity_ah"),
                Tie::Setting(Control::DemandValue),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The cut-off the run marks itself at, which is this chemistry's own end-of-
        // discharge and not the page's choice.
        phrase: "chemistry's own {n} V",
        ties: &[Tie::Chemistry("cell.v_min")],
        pow10: 0,
    },
    LedgerRule {
        // The whole point of the chemistry, and a statement about the table rather than
        // about any node of it.
        phrase: "spans only {n} mV of open-circuit voltage",
        ties: &[Tie::Span("ocv.volts.*")],
        pow10: 3,
    },
    LedgerRule {
        phrase: "the LFP cell of step {n}",
        ties: &[Tie::Ordinal("bare-curve")],
        pow10: 0,
    },
    LedgerRule {
        // The nameplate a third time, now as the thing the delivered charge is measured
        // against. The claim beside it stops at `A·h`, which is what leaves this number to
        // the ledger.
        phrase: "of the {n} on the label",
        ties: &[Tie::Chemistry("cell.capacity_ah")],
        pow10: 0,
    },
    LedgerRule {
        // Not the mark (69620.5 s) and not any file's number: what one row prints when the
        // run stops there.
        phrase: "the panel reads `{n}h`",
        ties: &[Tie::Clock],
        pow10: 0,
    },
    // Step 23 — the same cell and the same file at sixty times the current. Its opening
    // sentences are all comparisons with the step before, which is what `Tie::Elsewhere`
    // and `Tie::Quoted` are for: half the numbers here are facts about step 22.
    LedgerRule {
        // The one field that differs, and the one it differs from. Two lessons, one rule,
        // because the sentence puts them a word apart.
        phrase: "different: {n} A instead of {n}",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Elsewhere {
                step: "slow-and-patient",
                tie: &Tie::Setting(Control::DemandValue),
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        // The cell's own continuous rating, which is what the clause after the comma says
        // this current is the top of. It is ALSO 21.6 A over the 7.2 Ah nameplate, and
        // `Tie::Ratio` would carry that reading — the choice against it is argued in
        // `docs/plans/path-ledger-last-two-steps.md`: the ratio's unique failure (the
        // rating moving) would leave the sentence false with nothing in the repo pinning
        // `max_discharge_c`, where this reading's unique failure (the box moving) is
        // already caught by the `21.6` in the sentence before it.
        phrase: "That is {n} C, 60 times",
        ties: &[Tie::Chemistry("cell.max_discharge_c")],
        pow10: 0,
    },
    LedgerRule {
        // Everything the step holds, in one breath: the topology, and the slider. The `dt`
        // beside them is spelled "half-second" and stays invisible, which is the digit
        // scanner's standing limit rather than a gap in this rule.
        phrase: "same {n}S{n}P, same {n} °C",
        ties: &[
            Tie::Scenario("pack.series"),
            Tie::Scenario("pack.parallel"),
            Tie::Setting(Control::Ambient),
        ],
        pow10: 0,
    },
    LedgerRule {
        // Both clocks: this step's mark rendered by the `sim time` row, and step 22's.
        phrase: "reached in `{n}m` instead of `{n}h`",
        ties: &[
            Tie::Clock,
            Tie::Elsewhere {
                step: "slow-and-patient",
                tie: &Tie::Clock,
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        // The step before's amp-hours, and the difference the sentence draws from them. The
        // claim beside it stops at `A·h came out`, which is what leaves both of these here.
        phrase: "the last step's {n}, and the missing {n} A·h",
        ties: &[
            Tie::Quoted {
                step: "slow-and-patient",
                arm: None,
                quantity: "delivered_ah",
                states: QuotedAs::Same,
            },
            Tie::Derived {
                op: LedgerOp::Difference,
                operands: &[Operand::Sibling("6.9620"), Operand::Sibling("4.4190")],
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "Step {n} finds the place",
        ties: &[Tie::Ordinal("and-it-is-still-in-there")],
        pow10: 0,
    },
    // The heat comparison, in four numbers where it used to be three. The sentence quotes
    // step 22's heat twice — as the panel rounds it and as the claim values it — and prints
    // both ratios, because they are not the same number and the gap between them is the
    // lesson. See `docs/plans/path-ledger-last-two-steps.md` for what it used to say.
    LedgerRule {
        phrase: "against `{n} W` where the last step stopped",
        ties: &[Tie::Quoted {
            step: "slow-and-patient",
            arm: None,
            quantity: "q_gen_at",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "divides out to {n} times as much",
        ties: &[Tie::Derived {
            op: LedgerOp::Ratio,
            operands: &[Operand::Sibling("6.09"), Operand::Sibling("0.07")],
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "the honest figure is {n}, because",
        ties: &[Tie::Derived {
            op: LedgerOp::Ratio,
            operands: &[Operand::Sibling("6.09"), Operand::Sibling("0.0746")],
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "is a rounded {n} and at two decimal",
        ties: &[Tie::Quoted {
            step: "slow-and-patient",
            arm: None,
            quantity: "q_gen_at",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "expecting steps {n} and {n}",
        ties: &[Tie::Ordinal("past-empty"), Tie::Ordinal("what-it-cost")],
        pow10: 0,
    },
    // Step 24 — the rest, and the second discharge out of a cell that had already stopped.
    // Six of its numerals point at other steps and the rest are measurements, so this block
    // is ordinals and one setting; everything else on the step is claimed.
    LedgerRule {
        phrase: "the same {n} C discharge for the same {n} seconds",
        ties: &[
            Tie::Chemistry("cell.max_discharge_c"),
            Tie::Setting(Control::PulseOn),
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "leg one is step {n}, exactly",
        ties: &[Tie::Ordinal("sixty-times-the-current")],
        pow10: 0,
    },
    LedgerRule {
        phrase: "Leg one is step {n} run again, and it ends where step {n} ended",
        ties: &[
            Tie::Ordinal("sixty-times-the-current"),
            Tie::Ordinal("sixty-times-the-current"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The cutoff the panel would have printed, which is the chemistry's own end of
        // discharge — the same field step 22's `chemistry's own 1.750 V` reads, said a
        // third way. The step beside it is where that instant is readable.
        phrase: "without ever printing `{n} V`, and step {n}",
        ties: &[
            Tie::Chemistry("cell.v_min"),
            Tie::Ordinal("sixty-times-the-current"),
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "what step {n} had to say",
        ties: &[Tie::Ordinal("past-empty")],
        pow10: 0,
    },
    LedgerRule {
        // Leg one's amp-hours, quoted off the step that measured them...
        phrase: "A·h against the first leg's {n}",
        ties: &[Tie::Quoted {
            step: "sixty-times-the-current",
            arm: None,
            quantity: "delivered_ah",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // ...and the fraction the sentence works out from it. A separate rule because
        // `pow10` is a property of the rule and these two numbers are in different units:
        // amp-hours and a percentage.
        phrase: "{n} % of it, from a cell",
        ties: &[Tie::Derived {
            op: LedgerOp::Ratio,
            operands: &[Operand::Sibling("1.4220"), Operand::Sibling("4.4190")],
        }],
        pow10: 2,
    },
    LedgerRule {
        phrase: "unlike steps {n} and {n} there is",
        ties: &[Tie::Ordinal("past-empty"), Tie::Ordinal("what-it-cost")],
        pow10: 0,
    },
    // Step 8 — the pack that wears out while nothing happens. The step with the fewest
    // unaccounted numerals of the fourteen, measured rather than guessed
    // (docs/plans/path-self-description-sweep.md), and every one of them is a control or a
    // topology figure except the ratio at the end.
    //
    // Its topology, split across two rules because `pow10` is a property of the rule and a
    // charge level is a percentage where a cell count is not.
    LedgerRule {
        phrase: "{n}S{n}P LFP at",
        ties: &[Tie::Scenario("pack.series"), Tie::Scenario("pack.parallel")],
        pow10: 0,
    },
    LedgerRule {
        phrase: "LFP at {n} % charge",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    LedgerRule {
        phrase: "This runs to {n} s of simulation",
        ties: &[Tie::Setting(Control::Until)],
        pow10: 0,
    },
    LedgerRule {
        phrase: "of watching at {n}×",
        ties: &[Tie::Setting(Control::Speed)],
        pow10: 0,
    },
    // The two sentences that dial the slider somewhere the step does not leave it. Same
    // control, two arms, and that is the distinction `Tie::OnArm` was built to make: the
    // first continues from the mark, the second rebuilds from new.
    LedgerRule {
        phrase: "raise the ambient slider to {n} \u{b0}C",
        ties: &[Tie::OnArm {
            arm: "hot",
            tie: &Tie::Setting(Control::Ambient),
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "with the slider still at {n} \u{b0}C",
        ties: &[Tie::OnArm {
            arm: "hot from new",
            tie: &Tie::Setting(Control::Ambient),
        }],
        pow10: 0,
    },
    // What the whole step is for: the temperature's own factor, which the second leg does
    // NOT show. Both operands are this step's, on two trajectories — the fresh pack's loss
    // over the ratio of the one that had already aged — so the arm is part of each address
    // and neither is readable off the prose. A `Tie::Derived` over the sentence's siblings
    // could not express it: the sentence prints the answer and neither operand.
    LedgerRule {
        phrase: "Not the {n}× two fresh packs would show",
        ties: &[Tie::Ratio(&[
            Tie::Quoted {
                step: "wearing-out-while-idle",
                arm: Some("hot from new"),
                quantity: "soh_cap_at:200000",
                states: QuotedAs::Complement,
            },
            Tie::Quoted {
                step: "wearing-out-while-idle",
                arm: None,
                quantity: "soh_cap_at:200000",
                states: QuotedAs::Complement,
            },
        ])],
        pow10: 0,
    },
    // Step 9 — the charge, and the first ledgered step whose subject is the PAGE'S OWN
    // POLICY rather than the engine's behaviour. `CC-CV` is not a demand `sim-core` has:
    // it is two of them with a rule between, which is where CLAUDE.md puts a charge
    // policy. So three of this step's numbers are the policy's — the current it asks for,
    // the voltage it aims at, the current it stops below — and a fourth is how often the
    // rule is allowed to fire, which is a constant of the client and of nothing else.
    //
    // Where it starts. A different phrase from the four other "{n} %" rules above for
    // their reason: a phrase generous enough to cover two sentences is generous enough to
    // cover a third that means something else.
    LedgerRule {
        phrase: "starting at {n} % instead of full",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    // Which cell it is. The ordinal is checked and the SAMENESS is not: `Tie::Ordinal`
    // pins that `same-discharge-other-chemistry` is still the second lesson, so inserting
    // a step ahead of it turns this red — but nothing here compares the two scenarios'
    // chemistries, and no arm in this taxonomy can. The sentence's claim that it is the
    // same cell rests on a reader opening both files.
    LedgerRule {
        phrase: "cell as step {n}, starting",
        ties: &[Tie::Ordinal("same-discharge-other-chemistry")],
        pow10: 0,
    },
    // The cutoff, which is the page's and not the file's — the lesson block's own comment
    // says so: "a scenario is an initial condition and never a demand program".
    LedgerRule {
        phrase: "falling below the {n} A cutoff",
        ties: &[Tie::Setting(Control::Taper)],
        pow10: 0,
    },
    // How often the controller may change its mind, read off the page's own constant.
    LedgerRule {
        phrase: "checked every {n} s of",
        ties: &[Tie::Page("CCCV_PERIOD_S")],
        pow10: 0,
    },
    // The speed this step watches at, in the sentence that says the speed changes nothing.
    LedgerRule {
        phrase: "real time than at {n}×",
        ties: &[Tie::Setting(Control::Speed)],
        pow10: 0,
    },
    // THE HEADLINE, and both halves are ratios of this step's own claims. Neither operand
    // is printed in the sentence that prints the answer — they are two sentences above it
    // — so `Tie::Derived` cannot reach them and this is the file-and-claim family instead:
    // `Tie::Ratio` over `Tie::Difference` over `Tie::Quoted`.
    //
    // The two denominators are NOT parallel, and that is the sentence rather than a slip:
    // the time is measured from t = 0, where the charge is measured from the 20 % the pack
    // started at. A charge "of the time" from the same origin would be nonsense — the run
    // does not begin part-way through a clock — and a charge fraction over 100 % would
    // count 20 points the reader never put in.
    //
    // KNOWN GRANULARITY, stated rather than discovered: both tokens commit to one digit,
    // so `13` survives anything in [12.5, 13.5) — about 50 s of movement in the leg
    // boundary — and `5` anything in [4.5, 5.5). The arms are as tight as the prose is,
    // which is the rule `tol_from = "spelled"` keeps on the claims side.
    LedgerRule {
        // 6210 - 5420 over 6210 = 12.72 %, which the sentence prints as 13.
        phrase: "last leg is {n} % of the time",
        ties: &[Tie::Ratio(&[
            Tie::Difference(&[
                Tie::Quoted {
                    step: "two-legs",
                    arm: None,
                    quantity: "cccv_taper_s",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "two-legs",
                    arm: None,
                    quantity: "cccv_cc_ends_s",
                    states: QuotedAs::Same,
                },
            ]),
            Tie::Quoted {
                step: "two-legs",
                arm: None,
                quantity: "cccv_taper_s",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 2,
    },
    LedgerRule {
        // 99.52 - 95.28 over 99.52 - 20 = 5.33 %, which the sentence prints as 5.
        //
        // The token discriminates one candidate denominator and not the other, which is
        // worth stating exactly. Over the cell's WHOLE capacity the last leg is 4.24 points
        // of 100, which prints `4` — so the sentence is not about that, and the green says
        // so. Over `1 - initial_soc` — the charge a full run would put in rather than the
        // charge this one did — it is 5.30, and that prints `5` too. The measured pair is
        // the one this arm can express and the one the sentence means; what the green does
        // not do is choose between it and that third reading.
        phrase: "for {n} % of the charge",
        ties: &[Tie::Ratio(&[
            Tie::Difference(&[
                Tie::Quoted {
                    step: "two-legs",
                    arm: None,
                    quantity: "soc_at:6210",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "two-legs",
                    arm: None,
                    quantity: "soc_at:5420",
                    states: QuotedAs::Same,
                },
            ]),
            Tie::Difference(&[
                Tie::Quoted {
                    step: "two-legs",
                    arm: None,
                    quantity: "soc_at:6210",
                    states: QuotedAs::Same,
                },
                Tie::Scenario("pack.initial_soc"),
            ]),
        ])],
        pow10: 2,
    },
    // Step 19 — the weaker short, and the step where the protection layer has nothing to
    // clamp. Twenty-three of its thirty-four numerals were claims before it was scanned, so
    // this block is small for a step this dense: the two shorts, the two thresholds the
    // rungs sit at, the mark, and two durations measured from the fault.
    //
    // Its two shorts in one rule, because the sentence is the comparison: this file's
    // resistance and the twin's, in the milliohms both are quoted in.
    LedgerRule {
        phrase: "the short is {n} milliohms instead of {n}",
        ties: &[
            Tie::Scenario("faults.*.fault.ExternalShort.ohms"),
            Tie::Elsewhere {
                step: "one-step-that-got-through",
                tie: &Tie::Scenario("faults.*.fault.ExternalShort.ohms"),
            },
        ],
        pow10: 3,
    },
    // The two durations, and both are the same subtraction: an instant this step's own
    // claims pin, less the instant the fault lands. Neither is a number any file holds —
    // the scenario says when the short appears and the engine says when the BMS answers,
    // and the sentence prints the gap between them.
    //
    // THE FIRST OF THEM MOVED THE PROSE. It read "73 seconds of no flags at all", and the
    // subtraction is 73.5 — true of a duration rounded down, false of the arithmetic, and a
    // computed tie compares at the prose's own precision, so a whole number here would have
    // to be 74. The digit changed rather than the rule.
    LedgerRule {
        phrase: "{n} seconds of no flags at all",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "nothing-to-clamp",
                arm: None,
                quantity: "flag_first_s:OT",
                states: QuotedAs::Same,
            },
            Tie::Scenario("faults.*.at_s"),
        ])],
        pow10: 0,
    },
    LedgerRule {
        phrase: "{n} s after the short",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "nothing-to-clamp",
                arm: None,
                quantity: "flag_first_s:CONTACTOR_OPEN",
                states: QuotedAs::Same,
            },
            Tie::Scenario("faults.*.at_s"),
        ])],
        pow10: 0,
    },
    // The rung that did fire, and the reason this step needed `Tie::Sum`: the trip point is
    // in neither file on its own. The chemistry says what the cell may reach and the
    // scenario says how far past that the BMS will let it go before opening the contactor.
    LedgerRule {
        phrase: "crossing {n} K",
        ties: &[Tie::Sum(&[
            Tie::Chemistry("cell.t_max_k"),
            Tie::Scenario("pack.bms.protection.t_hard_margin_k"),
        ])],
        pow10: 0,
    },
    // The twin's temperature, assembled the same way out of the step next door: its ambient
    // plus the rise its own claim pins. `Elsewhere` for the file read and `Quoted` for the
    // measurement, which is the division of labour those two arms were built for.
    //
    // THE SECOND SENTENCE THIS BLOCK MOVED. It said the twin "peaks" at this figure, and
    // what the claim below it measures is that pack at ITS mark — 299.075 K against a true
    // peak of 299.112 K one step after the tooth. Both print 299.1, so no tolerance here
    // could have told them apart; the verb is what changed.
    //
    // WHAT THIS SENTENCE INHERITS, NAMED RATHER THAN LEFT: a `Quoted` tie resolves to the
    // other claim's STORED value, and what ties that value to the engine is check 7 at that
    // claim's own tolerance. Step 18's rise is declared `tighter` at 5e-3 K, so the twin's
    // cell can drift up to five thousandths of a kelvin without reddening anything and this
    // sum goes on resolving off the stored figure. Five mK is far under the tenth this
    // sentence prints, so the hole cannot reach the prose today — but it is the source
    // claim's tolerance and not this rule's that decides how wide it is, which is worth
    // knowing before a rule quotes a claim declared `spelled` on a whole number.
    LedgerRule {
        phrase: "a twin whose run ends at {n} K",
        ties: &[Tie::Sum(&[
            Tie::Elsewhere {
                step: "one-step-that-got-through",
                tie: &Tie::Scenario("pack.initial_temp_k"),
            },
            Tie::Quoted {
                step: "one-step-that-got-through",
                arm: None,
                quantity: "t_rise_k_at:90",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        phrase: "this step's mark is {n} s",
        ties: &[Tie::Setting(Control::Until)],
        pow10: 0,
    },
    // Step 18 — the twin of the block above, and the first ledgered step whose subject is
    // the step LENGTH. Thirteen rules for fourteen of its thirty-one numerals (`**Step {n}**`
    // covers two); five more numerals became claims, two of them on the BMS's voltage
    // sensors, and one left the page.
    //
    // THE TWO NUMBERS IN THAT SENTENCE WERE WRONG WHEN IT WAS FIRST WRITTEN — it said
    // "fifteen of its thirty-five" — because it was written before the scan had been run and
    // nothing checks a count spelled in a comment. The step's own `[ledger]` entry carries
    // the same figure with a derivation behind it, which is why that one was right; this one
    // is a neighbour of a derived number, the shape three slices have now recorded as the way
    // these files rot.
    //
    // The fault, in the sentence that springs it. Written with the whole bracketed phrase
    // because the twin step next door says "the short is 100 milliohms instead of 30" —
    // two ledgered steps, one file apart, both printing a resistance in the same unit, and
    // a phrase like "{n} milliohm" would reach across.
    LedgerRule {
        phrase: "**{n} milliohm short across the pack's terminals**",
        ties: &[Tie::Scenario("faults.*.fault.ExternalShort.ohms")],
        pow10: 3,
    },
    LedgerRule {
        phrase: "like the leak in step {n}",
        ties: &[Tie::Ordinal("lying-sensor")],
        pow10: 0,
    },
    LedgerRule {
        phrase: "The pack was at {n} % when the short landed",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    // The rung that fires, assembled out of two files the way step 19's temperature rung is
    // — and this is the voltage half of that pair. `cell.v_min` is what the cell may reach
    // and `v_hard_margin_v` is how far past it the BMS calls a fault rather than a number to
    // derate from, which is what the scenario's own comment says in so many words.
    LedgerRule {
        phrase: "well under the {n} V that this file calls a fault",
        ties: &[Tie::Difference(&[
            Tie::Chemistry("cell.v_min"),
            Tie::Scenario("pack.bms.protection.v_hard_margin_v"),
        ])],
        pow10: 0,
    },
    // The `dt` box, three times: once as the arm's own setting and twice as the step's.
    // `Control::Dt` is new for these — before it, the ledger could read every box on the
    // page except the one this step is about.
    LedgerRule {
        phrase: "Put `dt` up to {n} s",
        ties: &[Tie::OnArm {
            arm: "dt 5",
            tie: &Tie::Setting(Control::Dt),
        }],
        pow10: 0,
    },
    LedgerRule {
        phrase: "`dt` included, and puts it back to {n}",
        ties: &[Tie::Setting(Control::Dt)],
        pow10: 0,
    },
    LedgerRule {
        phrase: "Put it back to {n} before moving on",
        ties: &[Tie::Setting(Control::Dt)],
        pow10: 0,
    },
    LedgerRule {
        phrase: "At this step's {n}\u{d7} that tooth",
        ties: &[Tie::Setting(Control::Speed)],
        pow10: 0,
    },
    // The button, twice, and the one arm in this table that reads the markup. See
    // [`Tie::Label`]: the digit is half a name, not a quantity.
    LedgerRule {
        phrase: "**Step {n}**",
        ties: &[Tie::Label {
            id: "stepone",
            prefix: "Step ",
        }],
        pow10: 0,
    },
    // What **Clear queued** removed, counted by the page's own note — so the number is the
    // shape of the scenario file rather than any value in it.
    LedgerRule {
        phrase: "counts the short it removed as *{n} fault*",
        ties: &[Tie::Count("faults")],
        pow10: 0,
    },
    // The two teeth against each other. Neither operand is in this sentence, which is what
    // separates `Tie::Difference` from `Tie::Derived` — the sentence prints the answer and
    // nothing else — and both are this step's own claims on two of its own trajectories.
    // Order is the claim: reversed, the second tooth would be the smaller one and the whole
    // paragraph after it would be about the wrong sign.
    LedgerRule {
        phrase: "It is {n} A *bigger* than the first tooth",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "one-step-that-got-through",
                arm: Some("unlatched"),
                quantity: "i_at",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "one-step-that-got-through",
                arm: None,
                quantity: "i_at",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    // The step quoting its own mark-side reading in a later sentence — the 90 s rise, not
    // the 60.5 s peak, which is why both are tagged.
    LedgerRule {
        phrase: "left the cells about {n} K warmer",
        ties: &[Tie::Quoted {
            step: "one-step-that-got-through",
            arm: None,
            quantity: "t_rise_k_at:90",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    // The plateau itself. Four cells in series, times what the curve does between the charge
    // the pack started at and the charge the tooth left it with. See [`Tie::Ocv`], including
    // what this does not settle.
    LedgerRule {
        phrase: "moves the open-circuit voltage by {n} mV",
        ties: &[Tie::Product(&[
            Tie::Scenario("pack.series"),
            Tie::Difference(&[
                Tie::Ocv(&Tie::Scenario("pack.initial_soc")),
                Tie::Ocv(&Tie::Quoted {
                    step: "one-step-that-got-through",
                    arm: None,
                    quantity: "soc_at",
                    states: QuotedAs::Same,
                }),
            ]),
        ])],
        pow10: 3,
    },
    // Step 11 — what the protection costs. Thirteen of its twenty-five numerals are here and
    // the other twelve are claims: this is the first ledgered step whose prose runs on four
    // trajectories, and every one of the four is a control a reader is told to change.
    LedgerRule {
        // The topology, in the sentence that opens the step. Step 6's "cells in {n}S{n}P is"
        // and step 3's two spelled-out rules already read the same two fields; a third
        // vocabulary because a third sentence says it a third way, on the terms those two
        // set. Note this one has NMC in it, which is what stops it reaching the LFP steps
        // that open the same way.
        phrase: "A {n}S{n}P NMC pack with",
        ties: &[Tie::Scenario("pack.series"), Tie::Scenario("pack.parallel")],
        pow10: 0,
    },
    LedgerRule {
        // The bleed threshold, in the sentence that lists what the step is configured with.
        // Its twin two rules down reads the same field out of the sentence that watches the
        // flag arrive — two sentences, two vocabularies, as everywhere else here.
        phrase: "passive balancing above {n} V per group",
        ties: &[Tie::Scenario("pack.bms.balancing.v_threshold_v")],
        pow10: 0,
    },
    LedgerRule {
        // The demand box and what it comes to as a rate, in one sentence and one rule
        // because the sentence is one clause: "3 A is 0.5 C". The rate is the box over the
        // pack's own capacity — cells in parallel add amp-hours — which is the same
        // arithmetic step 6's "is {n} Ah at pack level" does one factor short of.
        //
        // Two placeholders, so the rule cannot be satisfied by the demand alone. What is
        // measured: moving the box reddens (on the FIRST tie, which the scan reaches first
        // in text order) and moving the prose's own `0.5 C` reddens on the second. What is
        // NOT measured is the pairing — nothing here shows the rule would refuse two
        // placeholders satisfied from unrelated positions, because no single edit produces
        // that state.
        phrase: "per group. {n} A is {n} C",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Product(&[
                    Tie::Chemistry("cell.capacity_ah"),
                    Tie::Scenario("pack.parallel"),
                ]),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        phrase: "the first group crosses {n} V",
        ties: &[Tie::Scenario("pack.bms.balancing.v_threshold_v")],
        pow10: 0,
    },
    LedgerRule {
        // How far past the mark the unprotected charge runs. Both operands are numbers this
        // file already answers for — the boundary is a claim on the arm, the mark is the
        // step's own control — so the rule is the subtraction and nothing else.
        //
        // Not a `Derived`: that arm reads the sentence's printed siblings and the mark is
        // not printed in this sentence. It is named in words ("past the mark"), which the
        // phrase pins and the tie reads off the lesson block.
        phrase: "{n} s past the mark",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "what-protection-costs",
                arm: Some("unprotected"),
                quantity: "cccv_window_close_s",
                states: QuotedAs::Same,
            },
            Tie::Setting(Control::Until),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // THE HEADLINE. The step's title is "what the protection costs" and this is the
        // number: the unprotected charge's finish less the protected one's, in points. Both
        // sides are this step's own claims, quoted by address — which is why both of those
        // claims carry an instant tag, and why the two trajectories are two arms rather
        // than one run read twice.
        //
        // The sentence prints neither operand, so nothing here rests on the prose being
        // consistent with itself: move either trajectory and this reddens on the number a
        // reader is shown.
        phrase: "Those {n} points are what protection costs",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "what-protection-costs",
                arm: Some("unprotected"),
                quantity: "soc_at:4820",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "what-protection-costs",
                arm: None,
                quantity: "soc_at:3986",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 2,
    },
    LedgerRule {
        // What the charge is aiming at, which is `ccCvNote`'s own arithmetic:
        // `const target = cfg.v_cell * series`. The per-cell half is the CC-CV box and NOT
        // the chemistry's ceiling, though the two spell the same digits on this pack — see
        // [`Control::CcCvVoltage`], and the rule two entries down that reads the ceiling.
        phrase: "short of the {n} V it is aiming for",
        ties: &[Tie::Product(&[
            Tie::Setting(Control::CcCvVoltage),
            Tie::Scenario("pack.series"),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The per-cell limit the over-voltage rung trips on, which is what "already crossed"
        // is about: the pack is short of its target and one cell is not. `cell.v_max`, the
        // same field step 1 reads for the other end of the same chemistry.
        phrase: "the top one has already crossed {n}",
        ties: &[Tie::Chemistry("cell.v_max")],
        pow10: 0,
    },
    LedgerRule {
        // The current the reader types into the CC-CV group on the `one C` arm. `OnArm`
        // rather than `Setting`, because the step itself asks for 3 A and a rule reading the
        // step here would account 6 against 3 and fail — or, worse on some other lesson,
        // succeed.
        phrase: "ask for {n} A",
        ties: &[Tie::OnArm {
            arm: "one C",
            tie: &Tie::Setting(Control::DemandValue),
        }],
        pow10: 0,
    },
    LedgerRule {
        // The rating the derate lands on. A chemistry constant, and the sentence's other
        // number — the 4.2 A itself — is a claim, because "exactly" is an assertion about
        // the BMS and not about what 0.7 C multiplies out to.
        phrase: "{n} C, its charge rating",
        ties: &[Tie::Chemistry("cell.max_charge_c")],
        pow10: 0,
    },
    LedgerRule {
        // The ambient the two cold arms are dragged to. The minus is in the phrase and the
        // tie takes the magnitude, because the scanner finds digits and never a sign — see
        // [`Tie::Magnitude`], which this is the only user of.
        //
        // Reads the `cold` arm rather than `cold, protection off`: they set the same -5 and
        // either would answer, so the choice is arbitrary and the first is the one the
        // sentence's first clause is about.
        phrase: "drag the ambient to -{n} \u{b0}C",
        ties: &[Tie::Magnitude(&Tie::OnArm {
            arm: "cold",
            tie: &Tie::Setting(Control::Ambient),
        })],
        pow10: 0,
    },
    // Step 10 - the charge whose second leg never arrives, and the cheapest of the six that
    // were left. Twenty-three of its numerals were unaccounted and NOT ONE OF THEM NEEDED AN
    // ARM: every reading this step prints is on its own run to the 7000 s mark. The step
    // prints `3.65` three times and `3.60` three times, so every phrase below is long enough
    // to be unique inside the step as well as across the path - the trap the
    // "subject of step {n}" rule records.
    LedgerRule {
        // Which cell it is. Step 1's LFP, charged instead of discharged, and the shared file
        // is the whole reason step 1's plateau argument carries over to this end of it.
        phrase: "on the LFP cell from step {n}",
        ties: &[Tie::Ordinal("bare-curve")],
        pow10: 0,
    },
    LedgerRule {
        // How far short the charge stalls, worked out from the two voltages the same
        // sentence prints: the cell's limit less the stall. Neither operand is free - the
        // limit is the rule below and the stall is this step's own claim - so this is the
        // subtraction and nothing else.
        phrase: "and stops \u{2014} {n} mV short",
        ties: &[Tie::Derived {
            op: LedgerOp::Difference,
            operands: &[Operand::Sibling("3.65"), Operand::Sibling("3.6357")],
        }],
        pow10: 3,
    },
    LedgerRule {
        // THE FIRST OF THIS STEP'S THREE READINGS OF 3.65, and the phrase carries "this
        // cell's own" because that is what picks it out: `cell.v_max`, the ceiling the file
        // declares. The second is the CC-CV box the page aims at (four rules down), and the
        // third - the voltage real LFP cells are charged to - is a fact about the world that
        // no file in this tree decides, so it now prints no number at all. See
        // `docs/plans/path-ledger-leg-that-is-not-there.md`.
        phrase: "short of this cell's own {n} V limit",
        ties: &[Tie::Chemistry("cell.v_max")],
        pow10: 0,
    },
    LedgerRule {
        // The same ordinal again, in the sentence that does the arithmetic. Two rules rather
        // than one loose one, on step 6's terms.
        phrase: "the plateau from step {n}",
        ties: &[Tie::Ordinal("bare-curve")],
        pow10: 0,
    },
    LedgerRule {
        // Where the open-circuit table ends: the last entry of `[ocv].volts`, which on this
        // chemistry is 3.6000 and is the number the whole step turns on. Indexed rather than
        // starred - `*` would demand every node be 3.60 - and 33 is the last of the table's
        // thirty-four, so a shortened table fails here as a broken rule rather than reading
        // some interior node.
        phrase: "curve tops out at {n} V",
        ties: &[Tie::Chemistry("ocv.volts.33")],
        pow10: 0,
    },
    LedgerRule {
        // THE SENTENCE THIS SLICE REWROTE. It used to name R0 alone - "its resistance is
        // about 21 milliohms" - and then add 0.024 V to 3.60, which is two thirds of the
        // drop and lands at 3.624 where the step's own headline says 3.6357. The RC pair is
        // the missing third: at 5769 s it has been carrying the same current for eleven time
        // constants, so it contributes its full `I*R` and the DC resistance is 21 + 10
        // milliohms. All three numbers are off the chemistry file, which is why one rule
        // carries them: the sum first, because that is what the sentence leads with.
        //
        // `r0.ohms.2.1` is the grid position, and the path does not say why it is the right
        // one: `[r0].soc` is `[0, 0.5, 1]` and `[r0].temp_k` is `[263.15, 298.15, 318.15]`,
        // so index `2.1` is the top of the charge range at 25 C - which is where this cell
        // sits for the whole stretch the sentence is about, on a scenario that is isothermal
        // at exactly 298.15 K.
        phrase:
            "resistances come to about {n} m\u{3a9} ({n} of ohmic drop, and a {n} m\u{3a9} RC pair",
        ties: &[
            Tie::Sum(&[Tie::Chemistry("r0.ohms.2.1"), Tie::Chemistry("rc.0.r_ohms")]),
            Tie::Chemistry("r0.ohms.2.1"),
            Tie::Chemistry("rc.0.r_ohms"),
        ],
        pow10: 3,
    },
    LedgerRule {
        // The rate and the box, in one rule because the sentence is one clause - step 11's
        // shape, and `pack.parallel` is kept in the denominator though it is 1 here, because
        // the arithmetic a reader is being shown is the pack's capacity and not the cell's.
        phrase: "and {n} C is {n} A",
        ties: &[
            Tie::Ratio(&[
                Tie::Setting(Control::DemandValue),
                Tie::Product(&[
                    Tie::Chemistry("cell.capacity_ah"),
                    Tie::Scenario("pack.parallel"),
                ]),
            ]),
            Tie::Setting(Control::DemandValue),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The table's end again, now as the first term of the sentence's own sum.
        phrase: "so {n} + ",
        ties: &[Tie::Chemistry("ocv.volts.33")],
        pow10: 0,
    },
    LedgerRule {
        // And the second term: the DC resistance this sentence just gave, times the current
        // it just gave. Both operands are tied by the two rules above, so what this adds is
        // the multiplication - and it is the number that moved when the RC pair was named.
        phrase: "+ {n} never reaches",
        ties: &[Tie::Derived {
            op: LedgerOp::Product,
            operands: &[Operand::Sibling("31"), Operand::Sibling("1.15")],
        }],
        pow10: -3,
    },
    LedgerRule {
        // THE SECOND READING OF 3.65: what the CC-CV controller is aiming at, which is
        // `ccCvNote`'s own arithmetic (the box times the series count) and not the cell's
        // ceiling three sentences up. They spell the same digits on this pack for the same
        // reason step 11's `16.80` and `4.20` do, and they are read off different files for
        // the same reason: retype the page's CC-CV box and this number moves while
        // `cell.v_max` stays. What "never reaches" is about is the leg change, and the leg
        // change is the box's.
        phrase: "never reaches {n}",
        ties: &[Tie::Product(&[
            Tie::Setting(Control::CcCvVoltage),
            Tie::Scenario("pack.series"),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The `0` of `R0` - a section of the chemistry file, in the same position as step
        // 1's `I*R0` and step 13's `[r0]`. The provenance string is where this file DECLARES
        // R0 a placeholder, which is exactly what the sentence says about it. `RC` sits two
        // characters later in the same string with an empty digit run after it, so the
        // prefix reaches one number and not two - and this sentence now names the RC pair as
        // well, in words, which is the other half of what this slice fixed.
        phrase: "how large R{n} and the RC pair are",
        ties: &[Tie::Name {
            field: "meta.provenance",
            prefix: "R",
        }],
        pow10: 0,
    },
    LedgerRule {
        // The charge current a third time, and the ceiling a third time, in the sentence
        // that prices the side reaction. Both are pow10 0, which is what lets one rule carry
        // the clause.
        phrase: "because {n} A at this cell's {n} V open-circuit ceiling",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Chemistry("ocv.volts.33"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // What the refused current costs, worked out from the two numbers the same sentence
        // prints one clause earlier. The engine's own figure for it is 4.181 W less the
        // 0.041 W of resistive loss, which is this product to four places - but the sentence
        // states it as an argument a reader can do, so the arm is the multiplication.
        phrase: "ceiling is {n} W of side reaction",
        ties: &[Tie::Derived {
            op: LedgerOp::Product,
            operands: &[Operand::Sibling("1.15"), Operand::Sibling("3.60")],
        }],
        pow10: 0,
    },
    LedgerRule {
        // The same heat this step claims at 5000 s, in milliwatts - the step quoting its own
        // measurement, which is what `Tie::Quoted` allows and what the instant tag on
        // `q_gen_at` is for. Not a derivation over the sentence's `0.041`: that would say
        // only that the sentence is consistent with itself, where this answers to the engine
        // through a claim that check 7 runs.
        phrase: "against {n} mW of ohmic loss",
        ties: &[Tie::Quoted {
            step: "leg-that-is-not-there",
            arm: None,
            quantity: "q_gen_at:5000",
            states: QuotedAs::Same,
        }],
        pow10: 3,
    },
    LedgerRule {
        // WHERE THE ENTRY STEP IS, and the step quoting its own flag claim a second time.
        // The cheap route was to grow that claim's literal until the instant sat inside it
        // and let `read at` account for it; check 6 refuses exactly that, because a sentence
        // naming the moment something happens is claiming the moment and not the reading.
        // So the tie is to the event: move the flag and this reddens on the prose a reader
        // is shown.
        phrase: "Watch the entry step at {n} s",
        ties: &[Tie::Quoted {
            step: "leg-that-is-not-there",
            arm: None,
            quantity: "flag_first_s:SOC_CLAMPED_HIGH",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The charge current a fourth time, as the thing the entry step's refusal is
        // measured against. The `0.822` beside it is inside a claim's literal; this is not.
        phrase: "rather than the full {n}",
        ties: &[Tie::Setting(Control::DemandValue)],
        pow10: 0,
    },
    // Step 20 - what a cell does when you keep pulling past empty. When the scan was pointed
    // at it, 27 of its 42 numerals were unaccounted: the fewest of the five steps that were
    // left, and the shape is the reason. Half of them are constants of ONE chemistry file,
    // whose `[reversal]` section no earlier ledgered step reads at all. The step runs on two
    // trajectories - its own to the mark, and the charge leg the reader types `-2` into - so
    // two of the rules below read a control off the ARM rather than off the step. (It ends
    // at 41 numerals, and the difference is this slice's own: the prose lost a pair of
    // volts no file decides and gained an amp-hour total. Re-measure AFTER an edit; the
    // count beside the ledger entry is derived and checked.)
    LedgerRule {
        // Where the cell comes from and what it is asked for, in the sentence that opens the
        // step. Step 1's file and step 1's current, which is the whole comparison: this is
        // the first run in the path that does not stop at the knee.
        phrase: "The cell from step {n}, the same {n} A",
        ties: &[
            Tie::Ordinal("bare-curve"),
            Tie::Setting(Control::DemandValue),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The interval the charge state is confined to, which is the sentence's own reason
        // for carrying a deficit beside it. Both ends off the chemistry's charge column -
        // the table the next clause says stays indexable - and each read EXACTLY: the low
        // end is that column's first node, the high end is its span. A `Tie::Member` would
        // have accounted either token against any node of the table, so `[0, 0.4]` would
        // have gone green.
        phrase: "confined to `[{n}, {n}]`",
        ties: &[Tie::Chemistry("ocv.soc.0"), Tie::Span("ocv.soc.*")],
        pow10: 0,
    },
    LedgerRule {
        // The low end again, in the sentence that says what the clamp buys. Same field, two
        // rules, on step 6's terms.
        phrase: "`soc` stays pinned at {n}",
        ties: &[Tie::Chemistry("ocv.soc.0")],
        pow10: 0,
    },
    LedgerRule {
        // How long the whole event takes: the instant the fall stops, less the knee. Neither
        // number is in any file and both are this step's own claims, which is what a
        // difference of two quotations is for - and the lesson block's own comment sizes the
        // playback speed off this same 83 s, so a trajectory that moved would leave the
        // comment stale with the prose.
        phrase: "happens in the {n} seconds after the knee",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "past-empty",
                arm: None,
                quantity: "v_floor_s",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "past-empty",
                arm: None,
                quantity: "flag_first_s:SOC_CLAMPED_LOW",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The demand box, in the clause that says why the debt keeps growing after the
        // voltage stops moving.
        phrase: "because {n} A is still leaving",
        ties: &[Tie::Setting(Control::DemandValue)],
        pow10: 0,
    },
    LedgerRule {
        // The cell's format, inside the chemistry file's own name. A part number in the same
        // position as step 12's `M50` and step 1's `R0`, and anchored on `meta.id` rather
        // than on the prose's filename: the two agree by construction, since `sim-data`
        // loads `chemistries/<id>.toml`.
        phrase: "placeholders in `lfp_{n}_generic.toml`",
        ties: &[Tie::Name {
            field: "meta.id",
            prefix: "lfp_",
        }],
        pow10: 0,
    },
    LedgerRule {
        // How fast the open-circuit voltage falls below empty - the first `[reversal]` field
        // any ledgered step has read.
        phrase: "the fall is `v_per_soc = {n}`",
        ties: &[Tie::Chemistry("reversal.v_per_soc")],
        pow10: 0,
    },
    LedgerRule {
        // And how wide that fall is, which is the arithmetic the chemistry file's own
        // provenance comment states: the empty-endpoint open-circuit voltage over the rate
        // it falls at. Not `cell.v_min`, though the two spell 2.00 on this chemistry - the
        // ramp starts where the OCV TABLE ends, and a file whose declared cutoff sat above
        // its own curve would move one and not the other.
        phrase: "collapses over {n} % of its capacity",
        ties: &[Tie::Ratio(&[
            Tie::Difference(&[
                Tie::Chemistry("ocv.volts.0"),
                Tie::Chemistry("reversal.floor_v"),
            ]),
            Tie::Chemistry("reversal.v_per_soc"),
        ])],
        pow10: 2,
    },
    LedgerRule {
        // Where the fall stops, declared rather than measured - which is what the sentence
        // says about it.
        phrase: "the floor is `floor_v = {n}`",
        ties: &[Tie::Chemistry("reversal.floor_v")],
        pow10: 0,
    },
    LedgerRule {
        // What the reader types to start the second half of the step. The minus is in the
        // phrase and the tie takes the magnitude, on step 11's terms - and the phrase runs
        // to "which charges at the same rate" because step 21 tells a reader to put the same
        // box to the same `-2`, and both steps declare a `charge leg`.
        phrase: "put the demand box to **-{n}**, which charges at the same rate",
        ties: &[Tie::Magnitude(&Tie::OnArm {
            arm: "charge leg",
            tie: &Tie::Setting(Control::DemandValue),
        })],
        pow10: 0,
    },
    LedgerRule {
        // The same width as the `2 %` above, named as a depth of debt rather than as a
        // fraction of capacity, because that is the axis the sentence is on. Two rules for
        // one arithmetic, in the two sentences that print it.
        phrase: "re-enters the {n}-point ramp",
        ties: &[Tie::Ratio(&[
            Tie::Difference(&[
                Tie::Chemistry("ocv.volts.0"),
                Tie::Chemistry("reversal.floor_v"),
            ]),
            Tie::Chemistry("reversal.v_per_soc"),
        ])],
        pow10: 2,
    },
    LedgerRule {
        // THE SENTENCE THIS SLICE REWROTE, and the two amp-hour figures are not the same
        // number. `254 s at 2 A` is 0.1411 A*h - the duration the debt takes to clear, times
        // the current the reader typed. What came out below empty is 0.1410 - the debt at
        // the mark, times the nameplate. They differ by one in the last place because the
        // clock is on a half-second grid and the debt really clears 253.79 s in, and the
        // sentence used to print the second while showing the arithmetic for the first.
        //
        // Four slots, and every one of them is read somewhere else: the duration is this
        // step's own claim less its mark, the current is the arm's box, and the two totals
        // are the products those make with the hour and with the cell.
        phrase: "And {n} s at {n} A is {n} A\u{b7}h",
        ties: &[
            Tie::Difference(&[
                Tie::Quoted {
                    step: "past-empty",
                    arm: Some("charge leg"),
                    quantity: "deficit_zero_s",
                    states: QuotedAs::Same,
                },
                Tie::Setting(Control::Until),
            ]),
            Tie::Magnitude(&Tie::OnArm {
                arm: "charge leg",
                tie: &Tie::Setting(Control::DemandValue),
            }),
            Tie::Product(&[
                Tie::Hours(&Tie::Difference(&[
                    Tie::Quoted {
                        step: "past-empty",
                        arm: Some("charge leg"),
                        quantity: "deficit_zero_s",
                        states: QuotedAs::Same,
                    },
                    Tie::Setting(Control::Until),
                ])),
                Tie::Magnitude(&Tie::OnArm {
                    arm: "charge leg",
                    tie: &Tie::Setting(Control::DemandValue),
                }),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The other half of that sentence, and a SECOND rule rather than a fourth slot on
        // the one above: `pow10` belongs to the rule, and these two products are in
        // different units. The debt is in points, so the cell's nameplate times it is a
        // hundred times an amp-hour figure; the duration times the current is already one.
        // A rule spanning both would have to scale them alike and could only be right about
        // one.
        phrase: "against the {n} A\u{b7}h that came out below empty",
        ties: &[Tie::Product(&[
            Tie::Quoted {
                step: "past-empty",
                arm: None,
                quantity: "deficit_pts_at:4400",
                states: QuotedAs::Same,
            },
            Tie::Chemistry("cell.capacity_ah"),
        ])],
        pow10: -2,
    },
    LedgerRule {
        // What the engine used to do, and the number is this step's own knee: with no
        // deficit to fall through, an empty cell held the voltage it emptied at forever.
        // Quoted rather than re-measured, because the run that would produce it does not
        // exist any more - which is the point of the sentence.
        phrase: "a flat line at {n} V that never ended",
        ties: &[Tie::Quoted {
            step: "past-empty",
            arm: None,
            quantity: "v_at:4146.5",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    // Step 21 - what the excursion COST, and the first ledgered step whose prose runs on a
    // trajectory no reader can produce. Thirty-three of its fifty-eight numerals were
    // unaccounted when the scan was pointed here. Four needed claims - the crossing, the
    // resistance at the knee, the plot's own origin, and the control's mark reading - and
    // the rest divide into constants a file decides and arithmetic the sentences do in
    // front of the reader. This is the densest product-and-derivation block in the
    // vocabulary, because the step's whole method is "here is the number and here is the
    // multiplication that gives it".
    LedgerRule {
        // Where this scenario starts, which is the one thing separating it from step 20's:
        // 5 % rather than full, so the knee is three and a half minutes in.
        phrase: "starting at {n} % charge so that empty",
        ties: &[Tie::Scenario("pack.initial_soc")],
        pow10: 2,
    },
    LedgerRule {
        // The attribution, first half: this step's own subtraction, the knee against the
        // mark.
        phrase: "So of the {n} points lost after the knee",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "what-it-cost",
                arm: None,
                quantity: "soh_cap_at:207.5",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "what-it-cost",
                arm: None,
                quantity: "soh_cap_at:600",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 2,
    },
    LedgerRule {
        // And the second half, which is the same subtraction against the CONTROL and is
        // the whole reason `Arm::fade_per_ah` exists. Move the control arm and this number
        // moves with it, which is what makes the sentence a measurement rather than an
        // author's arithmetic.
        phrase: "**{n} are the reversal**",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "what-it-cost",
                arm: Some("no reversal damage"),
                quantity: "soh_cap_at:600",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "what-it-cost",
                arm: None,
                quantity: "soh_cap_at:600",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 2,
    },
    LedgerRule {
        // Step 20's floor, quoted for the first of three times. The step next door claims
        // it at its own mark; this step says it was flat there and is not flat here, so the
        // tie has to be to THAT reading and not to anything on this trajectory. It is why
        // `past-empty`'s ten `v_at` claims now include one tagged instant.
        phrase: "floor was a flat line at -{n} V",
        ties: &[Tie::Magnitude(&Tie::Quoted {
            step: "past-empty",
            arm: None,
            quantity: "v_at:4400",
            states: QuotedAs::Same,
        })],
        pow10: 0,
    },
    LedgerRule {
        // The same reading a second time, and STEP 20'S demand box beside it - not this
        // step's, though both are 2 A. The sentence is about what the last step's floor was
        // made of, so a rule pointed at this step's `Setting(DemandValue)` would be right
        // off the wrong field: retype step 21's box and the sentence would still be about
        // step 20's cell. That is the hazard `docs/plans/path-setting-arm.md` records, met
        // a second time and refused the same way.
        phrase: "The last step's -{n} V is `{n} A × {n} Ω`, and that",
        ties: &[
            Tie::Magnitude(&Tie::Quoted {
                step: "past-empty",
                arm: None,
                quantity: "v_at:4400",
                states: QuotedAs::Same,
            }),
            Tie::Elsewhere {
                step: "past-empty",
                tie: &Tie::Setting(Control::DemandValue),
            },
            Tie::Sum(&[Tie::Chemistry("r0.ohms.0.1"), Tie::Chemistry("rc.0.r_ohms")]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The same sum, in the clause that takes it apart. `[r0]` is a grid over charge and
        // temperature and this cell sits at the bottom of it at 25 degC, so the path names
        // the row and the column rather than letting a `*` find whichever 0.022 it reached
        // first - the generous match the whole table refuses.
        phrase: "and that {n} has two parts",
        ties: &[Tie::Sum(&[
            Tie::Chemistry("r0.ohms.0.1"),
            Tie::Chemistry("rc.0.r_ohms"),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The `0` of `R0`, a section of the chemistry file rather than a quantity, in the
        // same position as step 1's and step 11's. Same chemistry, same provenance string,
        // same prefix.
        phrase: "this cell's `R{n}` is",
        ties: &[Tie::Name {
            field: "meta.provenance",
            prefix: "R",
        }],
        pow10: 0,
    },
    LedgerRule {
        // The two numbers that make the 0.032, read off the two places the chemistry file
        // keeps them - and the ambient the table is read AT, which is the slider and not a
        // column heading: `[r0].temp_k` spells 298.15 and this sentence spells 25.
        phrase:
            "is {n} Ω at the bottom of its table at {n} °C, and its one RC pair adds {n} Ω more",
        ties: &[
            Tie::Chemistry("r0.ohms.0.1"),
            Tie::Setting(Control::Ambient),
            Tie::Chemistry("rc.0.r_ohms"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // How much aging has grown them by, as a percentage - and it is NOT measured here.
        // It is the coupling CLAUDE.md will not let a chemistry model alone: resistance
        // growth is capacity loss times `r_growth_per_capacity_loss`. So the tie is that
        // constant against this step's own claimed capacity loss, and the `soh res` claim
        // at the mark is left for the arithmetic sentence to quote. Two readings of one
        // number, and this is the one that would notice the coupling constant moving.
        phrase: "grows **both** by the same {n} %",
        ties: &[Tie::Product(&[
            Tie::Quoted {
                step: "what-it-cost",
                arm: None,
                quantity: "soh_cap_at:600",
                states: QuotedAs::Complement,
            },
            Tie::Chemistry("aging.r_growth_per_capacity_loss"),
        ])],
        pow10: 2,
    },
    LedgerRule {
        // The arithmetic sentence, in four rules. This one is the three FACTORS it
        // multiplies: this step's own demand box, the sum of the two resistances, and the
        // growth as a fraction. Its product is a rule of its own next door, because
        // `pow10` belongs to the rule and not to the tie - these three are amps, ohms and a
        // bare fraction, and the answer is in millivolts. Writing them as one rule is the
        // mistake this pair records: it scales every tie it carries, so the demand box came
        // out as 2000 A.
        phrase: "the extra sag is {n} A × {n} Ω × {n} =",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Sum(&[Tie::Chemistry("r0.ohms.0.1"), Tie::Chemistry("rc.0.r_ohms")]),
            Tie::Product(&[
                Tie::Quoted {
                    step: "what-it-cost",
                    arm: None,
                    quantity: "soh_cap_at:600",
                    states: QuotedAs::Complement,
                },
                Tie::Chemistry("aging.r_growth_per_capacity_loss"),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // And the answer, read off the sentence's OWN three numbers rather than off the
        // files they came from. That is the honest reading - a reader multiplies what is
        // printed - and it is also the safe one: a `Tie::Sum` of the two halves the
        // sentence prints next would give 4.7 and be wrong about a number that is right.
        phrase: "= **{n} mV**",
        ties: &[Tie::Derived {
            op: LedgerOp::Product,
            operands: &[
                Operand::Sibling("2"),
                Operand::Sibling("0.032"),
                Operand::Sibling("0.0726"),
            ],
        }],
        pow10: 3,
    },
    LedgerRule {
        // The two halves of that sag, each a product of four file-and-claim facts. NOT
        // siblings: the resistances they need are in the sentence BEFORE this one, and
        // `Operand::Sibling` reaches one sentence only.
        //
        // **The `1.5` is this step's trap.** `aging.r_growth_per_capacity_loss` is also
        // 1.5, and a rule pointed at it would have gone green on a number that means
        // millivolts. Read the sentence: the slow half is 2 A across the RC pair grown by
        // 7.26 %, which is 1.4529 mV and prints 1.5 by rounding. The coupling constant is a
        // FACTOR of it and not the number.
        phrase: "{n} of it from the instant resistance and {n} from the slow one",
        ties: &[
            Tie::Product(&[
                Tie::Setting(Control::DemandValue),
                Tie::Chemistry("r0.ohms.0.1"),
                Tie::Quoted {
                    step: "what-it-cost",
                    arm: None,
                    quantity: "soh_cap_at:600",
                    states: QuotedAs::Complement,
                },
                Tie::Chemistry("aging.r_growth_per_capacity_loss"),
            ]),
            Tie::Product(&[
                Tie::Setting(Control::DemandValue),
                Tie::Chemistry("rc.0.r_ohms"),
                Tie::Quoted {
                    step: "what-it-cost",
                    arm: None,
                    quantity: "soh_cap_at:600",
                    states: QuotedAs::Complement,
                },
                Tie::Chemistry("aging.r_growth_per_capacity_loss"),
            ]),
        ],
        pow10: 3,
    },
    LedgerRule {
        // The closing arithmetic: step 20's floor times this step's resistance health is
        // this step's floor. Three tokens, one rule, and no literal digit in the phrase -
        // the third is the product of the first two as the sentence writes them.
        phrase: "and -{n} × {n} = -{n} V is the reading",
        ties: &[
            Tie::Magnitude(&Tie::Quoted {
                step: "past-empty",
                arm: None,
                quantity: "v_at:4400",
                states: QuotedAs::Same,
            }),
            Tie::Quoted {
                step: "what-it-cost",
                arm: None,
                quantity: "soh_res_at:600",
                states: QuotedAs::Same,
            },
            Tie::Derived {
                op: LedgerOp::Product,
                operands: &[Operand::Sibling("0.0640"), Operand::Sibling("1.0726")],
            },
        ],
        pow10: 0,
    },
    LedgerRule {
        // The charge leg's own demand box. Step 20's twin phrase ends "which charges at the
        // same rate"; this one ends where this sentence does, which is what keeps the two
        // rules off each other.
        phrase: "put the demand box to **-{n}** and press Run, and compare",
        ties: &[Tie::Magnitude(&Tie::OnArm {
            arm: "charge leg",
            tie: &Tie::Setting(Control::DemandValue),
        })],
        pow10: 0,
    },
    LedgerRule {
        // The amp-hours in and the amp-hours out, read two different ways because they ARE
        // two different measurements.
        //
        // In: the leg's own duration times its own current, which is arithmetic step 20
        // does in front of the reader and this step does not.
        //
        // Out: NOT the deficit at the mark valued at the cell's final capacity - that is
        // 0.2127 and this sentence says 0.2182. The engine bills each step's charge past
        // empty against the capacity the cell had ON THAT STEP, and the cell shrank 4.8 %
        // while it was down there, so no endpoint product recovers the integral. What does
        // is the damage itself: the capacity the reversal cost, over the cost per amp-hour.
        // So this number is tied to the control arm too, and "even the arithmetic is exact"
        // is exact about a quantity the engine never prints.
        phrase: "it took **{n} A·h** to put back what **{n} A·h** took out",
        ties: &[
            Tie::Product(&[
                Tie::Hours(&Tie::Difference(&[
                    Tie::Quoted {
                        step: "what-it-cost",
                        arm: Some("charge leg"),
                        quantity: "deficit_zero_s",
                        states: QuotedAs::Same,
                    },
                    Tie::Setting(Control::Until),
                ])),
                Tie::Magnitude(&Tie::OnArm {
                    arm: "charge leg",
                    tie: &Tie::Setting(Control::DemandValue),
                }),
            ]),
            Tie::Ratio(&[
                Tie::Difference(&[
                    Tie::Quoted {
                        step: "what-it-cost",
                        arm: Some("no reversal damage"),
                        quantity: "soh_cap_at:600",
                        states: QuotedAs::Same,
                    },
                    Tie::Quoted {
                        step: "what-it-cost",
                        arm: None,
                        quantity: "soh_cap_at:600",
                        states: QuotedAs::Same,
                    },
                ]),
                Tie::Chemistry("reversal.fade_per_ah"),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // What FRACTION of the charge came back, read off the sentence's own two amp-hour
        // figures. It is here because the sentence used to offer the 4.8 % below as the
        // reason for a gap that is half that size, which is the "the prose's own arithmetic
        // does not close" defect `docs/plans/path-ledger-leg-that-is-not-there.md` records.
        // Printing the ratio is what makes the next clause necessary and checkable.
        phrase: "took out — **{n} %** of it",
        ties: &[Tie::Derived {
            op: LedgerOp::Ratio,
            operands: &[Operand::Sibling("0.2128"), Operand::Sibling("0.2182")],
        }],
        pow10: 2,
    },
    LedgerRule {
        // How much smaller the refilled cell ENDS UP: the complement of its own capacity
        // health at the mark. It is the direction of the effect and not its magnitude - the
        // shrink accrued while the charge was still coming out, so the charge-out integral
        // was billed against an average health well above this one, and the gap above is
        // 2.5 % against this 4.8. The sentence says so now instead of leaving the reader to
        // multiply.
        phrase: "Not by the whole **{n} %** it is down at the mark",
        ties: &[Tie::Quoted {
            step: "what-it-cost",
            arm: None,
            quantity: "soh_cap_at:600",
            states: QuotedAs::Complement,
        }],
        pow10: 2,
    },
    LedgerRule {
        // The mark's health printed a second time, in the sentence about the row that never
        // turns round. Quoted rather than re-claimed, on the terms this file has settled
        // on: one reading, one claim, and every other sentence that prints it says so.
        phrase: "never turns round: {n} % at the mark",
        ties: &[Tie::Quoted {
            step: "what-it-cost",
            arm: None,
            quantity: "soh_cap_at:600",
            states: QuotedAs::Same,
        }],
        pow10: 2,
    },
    LedgerRule {
        // And the debt at the mark printed a second time, in the caveat about the sampler.
        // The sentence's point is that the row catches up on a pause, so the number it
        // catches up TO is the step's own claimed reading.
        phrase: "after the pause read {n}, which is the mark's own value",
        ties: &[Tie::Quoted {
            step: "what-it-cost",
            arm: None,
            quantity: "deficit_pts_at",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },

    // Step 17 - the last step in the path to be ledgered, and the one whose numbers are
    // most nearly all its own: forty-four numerals, twenty-eight of them measurements on a
    // single trajectory. What is left for a rule is small and unusually varied - two
    // ordinals into the middle of the path and two more spanning its first twelve steps,
    // the demand box, the speed slider, the topology, a publication year inside a
    // parameter set's name, the two diffusion times the sentence works out in front of the
    // reader, and three figures worked out from this step's own claims. See
    // `docs/plans/path-ledger-the-gradient.md`.
    LedgerRule {
        // The two lessons this step's opening sentence builds on, in the order it names
        // them. Both point INTO the path rather than back at its start, which is what makes
        // an ordinal rule here worth more than the usual back-reference: this step sits
        // between the pair it is comparing.
        phrase: "Step {n}'s rebound was a gradient relaxing; step {n}'s collapse",
        ties: &[
            Tie::Ordinal("particle-remembers"),
            Tie::Ordinal("the-electrolyte-starves"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The whole equivalent-circuit half of the path, named by its two ends. `1` and `12`
        // are the positions of the first lesson and of the last one before the porous models
        // arrive, so inserting a particle step anywhere inside that run turns this sentence
        // red - which is exactly the sentence's claim, that the row says "no electrodes"
        // for every one of them.
        phrase: "on steps {n} to {n} that row says so",
        ties: &[
            Tie::Ordinal("bare-curve"),
            Tie::Ordinal("circuit-repeats-itself"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The demand box, and the twin that shares this file. Step 15 runs the same scenario
        // at the same current to a mark of 500 s; this one runs it as a pulse to the cut-off
        // and then rests. The `Setting` reads the pulse group's current, which is the box
        // `applyDemandMode` puts on screen here - see `Control::DemandValue`.
        phrase: "The same file and the same {n} A as step {n}",
        ties: &[
            Tie::Setting(Control::DemandValue),
            Tie::Ordinal("looks-fine-from-outside"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // A publication year inside a parameter set's name, which is `Tie::Name`'s shape
        // exactly: the `2020` of Chen2020 is no more a quantity than the `50` of an LG M50.
        // The prefix reaches one digit run and not two - the provenance names the same
        // authors again as "Chang-Hui Chen et al.", where the characters after `Chen` are
        // not digits at all.
        phrase: "two numbers from Chen{n} that are not placeholders",
        ties: &[Tie::Name {
            field: "meta.provenance",
            prefix: "Chen",
        }],
        pow10: 0,
    },
    LedgerRule {
        // The sentence states its own formula - radius squared over diffusivity - so the
        // rule is that formula and not a pair of remembered numbers. Both are extracted
        // Chen2020 keys, which is the sentence's other claim about them.
        //
        // **This is where the scan found its arithmetic defect.** The negative electrode's
        // 5.86e-6 m and 3.3e-14 m^2/s give 1040.594 s, and the prose said `1040 s`. A
        // computed tie is compared at the precision the sentence commits to and `to_fixed`
        // rounds away from zero, so the sentence now says 1041; the positive's 6812.100 was
        // right as written. A figure that had been truncated rather than rounded is the
        // narrowest kind of prose defect this scan can find, and it is invisible to every
        // check that ran before this step was ledgered.
        phrase: "which is **{n} s** for the negative and **{n} s** for the positive",
        ties: &[
            Tie::Ratio(&[
                Tie::Product(&[
                    Tie::Chemistry("spm.negative.particle_radius_m"),
                    Tie::Chemistry("spm.negative.particle_radius_m"),
                ]),
                Tie::Chemistry("spm.negative.diffusivity_m2_per_s"),
            ]),
            Tie::Ratio(&[
                Tie::Product(&[
                    Tie::Chemistry("spm.positive.particle_radius_m"),
                    Tie::Chemistry("spm.positive.particle_radius_m"),
                ]),
                Tie::Chemistry("spm.positive.diffusivity_m2_per_s"),
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // How much of the rebound is over at the instant the negative electrode finishes.
        // Three of this step's own claims and one of step 15's, and not one of the three
        // voltages the fraction is built from is printed in this clause - the sentence
        // prints the answer. That is `Tie::Difference`'s own distinction from
        // `Tie::Derived`, taken one level further: a ratio OF two differences, which is the
        // first place in this vocabulary where an arithmetic tie nests inside another.
        //
        // The floor of the rebound is the cut-off voltage, which this step never prints and
        // step 15 does - *"2.495 V at 1060 s"*, on its own continuation arm. Reading it
        // through `Tie::Quoted` rather than re-measuring is what keeps the two lessons'
        // accounts of one trajectory from drifting apart: they are the same file at the same
        // current, and step 15's claim is checked against the engine where it lives.
        phrase: "so about **{n} % of the rebound is already over**",
        ties: &[Tie::Ratio(&[
            Tie::Difference(&[
                Tie::Quoted {
                    step: "the-gradient-itself",
                    arm: None,
                    quantity: "v_at:1396",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "looks-fine-from-outside",
                    arm: Some("carries on"),
                    quantity: "v_at:1060",
                    states: QuotedAs::Same,
                },
            ]),
            Tie::Difference(&[
                Tie::Quoted {
                    step: "the-gradient-itself",
                    arm: None,
                    quantity: "v_at:2860",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "looks-fine-from-outside",
                    arm: Some("carries on"),
                    quantity: "v_at:1060",
                    states: QuotedAs::Same,
                },
            ]),
        ])],
        pow10: 2,
    },
    LedgerRule {
        // What is left of the rebound after the negative electrode has finished with it,
        // in millivolts: the mark's terminal less the terminal at that crossing, both of
        // them this step's own claims. `pow10 = 3` is the sentence's unit against the
        // engine's, the same conversion a claim's `spells_pow10` carries.
        phrase: "That final {n} mV takes 1464 s",
        ties: &[Tie::Difference(&[
            Tie::Quoted {
                step: "the-gradient-itself",
                arm: None,
                quantity: "v_at:2860",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "the-gradient-itself",
                arm: None,
                quantity: "v_at:1396",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 3,
    },
    LedgerRule {
        // The speed slider, in the footnote about the instrument rather than the cell. It is
        // the one control on this step that changes nothing about the trajectory and
        // everything about what a reader can read off it, which is the footnote's point.
        phrase: "so at {n}\u{d7} they step in jumps",
        ties: &[Tie::Setting(Control::Speed)],
        pow10: 0,
    },
    LedgerRule {
        // The topology, in the sentence explaining why this quantity lives on the readout row
        // rather than in the pack grid. Two ties and not one: `1S1P` is a series count and a
        // parallel count written together, and a single rule reading `pack.series` twice
        // would go green on a 1S4P pack.
        phrase: "this one is {n}S{n}P",
        ties: &[
            Tie::Scenario("pack.series"),
            Tie::Scenario("pack.parallel"),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The rate the demand box works out to on this cell, in the closing warning that the
        // pulse train has come round again. Same shape as step 1's, off this step's own
        // pulse box and this chemistry's nameplate.
        phrase: "the start of another {n} C discharge",
        ties: &[Tie::Ratio(&[
            Tie::Setting(Control::DemandValue),
            Tie::Chemistry("cell.capacity_ah"),
        ])],
        pow10: 0,
    },

    // --- step 17, and the first rules written for quantities the prose spells in WORDS ---
    //
    // Seven of the fifteen spelled quantities on this step needed no rule: they sit inside a
    // claimed sentence and name the instant that sentence's own claim is read at, so
    // `claimed_accounting` answers them through `Accounted::ReadAt` exactly as it does for a
    // number in digits. These are the nine that do not.
    LedgerRule {
        // The rest leg, in the unit the sentence writes it. `Written::scale` carries the
        // hour, so what is compared here is 1800 s against half of one.
        phrase: "left to rest for {n} s.",
        ties: &[Tie::Setting(Control::PulseOff)],
        pow10: 0,
    },
    LedgerRule {
        // Two spelled quantities in one clause, and the pair is why `Tie::Instant` exists.
        // The instant is the one a claim two sentences earlier is read at — pinned by that
        // claim's own value, so it is not a number this file supplies to itself. What moves
        // across the stretch is the difference between the two readings that bracket it,
        // which the prose states to the thousandth and nothing had ever divided.
        phrase: "Across that last stretch from {n} s on it moves by {n} points in total",
        ties: &[
            Tie::Instant {
                step: "the-gradient-itself",
                arm: None,
                quantity: "surface_gap_neg_pts:360",
            },
            Tie::Difference(&[
                Tie::Quoted {
                    step: "the-gradient-itself",
                    arm: None,
                    quantity: "surface_gap_neg_pts:1060",
                    states: QuotedAs::Same,
                },
                Tie::Quoted {
                    step: "the-gradient-itself",
                    arm: None,
                    quantity: "surface_gap_neg_pts:360",
                    states: QuotedAs::Same,
                },
            ]),
        ],
        pow10: 0,
    },
    LedgerRule {
        // The headline contrast of the step, and a ratio of two claims neither of which the
        // sentence prints. "More than six" is a computed tie rounded at the prose's own
        // precision, which for a word is the unit it is written in.
        phrase: "more than {n} times the negative's",
        ties: &[Tie::Ratio(&[
            Tie::Quoted {
                step: "the-gradient-itself",
                arm: None,
                quantity: "surface_gap_pos_pts:1060",
                states: QuotedAs::Same,
            },
            Tie::Quoted {
                step: "the-gradient-itself",
                arm: None,
                quantity: "surface_gap_neg_pts:1060",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The discharge leg, which no single control holds: the mark less the rest is what
        // the page runs under load. A difference is a computed tie, which is what makes the
        // sentence's rounding to the minute admissible where a restatement of a constant
        // would not be.
        phrase: "has time to settle inside a {n} s discharge",
        ties: &[Tie::Difference(&[
            Tie::Setting(Control::Until),
            Tie::Setting(Control::PulseOff),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // How long the last of the rebound takes: the mark, less the instant the negative
        // gap first reads zero. That instant is a measurement this step already claims, so
        // the sentence is tied to a trajectory through it.
        phrase: "That final 33 mV takes {n} s of simulated time",
        ties: &[Tie::Difference(&[
            Tie::Setting(Control::Until),
            Tie::Quoted {
                step: "the-gradient-itself",
                arm: None,
                quantity: "gap_neg_zero_s",
                states: QuotedAs::Same,
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The sampler's clock, stated as a rate where the page holds a period.
        phrase: "are sampled {n} times per second",
        ties: &[Tie::PerSecond(&Tie::Page("CELLS_PERIOD_MS"))],
        pow10: 0,
    },
    LedgerRule {
        // How far the simulation moves between two samples: the speed the slider is set to,
        // times the sampler's period. The page holds that period in milliseconds and the
        // sentence speaks seconds, which is the one thing `pow10` is for.
        phrase: "they step in jumps of roughly {n} simulated seconds and the exact peak",
        ties: &[Tie::Product(&[
            Tie::Setting(Control::Speed),
            Tie::Page("CELLS_PERIOD_MS"),
        ])],
        pow10: -3,
    },

    // --- the second batch of word-scanned steps -------------------------------------
    //
    // Six lessons, and the vocabulary they need is almost entirely one shape the first batch
    // never met: an instant stated RELATIVE to another one. "eight and a half seconds after
    // it", "thirty seconds later", "ten seconds after the flag" - none of these is a number
    // any file holds, and each is the difference between two instants that two claims are
    // read at. `Tie::Instant` and `Tie::Difference` both already existed; what the batch
    // needed was for the claims at BOTH ends to be addressable, which is why several
    // readings on `nothing-to-clamp` grew instant tags in the same slice.
    LedgerRule {
        // Step 1's two flags, and the gap between them is the whole sentence: the terminal
        // crosses the chemistry's floor first, and the coulomb counter reports empty after.
        // Neither instant is in any file - both are pinned by claims asserting that a flag
        // first appears there.
        phrase: "`SOC_CLAMPED_LOW`, {n} s after it,",
        ties: &[Tie::Difference(&[
            Tie::Instant {
                step: "bare-curve",
                arm: None,
                quantity: "flag_first_s:SOC_CLAMPED_LOW",
            },
            Tie::Instant {
                step: "bare-curve",
                arm: None,
                quantity: "flag_first_s:OPERATING_POINT_OUT_OF_WINDOW",
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The rest leg of the pulse train, in the minutes the sentence writes it.
        phrase: "climbs slowly for the rest of the {n} s.",
        ties: &[Tie::Setting(Control::PulseOff)],
        pow10: 0,
    },
    LedgerRule {
        // The step's whole premise: one field of one file changed, and the multiple is this
        // demand box over the previous lesson's. Neither current is spelled here - the
        // sentence prints the multiple and the two boxes sit in two different lessons.
        phrase: "That is 3 C, {n} times the last step's current",
        ties: &[Tie::Ratio(&[
            Tie::Setting(Control::DemandValue),
            Tie::Elsewhere {
                step: "slow-and-patient",
                tie: &Tie::Setting(Control::DemandValue),
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The step length, spelled. The hyphenated attributive is one token to the scanner
        // and half a second to the file.
        phrase: "same {n} s step.",
        ties: &[Tie::Setting(Control::Dt)],
        pow10: 0,
    },
    LedgerRule {
        // The lead-acid rating condition, twice: the label's 7.2 A.h is quoted AT a rate,
        // and the rate is the capacity over the demand box - which is a number of HOURS.
        // The sentence writes that number in hours too, so the ratio is read as it comes;
        // it took `Tie::Seconds` for as long as the prose spelled it "twenty hours", which
        // the scanner normalised to seconds before any tie could see it.
        phrase: "if you take {n} hours over it",
        ties: &[Tie::Ratio(&[
            Tie::Chemistry("cell.capacity_ah"),
            Tie::Setting(Control::DemandValue),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // ...and the sentence that says this step meets that condition. Two rules and not
        // one, because they are two sentences: the first quotes a datasheet and the second
        // claims the run obeys it, and a phrase loose enough to match both would be loose
        // enough to match a third.
        phrase: "This step takes {n} hours over it.",
        ties: &[Tie::Ratio(&[
            Tie::Chemistry("cell.capacity_ah"),
            Tie::Setting(Control::DemandValue),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // What is left in the cell at the mark, as the points the sentence says are NOT
        // delivered. A measurement, so it is quoted from the step's own claim rather than
        // read off a file.
        phrase: "gives you all but {n} points of what it holds",
        ties: &[Tie::Quoted {
            step: "slow-and-patient",
            arm: None,
            quantity: "soc_at",
            states: QuotedAs::Same,
        }],
        pow10: 2,
    },
    LedgerRule {
        // The mark in hours, and the FIRST sentence in the path to reach `Tie::Hours`
        // directly. Until this one, that variant's only use sat inside a product, which is
        // why it carries a test asking it the question no rule could.
        //
        // The sentence used to say "about two and a half days", which is 2.31 days, and no
        // fraction this scanner can spell rounds to that. See the plan doc.
        phrase: "of simulation — about {n} hours — which is",
        ties: &[Tie::Hours(&Tie::Setting(Control::Until))],
        pow10: 0,
    },
    LedgerRule {
        // How long the reader waits: the mark over the speed slider. The one control the
        // trajectory cannot see, and this sentence is entirely about it.
        phrase: "which is {n} s of watching at",
        ties: &[Tie::Ratio(&[
            Tie::Setting(Control::Until),
            Tie::Setting(Control::Speed),
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The square-root law, stated as the pair of readings that bracket it: four times
        // the time for twice the damage. Both instants are claims on this step, so the
        // multiple is a ratio of two `read_at_s` and not a number an author picked.
        phrase: "twice the damage for {n} times the time",
        ties: &[Tie::Ratio(&[
            Tie::Instant {
                step: "wearing-out-while-idle",
                arm: None,
                quantity: "soh_cap_at:200000",
            },
            Tie::Instant {
                step: "wearing-out-while-idle",
                arm: None,
                quantity: "soh_cap_at:50000",
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // The two shorts, which differ in one field of one scenario file and in nothing
        // else. The sentence prints the multiple; the files hold the two resistances.
        phrase: "instead of 30. {n} times the resistance is a third of the sag",
        ties: &[Tie::Ratio(&[
            Tie::Scenario("faults.*.fault.ExternalShort.ohms"),
            Tie::Elsewhere {
                step: "one-step-that-got-through",
                tie: &Tie::Scenario("faults.*.fault.ExternalShort.ohms"),
            },
        ])],
        pow10: 0,
    },
    LedgerRule {
        // What the STRONGER short cost the lesson that caught it, quoted here as the thing
        // this fault's fifty points are measured against. Before this rule the pair was
        // compared nowhere: each step measured its own loss, and the note on the claim
        // beside this sentence recorded the twin's figure in prose rather than reading it.
        //
        // The other half of the contrast needs no rule. `nothing-to-clamp`'s own claim
        // `spells` the word *fifty*, which is what the sentence writes, and that claim
        // answers to the engine where it lives.
        phrase: "against the twin's {n} points, this fault costs",
        ties: &[Tie::Quoted {
            step: "one-step-that-got-through",
            arm: None,
            quantity: "soc_lost_pts_at",
            states: QuotedAs::Same,
        }],
        pow10: 0,
    },
    LedgerRule {
        // The peak temperature lags the flag, which is the sentence's whole point: the
        // current is already collapsing while the cell is still getting hotter.
        //
        // The last relative instant on this step, and the only one left. Its two siblings -
        // "four and a half seconds later" and "thirty seconds later" - said in words what
        // this step's own reading list says in digits everywhere else, and the digits rule
        // turned them into the absolute instants their neighbours already were, which their
        // claims' own `read_at_s` decides. The claims still carry instant tags, because
        // without an address at each end `Tie::Instant` sees eight readings under one name
        // and refuses.
        phrase: "s**, {n} s after the flag,",
        ties: &[Tie::Difference(&[
            Tie::Instant {
                step: "nothing-to-clamp",
                arm: Some("bms off"),
                quantity: "t_max_at:245.5",
            },
            Tie::Instant {
                step: "nothing-to-clamp",
                arm: Some("bms off"),
                quantity: "flag_first_s:SOC_CLAMPED_LOW",
            },
        ])],
        pow10: 0,
    },
];

/// The scenario file, as the file writes it.
///
/// Raw TOML rather than `sim_data::Scenario`, so a rule's path is the key an author reads
/// in the file rather than a field name this workspace chose. The typed load is still what
/// the value check runs on; this is only for looking a constant up by name.
fn scenario_toml(file: &str) -> toml::Value {
    let text = read(&repo_root().join("scenarios").join(file));
    toml::from_str(&text).unwrap_or_else(|e| panic!("scenarios/{file} parses as TOML: {e}"))
}

/// Every number at a dotted key path, with `*` walking an array and a digit indexing one.
///
/// The two array segments answer two different sentences and neither is the other's
/// fallback. `*` is for prose about *every* member — step 5's "both faults land at 600 s",
/// where reaching only one of them would be a fail-toward-green — and it is read strictly
/// for that reason. An index is for prose about *one*: step 13 names the two RC pairs
/// separately ("9 s and 72 s"), and each is a product of that pair's own two fields, which
/// `*` cannot express because a [`Tie::Product`] factor reaching two values resolves to
/// nothing by design.
///
/// A digit segment only indexes when the value it is applied to is an array, so a table
/// whose key happens to be a numeral is still reached as a key.
///
/// Empty means the path is not in the file at all, which is a broken rule rather than a
/// disagreement, and the caller says so differently.
fn numbers_at_path<'a>(value: &'a toml::Value, path: &str) -> Vec<f64> {
    let mut here: Vec<&'a toml::Value> = vec![value];
    for seg in path.split('.') {
        let mut next = Vec::new();
        for v in here {
            if seg == "*" {
                if let Some(a) = v.as_array() {
                    next.extend(a.iter());
                }
            } else if let Some(a) = v.as_array() {
                if let Some(child) = seg.parse::<usize>().ok().and_then(|i| a.get(i)) {
                    next.push(child);
                }
            } else if let Some(child) = v.as_table().and_then(|t| t.get(seg)) {
                next.push(child);
            }
        }
        here = next;
    }
    here.iter()
        .filter_map(|v| match v {
            toml::Value::Integer(i) => Some(*i as f64),
            toml::Value::Float(f) => Some(*f),
            _ => None,
        })
        .collect()
}

/// The chemistry file the step's scenario names, as that file writes it.
///
/// Raw TOML for the reason [`scenario_toml`] is: a rule's path should be the key an author
/// reads in `chemistries/*.toml`, not a field name `sim-data` chose for the parsed struct.
fn chemistry_toml(scenario_file: &str) -> toml::Value {
    let scenario = scenario_toml(scenario_file);
    let id = scenario
        .get("chemistry")
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("scenarios/{scenario_file} still names a `chemistry`"))
        .to_string();
    let path = repo_root().join("chemistries").join(format!("{id}.toml"));
    let text = read(&path);
    toml::from_str(&text).unwrap_or_else(|e| panic!("chemistries/{id}.toml parses as TOML: {e}"))
}

/// The value a lesson's control box carries, or `None` if that lesson has no such box.
///
/// `Rest` has no demand value at all, so a rule reading one on a resting step resolves to
/// nothing and fails as a broken rule rather than matching by accident.
fn control_value(control: Control, lesson: &Lesson) -> Option<f64> {
    match control {
        Control::DemandValue => match lesson.demand {
            Prog::Current(i) => Some(i),
            Prog::CcCv { i, .. } => Some(i),
            Prog::Pulse { i, .. } => Some(i),
            Prog::Rest => None,
        },
        Control::PulseOn => match lesson.demand {
            Prog::Pulse { on_s, .. } => Some(on_s),
            _ => None,
        },
        Control::PulseOff => match lesson.demand {
            Prog::Pulse { off_s, .. } => Some(off_s),
            _ => None,
        },
        Control::Taper => match lesson.demand {
            Prog::CcCv { taper, .. } => Some(taper),
            _ => None,
        },
        Control::CcCvVoltage => match lesson.demand {
            Prog::CcCv { v_cell, .. } => Some(v_cell),
            _ => None,
        },
        Control::Ambient => Some(lesson.ambient_c),
        Control::Until => Some(lesson.until_s),
        Control::Speed => lesson.speed_x,
        Control::Dt => Some(lesson.dt),
    }
}

/// The same control read off an **arm** — what the reader dials in, rather than what the
/// step arrives with. See [`Tie::OnArm`].
///
/// `None` means this arm leaves that control alone, and the caller turns that into a failure
/// rather than into the step's own value: a silent fallback is the generous match the whole
/// vocabulary refuses.
fn arm_control_value(control: Control, arm: &Arm) -> Option<f64> {
    match control {
        // The current in whichever field group is on screen. `demand_a` is the simple box,
        // `cc_cv_a` is the CC-CV group's charge current and `pulse_a` is the pulse group's;
        // `applyDemandMode` shows one of the three and hides the other two, so at most one of
        // these is ever set and the `or` chain is not a preference between them. See
        // [`Arm::cc_cv_a`] and [`Arm::pulse_a`].
        Control::DemandValue => arm.cc_cv_a.or(arm.pulse_a).or(arm.demand_a),
        Control::Ambient => arm.ambient_c,
        Control::Dt => arm.dt,
        // An arm overrides the demand box, the `dt` box, the BMS checkbox and the ambient
        // slider, and nothing else. The pulse legs, the taper, the CC-CV target, the mark
        // and the speed are the step's for the whole of it, so a rule asking an arm for one
        // is asking a question the page cannot answer. (`demand_a` is the simple box's
        // *current*, which is what an arm types; an arm that switched the *mode* would be a
        // different trajectory and not an override — which is exactly why `cc_cv_a` is a
        // second field rather than `demand_a` learning to read the mode.)
        Control::PulseOn
        | Control::PulseOff
        | Control::Until
        | Control::Speed
        | Control::Taper
        | Control::CcCvVoltage => None,
    }
}

/// What a [`Tie::Derived`] reads: the step's prose, the numbers in it, how each of those is
/// accounted for, and which one is being accounted right now.
///
/// Carried alongside the files rather than folded into them, because this is the one arm
/// whose answer is in the sentence rather than in the tree.
struct SentenceCtx<'a> {
    step: &'a str,
    /// The step's whole prose, as [`every_numeral_in_a_ledgered_step_is_accounted_for`]
    /// scans it.
    text: &'a str,
    numbers: &'a [Written],
    /// Which vocabulary rule covers each number, as [`cover_by_rule`] reports it.
    cover: &'a [Option<(usize, usize)>],
    /// The index into `numbers` of the number being accounted for.
    at: usize,
    all: &'a [Claim],
    arms: &'a [Arm],
    derived: &'a [Derivation],
}

/// The sentence around a byte offset — as far as the nearest full stop, line break or
/// string boundary on each side.
///
/// Deliberately conservative at both ends. The prose is scraped as JavaScript source, so a
/// quote is where one paragraph stops being another; narrowing too far can only make an
/// operand unreachable, which fails loudly, while widening would let a [`Tie::Derived`]
/// reach a number in a sentence the reader is not looking at.
fn sentence_span(text: &str, at: usize) -> (usize, usize) {
    let before = &text[..at];
    let from = [
        before.rfind('\n').map(|i| i + 1),
        before.rfind('"').map(|i| i + 1),
        before.rfind(". ").map(|i| i + 2),
    ]
    .into_iter()
    .flatten()
    .max()
    .unwrap_or(0);
    let after = &text[at..];
    let to = [
        after.find('\n'),
        after.find('"'),
        after.find(". ").map(|i| i + 1),
    ]
    .into_iter()
    .flatten()
    .min()
    .map_or(text.len(), |i| at + i);
    (from, to)
}

/// What one operand of a [`Tie::Derived`] is worth, or `None` if the sentence does not
/// supply it — which the caller reports as a broken rule.
///
/// The two fences the arm rests on are here, and both panic rather than resolving: an
/// operand that matches two tokens of the same sentence, and one that nothing else accounts
/// for. See [`Operand`].
fn operand_value(op: &Operand, ctx: &SentenceCtx, lesson: &Lesson) -> Option<f64> {
    match op {
        Operand::Word(w) => WORD_NUMERALS
            .iter()
            .find(|(word, _)| word == w)
            .map(|(_, v)| *v),
        Operand::Sibling(token) => {
            let (from, to) = sentence_span(ctx.text, ctx.numbers[ctx.at].at);
            let mut hits = ctx.numbers.iter().enumerate().filter(|(i, w)| {
                *i != ctx.at && w.token == *token && w.at >= from && w.at + w.len <= to
            });
            let (i, w) = hits.next()?;
            assert!(
                hits.next().is_none(),
                "step `{}`: a derivation reads the operand `{token}`, and the sentence it \
                 is in prints that number more than once. Which one it means would be \
                 decided by scan order rather than by the sentence.",
                ctx.step,
            );
            let by_rule = ctx.cover[i].map(|(r, p)| &LEDGER_VOCABULARY[r].ties[p]);
            let claimed = claimed_accounting(
                w,
                ctx.text,
                ctx.step,
                ctx.all,
                lesson,
                ctx.arms,
                ctx.derived,
            );
            assert!(
                claimed.is_some()
                    || matches!(by_rule, Some(t) if !matches!(t, Tie::Derived { .. })),
                "step `{}`: a derivation reads the operand `{token}`, and nothing else \
                 accounts for that number — or the only thing that does is another \
                 derivation.\n\
                 An identity over unaccounted numbers says only that two free figures \
                 multiply into a third, which is the declared identity this arm exists \
                 instead of. Tie the operand to a file, a control or a claim first.",
                ctx.step,
            );
            number_of(&w.token)
        }
    }
}

/// Every number a tie resolves to. Empty means it resolves to nothing at all, which the
/// caller reports as a broken rule rather than as a disagreement.
fn tie_values(
    tie: &Tie,
    lesson: &Lesson,
    lessons: &[Lesson],
    scenario: &toml::Value,
    chemistry: &toml::Value,
    ctx: &SentenceCtx,
) -> Vec<f64> {
    match tie {
        Tie::Scenario(path) => numbers_at_path(scenario, path),
        Tie::Chemistry(path) | Tie::Member(path) => numbers_at_path(chemistry, path),
        Tie::Setting(control) => control_value(*control, lesson).into_iter().collect(),
        Tie::Product(factors) => {
            let mut product = 1.0;
            for factor in *factors {
                let values = tie_values(factor, lesson, lessons, scenario, chemistry, ctx);
                // Exactly one, never "the first of several": a wildcard under a product
                // would make which value it used the author's pick rather than the file's.
                let [only] = values[..] else {
                    return Vec::new();
                };
                product *= only;
            }
            vec![product]
        }
        Tie::Ratio(pair) => {
            let [over, by] = pair else {
                panic!(
                    "a `Tie::Ratio` takes exactly two ties; this one has {}",
                    pair.len()
                );
            };
            let (over, by) = (
                tie_values(over, lesson, lessons, scenario, chemistry, ctx),
                tie_values(by, lesson, lessons, scenario, chemistry, ctx),
            );
            // Exactly one on each side, for `Product`'s reason, and a zero divisor resolves
            // to nothing rather than to an infinity the comparison would then round.
            let ([over], [by]) = (&over[..], &by[..]) else {
                return Vec::new();
            };
            if *by == 0.0 {
                return Vec::new();
            }
            vec![over / by]
        }
        Tie::Difference(pair) => {
            let [less, by] = pair else {
                panic!(
                    "a `Tie::Difference` takes exactly two ties; this one has {}",
                    pair.len()
                );
            };
            let (less, by) = (
                tie_values(less, lesson, lessons, scenario, chemistry, ctx),
                tie_values(by, lesson, lessons, scenario, chemistry, ctx),
            );
            // Exactly one on each side, for `Product`'s reason: with several, which one the
            // sentence meant would be the author's pick rather than the file's.
            let ([less], [by]) = (&less[..], &by[..]) else {
                return Vec::new();
            };
            vec![less - by]
        }
        Tie::Sum(pair) => {
            let [one, other] = pair else {
                panic!(
                    "a `Tie::Sum` takes exactly two ties; this one has {}",
                    pair.len()
                );
            };
            let (one, other) = (
                tie_values(one, lesson, lessons, scenario, chemistry, ctx),
                tie_values(other, lesson, lessons, scenario, chemistry, ctx),
            );
            // Exactly one on each side, for `Product`'s reason. Order is not checked
            // because a sum has none; see the variant's docs.
            let ([one], [other]) = (&one[..], &other[..]) else {
                return Vec::new();
            };
            vec![one + other]
        }
        Tie::Instant {
            step,
            arm,
            quantity,
        } => {
            let hits: Vec<&Claim> = ctx
                .all
                .iter()
                .filter(|c| c.step == *step && c.arm.as_deref() == *arm && c.quantity == *quantity)
                .collect();
            let [first, rest @ ..] = &hits[..] else {
                return Vec::new();
            };
            // Agreement checked and not assumed, on `Tie::Quoted`'s terms: a quantity two
            // claims answer at two instants would hand the sentence whichever the file
            // listed first.
            assert!(
                rest.iter().all(|c| c.read_at_s == first.read_at_s),
                "a rule reads the instant of step `{step}`'s `{quantity}` on {}, and the \
                 claims on it were read at different times: {:?}. Which one the sentence \
                 means would be decided by file order rather than by the sentence. Name a \
                 quantity that is one reading \u{2014} tag its instant, as `v_at:400` does.",
                arm.map_or("its own run".to_string(), |a| format!("the arm `{a}`")),
                hits.iter().map(|c| c.read_at_s).collect::<Vec<_>>(),
            );
            vec![first.read_at_s]
        }
        Tie::Hours(seconds) => {
            let seconds = tie_values(seconds, lesson, lessons, scenario, chemistry, ctx);
            let [seconds] = &seconds[..] else {
                return Vec::new();
            };
            vec![seconds / 3600.0]
        }
        Tie::PerSecond(period_ms) => {
            let ms = tie_values(period_ms, lesson, lessons, scenario, chemistry, ctx);
            let [ms] = &ms[..] else {
                return Vec::new();
            };
            // A zero period resolves to nothing rather than to an infinity the comparison
            // would then round, which is `LedgerOp::Ratio`'s rule and for its reason.
            if *ms == 0.0 {
                return Vec::new();
            }
            vec![1000.0 / ms]
        }
        Tie::Span(path) => {
            let values = numbers_at_path(chemistry, path);
            // A span needs two ends. One value spans zero, and a sentence saying a
            // chemistry spans nothing would then be accounted by an emptied table.
            if values.len() < 2 {
                return Vec::new();
            }
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for v in values {
                lo = lo.min(v);
                hi = hi.max(v);
            }
            vec![hi - lo]
        }
        Tie::Clock => numeric_tokens(&fmt_time(lesson.until_s))
            .iter()
            .filter_map(|t| number_of(t))
            .collect(),
        // One constant, one parser. The name is matched rather than looked up in a table so
        // that a rule naming a constant nothing parses fails here by name, instead of
        // resolving to nothing and being reported as a restructured file.
        Tie::Page(name) => match *name {
            "CCCV_PERIOD_S" => vec![cccv_period_s()],
            "CELLS_PERIOD_MS" => vec![cells_period_ms()],
            other => panic!(
                "a rule reads the page constant `{other}`, and this file has no parser for \
                 it. A tie able to read any expression out of `web/app.js` would find a \
                 number for almost any token; each constant a sentence prints gets its own \
                 reader, on `default_dt`'s terms."
            ),
        },
        Tie::Derived { op, operands } => {
            let mut values = Vec::new();
            for operand in *operands {
                let Some(v) = operand_value(operand, ctx, lesson) else {
                    return Vec::new();
                };
                values.push(v);
            }
            let [first, rest @ ..] = &values[..] else {
                return Vec::new();
            };
            match op {
                LedgerOp::Product => vec![values.iter().product()],
                LedgerOp::Difference => vec![rest.iter().fold(*first, |a, b| a - b)],
                // A zero divisor resolves to nothing rather than to an infinity the
                // comparison would then round, exactly as `Tie::Ratio` refuses one.
                LedgerOp::Ratio if rest.contains(&0.0) => Vec::new(),
                LedgerOp::Ratio => vec![rest.iter().fold(*first, |a, b| a / b)],
            }
        }
        Tie::OnArm { arm, tie } => {
            let found = ctx
                .arms
                .iter()
                .find(|a| a.step == lesson.id && a.name == *arm)
                .unwrap_or_else(|| {
                    panic!(
                        "a rule on step `{}` reads the arm `{arm}`, and that step declares no \
                         arm of that name. An arm is what makes the reader's control change a \
                         trajectory; a rule naming one that is gone is reading a sentence \
                         nobody can follow.",
                        lesson.id,
                    )
                });
            match tie {
                Tie::Setting(control) => {
                    let value = arm_control_value(*control, found).unwrap_or_else(|| {
                        panic!(
                            "a rule reads the arm `{arm}`'s {control:?} on step `{}`, and that \
                             arm does not override it. Falling back to the step's own setting \
                             would account the sentence's number against a control the reader \
                             was never asked to touch — which is exactly what this arm \
                             exists to tell apart.",
                            lesson.id,
                        )
                    });
                    vec![value]
                }
                // The file half, and the whole of what an arm may answer for besides a
                // control. Only an arm that PICKS a file may be asked one: on any other the
                // original refusal still holds word for word, because the field would
                // resolve to the step's own number while wearing the arm's name.
                Tie::Chemistry(_) | Tie::Scenario(_) | Tie::Name { .. } => {
                    let Some(file) = found.scenario.as_deref() else {
                        panic!(
                            "a rule reads {} under the arm `{arm}` on step `{}`, and that arm \
                             picks no file. An arm overrides controls, not files: asking a \
                             scenario field under an arm that changed no scenario would \
                             resolve to the step's own number and claim it came from \
                             somewhere else.",
                            tie_describe(tie),
                            lesson.id,
                        )
                    };
                    let (picked_scenario, picked_chemistry) =
                        (scenario_toml(file), chemistry_toml(file));
                    tie_values(
                        tie,
                        lesson,
                        lessons,
                        &picked_scenario,
                        &picked_chemistry,
                        ctx,
                    )
                }
                other => panic!(
                    "a rule wraps `{}` in `OnArm`. An arm answers for exactly two things — a \
                     control it overrides, and the file it picks — and anything else under \
                     its name resolves against the STEP's lesson while reading as the arm's. \
                     That is the misattribution the original refusal was written against, and \
                     it survives the file half of this arm unchanged.",
                    tie_arm_name(other),
                ),
            }
        }
        Tie::Picker { arm, prefix } => {
            let found = ctx
                .arms
                .iter()
                .find(|a| a.step == lesson.id && a.name == *arm)
                .unwrap_or_else(|| {
                    panic!(
                        "a rule on step `{}` reads the file the arm `{arm}` picks, and that \
                         step declares no arm of that name.",
                        lesson.id,
                    )
                });
            let Some(file) = found.scenario.clone() else {
                panic!(
                    "a rule reads the digits after `{prefix}` in the file the arm `{arm}` \
                     picks on step `{}`, and that arm picks no file. A walk names a lesson \
                     and a pick names a file; only the second has a name for a sentence to \
                     spell.",
                    lesson.id,
                )
            };
            digits_after(&Some(file), prefix)
        }
        Tie::Magnitude(tie) => {
            let inner = tie_values(tie, lesson, lessons, scenario, chemistry, ctx);
            for v in &inner {
                assert!(
                    *v < 0.0,
                    "a rule reads the magnitude of {}, which resolves to {v} — not a \
                     negative number. On a value that is already positive this wrapper is \
                     `{}` with extra words, and it would go green for a reason the \
                     sentence does not state. Same fence, same wording, as the claim \
                     side's `states = \"magnitude\"`.",
                    tie_describe(tie),
                    tie_arm_name(tie),
                );
            }
            inner.into_iter().map(f64::abs).collect()
        }
        Tie::Elsewhere { step, tie } => {
            assert!(
                lesson.id != *step,
                "a rule on step `{}` reads `Elsewhere` about that same step. That is                  `{}` with extra words, and it would be green for a reason the sentence                  does not state.",
                lesson.id,
                tie_arm_name(tie),
            );
            assert!(
                !matches!(tie, Tie::Elsewhere { .. } | Tie::Derived { .. }),
                "a rule wraps `{}` in `Elsewhere`. Nesting has no floor, and a derivation                  reads THIS sentence's siblings — there is no such thing as the sibling of                  a token in another lesson.",
                tie_arm_name(tie),
            );
            let Some(other) = lessons.iter().find(|l| l.id == *step) else {
                return Vec::new();
            };
            let (scenario, chemistry) = (
                scenario_toml(&other.scenario),
                chemistry_toml(&other.scenario),
            );
            tie_values(tie, other, lessons, &scenario, &chemistry, ctx)
        }
        Tie::Quoted {
            step,
            arm,
            quantity,
            states,
        } => {
            let hits: Vec<&Claim> = ctx
                .all
                .iter()
                .filter(|c| c.step == *step && c.arm.as_deref() == *arm && c.quantity == *quantity)
                .collect();
            let [first, rest @ ..] = &hits[..] else {
                return Vec::new();
            };
            // Not "exactly one claim", which is what this used to demand and which step 12
            // cannot satisfy: it states its first rebound in two sentences, so
            // `pulse_rebound_mv:1` carries two claims — at one instant, with one value. The
            // hazard is a quantity two claims answer DIFFERENTLY (`v_at` on step 20 is ten
            // readings at ten instants), and that is what this refuses. Agreement is
            // checked rather than assumed, so two claims on one quantity that have drifted
            // apart fail here instead of being invisible. A step that wants to be quotable
            // tags its instants — `v_at:400` — which is what step 15 now does.
            //
            // The arm is part of the address now, so what is left here is genuine
            // ambiguity *within one trajectory* — two instants filed under one name. It no
            // longer fires on a quantity two RUNS of a step answer differently, which is
            // step 16's 464 against its own 3484 and is not a drift at all.
            assert!(
                rest.iter().all(|c| c.value == first.value),
                "a rule quotes step `{step}`'s `{quantity}` on {}, and the claims on it \
                 answer differently: {:?}. Which one the sentence means would be decided by \
                 file order rather than by the sentence. Quote a quantity that names one \
                 measurement — tag its instant, as `v_at:400` does — or, if these are meant \
                 to be the same reading, one of them has drifted.",
                arm.map_or("its own run".to_string(), |a| format!("the arm `{a}`")),
                hits.iter().map(|c| c.value).collect::<Vec<_>>(),
            );
            // The claim's own `spells_pow10` is deliberately NOT consulted, and the fence
            // that used to refuse a non-zero one is gone. It read: "the rule's `pow10`
            // would apply on top of it and the two scalings would multiply silently" — a
            // composition this arm rules out by construction, because what it resolves to
            // is `value`, which is in the engine's units. `spells_pow10` describes how the
            // OTHER step's prose renders that value, and no scan reads it here. The only
            // scaling is this rule's own, against this step's own sentence. What the fence
            // actually did was refuse step 13's `0.5 %`, whose source claim spells `99.5 %`.
            match states {
                QuotedAs::Same => vec![first.value],
                QuotedAs::Complement => vec![1.0 - first.value],
            }
        }
        Tie::Name { field, prefix } => digits_after(&string_at_path(chemistry, field), prefix),
        Tie::Label { id, prefix } => digits_after(&control_label(id), prefix),
        Tie::Count(path) => array_len(scenario, path)
            .map(|n| vec![n as f64])
            .unwrap_or_default(),
        Tie::Ocv(tie) => {
            let [soc] = tie_values(tie, lesson, lessons, scenario, chemistry, ctx)[..] else {
                return Vec::new();
            };
            ocv_at(chemistry, soc).map(|v| vec![v]).unwrap_or_default()
        }
        Tie::Ordinal(step) => lessons
            .iter()
            .position(|l| l.id == *step)
            .map(|i| vec![(i + 1) as f64])
            .unwrap_or_default(),
    }
}

/// The **label of one control** in `web/index.html`, or `None` if no element carries that
/// `id`.
///
/// From the `id` attribute to the next `<`, which on this page's markup is the element's own
/// text and nothing else. Deliberately not a parse of the HTML: a tie reads one anchored
/// span the way [`Tie::Name`]'s prefix does, and a document walker would find a number for
/// almost any token.
fn control_label(id: &str) -> Option<String> {
    let markup = index_html();
    let at = markup.find(&format!("id=\"{id}\""))?;
    let rest = &markup[at..];
    let open = rest.find('>')? + 1;
    let close = rest[open..].find('<')? + open;
    Some(rest[open..close].to_string())
}

/// How many entries the array at this dotted key path has, or `None` if the path names
/// something that is not an array.
///
/// `None` and never zero: "there is no such array" and "it is empty" are different facts
/// about the file, and only the second is a number a sentence could be printing.
fn array_len(value: &toml::Value, path: &str) -> Option<usize> {
    let mut here = value;
    for seg in path.split('.') {
        here = here.as_table()?.get(seg)?;
    }
    here.as_array().map(Vec::len)
}

/// The chemistry's open-circuit voltage at `soc` \[V\], per cell.
///
/// Linear interpolation over the `[ocv]` table with both ends clamped, which is what
/// `sim_core` does and what the table's own comment promises. `None` if the file has no
/// such table, or if its two arrays are not the same length — a shape this cannot average
/// over and must not guess at.
fn ocv_at(chemistry: &toml::Value, soc: f64) -> Option<f64> {
    let xs = numbers_at_path(chemistry, "ocv.soc.*");
    let ys = numbers_at_path(chemistry, "ocv.volts.*");
    if xs.len() < 2 || xs.len() != ys.len() {
        return None;
    }
    if soc <= xs[0] {
        return Some(ys[0]);
    }
    if soc >= xs[xs.len() - 1] {
        return Some(ys[ys.len() - 1]);
    }
    let i = xs.windows(2).position(|w| soc >= w[0] && soc <= w[1])?;
    let (x0, x1) = (xs[i], xs[i + 1]);
    let f = if (x1 - x0).abs() < f64::EPSILON {
        0.0
    } else {
        (soc - x0) / (x1 - x0)
    };
    Some(ys[i] + f * (ys[i + 1] - ys[i]))
}

/// One string field at a dotted key path, or `None` — the string twin of
/// [`numbers_at_path`], and deliberately not a wildcard walk: a name is one field.
fn string_at_path(value: &toml::Value, path: &str) -> Option<String> {
    let mut here = value;
    for seg in path.split('.') {
        here = here.as_table()?.get(seg)?;
    }
    here.as_str().map(str::to_string)
}

/// Every digit run in `text` that immediately follows `prefix`.
///
/// `("LG M50 21700 (NMC811/graphite)", "M")` is `[50.0]` — the `21700` follows a space and
/// the `811` follows `NMC`, so neither is reachable. That narrowing is the whole of
/// [`Tie::Name`]'s honesty; see its docs.
fn digits_after(text: &Option<String>, prefix: &str) -> Vec<f64> {
    let Some(text) = text else {
        return Vec::new();
    };
    text.match_indices(prefix)
        .filter_map(|(at, _)| {
            let rest = &text[at + prefix.len()..];
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            rest[..end].parse().ok()
        })
        .collect()
}

/// What a tie reads, in words, for the message an author gets when it reads nothing.
fn tie_describe(tie: &Tie) -> String {
    match tie {
        Tie::Scenario(path) => format!("the scenario's `{path}`"),
        Tie::Chemistry(path) => format!("the chemistry's `{path}`"),
        Tie::Setting(control) => format!("the lesson's {control:?} control"),
        Tie::Product(factors) => factors
            .iter()
            .map(tie_describe)
            .collect::<Vec<_>>()
            .join(" times "),
        Tie::Name { field, prefix } => {
            format!("the digits after `{prefix}` in the chemistry's `{field}`")
        }
        Tie::Ordinal(step) => format!("the position of the lesson `{step}` in the path"),
        Tie::Member(path) => format!("the nodes of the chemistry's `{path}`"),
        Tie::Ratio(pair) => pair
            .iter()
            .map(tie_describe)
            .collect::<Vec<_>>()
            .join(" divided by "),
        Tie::Difference(pair) => pair
            .iter()
            .map(tie_describe)
            .collect::<Vec<_>>()
            .join(" less "),
        Tie::Sum(pair) => pair
            .iter()
            .map(tie_describe)
            .collect::<Vec<_>>()
            .join(" plus "),
        Tie::Hours(seconds) => format!("{}, in hours", tie_describe(seconds)),
        Tie::Instant {
            step,
            arm,
            quantity,
        } => format!(
            "the instant step `{step}`'s `{quantity}` is read at on {}",
            arm.map_or("its own run".to_string(), |a| format!("the arm `{a}`"))
        ),
        Tie::PerSecond(ms) => format!("{}, per second", tie_describe(ms)),
        Tie::Span(path) => format!("the span of the chemistry's `{path}`"),
        Tie::Clock => "the `sim time` row's rendering of the step's mark".to_string(),
        Tie::Page(name) => format!("the page's `{name}` constant"),
        Tie::Label { id, prefix } => {
            format!("the digits after `{prefix}` in the label of the control `{id}`")
        }
        Tie::Count(path) => format!("how many entries the scenario's `{path}` has"),
        Tie::Ocv(tie) => format!(
            "the chemistry's open-circuit voltage at the charge {} gives",
            tie_describe(tie)
        ),
        Tie::Derived { op, operands } => format!(
            "this sentence's own {}",
            operands
                .iter()
                .map(|operand| match operand {
                    Operand::Sibling(t) => format!("`{t}`"),
                    Operand::Word(w) => format!("`{w}` (spelled in letters)"),
                })
                .collect::<Vec<_>>()
                .join(match op {
                    LedgerOp::Product => " times ",
                    LedgerOp::Difference => " less ",
                    LedgerOp::Ratio => " divided by ",
                })
        ),
        Tie::Magnitude(tie) => format!("the magnitude of {}", tie_describe(tie)),
        Tie::Elsewhere { step, tie } => {
            format!("{}, read on the lesson `{step}`", tie_describe(tie))
        }
        Tie::OnArm { arm, tie } => {
            format!("{}, as the arm `{arm}` sets it", tie_describe(tie))
        }
        Tie::Picker { arm, prefix } => {
            format!("the digits after `{prefix}` in the name of the file the arm `{arm}` picks")
        }
        Tie::Quoted {
            step,
            arm,
            quantity,
            states,
        } => {
            let whose = arm.map_or_else(
                || format!("the lesson `{step}`'s own run"),
                |a| format!("the lesson `{step}`'s arm `{a}`"),
            );
            match states {
                QuotedAs::Same => format!("{whose}'s claim on `{quantity}`"),
                QuotedAs::Complement => format!("one less {whose}'s claim on `{quantity}`"),
            }
        }
    }
}

/// The name the ledger's own prose gives this kind of tie.
///
/// Exhaustive on purpose, the same contract [`Accounted::arm_name`] keeps: a new variant
/// does not compile until it is named here, so the arm count both files state about
/// themselves ([`n_ledger_arms`]) cannot go stale by omission. Three sentences claiming an
/// arm was missing after it had been built is a defect this file has already shipped once.
fn tie_arm_name(tie: &Tie) -> &'static str {
    match tie {
        Tie::Scenario(_) => "scenario field",
        Tie::Chemistry(_) => "chemistry field",
        Tie::Setting(_) => "control",
        Tie::Product(_) => "product",
        Tie::Name { .. } => "name",
        Tie::Ordinal(_) => "ordinal",
        Tie::Member(_) => "table node",
        Tie::Ratio(_) => "ratio",
        Tie::Difference(_) => "difference",
        Tie::Sum(_) => "sum",
        Tie::Hours(_) => "duration in hours",
        Tie::Span(_) => "table span",
        Tie::Clock => "clock",
        Tie::Page(_) => "page constant",
        Tie::OnArm { .. } => "an arm's control or picked file",
        Tie::Picker { .. } => "picked file name",
        Tie::Derived { .. } => "derived",
        Tie::Elsewhere { .. } => "another lesson",
        Tie::Quoted { .. } => "quoted claim",
        Tie::Instant { .. } => "a claim's instant",
        Tie::PerSecond(_) => "rate per second",
        Tie::Label { .. } => "control label",
        Tie::Count(_) => "array length",
        Tie::Ocv(_) => "open-circuit voltage at a charge",
        Tie::Magnitude(_) => "magnitude",
    }
}

/// Does what this tie reads agree with the number the prose printed?
///
/// Three comparisons, and which one is used is a property of the tie rather than a per-rule
/// choice. A constant is compared exactly — the prose either prints the file's number or is
/// wrong about it. A **computed** tie is compared at the prose's own precision, because the
/// sentence is doing arithmetic and no author would print `4.606902` — that is
/// [`Tie::Product`], [`Tie::Ratio`], [`Tie::Sum`], [`Tie::Span`] and [`Tie::Derived`], each of which lands
/// on a number binary floating point does not spell round (`20.000000000000004`,
/// `0.17999999999999994`). A [`Tie::Member`] asks whether *any* value is the number, because
/// "a node of that table" is what its sentence says; every other tie requires all of them.
fn tie_agrees(tie: &Tie, values: &[f64], token: &str, pow10: i32, unit: f64) -> bool {
    let written = token.replace(' ', "");
    let scale = 10f64.powi(pow10) / unit;
    match tie {
        // A wrapper compares the way the thing it wraps compares: `Elsewhere` changes
        // WHICH lesson answers and `Magnitude` drops a sign the sentence carries in its
        // own characters, never how exactly the answer has to match.
        Tie::Elsewhere { tie, .. } | Tie::Magnitude(tie) => {
            tie_agrees(tie, values, token, pow10, unit)
        }
        Tie::Product(_)
        | Tie::Ratio(_)
        | Tie::Difference(_)
        | Tie::Sum(_)
        | Tie::Hours(_)
        | Tie::PerSecond(_)
        | Tie::Span(_)
        | Tie::Derived { .. }
        | Tie::Ocv(_)
        | Tie::Quoted { .. } => values.iter().all(|v| {
            let places = decimals_of(&written).max(0) as usize;
            to_fixed(v * scale, places) == written
        }),
        Tie::Member(_) => match number_of(token) {
            Some(spelled) => values.iter().any(|v| tol_eq(v * scale, spelled)),
            None => false,
        },
        _ => {
            let Some(spelled) = number_of(token) else {
                return false;
            };
            values.iter().all(|v| tol_eq(v * scale, spelled))
        }
    }
}

/// Where a rule's phrase matches, as indices into `numbers` — one per `{n}`, in order.
///
/// A phrase matches only if its literal parts sit *immediately* around the numbers: the
/// prefix ends where the first number starts, and the next part begins where that number's
/// characters end. That is what makes the arm about this sentence rather than about the
/// step — "20 mA high" a paragraph away from "current sensor reads" is not the same claim.
///
/// A phrase may begin with its number (`"{n} in series"`), and that is not the loophole it
/// looks like: an empty prefix matches at every position, and the number still has to start
/// exactly there and be followed by the words. What is refused is a placeholder with
/// nothing on either side — see [`every_ledger_rule_is_a_phrase_and_is_used`].
fn rule_matches(text: &str, numbers: &[Written], phrase: &str) -> Vec<Vec<usize>> {
    let parts: Vec<&str> = phrase.split("{n}").collect();
    let placeholders = parts.len() - 1;
    let mut out = Vec::new();
    for (start, _) in text.match_indices(parts[0]) {
        let mut pos = start + parts[0].len();
        let mut hit = Vec::new();
        for part in &parts[1..] {
            let Some(i) = numbers.iter().position(|w| w.at == pos) else {
                break;
            };
            pos = numbers[i].at + numbers[i].len;
            if !text[pos..].starts_with(part) {
                break;
            }
            pos += part.len();
            hit.push(i);
        }
        if hit.len() == placeholders && placeholders > 0 {
            out.push(hit);
        }
    }
    out
}

/// Which rule covers each number in `text` — `(rule, which of its placeholders)`.
///
/// Two different rules covering one number is refused rather than resolved. It is the same
/// hazard the [`Accounted::ReadAt`] fence exists for: given two readings an author keeps
/// whichever one passes, and then the check is about the author rather than the prose.
fn cover_by_rule(text: &str, numbers: &[Written], step: &str) -> Vec<Option<(usize, usize)>> {
    let mut out: Vec<Option<(usize, usize)>> = vec![None; numbers.len()];
    for (r, rule) in LEDGER_VOCABULARY.iter().enumerate() {
        for hit in rule_matches(text, numbers, rule.phrase) {
            for (p, &i) in hit.iter().enumerate() {
                if let Some((r0, p0)) = out[i] {
                    assert!(
                        r0 == r && p0 == p,
                        "step `{step}`: the number `{}` is covered by two vocabulary \
                         rules — `{}` and `{}`. Two readings of one number means the \
                         check is about which rule an author tried first. Make the \
                         phrases specific enough to disagree.",
                        numbers[i].token,
                        LEDGER_VOCABULARY[r0].phrase,
                        rule.phrase,
                    );
                }
                out[i] = Some((r, p));
            }
        }
    }
    out
}

/// The `claimed` arm — how a claim on this step accounts for a number *inside its own
/// sentence*, if one does.
///
/// This is check 6's [`accounting_for`] reused whole rather than a second reading of the
/// same claims: the ledger asks the question about a number found by scanning the step's
/// prose, and check 6 asks it about a number found by scanning a literal, and those are the
/// same question when the number lies inside the literal. Sharing the function is what
/// stops the two answering differently while both stay green — the defect this whole file
/// is arranged around.
///
/// **Positional, not step-wide, and that is the whole strength of it.** "Some claim on this
/// step spells 14" would account for the `14` in *any* sentence of the step, including one
/// no claim is about — which is exactly the prose the ledger exists to reach. So the
/// number has to sit inside the literal of the sentence whose claims account for it. The
/// cost is visible in this slice: step 6 states its clamped current in two sentences, and
/// closing the second one meant claiming it, not waiving it.
fn claimed_accounting(
    number: &Written,
    text: &str,
    step: &str,
    all: &[Claim],
    lesson: &Lesson,
    arms: &[Arm],
    derived: &[Derivation],
) -> Option<&'static str> {
    for (owner, literal) in sentences(all) {
        if owner != step {
            continue;
        }
        let quoted = ascii_minus(literal);
        for (at, _) in text.match_indices(&quoted) {
            let inside = number.at >= at && number.at + number.len <= at + quoted.len();
            if !inside {
                continue;
            }
            let group = sentence_group(all, step, literal);
            if let Some(accounted) =
                accounting_for(&Reading::from(number), &group, lesson, arms, derived)
            {
                return Some(accounted.arm_name());
            }
        }
    }
    None
}

/// The ledger — every numeral in a ledgered step's whole prose is tied to something.
///
/// Check 6 requires an accounting for every number inside a sentence some claim already
/// quotes, and that is as far as it reaches: a step with no claims has no literal to scan.
/// Fourteen of the twenty-four steps were in that position, and the two defects the last
/// slice found there — six figures gone stale under a change to how aging grows the RC
/// resistances, and a 600-second contrast between two models that has never existed — were
/// both introduced by slices that ran a fully green suite. Nothing could have reddened:
/// there was no claim to redden.
///
/// So this scans a step's prose end to end. The first three steps ledgered were chosen
/// because every *numeral* in them is a scenario constant:
/// `docs/plans/path-prose-ledger.md` measured all fourteen steps and found those three carry
/// no measurement-shaped figure at all, so they could be closed before a single number was
/// measured. The fourth, `protection-on`, is the first that could not be — it prints its
/// pack, its cell's rating, its demand box and the current the BMS clamps it to — and it is
/// what [`Tie::Chemistry`], [`Tie::Setting`], [`Tie::Product`] and [`claimed_accounting`]
/// were built for. The sixth, `slow-and-patient`, is what closed the taxonomy: its twelve
/// unclaimed numerals are all constants and not one of them is a field a rule could read
/// straight off a file, which took [`Tie::Derived`] — the figure worked out from its
/// siblings, the last arm the plan named — along with [`Tie::Ratio`], [`Tie::Span`] and
/// [`Tie::Clock`]. Check 6's arm of that name is a different scan and closes nothing here.
/// See `docs/plans/path-ledger-sixth-step.md`.
///
/// The tenth, `the-electrolyte-starves`, is the densest step in the path and the one whose
/// numbers are least its own: nine of them are readings the step next door measured, or
/// arithmetic over those, and four more are its own readings claimed in some other sentence
/// of the same step. So it is [`Tie::Quoted`]'s step — including quoting *itself*, which no
/// earlier step needed — plus [`Tie::Difference`] for the three sentences that print what
/// two measurements come to and neither of the measurements, and [`Tie::Hours`] for the one
/// amp-hour figure. **Five of its numbers left the page instead**: two per-step cost ratios,
/// which no trajectory settles, and a three-number comparison at 1 C, which needs a second
/// pack this harness cannot build. Spelling them in words would have cleared this scan and
/// checked nothing, which is the one option that was refused.
/// See `docs/plans/path-ledger-dfn-step.md`.
///
/// The thirteenth, `nothing-to-clamp`, is the densest step left and was the cheapest of
/// them to scan, which is the opposite of how the ranking read: twenty-seven of its
/// thirty-four numerals sit inside a claimed sentence, so the scan asked for seven. Six
/// were rules and the seventh was [`Tie::Sum`] twice over — a trip point neither file holds
/// on its own (`cell.t_max_k` plus `pack.bms.protection.t_hard_margin_k`) and the twin's
/// temperature, its ambient plus the rise its own claim pins. **Two of its numbers moved
/// instead of being tied, and both were prose defects the scan found rather than gaps in
/// this taxonomy**: "73 seconds of no flags" against a subtraction of 73.5, which a
/// computed tie reads as 74; and a twin that "peaks" at a figure measured at its mark, one
/// step after the peak, where nothing at one decimal could tell the two apart. A third left
/// the page on the DFN step's precedent — an instruction to run "to about 400 s", which no
/// file decides and which a tie reading a claim's own `read_at_s` would have declared from
/// both sides. See `docs/plans/path-ledger-weaker-short.md`.
///
/// **The twenty-fourth, `the-gradient-itself`, is the last step in the path**, and it is
/// the one whose numbers are most nearly all its own: twenty-nine of its forty-four sit
/// inside a claimed sentence, and the step carries twenty-nine claims of which twenty-five
/// were written for this scan — the most heavily claimed step in the path, all on one
/// trajectory. It needed no new
/// tie, no new arm and no new accounting arm; what it needed was a **quantity**,
/// `gap_neg_zero_s`, because its central sentence is about the instant a readout row first
/// prints `0.00` and a claim read at an instant an author picked would not have said
/// "first". Its one genuinely new shape on this side is a [`Tie::Ratio`] whose two operands
/// are themselves [`Tie::Difference`]es — the fraction of a rebound that is over, built from
/// three of this step's own claims and one of step 15's, and printed by a sentence that
/// shows none of the three voltages it is a ratio of.
///
/// **Three numbers moved, and two of them were added rather than corrected.** `1040 s` is
/// 5.86e-6 squared over 3.3e-14, which is 1040.594 and had been truncated where a computed
/// tie rounds; it is 1041 now. The other two are instants the prose did not print: 1396 s,
/// so that "the negative gap first reads 0.00" is a crossing rather than a row; and 518 s,
/// replacing an "about halfway through it" whose two readings disagreed — true of the
/// discharge (half of 1060) and false of the stretch the same clause had just named (half of
/// 360 → 1060 is 710). See `docs/plans/path-ledger-the-gradient.md`.
///
/// **Numeral is the operative word, and it is a real limit rather than pedantry.**
/// [`written_numbers`] finds digits. A quantity spelled in English is invisible to it, and
/// two of the ledgered steps state four of them: "about half a point across the whole
/// grid" and "a quarter of a point" between the cells of one group in step 3, "a gap of about
/// three points" and "a fraction of a point" of sensor drift in step 4. Every one is an
/// engine measurement, and they are the sentences a reader leans on. A green ledger here says
/// the step's *digits* are tied to something — it does not say the step is checked.
///
/// One of those four now has claims on it — the estimator gap, checked at the mark and at
/// its narrowest — but they are *claims*, and this scan is still blind to the sentence
/// they are about. The blindness is what keeps the two things separate: a green ledger on
/// `belief-drifts` says its three digits are scenario constants, and the two claims say
/// what the gap is. Neither says the other. A scanner that saw word numerals would have to
/// see "half a point" and "a quarter of a point" too, which are step 3's per-cell spreads
/// and a measurement this file has not made — so the vocabulary, the `claimed` arm and
/// those two spread claims are one future slice and not three.
///
/// **A ledgered step may carry claims, and one of its numerals may now be decided by one.**
/// This test used to refuse the combination outright, on the grounds that a number a claim
/// ties to the engine has no accounting here. That fence came down for `belief-drifts`,
/// whose two claims are on a quantity the sentence spells in *letters* ("about three
/// points") — invisible to a digit scan, so the step's three digits stayed the scenario
/// constants they always were and the fence was never really tested.
///
/// `protection-on` is what tests it: its clamped current is a numeral, in two sentences, and
/// only a claim decides it. [`claimed_accounting`] is check 6's [`accounting_for`] asked
/// about a number this scan found rather than one a literal scan found — the same question,
/// so the same function, which is what stops the two answering differently while both stay
/// green. It is **positional**: the number must sit inside the literal of the sentence whose
/// claims account for it, or "some claim on this step spells 14" would cover the `14` in a
/// sentence no claim is about — which is the prose this whole scan exists to reach.
/// See `docs/plans/path-ledger-fourth-step.md`.
#[test]
fn every_numeral_in_a_ledgered_step_is_accounted_for() {
    let lessons = lessons();
    let ledger = ledger();
    let all = claims();
    let arms = arms();
    let derived = derivations();

    for step in &ledger.steps {
        let lesson = lessons
            .iter()
            .find(|l| l.id == *step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));

        let text = ascii_minus(&lesson.text);
        // Digits, and - where the step is word-scanned - the quantities its prose spells
        // in English. Merged into one vector rather than scanned separately, and that is
        // not tidiness: `cover_by_rule` refuses two readings of one number, and two passes
        // each with their own cover could not see each other's. Safe to merge because
        // `rule_matches` locates a rule's `{n}` by **byte offset** and not by index, so
        // inserting a word between two digits cannot shift a rule onto the wrong number.
        let numbers = ledger_numbers(&text, step, &ledger);
        assert!(
            !numbers.is_empty(),
            "step `{step}` is ledgered and prints no number at all. That is not a \
             failure of the prose, but it is not coverage either — drop it from `steps` \
             and say so in `unledgered`."
        );
        let scenario = scenario_toml(&lesson.scenario);
        let chemistry = chemistry_toml(&lesson.scenario);
        let cover = cover_by_rule(&text, &numbers, step);

        for (index, (w, covered)) in numbers.iter().zip(&cover).enumerate() {
            // The sentence around the number, which is what an author has to look at:
            // the ledger's failures are all "this figure and this file disagree", and the
            // figure on its own does not say where in the step to look.
            let context = {
                let a = text[..w.at]
                    .char_indices()
                    .rev()
                    .nth(45)
                    .map_or(0, |(i, _)| i);
                let after = w.at + w.len;
                let b = text[after..]
                    .char_indices()
                    .nth(45)
                    .map_or(text.len(), |(i, _)| after + i);
                text[a..b].split_whitespace().collect::<Vec<_>>().join(" ")
            };
            let claimed = claimed_accounting(w, &text, step, &all, lesson, &arms, &derived);
            if let (Some((r, _)), Some(arm)) = (*covered, claimed) {
                panic!(
                    "step `{step}`: the number `{}` is accounted for twice — by the \
                     vocabulary rule `{}`, and as `{arm}` by a claim whose literal \
                     contains it:\n  …{context}…\n\
                     Two readings of one number means the accounting is decided by \
                     which arm this check tried first rather than by the sentence. It \
                     is the same hazard `cover_by_rule`'s double-cover panic and check \
                     6's `clash` both exist for. Narrow the rule's phrase, or shorten \
                     the claim's literal so it stops before the number the rule is \
                     about.",
                    w.phrase, LEDGER_VOCABULARY[r].phrase,
                );
            }
            if claimed.is_some() {
                continue;
            }
            let Some((r, p)) = *covered else {
                panic!(
                    "step `{step}` prints `{}` and nothing accounts for it:\n  \
                     …{context}…\n\
                     No vocabulary rule spells that position and no claim's literal \
                     covers it, so this file cannot say what decides the number. If a \
                     scenario or chemistry field does, add a rule to \
                     `LEDGER_VOCABULARY` naming the field; if a control on the lesson \
                     block does, add one tied to a `Control`. If the engine does, it \
                     needs a claim in web/path-claims.toml quoting the sentence it is \
                     printed in. If it is a figure this sentence works out from its \
                     own siblings, `Tie::Derived` accounts for it — name the operands, \
                     never the value, and every operand must already be accounted for by \
                     something that is not itself a derivation. There is no waiver.",
                    w.phrase,
                );
            };
            let rule = &LEDGER_VOCABULARY[r];
            let tie = &rule.ties[p];
            let ctx = SentenceCtx {
                step,
                text: &text,
                numbers: &numbers,
                cover: &cover,
                at: index,
                all: &all,
                arms: &arms,
                derived: &derived,
            };
            let found = tie_values(tie, lesson, &lessons, &scenario, &chemistry, &ctx);
            assert!(
                !found.is_empty(),
                "step `{step}`: the rule `{}` reads {}, and that resolves to no number \
                 for this step. The file it names was restructured — or, on a product, \
                 one factor reached several values and none of them can be the one the \
                 sentence means. The rule has to follow the file.",
                rule.phrase,
                tie_describe(tie),
            );
            assert!(
                tie_agrees(tie, &found, &w.token, rule.pow10, w.scale),
                "step `{step}` says `{}` where {} says {found:?}:\n  …{context}…\n\
                 The prose and the file have parted. One of them moved; the sentence is \
                 what a reader is shown, so fix whichever is wrong rather than the \
                 rule.\n\
                 Note that a `*` in a path is read strictly — EVERY value it reaches \
                 has to be this number, which is what lets `faults.*.at_s` carry \
                 \"in the same instant\". If the sentence really is about one of \
                 several, it needs a path that says which. A product is the one tie \
                 compared at the prose's own precision, so this fires there only when \
                 the sentence's own arithmetic no longer rounds to what it prints.",
                w.phrase,
                tie_describe(tie),
            );
        }
    }
}

/// A spelled quantity reaches the duration arms only when its unit **is** a duration.
///
/// Asked directly, for [`an_hours_tie_rounds_the_way_a_computed_tie_does`]'s reason: no
/// sentence in the path reaches this fence any more, and a fence nothing reaches is pinned
/// and consulted by nothing, which is the shape this file rejects everywhere else.
///
/// **It was reachable, and that is how it was found.** Adding `percent` to [`UNIT_NOUNS`]
/// made *"against the twin's half a percent"* visible on `nothing-to-clamp`, whose step
/// length is exactly 0.5 s. The two arms of [`accounting_without_arithmetic`] that compare
/// *numbers* are both in seconds and are offered every spelled quantity, so check 6 tied a
/// percentage to a duration and went **green**. The sentence has since moved — the twin lost
/// 0.55719 points, which is not half a percent — and with it went the only data that
/// exercises this. So the question is put here instead, with both sides handed in directly.
///
/// The pair is the test: the same token, the same claims, the same lesson, and the *only*
/// difference is the noun the words were read off. One resolves to the step length and the
/// other to nothing. Widen [`unit_is_time`] to admit a percent and the second assert fails.
#[test]
fn a_spelled_quantity_reaches_the_duration_arms_only_when_it_is_one() {
    let lessons = lessons();
    let all = claims();
    let arms = arms();
    let step = "nothing-to-clamp";
    let lesson = lessons
        .iter()
        .find(|l| l.id == step)
        .expect("nothing-to-clamp is a lesson");
    assert!(
        (lesson.dt - 0.5).abs() < 1e-12,
        "this test rests on `{step}` holding its step length at 0.5 s, and it holds {}. \
         The collision it reproduces is between that number and the token 0.5, so a step \
         length anywhere else makes both asserts below pass for the wrong reason.",
        lesson.dt,
    );
    let group = sentence_group(&all, step, "this fault costs **50 points**");
    assert!(
        !group.is_empty(),
        "the sentence this test reads has no claims on it any more; it needs a group whose \
         trajectory has a step length, or it is asking about nothing."
    );

    let seconds = Reading {
        token: "0.5",
        scale: 1.0,
        unit: "seconds",
        in_words: true,
    };
    let as_seconds = accounting_without_arithmetic(&seconds, &group, lesson, &arms);
    assert!(
        matches!(as_seconds, Some(Accounted::Setting(_))),
        "half a SECOND is the step length of this trajectory, and the arm that reads a \
         control did not find it: {as_seconds:?}. Without this half the test below would \
         pass on an arm that never fires for any unit."
    );

    let percent = Reading {
        unit: "percent",
        ..seconds
    };
    let as_percent = accounting_without_arithmetic(&percent, &group, lesson, &arms);
    assert!(
        as_percent.is_none(),
        "half a PERCENT was accounted as {as_percent:?} — the step length, which is 0.5 \
         SECONDS. The two arms that compare numbers are in seconds; a quantity the prose \
         writes in some other unit is not a candidate for them, whatever it happens to \
         equal. See `Written::unit`."
    );
}

/// The scanner's unit noun reaches [`Reading`], which is the wire the gate hangs off.
///
/// [`a_spelled_quantity_reaches_the_duration_arms_only_when_it_is_one`] asks the *predicate*
/// the right question and hand-builds its [`Reading`] to do it, so it never crosses this
/// conversion. Nothing else did either: the scanner's own test compares tokens and scales,
/// and the ledger's sentences are all repaired. So `From<&Written>` could have passed `""`,
/// which [`unit_is_time`] reads as "the sentence wrote digits" and admits, and the gate would
/// have been dead with every test in this file green.
///
/// The whole point of the field is that it survives the trip, so the trip is what is asked
/// about here.
#[test]
fn a_spelled_quantitys_unit_survives_the_trip_into_a_reading() {
    let found = spelled_numbers(
        "against the twin's half a percent, and 40.33 A four and a half seconds later",
    );
    let units: Vec<&str> = found.iter().map(|w| Reading::from(w).unit).collect();
    assert_eq!(
        units,
        vec!["percent", "seconds"],
        "the noun the scanner read did not arrive in the `Reading` check 6 asks about: {found:?}"
    );
    assert!(
        found.iter().all(|w| Reading::from(w).in_words),
        "a quantity spelled in words reached a `Reading` that says it was written in \
         digits, which hands it the arms that compare characters."
    );
    // ...and the two halves mean opposite things to the gate, which is the whole reason the
    // noun is carried rather than the scale: both of these scale by one.
    assert!(!unit_is_time(Reading::from(&found[0]).unit));
    assert!(unit_is_time(Reading::from(&found[1]).unit));
}

/// [`Tie::Hours`] rounds like a computed tie, asked directly because no rule can ask it.
///
/// The conversion's only user is a factor of a [`Tie::Product`], and [`tie_agrees`] sees a
/// rule's outermost tie — so the product decides that sentence and this variant's own arm is
/// never entered. Measured rather than reasoned: lifting `Tie::Hours` out of the rounding
/// group in `tie_agrees` leaves all 28 tests green, which is exactly the "pinned and
/// consulted by nothing" shape this file rejects everywhere else.
///
/// So the question is put here, with the comparison's two sides handed in directly. The
/// numbers are step 16's: 464 s of 15.459594 A is 1.9925... A·h, and the sentence prints
/// `1.99`. The second half is what makes it a test rather than a restatement — a constant tie
/// with the identical numbers must REFUSE, because a constant is compared exactly. Swap the
/// variants and one of the two asserts fails whichever way round the arms are written.
#[test]
fn an_hours_tie_rounds_the_way_a_computed_tie_does() {
    let hours = Tie::Hours(&Tie::Clock);
    let exact = Tie::Chemistry("cell.capacity_ah");
    let amp_hours = 15.459594 * 464.0 / 3600.0;
    assert!(
        tie_agrees(&hours, &[amp_hours], "1.99", 0, 1.0),
        "an hours tie is a computed quantity and has to be compared at the prose's own \
         precision: {amp_hours} does not spell round, and no sentence would print it whole."
    );
    assert!(
        !tie_agrees(&exact, &[amp_hours], "1.99", 0, 1.0),
        "a constant tie must refuse the same pair, or the distinction this test is about \
         does not exist — and the `15.46` that could not be tied to the demand box would \
         have had an arm all along."
    );
}

/// [`Tie::Ocv`] rounds like a computed tie, asked directly for [`Tie::Hours`]'s reason.
///
/// Its one user sits two levels inside a [`Tie::Product`] — four cells in series times the
/// difference of the curve at two charges — and [`tie_agrees`] is asked about a rule's
/// *outermost* tie, so the product decides step 18's `0.4 mV` and this variant's own arm is
/// never entered. Measured rather than reasoned, exactly as its neighbour was: lifting
/// `Tie::Ocv` out of the rounding group leaves the whole suite green.
///
/// The numbers are step 18's own. LFP's `[ocv]` table has nodes at 85 % and 90 % charge, so
/// the curve at the 89.4428 % the tooth leaves behind is an interpolation and lands on
/// 3.31408856 — a figure with no round spelling, which is the whole reason an interpolated
/// value cannot be compared exactly. The second half is what makes this a test rather than a
/// restatement: a constant tie handed the identical pair must REFUSE.
#[test]
fn an_ocv_tie_rounds_the_way_a_computed_tie_does() {
    let curve = Tie::Ocv(&Tie::Scenario("pack.initial_soc"));
    let exact = Tie::Chemistry("cell.v_min");
    let between_nodes = 3.314_088_56;
    assert!(
        tie_agrees(&curve, &[between_nodes], "3.3141", 0, 1.0),
        "an open-circuit voltage read between two nodes is a computed quantity and has to          be compared at the prose's own precision: {between_nodes} does not spell round,          and no sentence would print it whole."
    );
    assert!(
        !tie_agrees(&exact, &[between_nodes], "3.3141", 0, 1.0),
        "a constant tie must refuse the same pair, or the distinction this test is about          does not exist."
    );
}

/// The fence [`Tie::Derived`] rests on: an operand nothing else accounts for is refused.
///
/// Hand-built rather than perturbed, and the reason is worth writing down. The scan visits a
/// step's numbers in **text order**, so step 22's `2 V` is reached before the `12 V` derived
/// from it: every edit that un-accounts the operand — deleting its rule, breaking its phrase
/// — reddens on the operand's own line, one number sooner, and never reaches this assert.
/// It is a confounded perturbation in the sense `docs/plans/path-derived-arm.md` names, so
/// the fence is exercised directly instead: the step's real prose, and an empty cover map
/// standing in for "nothing accounts for it".
///
/// Without this assert the arm would say only that two unaccounted numbers multiply into a
/// third, which is the declared identity the ledger's design refuses.
#[test]
#[should_panic(expected = "accounts for that number")]
fn a_derivation_refuses_an_operand_nothing_else_accounts_for() {
    let lessons = lessons();
    let lesson = lessons
        .iter()
        .find(|l| l.id == "slow-and-patient")
        .expect("step 22 is still in the path");
    let text = ascii_minus(&lesson.text);
    let numbers = written_numbers(&text);
    let at = numbers
        .iter()
        .position(|w| w.token == "12")
        .expect("step 22 still prints the 12 V battery");
    let cover = vec![None; numbers.len()];
    let ctx = SentenceCtx {
        step: lesson.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at,
        all: &[],
        arms: &[],
        derived: &[],
    };
    operand_value(&Operand::Sibling("2"), &ctx, lesson);
}

/// [`Tie::Elsewhere`] reads the **named** lesson's own files, and its two users cannot say so.
///
/// Both of them — step 23's `0.36` and its `19.3h` — name step 22, which runs step 23's own
/// scenario file on step 23's own chemistry. So an implementation that quietly resolved
/// against *this* step's files would pass both sentences and the whole suite with them: the
/// arm's one substantive property would be untested by construction. That is the shape
/// `docs/plans/path-derived-arm.md` calls a confounded perturbation, and the answer is the
/// same one — exercise the property directly.
///
/// So this points the wrapper at `bare-curve`, an LFP step on `cc_discharge_lfp.toml`, and
/// asserts it comes back with **that** lesson's numbers rather than the lead-acid ones it is
/// invoked from.
///
/// **Two ties, because the wrapper hands its inner tie three things and the two halves are
/// separately blind.** Measured rather than assumed: neutering the *lesson* argument reddens
/// the ledger by itself, because step 22's clock and demand box are not step 23's however
/// much file they share — so that half was never blind and the sentence this file first
/// wrote about it was wrong. What the two users genuinely cannot reach is the **scenario and
/// chemistry** pair, which for both of them is the same file they are invoked from. So the
/// second tie reads a chemistry field: LFP's 2.303451 Ah against the lead-acid 7.2, off a
/// wrapper called from a lead-acid step.
#[test]
fn an_elsewhere_reads_the_named_lessons_own_files() {
    let lessons = lessons();
    let from = lessons
        .iter()
        .find(|l| l.id == "sixty-times-the-current")
        .expect("step 23 is still in the path");
    let named = lessons
        .iter()
        .find(|l| l.id == "bare-curve")
        .expect("step 1 is still in the path");
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &[],
        derived: &[],
    };
    // Resolved from step 23, whose own box is 21.6 A, and asked about step 1, whose is 2.
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );

    // Half one: the lesson block.
    let tie = Tie::Elsewhere {
        step: "bare-curve",
        tie: &Tie::Setting(Control::DemandValue),
    };
    let got = tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
    let want = control_value(Control::DemandValue, named).expect("step 1 has a demand box");
    assert_eq!(
        got,
        vec![want],
        "`Tie::Elsewhere` pointed at `bare-curve` resolved to {got:?}, and that lesson's          demand box is {want}. It read the wrong lesson — most likely the one the rule is          written on, which is step 23 at {:?}.",
        control_value(Control::DemandValue, from),
    );
    assert_ne!(
        control_value(Control::DemandValue, from),
        Some(want),
        "this test is only evidence while the two lessons' demand boxes DIFFER. They now          agree, so an implementation reading either one would pass — repoint it at a step          whose box is different."
    );

    // Half two: the chemistry the named lesson's scenario names, which is the half no
    // sentence in the path can reach.
    let tie = Tie::Elsewhere {
        step: "bare-curve",
        tie: &Tie::Chemistry("cell.capacity_ah"),
    };
    let got = tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
    let want = numbers_at_path(&chemistry_toml(&named.scenario), "cell.capacity_ah");
    let mine = numbers_at_path(&chemistry, "cell.capacity_ah");
    assert_eq!(
        got, want,
        "`Tie::Elsewhere` pointed at `bare-curve` read a capacity of {got:?}. That          lesson's chemistry says {want:?} and the one this wrapper was invoked from says          {mine:?} — so it resolved the inner tie against the CALLING step's files."
    );
    assert_ne!(
        want, mine,
        "this test is only evidence while the two chemistries' capacities DIFFER."
    );
}

/// A quotation of a quantity whose claims **disagree** is refused rather than resolved.
///
/// [`Tie::Quoted`] used to demand exactly one claim per `(step, quantity)`, which step 12
/// cannot satisfy — it states its first rebound in two sentences, one value, one instant —
/// so the fence now checks agreement instead of arity. The hazard it still has to refuse is
/// the one the old wording named: a step that files several readings under one name, where
/// "that step's `v_at`" would be decided by whichever the file happens to list first.
///
/// **It used to point at step 15, and that is why the instant tag exists.** Step 15 filed
/// eight voltages under `v_at`, which made it unquotable — step 16 is written almost
/// entirely against it and could borrow nothing. Those eight now name their instants
/// (`v_at:400`), so the case moved to step 20, which files ten under one name and has no
/// sentence quoting it yet.
///
/// No claim is synthesised for this. The pair is real, which is what makes it evidence: if
/// some future slice tags step 20's readings too, this test stops having a case and says so
/// through the assertion below rather than passing on nothing.
#[test]
#[should_panic(expected = "answer differently")]
fn a_quotation_of_a_quantity_two_claims_disagree_on_is_refused() {
    let lessons = lessons();
    let all = claims();
    let from = lessons
        .iter()
        .find(|l| l.id == "particle-remembers")
        .expect("step 13 is still in the path");
    let values: Vec<f64> = all
        .iter()
        .filter(|c| c.step == "past-empty" && c.quantity == "v_at" && c.arm.is_none())
        .map(|c| c.value)
        .collect();
    assert!(
        values.len() > 1 && values.iter().any(|v| *v != values[0]),
        // This message must NOT contain the phrase `should_panic` expects. It used to, and
        // a perturbation caught it: repointing the filter at a step whose readings are
        // tagged empties `values`, the guard fires — and the guard's own words satisfied
        // the `expected =` match, so the test passed while proving nothing. A guard on a
        // `should_panic` test has to fail in a way the attribute cannot mistake for the
        // panic it is waiting for.
        "this test needs a quantity that two claims on one step give unequal values for, \
         and step 20's `v_at` is no longer one: {values:?}. Point it at another, or the \
         refusal below would be evidence about nothing."
    );
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &all,
        arms: &[],
        derived: &[],
    };
    let tie = Tie::Quoted {
        step: "past-empty",
        arm: None,
        quantity: "v_at",
        states: QuotedAs::Same,
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// A tagged reading **is** quotable, which is the whole point of tagging one.
///
/// The mirror of the test above, and the reason this slice exists: step 15 filed eight
/// voltages under `v_at` and could not be quoted at all, so step 16 — which is written
/// almost entirely against it — could borrow nothing. Each now names its instant, and this
/// asserts that naming it resolves to that reading and not to some other.
///
/// Two things make it evidence rather than decoration. It picks an instant in the *middle*
/// of the run, so a resolution that quietly took the first or the last claim would fail it.
/// And it checks the tag against the claim's own `read_at_s`, so the pair cannot drift into
/// agreeing about a number while disagreeing about when it was read.
///
/// No rule quotes step 15 yet — `Tie::Quoted` reaches it only from a ledgered step's
/// vocabulary, and step 16 is not ledgered. This test is what stands in for that user until
/// it is, which is the honest version of shipping a capability ahead of its sentence.
#[test]
fn a_tagged_reading_resolves_to_that_reading() {
    let lessons = lessons();
    let all = claims();
    let from = lessons
        .iter()
        .find(|l| l.id == "particle-remembers")
        .expect("step 13 is still in the path");
    let want = all
        .iter()
        .find(|c| c.step == "looks-fine-from-outside" && c.quantity == "v_at:400")
        .unwrap_or_else(|| {
            panic!(
                "step 15's 400 s reading is no longer filed as `v_at:400`. If its instants \
                 were renamed again, rename them here; if the tag was dropped, step 15 is \
                 unquotable again and step 16 has lost the arm this slice built for it."
            )
        });
    assert_eq!(
        want.read_at_s, 400.0,
        "`v_at:400` on step 15 declares `read_at_s = {}`. The tag is the instant, so this \
         pair cannot disagree — see `measure`, which asserts the same thing from the other \
         side, against the trajectory.",
        want.read_at_s
    );
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &all,
        arms: &[],
        derived: &[],
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    let tie = Tie::Quoted {
        step: "looks-fine-from-outside",
        arm: None,
        quantity: "v_at:400",
        states: QuotedAs::Same,
    };
    assert_eq!(
        tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx),
        vec![want.value],
        "quoting step 15's `v_at:400` resolved to something other than that claim's own \
         value. A tagged reading names one measurement; if this resolves to a different \
         one, the tag is not doing the addressing the fence assumes it does."
    );
    // And the middle-of-the-run instant is what makes the line above evidence: a
    // resolution that took the file's first or last claim on the step would land on a
    // different voltage, not on this one.
    let others: Vec<f64> = all
        .iter()
        .filter(|c| c.step == "looks-fine-from-outside" && c.quantity.starts_with("v_at:"))
        .map(|c| c.value)
        .collect();
    assert!(
        others.len() > 2
            && others.first() != Some(&want.value)
            && others.last() != Some(&want.value),
        "this test is only evidence while step 15's tagged voltages are several and the \
         one it quotes is not the first or the last of them: {others:?}."
    );
}

/// A wrapper naming its own lesson is refused rather than resolved.
#[test]
#[should_panic(expected = "with extra words")]
fn an_elsewhere_may_not_name_its_own_step() {
    let lessons = lessons();
    let from = lessons
        .iter()
        .find(|l| l.id == "sixty-times-the-current")
        .expect("step 23 is still in the path");
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &[],
        derived: &[],
    };
    let tie = Tie::Elsewhere {
        step: "sixty-times-the-current",
        tie: &Tie::Setting(Control::DemandValue),
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// An arm as [`assert_walkable`] wants to see one, for the three tests that ask it
/// questions the file itself cannot ask.
///
/// Only the three fields those fences read are settable; everything else is the plainest
/// value that parses. It is deliberately *not* a `Default` on [`Arm`] — an arm with a
/// default is an arm somebody can forget to fill in, and every field on the real ones is
/// there because a check reads it.
fn walkable_probe(step: &str, pack_from: &str, start: Start) -> Arm {
    Arm {
        step: step.to_string(),
        name: "probe".to_string(),
        pack_from: Some(pack_from.to_string()),
        scenario: None,
        instruction: String::new(),
        start,
        demand_a: None,
        cc_cv_a: None,
        pulse_a: None,
        dt: None,
        bms: None,
        fade_per_ah: None,
        ambient_c: None,
        actions: vec![Action::Run { to_s: 1.0 }],
        identical_to: None,
        note: String::new(),
    }
}

/// An arm as [`assert_picker`] wants to see one — the picker's sibling of
/// [`walkable_probe`], and separate for the same reason the fields are: a walk names a
/// lesson and a pick names a file.
fn picker_probe(step: &str, file: &str, start: Start, instruction: &str) -> Arm {
    Arm {
        step: step.to_string(),
        name: "probe".to_string(),
        pack_from: None,
        scenario: Some(file.to_string()),
        instruction: instruction.to_string(),
        start,
        demand_a: None,
        cc_cv_a: None,
        pulse_a: None,
        dt: None,
        bms: None,
        fade_per_ah: None,
        ambient_c: None,
        actions: vec![Action::Run { to_s: 1.0 }],
        identical_to: None,
        note: String::new(),
    }
}

/// The lesson these fence tests are asked on — step 2, the one step with a picker arm.
fn picker_step(lessons: &[Lesson]) -> &Lesson {
    lessons
        .iter()
        .find(|l| l.id == "same-discharge-other-chemistry")
        .expect("step 2 is still in the path")
}

/// A picked file that is the step's own is refused.
///
/// None of these five is reachable from `path-claims.toml` — the one picker arm in the file
/// satisfies all of them — so each is priced here rather than resting on a paragraph, on the
/// terms the `pack_from` fences below already established.
#[test]
#[should_panic(expected = "the file the step is already on")]
fn a_picked_file_may_not_be_the_steps_own() {
    let lessons = lessons();
    let here = picker_step(&lessons);
    let arm = picker_probe(&here.id, &here.scenario, Start::Restart, &here.scenario);
    assert_walkable(&arm, here, here);
}

/// A picked file the page's own picker does not list is refused.
#[test]
#[should_panic(expected = "picker does not offer it")]
fn a_picked_file_must_be_in_the_picker() {
    let lessons = lessons();
    let here = picker_step(&lessons);
    let missing = "cc_cv_charge_lfp.toml";
    assert!(
        !picker_files().iter().any(|f| f == missing),
        "this test is only evidence while `{missing}` is absent from the picker; it is \
         listed now, so the fence needs a file that is not."
    );
    let arm = picker_probe(&here.id, missing, Start::Restart, missing);
    assert_walkable(&arm, here, here);
}

/// A picked file whose name the instruction does not spell is refused.
#[test]
#[should_panic(expected = "does not appear in the instruction")]
fn a_picked_file_must_be_named_in_its_instruction() {
    let lessons = lessons();
    let here = picker_step(&lessons);
    let arm = picker_probe(
        &here.id,
        "cc_discharge_lgm50.toml",
        Start::Restart,
        "load the other one from the picker",
    );
    assert_walkable(&arm, here, here);
}

/// A picked file on a continuation is refused: loading one *is* a restart.
#[test]
#[should_panic(expected = "closes the backend")]
fn a_picked_file_cannot_continue_this_steps_mark() {
    let lessons = lessons();
    let here = picker_step(&lessons);
    let arm = picker_probe(
        &here.id,
        "cc_discharge_lgm50.toml",
        Start::Mark,
        "cc_discharge_lgm50",
    );
    assert_walkable(&arm, here, here);
}

/// A picked file composed with a walk is refused: two navigations, one sentence.
#[test]
#[should_panic(expected = "two navigations composed")]
fn a_picked_file_may_not_be_combined_with_a_walk() {
    let lessons = lessons();
    let here = picker_step(&lessons);
    let mut arm = picker_probe(
        &here.id,
        "cc_discharge_lgm50.toml",
        Start::Restart,
        "cc_discharge_lgm50",
    );
    arm.pack_from = Some("bare-curve".to_string());
    assert_walkable(&arm, here, here);
}

/// An arm whose pack comes from the step it is declared on is refused.
///
/// Unreachable from the claims file — the one twin arm names the lesson next door — so the
/// fence is priced here instead of resting on a paragraph. Same argument, and the same
/// wording, as [`an_elsewhere_may_not_name_its_own_step`].
#[test]
#[should_panic(expected = "own pack with extra words")]
fn a_pack_from_may_not_name_its_own_step() {
    let lessons = lessons();
    let here = lessons
        .iter()
        .find(|l| l.id == "the-electrolyte-starves")
        .expect("step 16 is still in the path");
    let arm = walkable_probe(&here.id, &here.id, Start::Restart);
    assert_walkable(&arm, here, here);
}

/// An arm whose named lesson is on the *same* scenario file is refused.
///
/// The subtler of the three: `pack_from` would resolve, the run would build, and every
/// number on the arm would be identical to one the step could produce without sending the
/// reader anywhere. Steps 3 to 7 all sit on one file, which is what makes this askable with
/// two real lessons rather than two invented ones.
#[test]
#[should_panic(expected = "not another pack at all")]
fn a_pack_from_may_not_name_a_lesson_on_the_same_file() {
    let lessons = lessons();
    let here = lessons
        .iter()
        .find(|l| l.id == "pack-disagrees")
        .expect("step 3 is still in the path");
    let there = lessons
        .iter()
        .find(|l| l.id == "belief-drifts")
        .expect("step 4 is still in the path");
    assert_eq!(
        here.scenario, there.scenario,
        "this test is only evidence while those two lessons share a scenario file; they now \
         read `{}` and `{}`, so it is asking nothing and needs a new pair.",
        here.scenario, there.scenario
    );
    let arm = walkable_probe(&here.id, &there.id, Start::Restart);
    assert_walkable(&arm, here, there);
}

/// An arm cannot continue this step's mark on another lesson's pack.
///
/// There is no such position to continue from: walking next door reloads that lesson and
/// re-dials its controls, so a typed current there can only precede a Restart.
#[test]
#[should_panic(expected = "There is no such position")]
fn a_pack_from_cannot_continue_this_steps_mark() {
    let lessons = lessons();
    let here = lessons
        .iter()
        .find(|l| l.id == "the-electrolyte-starves")
        .expect("step 16 is still in the path");
    let there = lessons
        .iter()
        .find(|l| l.id == "wearing-out-while-idle")
        .expect("step 8 is still in the path");
    let arm = walkable_probe(&here.id, &there.id, Start::Mark);
    assert_walkable(&arm, here, there);
}

/// [`Tie::OnArm`] reads a file only off an arm that **picked** one.
///
/// This is the original refusal, kept word for word and narrowed to where it is still true.
/// `OnArm` used to wrap a control and nothing else, on the argument that "a file read does
/// not become a different fact for being asked under an arm's name" — which is right for
/// every arm that changes only controls, and false for one that loads another scenario from
/// the picker. So the refusal now fires on the arms it was always about: step 8's `hot`
/// dials the ambient slider and leaves the file alone, and a rule asking it for a scenario
/// field would be reading step 8's own number while naming an arm.
///
/// None of this arm's three fences is reachable from `path-claims.toml`, because a rule is
/// code. So they are asked directly, on the terms `an_elsewhere_may_not_wrap_another_one`
/// established — a fence no run enters is a paragraph, and this file has been caught by that
/// twice.
#[test]
#[should_panic(expected = "An arm overrides controls, not files")]
fn an_on_arm_may_not_read_a_file_off_an_arm_that_picks_none() {
    let lessons = lessons();
    let from = lessons
        .iter()
        .find(|l| l.id == "wearing-out-while-idle")
        .expect("step 8 is still in the path");
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let arms = arms();
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &arms,
        derived: &[],
    };
    let tie = Tie::OnArm {
        arm: "hot",
        tie: &Tie::Scenario("pack.initial_soc"),
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// [`Tie::OnArm`] wraps a control or a file read, and nothing else.
///
/// The fence that survives the file half of this arm. An `Ordinal` — or an `Elsewhere`, or a
/// `Derived`, or a second `OnArm` — resolves against the **step's** lesson while reading as
/// the arm's, which is the misattribution the original refusal was written against. Asked on
/// step 2's picker arm, so the failure is the one this test names rather than the
/// picks-no-file one above.
#[test]
#[should_panic(expected = "answers for exactly two things")]
fn an_on_arm_may_not_wrap_anything_else() {
    let lessons = lessons();
    let from = picker_step(&lessons);
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let arms = arms();
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &arms,
        derived: &[],
    };
    let tie = Tie::OnArm {
        arm: "the third cell",
        tie: &Tie::Ordinal("bare-curve"),
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// [`Tie::Picker`] needs an arm that picks a file.
///
/// A walk names a lesson and a pick names a file, and only the second has a name with digits
/// in it for a sentence to spell. Pointed at step 8's `hot`, which changes the ambient
/// slider and no scenario at all, this resolves to nothing — and resolving to nothing would
/// be reported as a restructured file rather than as a rule pointed at the wrong kind of
/// arm, so it panics by name instead.
#[test]
#[should_panic(expected = "only the second has a name")]
fn a_picker_tie_needs_an_arm_that_picks_a_file() {
    let lessons = lessons();
    let from = lessons
        .iter()
        .find(|l| l.id == "wearing-out-while-idle")
        .expect("step 8 is still in the path");
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let arms = arms();
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &arms,
        derived: &[],
    };
    let tie = Tie::Picker {
        arm: "hot",
        prefix: "calendar_fade_hot",
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// And it may only read a control that arm actually overrides.
///
/// The fallback this refuses is the one that would have accounted step 8's `45 °C` against
/// the 25 the step arrives with — right-shaped, green, and about a control the reader was
/// never asked to touch.
#[test]
#[should_panic(expected = "does not override it")]
fn an_on_arm_reads_a_control_the_arm_overrides() {
    let lessons = lessons();
    let from = lessons
        .iter()
        .find(|l| l.id == "wearing-out-while-idle")
        .expect("step 8 is still in the path");
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let arms = arms();
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &arms,
        derived: &[],
    };
    // The `hot` arm drags the ambient and nothing else, so asking it for the mark is asking
    // for a control it leaves where the step left it.
    let tie = Tie::OnArm {
        arm: "hot",
        tie: &Tie::Setting(Control::Until),
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// And the arm it names has to exist.
///
/// The third of [`Tie::OnArm`]'s panics, and the one no perturbation reaches: renaming the arm
/// in `path-claims.toml` breaks the claim that reads it first, so the suite fails on a claim
/// naming nothing before this is ever asked. Its own test for that reason.
#[test]
#[should_panic(expected = "declares no arm of that name")]
fn an_on_arm_names_an_arm_that_exists() {
    let lessons = lessons();
    let from = lessons
        .iter()
        .find(|l| l.id == "wearing-out-while-idle")
        .expect("step 8 is still in the path");
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let arms = arms();
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &arms,
        derived: &[],
    };
    let tie = Tie::OnArm {
        arm: "hot from nowhere",
        tie: &Tie::Setting(Control::Ambient),
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// A wrapper around a wrapper, or around a derivation, is refused rather than resolved.
#[test]
#[should_panic(expected = "Nesting has no floor")]
fn an_elsewhere_may_not_wrap_another_one() {
    let lessons = lessons();
    let from = lessons
        .iter()
        .find(|l| l.id == "sixty-times-the-current")
        .expect("step 23 is still in the path");
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &[],
        derived: &[],
    };
    let tie = Tie::Elsewhere {
        step: "slow-and-patient",
        tie: &Tie::Elsewhere {
            step: "bare-curve",
            tie: &Tie::Clock,
        },
    };
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// Every lesson is either ledgered or named as not ledgered.
///
/// The half of the ledger that is about the future. Coverage grows one step at a time and
/// will be partial for several slices yet, so the risk is not the gap — it is a gap nobody
/// wrote down. Adding a twenty-fifth lesson has to be a decision; without this it is a
/// default.
#[test]
fn every_lesson_is_ledgered_or_named_as_not() {
    let lessons = lessons();
    let ledger = ledger();
    assert!(
        !ledger.steps.is_empty(),
        "web/path-claims.toml's `[ledger]` names no step. An empty ledger passes every \
         scan below and proves nothing."
    );

    let mut listed: Vec<&str> = ledger
        .steps
        .iter()
        .chain(&ledger.unledgered)
        .map(String::as_str)
        .collect();
    for id in &listed {
        assert!(
            lessons.iter().any(|l| l.id == *id),
            "`[ledger]` names `{id}`, which is not a lesson in web/app.js."
        );
    }
    listed.sort_unstable();
    let before = listed.len();
    listed.dedup();
    assert_eq!(
        before,
        listed.len(),
        "`[ledger]` lists a step twice. A step in both lists is ledgered and excused at \
         once, and the excuse is what a reader would believe."
    );
    for l in &lessons {
        assert!(
            listed.binary_search(&l.id.as_str()).is_ok(),
            "lesson `{}` is in neither `[ledger].steps` nor `[ledger].unledgered`.\n\
             Every step has to be in exactly one. Ledger it if its numbers can be tied to \
             something today; otherwise add it to `unledgered`, which is how this file \
             says out loud what it is not checking. Silence is what left fourteen steps \
             unguarded.",
            l.id,
        );
    }
}

/// A vocabulary rule is a phrase, and every rule is used.
///
/// Two failure shapes, both of which would leave the ledger looking like more than it is.
/// A rule with no words in it — `"{n} %"` — accounts for numbers by their unit rather than
/// by what the sentence says about them. And a rule that matches nothing is the shape this
/// file has already been caught by once: `CCCV_PERIOD_S` sat pinned in [`MIRRORED`] for six
/// slices with nothing reading it, and the mirror it was supposed to guard was wrong the
/// whole time.
#[test]
fn every_ledger_rule_is_a_phrase_and_is_used() {
    let lessons = lessons();
    let ledger = ledger();

    let mut used = vec![0usize; LEDGER_VOCABULARY.len()];
    for step in &ledger.steps {
        let lesson = lessons
            .iter()
            .find(|l| l.id == *step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));
        let text = ascii_minus(&lesson.text);
        let numbers = ledger_numbers(&text, step, &ledger);
        for (r, rule) in LEDGER_VOCABULARY.iter().enumerate() {
            used[r] += rule_matches(&text, &numbers, rule.phrase).len();
        }
    }

    for (r, rule) in LEDGER_VOCABULARY.iter().enumerate() {
        let parts: Vec<&str> = rule.phrase.split("{n}").collect();
        assert!(
            parts.len() > 1,
            "vocabulary rule `{}` has no `{{n}}` — there is no number in it to account for.",
            rule.phrase
        );
        assert_eq!(
            parts.len() - 1,
            rule.ties.len(),
            "vocabulary rule `{}` has {} `{{n}}` and {} tie(s). Each number in the \
             phrase needs the thing that decides it.",
            rule.phrase,
            parts.len() - 1,
            rule.ties.len(),
        );
        for (p, pair) in parts.windows(2).enumerate() {
            assert!(
                !pair[0].is_empty() || !pair[1].is_empty(),
                "vocabulary rule `{}` has a `{{n}}` (number {}) with nothing on either \
                 side of it. A number needs the sentence around it, or the rule is \
                 accounting for a position rather than for a phrase.",
                rule.phrase,
                p + 1,
            );
        }
        // A name tie's prefix is what keeps it off the digits beside the one it means, so
        // an empty one is the generous match `Tie::Name`'s own docs say it exists to
        // prevent: `digits_after` would match at every position and collect every run in
        // the field. Refused rather than priced — unlike the existential `Tie::Member`,
        // nothing here needs it and there is no sentence to weigh against.
        for tie in rule.ties {
            if let Tie::Name { field, prefix } = tie {
                assert!(
                    !prefix.is_empty(),
                    "vocabulary rule `{}` reads `{field}` with an empty prefix, so it \
                     accounts for a number that appears ANYWHERE in that string. The \
                     prefix is the whole of this tie's specificity.",
                    rule.phrase
                );
            }
            // A ratio is written as a pair, so a third tie under one would be silently
            // dropped by the division rather than reported.
            if let Tie::Ratio(pair) = tie {
                assert_eq!(
                    pair.len(),
                    2,
                    "vocabulary rule `{}` divides {} ties. A ratio has a numerator and a \
                     denominator and nothing else.",
                    rule.phrase,
                    pair.len(),
                );
            }
            // The word an operand reads has to be IN the phrase, which is what pins it to
            // the sentence: the phrase match already requires those exact words around the
            // number, so `six` cannot be a word the author supplied from outside. Without
            // this the operand would be a declared 6 with a label on it.
            if let Tie::Derived { operands, .. } = tie {
                for op in *operands {
                    if let Operand::Word(w) = op {
                        assert!(
                            rule.phrase.contains(w),
                            "vocabulary rule `{}` derives a number from the word `{w}`, \
                             and its own phrase does not contain that word. An operand \
                             the phrase does not pin is a number the author supplied.",
                            rule.phrase,
                        );
                    }
                }
            }
        }
        let words: String = parts.concat();
        assert!(
            words.chars().any(|c| c.is_ascii_alphabetic()) && words.trim().len() >= 4,
            "vocabulary rule `{}` carries no words. A phrase made of punctuation and a \
             unit accounts for a number by its unit, which is exactly the generous match \
             this arm exists to avoid.",
            rule.phrase
        );
        assert!(
            used[r] > 0,
            "vocabulary rule `{}` matches nothing in any ledgered step. Either the prose \
             it was written for was edited — in which case the number it used to account \
             for is now failing the ledger and this rule is why it is hard to see — or the \
             rule was never used at all. A rule nothing consults is the `CCCV_PERIOD_S` \
             shape: it reads as coverage and is not.",
            rule.phrase
        );
    }
}

/// Check 1 — the claim's text is still in the prose it is a claim about.
#[test]
fn every_claim_appears_in_its_own_step() {
    let lessons = lessons();
    for c in claims() {
        let lesson = lessons
            .iter()
            .find(|l| l.id == c.step)
            .unwrap_or_else(|| panic!("no lesson `{}`", c.step));
        assert!(
            lesson.text.contains(&c.literal),
            "step `{}` no longer contains the claim `{}`.\n\
             The prose was edited and web/path-claims.toml was not. Either restore the \
             sentence or re-measure the claim and update BOTH halves — the literal and \
             the value.",
            c.step,
            c.literal
        );
        // The opt-in half of the display check, and the cheap one: it needs no run,
        // only the sentence. `shows` is what the row prints; `quoted` says the sentence
        // hands the reader that string to look for.
        if c.quoted {
            let (row, shows) = c
                .display_claim()
                .expect("quoted without a display claim is rejected in display_claim");
            assert!(
                ascii_minus(&lesson.text).contains(&ascii_minus(shows)),
                "step `{}` says the `{row}` row shows `{shows}`, and quotes it — but that \
                 string is not in the step's own prose.\n\
                 Either the sentence was reworded away from what the panel prints (which \
                 is the defect this flag exists to catch: a reader told to look at a row \
                 must be told what the row says), or the claim's `shows` is stale.",
                c.step
            );
        }
    }
}

/// Check 3 — the claim is read at a time the step actually runs to.
#[test]
fn every_claim_is_reachable_in_its_own_step() {
    let lessons = lessons();
    let arms = arms();
    for c in claims() {
        let lesson = lessons
            .iter()
            .find(|l| l.id == c.step)
            .unwrap_or_else(|| panic!("no lesson `{}`", c.step));

        let Some(arm) = arm_of(&arms, &c) else {
            assert!(
                c.read_at_s <= lesson.until_s,
                "step `{}` claims `{}` at t = {} s, but the step stops at its mark of {} \
                 s. The number may well be true; a reader cannot get to it. This is the \
                 'right but unreachable' defect this repo has shipped twice.\n\
                 If the claim is about something the reader does next — a demand typed in, \
                 the BMS unchecked, a button pressed — it needs an `[[arm]]` and an `arm` \
                 naming it.",
                c.step,
                c.literal,
                c.read_at_s,
                lesson.until_s
            );
            continue;
        };

        // A twin arm's timeline is the lesson its pack comes from, not the step it is
        // declared on — the same resolution `run` makes, through the same function.
        let pack_lesson = pack_lesson_of(Some(arm), lesson, &lessons);
        let earliest = arm.earliest_s(pack_lesson);
        assert!(
            c.read_at_s >= earliest,
            "claim `{}` on step `{}` reads arm `{}` at t = {} s, which is before that arm \
             begins at {earliest} s. On a continuation that means it is measured on the \
             step's own run rather than on the change the arm is about — drop the `arm` or \
             fix the time.",
            c.literal,
            c.step,
            arm.name,
            c.read_at_s,
        );
        // A continuation must read *strictly* past the mark. The mark's own row is the
        // step's, and a claim reading it through an arm would be a claim about a
        // trajectory the reader has not started yet.
        assert!(
            arm.start == Start::Restart || c.read_at_s > earliest,
            "claim `{}` on step `{}` reads the continuation `{}` at the mark itself \
             ({earliest} s). That row belongs to the step's own run — it is the same \
             number every claim without an arm reads there.",
            c.literal,
            c.step,
            arm.name,
        );
        let end = arm.end_s(pack_lesson);
        assert!(
            c.read_at_s <= end,
            "claim `{}` on step `{}` reads at t = {} s; the arm `{}` runs to {end} s. \
             Lengthen it only if the prose still asks the reader to run that far — how far \
             an arm goes is this file's own choice and stretching it to cover a claim is \
             how the reachability check on an arm becomes a tautology.",
            c.literal,
            c.step,
            c.read_at_s,
            arm.name,
        );
    }
}

/// The engine contract every `probe` claim rests on: a zero-length step moves nothing.
///
/// Not a test about a claim, and it is here rather than in `sim-core` because it is *this*
/// file that depends on it. [`run`] takes the probe and then drives the same pack, so a
/// probe that advanced anything would corrupt every stepped row behind it — and the page
/// takes **two** probes on a reloading step (`loadScenario` under the stale demand box,
/// then `applyStep` under the step's own), of which this harness reproduces only the
/// second. Both of those are only sound because probing is a read.
///
/// Written because the invariant has been broken before: `docs/plans/phase-3-aging-faults.md`
/// records end-of-step temperature reported with no `dt > 0` gate, where a zero-length
/// probe on a pack did move something. A doc comment saying "probes do not mutate" is the
/// thing that was true until it wasn't.
///
/// Three lessons rather than one, because the models differ in what they *have* to carry
/// across a step: a particle with 20 shells of diffusion state, a pack with a BMS holding
/// its own estimate and a noisy current sensor, and an equivalent circuit with RC pairs.
#[test]
fn a_zero_length_probe_moves_nothing() {
    let lessons = lessons();
    for id in [
        "the-gradient-itself",
        "belief-drifts",
        "circuit-repeats-itself",
    ] {
        let lesson = lessons
            .iter()
            .find(|l| l.id == id)
            .unwrap_or_else(|| panic!("no lesson `{id}`"));
        let mut pack = build(lesson);
        let env = Env {
            t_ambient: lesson.ambient_c + K,
            t_coolant: None,
        };
        let json = |p: &Pack| serde_json::to_string(&p.snapshot()).expect("a snapshot serialises");

        let before = json(&pack);
        let d = demand_now(lesson.demand, &pack, lesson.dt, None);
        let first = pack.step(0.0, d, &env);
        let between = json(&pack);
        let second = pack.step(0.0, d, &env);
        let after = json(&pack);

        // The snapshot is the whole of the state, so this is the strong half: nothing
        // moved, including the parts no telemetry field reports.
        assert_eq!(
            before, between,
            "`{id}`: one zero-length probe changed the pack's snapshot. Every `probe` \
             claim in web/path-claims.toml is a claim about the pack as it stands before \
             the first step, and this harness probes that pack and then drives it — so a \
             probe that mutates makes those claims wrong AND corrupts every stepped row \
             behind them."
        );
        assert_eq!(
            before, after,
            "`{id}`: two zero-length probes changed the snapshot."
        );

        // And the weak half stated separately, because it is the one an author reads off
        // the panel: the same probe answers the same way however many times it is taken.
        // Bit-identical, not within a tolerance — there is no arithmetic between them.
        assert_eq!(
            (
                first.v_terminal,
                first.soc_true,
                first.t_max,
                first.i_actual
            ),
            (
                second.v_terminal,
                second.soc_true,
                second.t_max,
                second.i_actual
            ),
            "`{id}`: two zero-length probes on an unchanged pack reported different \
             telemetry."
        );
    }
}

/// Check 3's sibling — a `probe` claim is really about the instant before the run.
///
/// Three fences, and each one closes a way the declaration could be true of the file and
/// false of the page:
///
/// * **`read_at_s` must be 0.** The probe is taken before the first step, so it has no
///   other instant. A probe claim carrying a decorative time would be the shape
///   `soc_gap_pts_min` was fenced against a slice ago, with the added trap that the number
///   *looks* addressed by time and is not.
/// * **It cannot name an `arm`.** Every arm is something the reader does *next*, and the
///   probe is the panel before they have done anything. A restart arm does take a probe of
///   its own — the page shows one after Restart — and no sentence quotes it, so claiming it
///   is refused here rather than half-supported.
/// * **The step must reload.** [`run`] builds a fresh pack for every step, which a stepped
///   trajectory mostly absorbs — but the probe *is* the fresh pack, so on a step that
///   inherits its pack from its predecessor the harness would be mirroring a reading the
///   page never shows. `applyStep` reloads on `reload: true` or a changed scenario file;
///   only the first is knowable here, and it is the conservative half.
///
/// The quantity is fenced elsewhere, in [`measure`]: a reduction over the whole run has no
/// probe reading, and asking for one is refused rather than quietly answered from `rows`.
#[test]
fn every_probe_claim_is_taken_before_the_run() {
    let lessons = lessons();
    for c in claims().iter().filter(|c| c.probe) {
        let lesson = lessons
            .iter()
            .find(|l| l.id == c.step)
            .unwrap_or_else(|| panic!("no lesson `{}`", c.step));
        assert_eq!(
            c.read_at_s, 0.0,
            "claim `{}` on step `{}` reads the zero-length probe and sets read_at_s = {}. \
             The probe is taken before the first step and has no other instant; a time \
             here reads as an address it is not.",
            c.literal, c.step, c.read_at_s
        );
        assert!(
            c.arm.is_none(),
            "claim `{}` on step `{}` is a `probe` claim and reads the arm `{}`. Those are \
             opposite instants — before the reader arms the run, and after they have \
             pressed something.",
            c.literal,
            c.step,
            c.arm.as_deref().unwrap_or_default()
        );
        assert!(
            lesson.reload,
            "claim `{}` reads the zero-length probe on step `{}`, and that step does not \
             set `reload: true`. `applyStep` then keeps whatever pack is on screen when \
             the scenario file has not changed, so what the panel shows before the reader \
             presses Run depends on the step they arrived from — while this harness always \
             builds a fresh one. The claim would be about a pack no reader is guaranteed.",
            c.literal, c.step
        );
    }
}

/// Check 2 — the engine still produces the number.
#[test]
fn every_claim_matches_the_engine() {
    let lessons = lessons();
    let all = claims();

    let derived = derivations();
    // Group by *trajectory* and run each one once. Not a micro-optimisation: step 8 is a
    // 200 000 s rest at dt = 0.5 — 400 000 engine steps — and it carries three claims.
    // Re-running it per claim tripled this test's cost for nothing.
    //
    // A trajectory is a step and one of its arms, or a step and none. Arms cannot share a
    // run the way a leg used to share its step's: a leg only appended rows to the step's
    // own trajectory, where an arm can rebuild the pack under different controls and
    // report *different numbers at the same instant* — which is the whole of step 18's
    // `dt = 5` contrast.
    let arms = arms();
    let mut trajectories: Vec<(&str, Option<&str>)> = all
        .iter()
        .map(|c| (c.step.as_str(), c.arm.as_deref()))
        .collect();
    trajectories.sort_unstable();
    trajectories.dedup();

    for (step, arm_name) in trajectories {
        let lesson = lessons
            .iter()
            .find(|l| l.id == step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));
        let arm = arm_name.map(|name| {
            arms.iter()
                .find(|a| a.step == step && a.name == name)
                .unwrap_or_else(|| panic!("no arm `{name}` on step `{step}`"))
        });
        // The instants this trajectory's claims want the overpotential split at, gathered
        // before the run because that read is the one thing a row cannot afford to take
        // speculatively. See [`Row::rc_overpotential_v`].
        let capture: Vec<f64> = all
            .iter()
            .filter(|c| {
                c.step == step
                    && c.arm.as_deref() == arm_name
                    && matches!(
                        c.quantity.as_str(),
                        "rc_overpotential_mv_at" | "diffusion_overpotential_mv_at"
                    )
            })
            .map(|c| c.read_at_s)
            .collect();
        let r = run(lesson, arm, &capture, &lessons);

        // The fence on `Accounted::ReadAt`, which lives here because this is the only
        // place a trajectory exists. Check 6 accepts a number in a claimed sentence when
        // it is the instant that sentence's claims are read at — which says "we measured
        // then", not "this is when it happened". On `**99.98 %** when the cell empties at
        // **207.5 s and 1.9306 V**` those are two different statements and the reader is
        // given the second, so deleting the flag claim from that sentence left check 6
        // green with the sentence's own event time tied to nothing. An instant where the
        // run raises a flag it did not have a step earlier is an event, and an event a
        // sentence names has to be claimed.
        //
        // Scoped to the sentences whose claims are read on *this* trajectory: a flag
        // arriving on the unprotected arm says nothing about a sentence read on the
        // protected one, and asking the wrong run would be a false red.
        for (s, literal) in sentences(&all) {
            if s != step {
                continue;
            }
            let group = sentence_group(&all, s, literal);
            if !group.iter().any(|c| c.arm.as_deref() == arm_name) {
                continue;
            }
            for token in numeric_tokens(&ascii_minus(literal)) {
                let Some(Accounted::ReadAt(at_s)) =
                    accounting_for(&Reading::digits(&token), &group, lesson, &arms, &derived)
                else {
                    continue;
                };
                let arriving = r.flags_arriving_at(at_s);
                assert!(
                    arriving.is_empty(),
                    "step `{step}`, sentence `{literal}`:\n  it prints `{token}`, and the \
                     only thing accounting for that number is that this sentence's claims \
                     are read at t = {at_s} s — where the run raises {arriving:?}.\n\
                     A sentence naming the moment something happens is claiming that \
                     moment. `read at` is the weaker statement that we measured then, and \
                     it would stay green if the flag moved and the prose and the literal \
                     moved with it. Add a claim on this sentence against the event \
                     itself, e.g. `quantity = \"flag_first_s:{}\"`.",
                    arriving.first().map_or("FLAG", String::as_str),
                );
            }
        }

        for c in all
            .iter()
            .filter(|c| c.step == step && c.arm.as_deref() == arm_name)
        {
            let got = measure(&c.quantity, &r, c.read_at_s, c.probe, lesson.until_s);
            assert!(
                (got - c.value).abs() <= c.tol,
                "step `{}`, claim `{}`:\n  {} at t = {} s\n  prose says {}\n  engine says \
                 {got}\n  difference {:.3e}, tolerance {:.3e}\n\
                 The engine moved under the prose. Re-measure, then update the sentence \
                 in web/app.js AND the literal and value in web/path-claims.toml \
                 together.",
                c.step,
                c.literal,
                c.quantity,
                c.read_at_s,
                c.value,
                (got - c.value).abs(),
                c.tol
            );

            // Check 4, folded in here rather than given its own #[test] for two reasons.
            // The cheap one is that step 8 is 400 000 engine steps and a second test
            // would run it again. The load-bearing one is that this formats the value
            // just *measured*, never `c.value`: a drift inside `tol` can still cross a
            // printed digit, and formatting the stored value would be green through
            // exactly that failure.
            let Some((row, shows)) = c.display_claim() else {
                continue;
            };
            // The same row the value came from, probe included: a display claim renders
            // what the reader is looking at when the claim is read, and on a probe claim
            // that is the panel before the run is armed.
            let row_read = r.read(c.read_at_s, c.probe);
            let row_time_s = row_read.t_s;
            let printed = render_row(row, row_read);
            assert_eq!(
                printed, shows,
                "step `{}`, claim `{}`:\n  the `{row}` readout at t = {} s\n  \
                 path-claims.toml says it prints `{shows}`\n  the page's formatter prints \
                 `{printed}`\n\
                 The row's *rendering* moved, which the value check above can miss: a \
                 panel row is a step function of the number behind it, so a drift well \
                 inside this claim's tolerance still changes what a reader sees. Re-read \
                 the sentence: if it quotes the old string it is now wrong on the page.",
                c.step, c.literal, row_time_s
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The file's account of itself
// ---------------------------------------------------------------------------

/// Which file a self-count is written in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prose {
    /// `web/path-claims.toml` — its header, its section notes and its foot.
    ClaimsFile,
    /// This test's own module and item documentation.
    ThisTest,
}

impl Prose {
    fn path(self) -> PathBuf {
        match self {
            Prose::ClaimsFile => repo_root().join("web").join("path-claims.toml"),
            Prose::ThisTest => repo_root()
                .join("crates")
                .join("sim-data")
                .join("tests")
                .join("path_claims.rs"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Prose::ClaimsFile => "web/path-claims.toml",
            Prose::ThisTest => "crates/sim-data/tests/path_claims.rs",
        }
    }
}

/// One file's prose, comment markers removed and every run of whitespace collapsed.
///
/// A sentence in either file is broken over three lines and padded into a column, and
/// neither of those is something a tally should be asserting. Flattening leaves exactly
/// the words and the digits, which is what the counts below are about.
fn flattened(prose: Prose) -> String {
    let text = read(&prose.path());
    let mut out = String::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let stripped = match prose {
            Prose::ClaimsFile => trimmed.strip_prefix('#').unwrap_or(trimmed),
            Prose::ThisTest => trimmed
                .strip_prefix("//!")
                .or_else(|| trimmed.strip_prefix("///"))
                .or_else(|| trimmed.strip_prefix("//"))
                .unwrap_or(trimmed),
        };
        out.push(' ');
        out.push_str(stripped);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The English for the counts these files spell in letters.
///
/// Separate from [`WORD_NUMERALS`] on purpose, and it is not a second vocabulary by
/// accident: that table is the *prose's* words, and every entry in it must be spelled by
/// a claim ([`every_word_numeral_is_spelled_by_a_claim`]), so putting "twelve" there to
/// render a header sentence would redden a test about the lesson prose. Where the two
/// overlap they are asserted to agree, in the test below.
const HEADER_WORDS: &[(usize, &str)] = &[
    (0, "none"),
    (1, "one"),
    (2, "two"),
    (3, "three"),
    (4, "four"),
    (5, "five"),
    (6, "six"),
    (7, "seven"),
    (8, "eight"),
    (9, "nine"),
    (10, "ten"),
    (11, "eleven"),
    (12, "twelve"),
    (13, "thirteen"),
    (14, "fourteen"),
    (15, "fifteen"),
    (16, "sixteen"),
    (17, "seventeen"),
    (18, "eighteen"),
    (19, "nineteen"),
    (20, "twenty"),
    (21, "twenty-one"),
    (22, "twenty-two"),
    (23, "twenty-three"),
    (24, "twenty-four"),
    (25, "twenty-five"),
    (26, "twenty-six"),
    (27, "twenty-seven"),
    // The ledger's numeral count passed twenty-five with its fifth step and will keep
    // going; the tens are here so the next one does not have to stop and add a word.
    (30, "thirty"),
    (40, "forty"),
    (41, "forty-one"),
    (42, "forty-two"),
    (43, "forty-three"),
    (44, "forty-four"),
    (45, "forty-five"),
    (50, "fifty"),
    (58, "fifty-eight"),
    (60, "sixty"),
];

/// The same, for the counts a sentence writes as a position rather than a size — "and no
/// fifth" is a statement about how many there are.
const HEADER_ORDINALS: &[(usize, &str)] = &[
    (1, "first"),
    (2, "second"),
    (3, "third"),
    (4, "fourth"),
    (5, "fifth"),
    (6, "sixth"),
    (7, "seventh"),
    (8, "eighth"),
    (9, "ninth"),
    (10, "tenth"),
];

/// One count a file states about its **own contents**, and the derivation that produces
/// it.
///
/// The phrase is declared and the number never is — the same contract [`LedgerRule`]
/// keeps for the ledger and `spells` keeps for a claim. A tally that stored the number
/// would be a second copy of the thing it is meant to guard.
struct Tally {
    /// Which file the sentence is in.
    prose: Prose,
    /// The sentence, with `{n}` where a digit-written count sits, `{w}` where one is
    /// written in letters, `{W}` where it opens a sentence and `{o}` where the sentence
    /// counts by position ("no fifth"). Matched against the flattened file, so line
    /// breaks and column padding are not part of it.
    phrase: &'static str,
    /// One derivation per placeholder, in order.
    of: &'static [fn(&Facts) -> usize],
}

/// A count these files state about themselves that this check does **not** derive, and
/// why.
///
/// Without this, "which tallies are covered" would be whatever happens to be in the table
/// above, and a green run would read as "every number this file states about itself is
/// checked" — which is false, and is exactly the shape the ledger's `unledgered` list
/// exists to stop. The phrase is required to still be in the file, so rewording or
/// deleting the sentence reddens the waiver instead of silently retiring it.
struct NotDerived {
    prose: Prose,
    /// The sentence, verbatim (flattened). No placeholder: nothing renders it.
    phrase: &'static str,
    /// Why no derivation exists.
    #[allow(
        dead_code,
        reason = "the reason is for a human reader; the phrase is what is asserted"
    )]
    because: &'static str,
}

/// Everything a self-count can be derived from, parsed once.
struct Facts {
    claims: Vec<Claim>,
    arms: Vec<Arm>,
    ledger: Ledger,
    derived: Vec<Derivation>,
    lessons: Vec<Lesson>,
    /// The `[[english]]` backlog, as `(step, phrase)`. See [`english_exceptions`].
    english: Vec<(String, String)>,
}

impl Facts {
    fn gather() -> Facts {
        let parsed = parse_claims_file();
        Facts {
            claims: parsed.claim,
            arms: arms(),
            ledger: parsed.ledger,
            derived: parsed.derived,
            lessons: lessons(),
            english: parsed
                .english
                .into_iter()
                .map(|e| (e.step, e.phrase))
                .collect(),
        }
    }

    fn lesson(&self, step: &str) -> &Lesson {
        self.lessons
            .iter()
            .find(|l| l.id == step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"))
    }

    fn claims_on(&self, step: &str) -> usize {
        self.claims.iter().filter(|c| c.step == step).count()
    }

    /// How many numerals that step's whole prose prints — what the ledger scans.
    fn numerals_in(&self, step: &str) -> usize {
        written_numbers(&ascii_minus(&self.lesson(step).text)).len()
    }

    /// How many quantities a step's prose spells in ENGLISH, whether or not it is scanned.
    ///
    /// [`Facts::numerals_in`]'s twin, and deliberately answered for `word_blind` steps as
    /// well: a count beside a step nobody scans is the file's only statement of how much is
    /// there, and the one it would be easiest to write down once and never look at again.
    fn spelled_in(&self, step: &str) -> usize {
        spelled_numbers(&ascii_minus(&self.lesson(step).text)).len()
    }

    /// `(numbers accounted as `spelled`, numbers printed, claimed sentences)`.
    ///
    /// Run through [`accounting_for`] and [`sentences`] rather than re-scanned here. A
    /// second scan of the same prose could disagree with check 6 while both stayed green,
    /// which is the defect this whole file is built around.
    fn accounting(&self) -> (usize, usize, usize) {
        let sents = sentences(&self.claims);
        let (mut spelled, mut printed) = (0, 0);
        for (step, literal) in &sents {
            let lesson = self.lesson(step);
            let group = sentence_group(&self.claims, step, literal);
            for token in numeric_tokens(&ascii_minus(literal)) {
                printed += 1;
                if let Some(Accounted::Spelled) = accounting_for(
                    &Reading::digits(&token),
                    &group,
                    lesson,
                    &self.arms,
                    &self.derived,
                ) {
                    spelled += 1;
                }
            }
        }
        (spelled, printed, sents.len())
    }

    /// How many of check 6's arms the claims in this file actually use.
    ///
    /// Derived from use rather than from the enum, and that is the honest reading of the
    /// sentence it pins: this file's rule is that an arm nobody accounts anything with
    /// does not get built. An unused variant would leave the count where it was, and the
    /// author would find out when the arm's first number needed it.
    fn accounting_arms(&self) -> usize {
        let mut used: Vec<&'static str> = Vec::new();
        for (step, literal) in sentences(&self.claims) {
            let lesson = self.lesson(step);
            let group = sentence_group(&self.claims, step, literal);
            for token in numeric_tokens(&ascii_minus(literal)) {
                if let Some(a) = accounting_for(
                    &Reading::digits(&token),
                    &group,
                    lesson,
                    &self.arms,
                    &self.derived,
                ) {
                    let name = a.arm_name();
                    if !used.contains(&name) {
                        used.push(name);
                    }
                }
            }
        }
        used.len()
    }
}

impl Accounted {
    /// The name the header gives this arm. Exhaustive on purpose: a sixth variant does
    /// not compile until it is named here, so [`Facts::accounting_arms`] cannot go stale
    /// by omission.
    fn arm_name(&self) -> &'static str {
        match self {
            Accounted::Spelled => "spelled",
            Accounted::ReadAt(_) => "read at",
            Accounted::Shown => "shown",
            Accounted::Setting(_) => "setting",
            Accounted::Derived(_) => "derived",
        }
    }
}

fn n_claims(f: &Facts) -> usize {
    f.claims.len()
}
/// How many arms the ledger's scan has — the distinct kinds of tie its vocabulary reads,
/// plus the `claimed` arm, which is a claim rather than a tie.
///
/// Derived from **use** and not from the enum, on [`Facts::accounting_arms`]' terms: this
/// file's rule is that an arm nothing accounts anything with does not get built, so a
/// variant no rule names would be a gap rather than coverage.
fn n_ledger_arms(_f: &Facts) -> usize {
    /// Every arm this tie uses, itself and the ones it wraps.
    ///
    /// **Nested as well as top level, because an arm used only inside another is still an
    /// arm.** This walked the rules' outermost ties only, which was indistinguishable from
    /// the whole truth for as long as every arm was some rule's outermost one. `Tie::Hours`
    /// is the first that is not: step 16's amp-hour figure is a `Product` of the demand box
    /// and a duration read in hours, so the conversion never appears at the top of a rule.
    /// A count that missed it would have the file's own prose list one more arm than the
    /// count beside it, which is exactly the drift these tallies exist to catch.
    ///
    /// **[`Tie::Magnitude`] is in this list and its being here is unobservable today.**
    /// Measured, not assumed: dropping it leaves the whole suite green, because the
    /// `OnArm` it wraps and the `Setting` inside that are both some other rule's outermost
    /// tie. It is written the correct way round rather than the reachable way round, on the
    /// same terms as the environment split in [`run`] — the day a wrapper holds the only
    /// use of what it wraps, a walker that stopped here would undercount silently.
    fn walk(tie: &'static Tie, used: &mut Vec<&'static str>) {
        let name = tie_arm_name(tie);
        if !used.contains(&name) {
            used.push(name);
        }
        match tie {
            Tie::Product(ties) | Tie::Ratio(ties) | Tie::Difference(ties) | Tie::Sum(ties) => {
                for tie in *ties {
                    walk(tie, used);
                }
            }
            Tie::Hours(tie)
            | Tie::Ocv(tie)
            | Tie::Magnitude(tie)
            | Tie::Elsewhere { tie, .. }
            | Tie::OnArm { tie, .. } => walk(tie, used),
            _ => {}
        }
    }
    let mut used: Vec<&'static str> = Vec::new();
    for rule in LEDGER_VOCABULARY {
        for tie in rule.ties {
            walk(tie, &mut used);
        }
    }
    used.len() + 1
}
fn n_lessons(f: &Facts) -> usize {
    f.lessons.len()
}
fn n_quoted(f: &Facts) -> usize {
    f.claims.iter().filter(|c| c.quoted).count()
}
fn n_spelled(f: &Facts) -> usize {
    f.claims
        .iter()
        .filter(|c| matches!(c.tol_from, TolFrom::Spelled))
        .count()
}
fn n_tighter(f: &Facts) -> usize {
    f.claims
        .iter()
        .filter(|c| matches!(c.tol_from, TolFrom::Tighter))
        .count()
}
fn n_grid(f: &Facts) -> usize {
    f.claims
        .iter()
        .filter(|c| matches!(c.tol_from, TolFrom::Grid))
        .count()
}
fn n_states(f: &Facts, want: fn(&Claim) -> bool) -> usize {
    f.claims.iter().filter(|c| want(c)).count()
}
fn n_same(f: &Facts) -> usize {
    n_states(f, |c| matches!(c.states, States::Same))
}
fn n_magnitude(f: &Facts) -> usize {
    n_states(f, |c| matches!(c.states, States::Magnitude))
}
fn n_complement(f: &Facts) -> usize {
    n_states(f, |c| matches!(c.states, States::Complement))
}
fn n_since_mark(f: &Facts) -> usize {
    n_states(f, |c| matches!(c.states, States::SinceMark))
}
fn n_until_end(f: &Facts) -> usize {
    n_states(f, |c| matches!(c.states, States::UntilEnd))
}
fn n_displayed(f: &Facts) -> usize {
    n_states(f, |c| matches!(c.states, States::Displayed))
}
fn n_departure(f: &Facts) -> usize {
    n_states(f, |c| matches!(c.states, States::Departure))
}
fn n_nothing(f: &Facts) -> usize {
    n_states(f, |c| matches!(c.states, States::Nothing))
}
fn n_word_numerals(_: &Facts) -> usize {
    WORD_NUMERALS.len()
}
fn n_ledgered(f: &Facts) -> usize {
    f.ledger.steps.len()
}
fn n_ledgered_numerals(f: &Facts) -> usize {
    f.ledger.steps.iter().map(|s| f.numerals_in(s)).sum()
}
fn n_unclaimed_steps(f: &Facts) -> usize {
    f.lessons.iter().filter(|l| f.claims_on(&l.id) == 0).count()
}
fn n_unledgered(f: &Facts) -> usize {
    f.ledger.unledgered.len()
}
fn n_unledgered_claimed(f: &Facts) -> usize {
    f.ledger
        .unledgered
        .iter()
        .filter(|s| f.claims_on(s) > 0)
        .count()
}
fn n_unledgered_unclaimed(f: &Facts) -> usize {
    f.ledger.unledgered.len() - n_unledgered_claimed(f)
}
fn n_accounting_arms(f: &Facts) -> usize {
    f.accounting_arms()
}
fn n_accounting_arms_plus_one(f: &Facts) -> usize {
    f.accounting_arms() + 1
}
fn n_spelled_accountings(f: &Facts) -> usize {
    f.accounting().0
}
/// How many phrases `[[english]]` still lists, and how many steps they are spread over.
///
/// Derived from the list rather than from a scan of the prose, which is the right side to
/// read it from: the list is matched against the prose both ways by
/// [`no_lesson_spells_a_quantity_in_english`], so the two cannot differ, and counting the
/// list keeps this tally from needing a lesson scan of its own.
fn n_english_phrases(f: &Facts) -> usize {
    f.english.len()
}
fn n_english_steps(f: &Facts) -> usize {
    let mut steps: Vec<&str> = f.english.iter().map(|(s, _)| s.as_str()).collect();
    steps.sort_unstable();
    steps.dedup();
    steps.len()
}
fn n_numbers_in_claimed_sentences(f: &Facts) -> usize {
    f.accounting().1
}
fn n_claimed_sentences(f: &Facts) -> usize {
    f.accounting().2
}
fn n_claims_on_belief_drifts(f: &Facts) -> usize {
    f.claims_on("belief-drifts")
}
fn n_arms_on_step_18(f: &Facts) -> usize {
    f.arms
        .iter()
        .filter(|a| a.step == "one-step-that-got-through")
        .count()
}
fn n_mark_arms_on_step_18(f: &Facts) -> usize {
    f.arms
        .iter()
        .filter(|a| a.step == "one-step-that-got-through" && matches!(a.start, Start::Mark))
        .count()
}
fn n_claims_on_step_18(f: &Facts) -> usize {
    f.claims_on("one-step-that-got-through")
}
fn n_arms_on_step_11(f: &Facts) -> usize {
    f.arms
        .iter()
        .filter(|a| a.step == "what-protection-costs")
        .count()
}
fn n_claims_on_what_protection_costs(f: &Facts) -> usize {
    f.claims_on("what-protection-costs")
}

/// Every count these two files state about their own contents that is derivable from
/// them.
///
/// Opt-in, like the ledger and for the same reason: nothing can decide automatically
/// whether a number in a paragraph is *about this file*. So the gap is written down —
/// [`NOT_DERIVED`] — rather than left to be inferred from a green run.
const TALLIES: &[Tally] = &[
    // --- what the header says about its own claims -----------------------------
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "in all {n} claims below that set `quoted`",
        of: &[n_quoted],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "spelled {n} claims. `tol` is exactly the rule",
        of: &[n_spelled],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "tighter {n} claims. `tol` is strictly under the rule",
        of: &[n_tighter],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "grid {n} claims. The quantity is a time",
        of: &[n_grid],
    },
    // The `grid` count stated a SECOND time, in this test's own docs, in the sentence that
    // also says which claims survived the fence. It said "2 of 69" — true when four `grid`
    // claims were demoted and two were left, on a file that then held 69 claims — and stale
    // in both halves from the moment the ambient-arm slice added two more. Nothing covered
    // it: the tally above counts the same claims, but it matches a different sentence, and
    // a count sitting beside a derived one is not thereby derived. The `claims file` twin
    // of this sentence was reworded to carry no number at all, which is the other way to
    // stop a count rotting.
    Tally {
        prose: Prose::ThisTest,
        phrase: "misses by a whole one. {n} of {n}, every",
        of: &[n_grid, n_claims],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "deliberately tighter on {n} claims",
        of: &[n_tighter],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "same {n} claims. The sentence prints the quantity",
        of: &[n_same],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "magnitude {n}. The sentence prints the size",
        of: &[n_magnitude],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "complement {n}. The sentence prints how far below one",
        of: &[n_complement],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "since_mark {n}. A duration since the step's mark",
        of: &[n_since_mark],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "until_end {n}. A duration remaining to the mark",
        of: &[n_until_end],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "displayed {n}. The sentence prints what the ROW prints",
        of: &[n_displayed],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "departure {n}. The sentence names a value the quantity has LEFT",
        of: &[n_departure],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "nothing {n}. The sentence prints no number about this quantity",
        of: &[n_nothing],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "is the translation, {w} entries",
        of: &[n_word_numerals],
    },
    // --- what it says about check 6 --------------------------------------------
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "{W} accountings, and no {o}:",
        of: &[n_accounting_arms, n_accounting_arms_plus_one],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "the {n} claimed sentences need none",
        of: &[n_claimed_sentences],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "names it in `spells` — {n} of the {n} numbers the {n} claimed sentences \
                 print",
        of: &[
            n_spelled_accountings,
            n_numbers_in_claimed_sentences,
            n_claimed_sentences,
        ],
    },
    // --- what it says about the ledger -----------------------------------------
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "{W} steps so far. See \"THE LEDGER\"",
        of: &[n_ledgered],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "{w} steps of the {w} so far",
        of: &[n_ledgered, n_lessons],
    },
    Tally {
        // The backlog, in the header that describes it. `n_word_scanned` used to sit here
        // under the phrase "Word-scanned steps are {w} today"; that sentence went when the
        // digits rule made the reader's coverage beside the point, and this is what replaced
        // it. The size of what is still spelled in English is the number worth watching now,
        // and it is the one that has to fall.
        prose: Prose::ThisTest,
        phrase: "{n} of them across {w} steps",
        of: &[n_english_phrases, n_english_steps],
    },
    Tally {
        prose: Prose::ClaimsFile,
        // "all of them scenario constants" until step 6 joined, which is the sentence
        // moving because the fact did. A tally's phrase is allowed to follow its prose;
        // what it may never do is carry the number.
        //
        // The numeral count is in DIGITS where the step count is in words, and that is the
        // count outgrowing the English: this check refuses to render a number `HEADER_WORDS`
        // has no word for, and the ninth ledgered step would have taken it past a hundred
        // whatever happened. Extending the word table to three digits is a table that grows
        // every slice; writing this one in digits is the escape the refusal itself offers.
        phrase: "{w} steps, {n} numerals, and no longer all of them scenario constants",
        of: &[n_ledgered, n_ledgered_numerals],
    },
    // Phrased as a count of what is LEFT rather than "the remaining N steps", which was
    // the earlier wording and which reads as bad English at one and as nonsense at zero.
    // A tally has to survive its own count going to zero, or it forces a rewrite at the
    // exact moment the work it describes is finished.
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "carry no claim at all and need arms this file has not got: {w}",
        of: &[n_unledgered_unclaimed],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "{w} claims on its estimator gap",
        of: &[n_claims_on_belief_drifts],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "carry claims: {w} of them",
        of: &[n_unledgered_claimed],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "Unchecked entirely: {w}.",
        of: &[n_unledgered_unclaimed],
    },
    // --- and what this test's own docs say -------------------------------------
    Tally {
        prose: Prose::ThisTest,
        phrase: "`const LESSONS` is {n} teaching steps",
        of: &[n_lessons],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "The default shape: {n} of {n} claims",
        of: &[n_spelled, n_claims],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "only the declaration was wrong. {n} of {n}.",
        of: &[n_tighter, n_claims],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "The sentence prints the quantity itself. {n} of {n}, and the shape to \
                 prefer",
        of: &[n_same, n_claims],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "{W} steps are still in that position",
        of: &[n_unclaimed_steps],
    },
    // The unledgered split, stated once in each file. `path-claims.toml`'s copy has been
    // derived since the self-counts slice; this one is its twin and was not, so adding a
    // first claim to `the-electrolyte-starves` moved one and left the other saying five
    // and sixteen. Same shape as the `grid` pair above: a sentence next to a derived
    // sentence is not itself derived.
    Tally {
        prose: Prose::ThisTest,
        // Phrased so the count is never the subject of a verb. "{W} steps carry" was the
        // first wording and it breaks at one and reads as nonsense at zero — which is
        // exactly the moment the work it describes is finished, so a tally that cannot
        // survive its own count reaching zero forces a rewrite at the worst time.
        phrase: "carrying neither a claim nor a ledger entry: {w}. With claimed sentences \
                 checked and the rest of the prose free: {w}.",
        of: &[n_unledgered_unclaimed, n_unledgered_claimed],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "today it is {w} steps and {n} numbers",
        of: &[n_ledgered, n_ledgered_numerals],
    },
    // The three counts in the "what this does NOT cover" entry about the unledgered steps.
    // All three were stale — "fifteen" steps unreached and "nine" closed, on a ledger of ten
    // and fourteen — and all three are in WORDS, which is how they survived the self-counts
    // slice: it read digits. They sit two lines from a count that WAS derived (the
    // claimed/unclaimed split below), which is the third instance of this file's own lesson
    // that a sentence beside a derived sentence is not itself derived.
    Tally {
        prose: Prose::ThisTest,
        phrase: "in the steps the ledger has not reached — {w} of them",
        of: &[n_unledgered],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "has now closed {w} whole steps — but only {w}",
        of: &[n_ledgered, n_ledgered],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "names what is left — {w} of the {w} — one line each",
        of: &[n_unledgered, n_lessons],
    },
    // Check 6's arm count, stated in this test's docs and derived in the claims file's
    // header ("{W} accountings, and no {o}:"). Correct today; undeclared until now, which is
    // exactly the position the ledger-arm count was in one slice ago.
    Tally {
        prose: Prose::ThisTest,
        phrase: "the last of check 6's {w} accounting arms",
        of: &[n_accounting_arms],
    },
    // The ledger's arm count, stated once in each file. Neither was derived until this
    // slice, and both were stale the same way: three sentences saying an arm was missing
    // after it had been built, fixed by hand one slice earlier with nothing to stop the
    // next one. The count is a word in both, which is why the earlier self-count pass —
    // which reads digits — never saw them.
    Tally {
        prose: Prose::ThisTest,
        phrase: "{W} arms exist",
        of: &[n_ledger_arms],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "and there are {w} of them",
        of: &[n_ledger_arms],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "so several per step is now the normal case — step 18 has {w}.",
        of: &[n_arms_on_step_18],
    },
    Tally {
        prose: Prose::ThisTest,
        phrase: "it buys the thing step 18 needs: {w} arms branching off one mark",
        of: &[n_mark_arms_on_step_18],
    },
    // --- and what the ledger's own entries say about their steps ----------------
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "claims: {n}, on {w} arms.",
        of: &[n_claims_on_step_18, n_arms_on_step_18],
    },
    Tally {
        prose: Prose::ClaimsFile,
        phrase: "{n} claims on {w} arms, and the step is the eleventh lesson",
        of: &[n_claims_on_what_protection_costs, n_arms_on_step_11],
    },
];

/// The self-counts left out of [`TALLIES`], each with the reason no derivation exists.
///
/// Each phrase is required to still be in its file, which is what stops this being a
/// free-text waiver: reword the sentence and the entry reddens, so retiring one is a
/// decision somebody writes down.
const NOT_DERIVED: &[NotDerived] = &[
    NotDerived {
        prose: Prose::ClaimsFile,
        phrase: "reads this file and checks each entry up to SIX independent ways",
        because: "the number of checks is a property of the test code, not of any data \
                  this file parses. Counting `#[test]` functions would not give it \
                  either: two of the six run inside another test's loop.",
    },
    NotDerived {
        prose: Prose::ClaimsFile,
        phrase: "Four separate slices have found numbers in it",
        because: "a count of past slices, settled by git history rather than by the file.",
    },
    // This test's twin of the sentence above. Same count, same reason, and undeclared until
    // the sweep that found the four stale counts in these docs — three of which sat within
    // two lines of a derived one.
    NotDerived {
        prose: Prose::ThisTest,
        phrase: "Four slices found numbers in that prose",
        because: "the same count of past slices as the claims file's copy, settled by git \
                  history rather than by either file.",
    },
    // The count of checks, stated three times in this test's docs and once in the claims
    // file. Not derivable for the reason the claims file's entry gives, so each sentence
    // that carries the number is pinned here instead. The fourth statement of it — "read its
    // header for the four checks" — was the one that went stale, and it was reworded to
    // carry no number at all rather than waived.
    NotDerived {
        prose: Prose::ThisTest,
        phrase: "The six checks, and why none of them is redundant",
        because: "the number of checks is a property of the test code, not of data either \
                  file parses — the claims file's `up to SIX independent ways` entry has \
                  the argument in full.",
    },
    NotDerived {
        prose: Prose::ThisTest,
        phrase: "Beside all six sits **the ledger**",
        because: "the same count of checks, in the sentence that sets the ledger beside \
                  them.",
    },
    NotDerived {
        prose: Prose::ThisTest,
        phrase: "The five above are all about the number a claim *spells*",
        because: "the same count of checks less the one being described, so it moves \
                  whenever that one does and is no more derivable than it is.",
    },
    NotDerived {
        prose: Prose::ClaimsFile,
        phrase: "All seven are about a CLAIM",
        because: "the six checks plus the tolerance rule behind the second — a count of \
                  test code, as above, and the one place this file states the total.",
    },
    NotDerived {
        prose: Prose::ClaimsFile,
        phrase: "fourteen of the twenty-four steps were in exactly that position",
        because: "past tense: how many steps had no claim when the ledger was written. \
                  The present-tense version of it IS derived, in this test's own docs. \
                  Note that the `twenty-four` in it is FROZEN with the rest of the \
                  sentence — a twenty-fifth lesson reddens the tallies that count \
                  lessons today and must not be `fixed` here, where it would silently \
                  restate a past measurement as a present one.",
    },
    NotDerived {
        prose: Prose::ClaimsFile,
        phrase: "measured all fourteen by hand and found two defects in about 145 \
                 measurement-shaped numbers",
        because: "a hand measurement recorded in docs/plans/path-prose-ledger.md, and \
                  `about 145` is an estimate no scan reproduces.",
    },
    NotDerived {
        prose: Prose::ClaimsFile,
        phrase: "accounts for a third of the path's numbers and means nothing",
        because: "an estimate about a generous rule that was never built, so there is \
                  nothing to count it against.",
    },
    NotDerived {
        prose: Prose::ThisTest,
        phrase: "fourteen steps had no claim at all when this was written",
        because: "past tense, as above.",
    },
    // The claims file's twin of the sentence above, which was stale in both halves —
    // "two of these three steps state four that way", written when three steps were
    // ledgered and left alone through two more. A waiver rather than a derivation for the
    // reason above; what this adds is that rewording it now reddens.
    NotDerived {
        prose: Prose::ClaimsFile,
        phrase: "four of the ledgered steps state six MEASUREMENTS that way",
        because: "the same count, in the file's own header, and unreachable for the same \
                  reason. The word MEASUREMENTS is the scope step 9 made necessary: it \
                  states a word numeral too and it is a count of the path, not a reading.",
    },
];

/// Render one tally's phrase with its derived numbers in place.
fn render(tally: &Tally, facts: &Facts) -> String {
    let mut out = String::new();
    let mut rest = tally.phrase;
    let mut next = tally.of.iter();
    while let Some(open) = rest.find('{') {
        let close = open
            + rest[open..]
                .find('}')
                .unwrap_or_else(|| panic!("tally `{}` has an unclosed `{{`", tally.phrase));
        out.push_str(&rest[..open]);
        let of = next.next().unwrap_or_else(|| {
            panic!(
                "tally `{}` has more placeholders than derivations",
                tally.phrase
            )
        });
        let n = of(facts);
        out.push_str(&match &rest[open..=close] {
            "{n}" => n.to_string(),
            "{w}" => word(n, HEADER_WORDS, tally.phrase),
            "{o}" => word(n, HEADER_ORDINALS, tally.phrase),
            "{W}" => {
                let w = word(n, HEADER_WORDS, tally.phrase);
                let mut c = w.chars();
                match c.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                    None => w,
                }
            }
            other => panic!(
                "tally `{}` uses `{other}`, which is not a placeholder this check knows. \
                 Use `{{n}}` for digits, `{{w}}` for letters, `{{W}}` to open a \
                 sentence, `{{o}}` for a position.",
                tally.phrase
            ),
        });
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    assert!(
        next.next().is_none(),
        "tally `{}` gives more derivations than it has placeholders",
        tally.phrase
    );
    out
}

fn word(n: usize, table: &[(usize, &'static str)], phrase: &str) -> String {
    table
        .iter()
        .find(|(v, _)| *v == n)
        .unwrap_or_else(|| {
            panic!(
                "the tally `{phrase}` derives {n}, and this check has no English for that \
                 number. Add it to HEADER_WORDS / HEADER_ORDINALS, or write the sentence \
                 in digits."
            )
        })
        .1
        .to_string()
}

/// Every count these files state about themselves is derived from their contents.
///
/// The measurements in this file age the moment they are taken, and its own prose is no
/// exception: `docs/plans/path-ambient-arm.md` found the header's tallies stale by five
/// slices, re-derived them by hand, and left the same hole behind — nothing asserts them,
/// so the next slice starts the drift again. That is what this closes. The phrase is
/// declared, the number is derived, and a count that moves without its sentence moving
/// fails here by name.
#[test]
fn every_count_these_files_state_about_themselves_is_derived() {
    let facts = Facts::gather();

    // One vocabulary, not two: where the header's words and the prose's overlap, they
    // have to mean the same number.
    for (n, w) in HEADER_WORDS {
        if let Some((_, v)) = WORD_NUMERALS.iter().find(|(word, _)| word == w) {
            assert_eq!(
                *v, *n as f64,
                "HEADER_WORDS says `{w}` is {n} and WORD_NUMERALS says it is {v}. Two \
                 tables spelling one word two ways is how a number comes to mean \
                 different things in the prose and in the check."
            );
        }
    }

    for prose in [Prose::ClaimsFile, Prose::ThisTest] {
        let text = flattened(prose);

        for tally in TALLIES.iter().filter(|t| t.prose == prose) {
            let expected = render(tally, &facts);
            let hits = text.matches(expected.as_str()).count();
            assert_eq!(
                hits,
                1,
                "{}: this file's own prose should say\n  `{expected}`\nand {}.\n\
                 The number is derived from the file's contents, so a mismatch means the \
                 contents moved and the sentence describing them did not — or the \
                 sentence was reworded and this tally's phrase (`{}`) has to follow it. \
                 Fix the prose, not the derivation: the derivation is the fact.",
                prose.name(),
                match hits {
                    0 => "it does not".to_string(),
                    n => format!("it says it {n} times, which pins none of them"),
                },
                tally.phrase,
            );
        }

        for waived in NOT_DERIVED.iter().filter(|w| w.prose == prose) {
            // A phrase quoted in this table is itself in this test's source, so when the
            // sentence lives here the table's own copy is one of the matches.
            let own = usize::from(prose == Prose::ThisTest);
            let hits = flattened(prose).matches(waived.phrase).count();
            assert!(
                hits > own,
                "{}: NOT_DERIVED says this file states `{}` and it does not (any more).\n\
                 That entry is the written-down reason a self-count here is NOT checked, \
                 so it may not outlive the sentence it excuses. If the sentence is gone, \
                 drop the entry; if it was reworded, follow it; if it became derivable, \
                 move it to TALLIES.",
                prose.name(),
                waived.phrase,
            );
        }
    }
}

/// The comment beside one step's entry in a `[ledger]` list.
fn ledger_note<'a>(raw: &'a str, step: &str) -> &'a str {
    let entry = format!("\"{step}\",");
    let line = raw
        .lines()
        .find(|l| l.trim_start().starts_with(&entry))
        .unwrap_or_else(|| {
            panic!(
                "web/path-claims.toml's `[ledger]` lists `{step}`, and no line in the file \
                 starts with `{entry}`. The list was reformatted onto one line, and the \
                 per-step counts beside each entry cannot be read any more."
            )
        });
    line.split_once('#').map_or("", |(_, note)| note)
}

/// The first whole number in some text, if it has one.
fn first_count(text: &str) -> Option<usize> {
    written_numbers(text)
        .first()
        .and_then(|w| w.token.parse().ok())
}

/// The count beside every `spelled` and `word_blind` entry is what a scan finds there.
///
/// [`every_count_beside_a_ledger_entry_is_derived`] one list over, and written in the same
/// slice as the lists themselves rather than after the first count went stale. Those entries
/// carry a number each, and a number an author types beside a name is the shape this repo
/// has now found stale four separate times — twice in words, where no digit scanner could
/// see it.
///
/// **`word_blind` is checked too, and that is the half that matters.** A step nobody scans
/// has no other statement of how much English it states, so the count beside it is the only
/// thing standing between "declared, with its size" and "listed and forgotten". It is also
/// the number that moves when the SCANNER learns a shape rather than when the prose changes
/// — the day an article reader lands, several of these zeroes stop being zeroes, and this
/// is what says so.
#[test]
fn every_count_beside_a_word_list_entry_is_derived() {
    let facts = Facts::gather();
    let raw = read(&Prose::ClaimsFile.path());

    for (list, steps) in [
        ("spelled", &facts.ledger.spelled),
        ("word_blind", &facts.ledger.word_blind),
    ] {
        let block = word_list_block(&raw, list);
        for step in steps {
            let note = list_note(block, list, step);
            let stated = first_count(note).unwrap_or_else(|| {
                panic!(
                    "`[ledger].{list}` lists `{step}` with no count beside it. Write \
                     `# {}` there: for a scanned step it is what has to be accounted for, \
                     and for a word-blind one it is the whole of what this file says about \
                     the English it states.",
                    facts.spelled_in(step)
                )
            });
            assert_eq!(
                stated,
                facts.spelled_in(step),
                "web/path-claims.toml: the `{list}` entry for `{step}` says {stated}, and \
                 the scanner finds {} spelled quantities in that step's prose. Note that \
                 this count moves when the SCANNER changes as well as when the prose does \
                 — a new shape reads phrases that were invisible, which is a red here and \
                 is meant to be.",
                facts.spelled_in(step),
            );
        }
    }
}

/// The lines of one of `[ledger]`'s lists, as raw text.
///
/// [`ledger_note`] searches the whole file for the first line starting with the step's name,
/// which is right for `steps` (it comes first) and wrong for anything below it: every step
/// in `spelled` is also in `steps`, so a whole-file search would read the ledger's count and
/// compare it against a scan of the words. The block has to be cut first.
fn word_list_block<'a>(raw: &'a str, list: &str) -> &'a str {
    let open = format!("\n{list} = [");
    let from = raw
        .find(&open)
        .unwrap_or_else(|| panic!("web/path-claims.toml has no `{list} = [` list"))
        + open.len();
    let len = raw[from..]
        .find("\n]")
        .unwrap_or_else(|| panic!("`{list} = [` is never closed"));
    &raw[from..from + len]
}

fn list_note<'a>(block: &'a str, list: &str, step: &str) -> &'a str {
    let entry = format!("\"{step}\",");
    let line = block
        .lines()
        .find(|l| l.trim_start().starts_with(&entry))
        .unwrap_or_else(|| {
            panic!(
                "`[ledger].{list}` lists `{step}` and no line of that list starts with \
                 `{entry}`. The list was reformatted onto one line and the per-step counts \
                 cannot be read any more."
            )
        });
    line.split_once('#').map_or("", |(_, note)| note)
}

/// Every per-step count in the ledger's two lists is derived too.
///
/// These are the tallies that go stale first, because each one moves whenever a claim is
/// added to the step beside it — and two of them were wrong when this was written, both
/// off by exactly the arms on those steps. Nothing here is declared: the comment says a
/// number, the file says a number, and they have to be the same one.
#[test]
fn every_count_beside_a_ledger_entry_is_derived() {
    let facts = Facts::gather();
    let raw = read(&Prose::ClaimsFile.path());

    for step in &facts.ledger.steps {
        let note = ledger_note(&raw, step);
        let stated = first_count(note).unwrap_or_else(|| {
            panic!(
                "ledgered step `{step}` has no count beside it. A ledgered step's whole \
                 prose is scanned, so how many numerals that is belongs next to the \
                 entry: write `# {} numbers: …`.",
                facts.numerals_in(step)
            )
        });
        assert_eq!(
            stated,
            facts.numerals_in(step),
            "web/path-claims.toml: the `[ledger]` entry for `{step}` says {stated}, and \
             that step's prose prints {} numerals. The scan reads the prose, so the \
             comment is what is wrong — unless a number left the lesson, in which case \
             this is the notice that it did.",
            facts.numerals_in(step),
        );
    }

    for step in &facts.ledger.unledgered {
        let note = ledger_note(&raw, step);
        let actual = facts.claims_on(step);
        match note.split_once("claims:").and_then(|(_, n)| first_count(n)) {
            Some(stated) => assert_eq!(
                stated, actual,
                "web/path-claims.toml: the `unledgered` entry for `{step}` says it \
                 carries {stated} claims and it carries {actual}.\n\
                 This is the count that moves every time a claim is added, and it is the \
                 file's only per-step statement of how much of an unledgered step is \
                 checked at all. Note that ARMS are not claims: a step with an arm on it \
                 has one more `step = ` line in this file than it has claims."
            ),
            None => assert_eq!(
                actual, 0,
                "web/path-claims.toml: the `unledgered` entry for `{step}` says nothing \
                 about claims, and the step carries {actual}. Every entry that carries \
                 claims says how many — write `# claims: {actual}` beside it, so the one \
                 statement this file makes about a step it does not scan stays true."
            ),
        }
    }
}

/// The `cc_cv_a` half of an arm's override checks, split out so its three refusals can be
/// priced.
///
/// Every one of them is unreachable from `path-claims.toml` the moment it works — the one
/// arm in the file that sets this field satisfies all three — which is the shape
/// [`an_on_arm_may_not_read_a_file_off_an_arm_that_picks_none`] established a fence has to be asked about directly
/// rather than left as a paragraph. See [`an_arm_may_not_type_into_both_current_boxes`] and
/// [`a_cc_cv_current_needs_a_cc_cv_step`].
///
/// `pack_lesson` rather than the declaring one, on the same terms as every other override
/// check beside it: what the reader is holding when they type is the lesson the pack comes
/// from.
fn check_cc_cv_current(arm: &Arm, pack_lesson: &Lesson, instruction: &str) {
    let Some(cc_cv_a) = arm.cc_cv_a else {
        return;
    };
    let spelled = Arm::spelled(cc_cv_a);
    assert!(
        contains_number(instruction, &spelled),
        "arm `{}` on step `{}` types {cc_cv_a} A into the CC-CV charge-current box, and \
         `{spelled}` does not appear as a number in its instruction:\n  {}\n\
         Same contract as the simple box. Note the sign: this box charges with a POSITIVE \
         number, so an instruction that spells the current the page's other way round \
         fails here — see `contains_number`.",
        arm.name,
        arm.step,
        arm.instruction
    );
    assert!(
        arm.demand_a.is_none(),
        "arm `{}` on step `{}` sets both `demand_a` and `cc_cv_a`. Those are two different \
         boxes and `applyDemandMode` shows one at a time, so no reader can type in both — \
         and `arm_control_value` would have to prefer one, which is a preference nothing \
         in the page justifies.",
        arm.name,
        arm.step
    );
    let Prog::CcCv { i, .. } = pack_lesson.demand else {
        panic!(
            "arm `{}` on step `{}` types into the CC-CV charge-current box, but the lesson \
             its pack comes from (`{}`) is not on the CC-CV demand mode. That box is not \
             on screen there. An arm that switched the mode would be a different \
             trajectory and not an override — see `Arm::cc_cv_a`.",
            arm.name, arm.step, pack_lesson.id
        );
    };
    assert!(
        (cc_cv_a - i).abs() > f64::EPSILON,
        "arm `{}` on step `{}` declares a charge current of {cc_cv_a} A, which is what the \
         lesson its pack comes from (`{}`) already asks for. An override that changes \
         nothing is a control the reader was never asked to touch.",
        arm.name,
        arm.step,
        pack_lesson.id
    );
}

/// The same four questions of the pulse group's current box. See [`Arm::pulse_a`].
///
/// A sibling of [`check_cc_cv_current`] rather than a branch inside it, because the two boxes
/// are refused on *different* steps — one wants a CC-CV program and the other a pulse train —
/// and a single function taking "whichever current is set" would have to name the mode it was
/// looking for, which is the thing being checked.
fn check_pulse_current(arm: &Arm, pack_lesson: &Lesson, instruction: &str) {
    let Some(pulse_a) = arm.pulse_a else {
        return;
    };
    let spelled = Arm::spelled(pulse_a);
    assert!(
        contains_number(instruction, &spelled),
        "arm `{}` on step `{}` types {pulse_a} A into the pulse group's current box, and \
         `{spelled}` does not appear as a number in its instruction:\n  {}\n\
         Same contract as the other two boxes: the current and the sentence that tells the \
         reader to type it are two statements of one fact.",
        arm.name,
        arm.step,
        arm.instruction
    );
    assert!(
        arm.demand_a.is_none() && arm.cc_cv_a.is_none(),
        "arm `{}` on step `{}` sets `pulse_a` alongside another current box. Those are two \
         different boxes and `applyDemandMode` shows one at a time, so no reader can type in \
         both — and `arm_control_value` would have to prefer one, which is a preference \
         nothing in the page justifies.",
        arm.name,
        arm.step
    );
    let Prog::Pulse { i, .. } = pack_lesson.demand else {
        panic!(
            "arm `{}` on step `{}` types into the pulse group's current box, but the lesson \
             its pack comes from (`{}`) is not on the Pulse demand mode. That box is not on \
             screen there, and the simple box that IS on screen would replace the program \
             rather than retime it — see `Arm::pulse_a`.",
            arm.name, arm.step, pack_lesson.id
        );
    };
    assert!(
        (pulse_a - i).abs() > f64::EPSILON,
        "arm `{}` on step `{}` declares a pulse current of {pulse_a} A, which is what the \
         lesson its pack comes from (`{}`) already runs. An override that changes nothing is \
         a control the reader was never asked to touch.\n\
         Note which lesson that is: on a `pack_from` arm the box the reader is holding is the \
         OTHER lesson's, so this compares against the pack's lesson and not against the step \
         the arm is declared on. Step 14's own box is 15.459 A and its walk to the circuit \
         types the same number — identical to this step, and a threefold change to the lesson \
         it is typed into.",
        arm.name,
        arm.step,
        pack_lesson.id
    );
}

/// An arm may not type into the pulse box and another current box at once.
///
/// [`an_arm_may_not_type_into_both_current_boxes`]'s third case, and unreachable from the
/// claims file for the same reason: no arm sets two.
#[test]
#[should_panic(expected = "alongside another current box")]
fn an_arm_may_not_type_into_the_pulse_box_and_another() {
    let lessons = lessons();
    let lesson = lessons
        .iter()
        .find(|l| l.id == "circuit-repeats-itself")
        .expect("step 12 is still in the path");
    let mut arm = walkable_probe(&lesson.id, &lesson.id, Start::Restart);
    arm.pack_from = None;
    arm.instruction = "type the same 15.459 A".to_string();
    arm.demand_a = Some(15.459);
    arm.pulse_a = Some(15.459);
    check_pulse_current(&arm, lesson, &arm.instruction.clone());
}

/// And it may only type into the pulse box on a step whose pack lesson runs a pulse train.
///
/// [`a_cc_cv_current_needs_a_cc_cv_step`] one box over, and the failure it prices is worse
/// than that one's: [`arm_prog`]'s `(None, Some(_), _, other)` arm hands back the step's own
/// program, so such an arm would be the lesson under a second name while its claims read as
/// though a current had been typed.
#[test]
#[should_panic(expected = "not on the Pulse demand mode")]
fn a_pulse_current_needs_a_pulse_step() {
    let lessons = lessons();
    let lesson = lessons
        .iter()
        .find(|l| l.id == "looks-fine-from-outside")
        .expect("step 15 is still in the path");
    let mut arm = walkable_probe(&lesson.id, &lesson.id, Start::Restart);
    arm.pack_from = None;
    arm.instruction = "type the same 15.459 A".to_string();
    arm.pulse_a = Some(15.459);
    check_pulse_current(&arm, lesson, &arm.instruction.clone());
}

/// [`Tie::Magnitude`] refuses a value that is not already negative.
///
/// Unreachable from `path-claims.toml` for [`an_on_arm_may_not_read_a_file_off_an_arm_that_picks_none`]'s reason —
/// a rule is code, and the one `Magnitude` in the vocabulary wraps the ambient slider at
/// -5 °C, which satisfies the fence. So the question is asked directly rather than left to
/// a paragraph: on a positive value this wrapper is the tie it wraps with extra words, and
/// a sentence accounted through it would be green for a reason the sentence does not state.
///
/// Step 11's own step-side ambient stands in for that positive value: the slider the lesson
/// block arrives with is +25 °C, and it is the number the *arm* moves away from.
#[test]
#[should_panic(expected = "not a negative number")]
fn a_magnitude_refuses_a_value_that_is_not_negative() {
    let lessons = lessons();
    let from = lessons
        .iter()
        .find(|l| l.id == "what-protection-costs")
        .expect("step 11 is still in the path");
    let text = ascii_minus(&from.text);
    let numbers = written_numbers(&text);
    let cover = vec![None; numbers.len()];
    let arms = arms();
    let ctx = SentenceCtx {
        step: from.id.as_str(),
        text: &text,
        numbers: &numbers,
        cover: &cover,
        at: 0,
        all: &[],
        arms: &arms,
        derived: &[],
    };
    let tie = Tie::Magnitude(&Tie::Setting(Control::Ambient));
    let (scenario, chemistry) = (
        scenario_toml(&from.scenario),
        chemistry_toml(&from.scenario),
    );
    tie_values(&tie, from, &lessons, &scenario, &chemistry, &ctx);
}

/// An arm may not type into both current boxes.
///
/// `applyDemandMode` shows one field group at a time, so no reader can set both — and if
/// one ever did, [`arm_control_value`] would have to prefer one, which is a preference
/// nothing in the page justifies. Unreachable from the claims file the moment the fence
/// works, which is why it is priced here.
#[test]
#[should_panic(expected = "two different boxes")]
fn an_arm_may_not_type_into_both_current_boxes() {
    let lessons = lessons();
    let lesson = lessons
        .iter()
        .find(|l| l.id == "what-protection-costs")
        .expect("step 11 is still in the path");
    let mut arm = walkable_probe(&lesson.id, &lesson.id, Start::Restart);
    arm.pack_from = None;
    arm.instruction = "ask for 6 A".to_string();
    arm.demand_a = Some(-2.0);
    arm.cc_cv_a = Some(6.0);
    check_cc_cv_current(&arm, lesson, &arm.instruction.clone());
}

/// And it may only type into the CC-CV box on a step that is on the CC-CV demand mode.
///
/// The box is not on screen anywhere else, so an arm setting it there would be describing a
/// trajectory the page cannot reach — and [`arm_prog`]'s second match arm, which exists so a
/// fourth demand program cannot silently drop the field, would hand back the step's own
/// program and the arm would be the step under a second name.
#[test]
#[should_panic(expected = "not on the CC-CV demand mode")]
fn a_cc_cv_current_needs_a_cc_cv_step() {
    let lessons = lessons();
    let lesson = lessons
        .iter()
        .find(|l| l.id == "circuit-repeats-itself")
        .expect("step 12 is still in the path");
    let mut arm = walkable_probe(&lesson.id, &lesson.id, Start::Restart);
    arm.pack_from = None;
    arm.instruction = "ask for 6 A".to_string();
    arm.cc_cv_a = Some(6.0);
    check_cc_cv_current(&arm, lesson, &arm.instruction.clone());
}

/// `v_floor_s` refuses a run that STOPS on its lowest row.
///
/// Unreachable from `path-claims.toml` for [`a_magnitude_refuses_a_value_that_is_not_negative`]'s
/// reason: the one claim that reads this quantity is on step 20, whose run carries 340 more
/// rows of flat trace after the floor arrives. So the question is asked directly, with the
/// step's own run truncated at the row the fence is about — a run that would answer 4229.5 s
/// exactly as the real one does, while having no evidence at all that the fall had stopped
/// rather than paused.
///
/// **The `expected` fragment belongs to this fence and to nothing else in the file**, which is
/// the trap `docs/plans/path-instant-tag.md` recorded: a `should_panic` satisfied by a phrase
/// the test itself supplies — a lookup's own `expect`, say — passes while proving nothing.
#[test]
#[should_panic(expected = "is on its final row")]
fn a_floor_refuses_a_run_that_ends_on_it() {
    let lessons = lessons();
    let lesson = lessons
        .iter()
        .find(|l| l.id == "past-empty")
        .expect("step 20 is still in the path");
    let mut run = run(lesson, None, &[], &lessons);
    let floor = run
        .rows
        .iter()
        .map(|r| r.telemetry.v_terminal)
        .fold(f64::MAX, f64::min);
    let at = run
        .rows
        .iter()
        .position(|r| (r.telemetry.v_terminal - floor).abs() < 1e-9)
        .expect("the collapse reaches its floor before the mark");
    run.rows.truncate(at + 1);
    measure("v_floor_s", &run, 0.0, false, lesson.until_s);
}

/// And it refuses a run whose lowest row is not a floor at all.
///
/// The charge leg is that run and it needs no construction: it falls to −0.064 V at 4229.5 s,
/// sits there while the reader retypes the demand box, and then climbs to 2.07 V. A minimum
/// is not a floor unless the trace is still at it when the run ends, and that is the half of
/// "the fall simply stops" a first-match search cannot see.
#[test]
#[should_panic(expected = "so it did not stay there")]
fn a_floor_refuses_a_minimum_the_run_climbs_out_of() {
    let lessons = lessons();
    let lesson = lessons
        .iter()
        .find(|l| l.id == "past-empty")
        .expect("step 20 is still in the path");
    let arms = arms();
    let leg = arms
        .iter()
        .find(|a| a.step == "past-empty" && a.name == "charge leg")
        .expect("step 20 still declares its charge leg");
    let run = run(lesson, Some(leg), &[], &lessons);
    measure("v_floor_s", &run, 0.0, false, lesson.until_s);
}

/// `deficit_falls_below_pts` refuses a run where the debt never comes back.
///
/// Step 20's own run to the mark is that run, and it is the trajectory the SAME step's prose
/// is mostly about: the debt climbs to 6.121 points and the run ends there. Only the charge
/// leg repays it, which is why the claim reading this quantity names an arm — and this is
/// what stops a claim that forgot to.
#[test]
#[should_panic(expected = "never comes back through")]
fn a_repayment_refuses_a_run_that_never_repays() {
    let lessons = lessons();
    let lesson = lessons
        .iter()
        .find(|l| l.id == "past-empty")
        .expect("step 20 is still in the path");
    let run = run(lesson, None, &[], &lessons);
    measure(
        "deficit_falls_below_pts:2",
        &run,
        0.0,
        false,
        lesson.until_s,
    );
}
