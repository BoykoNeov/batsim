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
//! read its header for the four checks and why the literal is stored as a string
//! rather than formatted from the value.
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
//! requires every numeral in it to be tied to something, claimed or not. Numeral, not
//! number: a quantity spelled in English is invisible to it, and two ledgered steps state
//! four measurements that way ("about half a point across the whole grid", "a gap of about
//! three points" — the last of which now carries two claims of its own, through
//! [`WORD_NUMERALS`], while remaining invisible to this scan).
//! A ledgered step is digits-closed, which is less than closed. Check 6 can only
//! reach the sentences a claim already quotes, and fourteen steps had no claim at all when
//! this was written — which is how six figures in step 19 went stale, and how a contrast in
//! step 14 that never existed survived, both under a fully green suite. Seven steps are
//! still in that position. Coverage is opt-in per step
//! (`[ledger]` in `path-claims.toml`) and today it is three steps and fourteen numbers,
//! all of them scenario constants. One arm exists, the scenario file; the rest of the
//! design is in `docs/plans/path-prose-ledger.md`.
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
//!   `ambient_c` is **restart-side only**: [`run`] keeps one [`Env`] for the whole
//!   trajectory, which is sound exactly because an ambient override implies
//!   [`Start::Restart`] and a restart arm has no pre-mark segment. Step 8 asks a reader to
//!   raise the slider *at* the mark, and that arm needs the environment split in two — and
//!   the sentence that would pay for it prints `20 K` and `2.7×`, both figures derived from
//!   their siblings, so it is blocked on the accounting arm below as well. Also left out is
//!   a sentence comparing two *scenario files* (step 16's 1 C rerun of both porous models),
//!   which is a second pack rather than a second trajectory — [`run`] builds one pack per
//!   arm.
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
//! * **Sentences no claim is about, in the twenty-one steps the ledger has not reached.**
//!   Check 6 closed the half of this that lived *inside* a claimed literal, and the ledger
//!   has now closed three whole steps — but only three. Five steps carry neither a claim
//!   nor a ledger entry and are untouched by anything here; the other sixteen have their
//!   claimed sentences checked and the rest of their prose free. `[ledger].unledgered`
//!   names all twenty-one, one line each, so this list cannot go quietly out of date.
//!   What the remaining steps need is arms the ledger has not got — chemistry constants,
//!   ordinals naming other steps, and figures derived from other figures in the same
//!   sentence. Both of the *harness* capabilities that list used to name have landed — the
//!   zero-length probe, and instructed control changes — and so has the first of the
//!   accounting arms: [`Accounted::Setting`], which is what let step 18's headline be
//!   claimed after three slices of being the worked example of a sentence blocked on this
//!   check rather than on the harness. Two arms of that taxonomy are still missing, and
//!   both were met head-on by the slice that added the first: a chemistry constant, and a
//!   figure derived from its siblings in the same sentence. Each cost a literal that had
//!   to stop short of the fragment naming it. Check 6 could refuse a waiver variant
//!   because its claimed sentences happen to need none; a whole-prose ledger cannot, and
//!   it still refuses one.
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
/// exactly what step 18's headline turns on. A uniform particle does not read a hard zero:
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
/// **`surface gap` used to sit beside it and no longer does.** It is per-cell in the same
/// way — `Pack::cell` rather than `Telemetry` — but it carries no throttle, so unlike
/// `past empty` it *does* have a value at a given simulation time. [`Row`] now carries it
/// and this renders it, which is what lets step 18's headline (`0.00 / 0.00` before the
/// reader presses Run) be a display claim rather than a number with no panel behind it.
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

/// The clock's four branches, at and around each boundary.
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
    /// The `surface gap` row's two numbers, bulk minus surface on each electrode, as
    /// fractions — `None` on an equivalent circuit, which has no electrodes.
    ///
    /// Per-cell like [`Self::deficit_max`] and read the same way, but **unlike** it this
    /// one is mirrorable: `past empty` is sampled on a 250 ms wall-clock throttle and this
    /// row is not, so "what does that row show at simulation time t" has an answer. Cell
    /// `(0, 0)` because the page's readout reads `cells[0]` — the packs that have this
    /// quantity are 1S1P, which is a fact the readout's own doc comment turns on.
    surface_gap: Option<(f64, f64)>,
    /// What the BMS had **measured** as of the end of this step — `None` on a pack with no
    /// BMS, which has no sensors at all.
    ///
    /// The first thing in this file that is not ground truth. Every other field here is
    /// read off the engine's own state; this one is read off the only thing the protection
    /// logic is allowed to see, which is CLAUDE.md's eighth principle made measurable.
    sensed: Option<Sensed>,
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
        sampled_at_s: frame.sampled_at_s,
    })
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
fn deficit_range(pack: &Pack) -> (f64, f64) {
    let mut worst = 0.0f64;
    let mut best = f64::INFINITY;
    for s in 0..usize::from(pack.series()) {
        for p in 0..usize::from(pack.parallel()) {
            let cell = pack
                .cell(s, p)
                .unwrap_or_else(|| panic!("pack has no cell at {s}S{p}P"));
            worst = worst.max(cell.soc_deficit);
            best = best.min(cell.soc_deficit);
        }
    }
    (best, worst)
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
/// of how the browser scheduled a frame. `CCCV_PERIOD_S` was pinned in [`MIRRORED`] from
/// the day this file was written and nothing here read it — the mirror decided every
/// step, which is the page's *inner* function without the loop around it, and a pinned
/// constant no code consults is the "looks like coverage" shape this file rejects
/// everywhere else.
///
/// Measured cost of the gap on `two-legs`, the step where the CV leg actually engages:
/// the switch lands one step late (5420.5 s rather than 5420.0). Nothing in
/// `path-claims.toml` moved, because the only claimed CC-CV step is
/// `leg-that-is-not-there`, whose LFP cell never reaches the band at all and is therefore
/// on a constant current under either rule. That is why this was invisible, not why it
/// was harmless.
fn cccv_window_steps(dt: f64) -> u64 {
    // `Math.max(1, Math.round(CCCV_PERIOD_S / dt))`.
    let k = (10.0 / dt).round();
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
/// behaviour is not a function of the simulation alone. No claim reads past a taper
/// today; a claim that wanted to would need the page's frame schedule, not just its
/// sub-clock.
fn drive(
    pack: &mut Pack,
    prog: Prog,
    dt: f64,
    end_s: f64,
    env: &Env,
    rows: &mut Vec<Row>,
    last: &mut Option<Telemetry>,
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
                held = Some(demand_now(prog, pack, dt, last.as_ref()));
            }
            held.expect("a window's demand is decided before it is used")
        } else {
            demand_now(prog, pack, dt, last.as_ref())
        };
        let t = pack.step(dt, d, env);
        *last = Some(t);
        let (deficit_min, deficit_max) = deficit_range(pack);
        rows.push(Row {
            t_s: pack.sim_time_s(),
            telemetry: t,
            deficit_max,
            deficit_min,
            surface_gap: surface_gap(pack),
            sensed: sensed(pack),
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
fn run(lesson: &Lesson, arm: Option<&Arm>) -> Run {
    let dt = arm.and_then(|a| a.dt).unwrap_or(lesson.dt);
    let mut pack = match arm.and_then(|a| a.bms) {
        Some(bms) => build_with_bms(lesson, bms),
        None => build(lesson),
    };
    // ONE environment for the whole run, and that is sound only because an ambient override
    // implies [`Start::Restart`] — a restart arm has no pre-mark segment, so there is no
    // stretch of this trajectory that ran under the step's own slider. The day the refusal
    // in `every_arm_is_instructed_by_its_own_step` is relaxed for a sentence that drags the
    // slider *at* the mark, this becomes two: the step's before, the arm's after.
    let env = Env {
        t_ambient: arm.and_then(|a| a.ambient_c).unwrap_or(lesson.ambient_c) + K,
        t_coolant: None,
    };
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
    let probe_telemetry = pack.step(0.0, demand_now(lesson.demand, &pack, dt, None), &env);
    let (probe_deficit_min, probe_deficit_max) = deficit_range(&pack);
    let probe = Row {
        t_s: pack.sim_time_s(),
        telemetry: probe_telemetry,
        deficit_max: probe_deficit_max,
        deficit_min: probe_deficit_min,
        surface_gap: surface_gap(&pack),
        sensed: sensed(&pack),
    };
    let mut rows = Vec::new();
    let mut last: Option<Telemetry> = None;

    // A continuation arm, and a step with no arm at all, both run the step as configured
    // first. A restart arm does not: its pack is the rebuilt one and its clock starts at
    // zero, so the actions are the whole trajectory.
    if arm.is_none_or(|a| a.start == Start::Mark) {
        drive(
            &mut pack,
            lesson.demand,
            dt,
            lesson.until_s,
            &env,
            &mut rows,
            &mut last,
        );
    }

    if let Some(arm) = arm {
        // The demand box as it stands while the buttons are pressed: the arm's own current
        // if it typed one in, else whatever the step dialled in. Note that typing a current
        // replaces the step's *program* — a reader who types a number into the box on a
        // `Pulse` step has left the pulse train, which is what the box does.
        let prog = arm.demand_a.map_or(lesson.demand, Prog::Current);
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
                    drive(&mut pack, prog, dt, to_s, &env, &mut rows, &mut last);
                }
                Action::Run { to_s } => {
                    drive(&mut pack, prog, dt, *to_s, &env, &mut rows, &mut last);
                }
            }
        }
    }

    let end_snapshot = serde_json::to_string(&pack.snapshot()).expect("a pack snapshot serialises");
    Run {
        rows,
        probe,
        end_snapshot,
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
    /// number's last printed place. The default shape: 53 of 69 claims.
    Spelled,
    /// Same, but `tol` is strictly *tighter* than that rule. Safe by construction — a
    /// smaller tolerance can only redden the test — so it needs no cap, only proof that
    /// the rule it beats is still computable. Used where a claim is pinned harder than
    /// the sentence needs (a chemistry constant, an exactly-1.0 starting point), where
    /// the prose hedges a round number the engine misses by more than its last place, and
    /// for four grid times whose prose *does* spell them: half a step is tighter than the
    /// whole second those sentences print, so the number was always right and only the
    /// declaration was wrong. 14 of 69.
    Tighter,
    /// The quantity is a time the engine can only report on the step grid, and the prose
    /// spells no number in it — it gives a consequence, or a rendering of the clock.
    /// `tol` is half a timestep, which for a grid time is the tightest meaningful bound:
    /// the engine either hits the claimed step or misses by a whole one. 2 of 69, both of
    /// them claims whose [`States`] is `nothing` or `displayed`: a claim that spells its
    /// own number takes that number's rule instead, however coarse the grid is.
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
    /// The sentence prints the quantity itself. 59 of 69, and the shape to prefer: it is
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
/// **One entry, and it is required to be used.** A word nothing spells is the
/// `CCCV_PERIOD_S` shape this file rejects everywhere else, so
/// [`every_word_numeral_is_spelled_by_a_claim`] fails on a table entry no claim consults —
/// the same guard [`every_ledger_rule_is_a_phrase_and_is_used`] keeps over the ledger's
/// vocabulary. Add the next word when the next claim needs it, not before.
///
/// **This is the claim side only.** The ledger's scanner still finds *digits*
/// ([`written_numbers`]), so a word quantity in a ledgered step's prose is invisible to it
/// whether or not a claim spells it — see the note in
/// [`every_numeral_in_a_ledgered_step_is_accounted_for`].
const WORD_NUMERALS: &[(&str, f64)] = &[("three", 3.0), ("fifty", 50.0)];

/// The number `spells` names, in the unit the sentence writes it in, or `None` if the
/// string is neither digits nor a word this file knows.
///
/// The one place words are resolved. `spelled_rule_tol` and `spelled_value` both used to
/// call `parse` themselves, and a second resolution site is how a word could come to be a
/// number for the tolerance and not for the value.
fn spells_as_number(spells: &str) -> Option<f64> {
    spells.parse::<f64>().ok().or_else(|| {
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
/// bytes of source it took.
///
/// The length is not `token.len()`: the scanner below truncates `at 5769.` and `1.2.3`,
/// so the token is shorter than the run it came from. The ledger's phrase matcher needs
/// the run, because what follows a number in the sentence begins after the characters
/// that were actually there.
#[derive(Debug, Clone)]
struct Written {
    /// Byte offset of the first digit, in the text this was scanned from.
    at: usize,
    /// The number as written, sign excluded — `"4146.5"`.
    token: String,
    /// Bytes of source the run consumed, from `at`.
    len: usize,
}

/// Every number written in `text`, in order, with the offset each one starts at.
///
/// The one scanner: [`numeric_tokens`] is this with the positions dropped. Signs are not
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
            let len = cur.len();
            runs.push(Written {
                at,
                token: std::mem::take(&mut cur),
                len,
            });
        }
    }
    if !cur.is_empty() {
        let len = cur.len();
        runs.push(Written {
            at,
            token: cur,
            len,
        });
    }
    // `at 5769.` and `1.2.3` both come out of the loop above; keep the leading number.
    runs.into_iter()
        .filter_map(|w| {
            let token = match w.token.match_indices('.').nth(1) {
                Some((i, _)) => w.token[..i].to_string(),
                None => w.token.trim_end_matches('.').to_string(),
            };
            (!token.is_empty()).then_some(Written { token, ..w })
        })
        .collect()
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
    /// The sentence in that step's prose that tells the reader to make this change,
    /// verbatim. Not a paraphrase: it is checked as a substring, like a claim's `literal`.
    instruction: String,
    /// Whether the reader is continuing the run or rebuilding it. See [`Start`].
    start: Start,
    /// The current the reader types into the demand box \[A\], discharge-positive, if this
    /// arm changes it. Must be spelled inside `instruction`.
    #[serde(default)]
    demand_a: Option<f64>,
    /// The step length the reader types into the `dt` box \[s\], if this arm changes it.
    /// Must be spelled inside `instruction`, and must differ from the step's own.
    #[serde(default)]
    dt: Option<f64>,
    /// The BMS checkbox, if this arm changes it. `instruction` must name the BMS, and the
    /// value must differ from what the step configures — an override that changes nothing
    /// is a declaration with no fact under it.
    #[serde(default)]
    bms: Option<bool>,
    /// The ambient slider \[°C\], if this arm drags it. Must be spelled inside
    /// `instruction`, and must differ from the step's own.
    ///
    /// Celsius, because that is what the slider, the lesson block and the prose all speak;
    /// it becomes kelvin at the one place it reaches [`Env`], like every other temperature
    /// this file reads out of the page.
    ///
    /// **Restart only, and that is a scoping refusal rather than a fidelity one** — see the
    /// assertion in [`every_arm_is_instructed_by_its_own_step`], which says what relaxing it
    /// would cost and which sentence would pay.
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
    /// Which steps have their whole prose scanned. Required, with no serde default: an
    /// absent `[ledger]` would read as "no step is ledgered and none needs listing", which
    /// is the state this contract exists to end.
    ledger: Ledger,
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
        // The debt below empty, in points of charge — the units the prose and the `past
        // empty` row both speak, so a claim reads as the sentence does. Ground truth, and
        // value-only: that row is sampled on a wall clock, so no claim measured here may
        // name a display. See `Row::deficit_max`.
        "deficit_pts_at" => row.deficit_max * 100.0,
        // The shallow end of the same spread. No row prints it — see `Row::deficit_min` —
        // so a claim measured here may name no `display` either.
        "deficit_pts_min_at" => row.deficit_min * 100.0,
        // The two halves of the `surface gap` row, each in points of charge — the units
        // the row prints and the prose speaks. Separate quantities and not a pair, because
        // a claim states one number: step 18's whole argument is that these two do
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
        _ => return None,
    })
}

/// The quantities a claim may name, and how each is read off a run.
///
/// Kept as an explicit match rather than a registry so an unknown quantity is a
/// compile-time-shaped failure with a list in the message, not a silent skip.
fn measure(quantity: &str, run: &Run, at_s: f64, probe: bool) -> f64 {
    if let Some(v) = measure_row(quantity, run.read(at_s, probe)) {
        return v;
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
    // Two families take an argument after a colon, because the thing being read is not
    // a fixed time: a flag's arrival, and the voltage at a charge level.
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
        // The coupling CLAUDE.md refuses to let a chemistry model one half of: points
        // of resistance growth per point of capacity lost.
        "soh_ratio_at" => {
            let t = run.at(at_s);
            (t.soh_resistance - 1.0) / (1.0 - t.soh_capacity)
        }
        other => panic!(
            "path-claims.toml names a quantity this test cannot measure: `{other}`. \
             Known: v_at_mark, v_at, v_cell_min_at, v_cell_max_at, soc_at, i_at, \
             t_max_at, soh_cap_at, soh_res_at, soh_ratio_at, q_gen_at, i_rejected_at, \
             deficit_pts_at, deficit_pts_min_at, deficit_zero_s, delivered_ah, \
             soc_lost_pts_at, t_rise_k_at, soc_gap_pts_at, soc_gap_pts_min, t_gap_k_at, \
             surface_gap_neg_pts, surface_gap_pos_pts, flag_first_s:<FLAG>, \
             v_at_soc_below:<fraction>."
        ),
    }
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

fn ledger() -> Ledger {
    parse_claims_file().ledger
}

/// The declared arms. A step may have several; their names must differ.
///
/// A step used to be allowed one leg and no more, because a claim pointed at it with a
/// bare `after_mark = true` and a second leg would have been silently ignored. An arm is
/// named and a claim names it, so several per step is now the normal case — step 18 has
/// four. What must stay unique is the name: two arms sharing one would hand every claim
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
                (dt - lesson.dt).abs() > f64::EPSILON,
                "arm `{}` on step `{}` declares dt = {dt} s, which is what the step \
                 already sets. An override that changes nothing is a control the reader \
                 was never asked to touch.",
                arm.name,
                arm.step
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
            let (scenario, _) = load(&lesson.scenario);
            let as_configured = lesson.bms.unwrap_or(scenario.pack.bms.is_some());
            assert!(
                bms != as_configured,
                "arm `{}` on step `{}` sets the BMS to {bms}, which is what the step is \
                 already configured with. The arm is then the step's own run under a \
                 second name, and its claims say nothing about unchecking anything.",
                arm.name,
                arm.step
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
                (ambient_c - lesson.ambient_c).abs() > f64::EPSILON,
                "arm `{}` on step `{}` declares an ambient of {ambient_c} °C, which is \
                 where the step already leaves the slider. An override that changes \
                 nothing is a control the reader was never asked to touch.",
                arm.name,
                arm.step
            );
            assert!(
                arm.start == Start::Restart,
                "arm `{}` on step `{}` drags the ambient on a continuation. **This is a \
                 scoping refusal, not a fidelity one**, and it is the weakest of the three \
                 in this test: `$(\"ambient\").oninput` calls `applyEnv` and rebuilds \
                 nothing, so unlike the BMS checkbox the page really can do this mid-run, \
                 and unlike `dt` there is a step in the path that asks for it — step 8's \
                 \"raise the ambient slider to 45 °C and press Run\", at the mark. What \
                 stops it being built today is that `run` keeps ONE environment for the \
                 whole trajectory; a mark-side drag needs two, split at `until_s`. The \
                 sentence that would pay for the split prints `20 K` and `2.7×`, both of \
                 which are figures derived from their siblings, so it cannot be claimed \
                 until that accounting arm exists either.",
                arm.name,
                arm.step
            );
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

        let mine = run(lesson, Some(arm));
        let theirs = run(lesson, Some(twin));
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
                let grid_quantity =
                    c.quantity.starts_with("flag_first_s:") || c.quantity == "deficit_zero_s";
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

/// Every word in [`WORD_NUMERALS`] is spelled by some claim.
///
/// The table is a translation from English to a number, and a translation nothing consults
/// is coverage-shaped: it reads as "this file understands written numbers" while the one
/// word it was added for could have been deleted from the claim beside it. Same guard, same
/// argument, as [`every_ledger_rule_is_a_phrase_and_is_used`] keeps over the ledger's
/// vocabulary — and the same history behind it, `CCCV_PERIOD_S` sitting pinned and unread
/// for six slices while the mirror it was meant to guard was wrong.
#[test]
fn every_word_numeral_is_spelled_by_a_claim() {
    let all = claims();
    for (word, value) in WORD_NUMERALS {
        assert!(
            all.iter().any(|c| c.spells.as_deref() == Some(*word)),
            "WORD_NUMERALS translates `{word}` to {value} and no claim in \
             web/path-claims.toml spells it. Either the sentence it was added for was \
             reworded — in which case its claim is failing elsewhere and this entry is why \
             that is hard to see — or the word was never used. Add words when a claim needs \
             them; a table read by nothing is the `CCCV_PERIOD_S` shape."
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
    /// [`ScenarioRule`] refuses when it insists a rule name its field rather than search
    /// the file.
    ///
    /// **That trap is reasoned rather than measured**, and the distinction is this file's
    /// own: [`Lesson`] does not scrape `speed_x` at all, so the generous version cannot be
    /// built — or perturbed into existence — without adding the field first. What *was*
    /// measured is the weaker half of the same property: tying this arm to the step's own
    /// `dt` instead of to each claim's trajectory leaves the headline's `5` and `10`
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
    /// **Ambient, demand and the mark are deliberately not here.** `dt` is the only
    /// control an arm can override that a sentence in this path also prints, and an arm
    /// for a control nothing reads would be `CCCV_PERIOD_S` again: pinned, and consulted
    /// by nothing. The next sentence that prints an ambient temperature is where that arm
    /// gets built — `docs/plans/path-prose-ledger.md` sizes the rest of the taxonomy.
    Setting(f64),
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

/// How this sentence's claims account for one number printed in it, or `None`.
///
/// Shared by check 6, which runs the whole scan without an engine, and by the event fence
/// in [`every_claim_matches_the_engine`], which re-derives it for the one arm that needs a
/// trajectory to be checked. Derived rather than declared: see [`Accounted`].
fn accounting_for(
    token: &str,
    group: &[&Claim],
    lesson: &Lesson,
    arms: &[Arm],
) -> Option<Accounted> {
    // A step length one of this sentence's own claims is measured at. Compared as a
    // number rather than as text, so a sentence writing `5.0` where the box takes 5 is
    // the same setting — the token is the reader's, the tie is to the run.
    //
    // Taken first although it is tried last, because the three arms below return early and
    // this one has to be able to refuse them. See the fence under `shown`.
    let setting = group.iter().find_map(|c| {
        let dt = arm_of(arms, c).and_then(|a| a.dt).unwrap_or(lesson.dt);
        token
            .parse::<f64>()
            .ok()
            .filter(|n| (n - dt).abs() < 1e-9)
            .map(|_| dt)
    });
    // Two readings of one number, which is the hazard this whole taxonomy is arranged
    // against. Refused rather than resolved by trial order: with an order, an author who
    // meant a measurement and wrote a step length gets whichever arm happens to be tried
    // first, and the check becomes a fact about this function.
    let clash = |other: &str| {
        assert!(
            setting.is_none(),
            "step `{}`, sentence `{}`:\n  it prints `{token}`, which is both {other} and \
             the step length of a trajectory this sentence's claims read ({} s).\n\
             Two readings of one number means the accounting is decided by which arm was \
             tried first rather than by the sentence. Reword the sentence, or split the \
             literal so the two readings sit in different groups.",
            group.first().map_or("", |c| c.step.as_str()),
            group.first().map_or("", |c| c.literal.as_str()),
            setting.unwrap_or(f64::NAN),
        );
    };

    let spelled = group.iter().any(|c| {
        c.spells
            .as_deref()
            .map(|s| ascii_minus(s).trim_start_matches('-').to_string())
            .is_some_and(|s| s == token)
    });
    if spelled {
        clash("spelled by a claim on it");
        return Some(Accounted::Spelled);
    }

    let read_at = group.iter().find_map(|c| {
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
        token
            .parse::<f64>()
            .ok()
            .filter(|n| instants.iter().any(|i| (n - i).abs() < 1e-9))
            .map(|_| c.read_at_s)
    });
    if let Some(absolute_s) = read_at {
        clash("an instant a claim on it is read at");
        return Some(Accounted::ReadAt(absolute_s));
    }

    let shown = group.iter().any(|c| {
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

    for (step, literal) in sentences(&all) {
        let lesson = lessons
            .iter()
            .find(|l| l.id == step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));
        let group = sentence_group(&all, step, literal);

        for token in numeric_tokens(&ascii_minus(literal)) {
            assert!(
                accounting_for(&token, &group, lesson, &arms).is_some(),
                "step `{step}`, sentence `{literal}`:\n  it prints `{token}`, and none of \
                 the {} claim(s) on it accounts for that number.\n\
                 Tried, in order:\n  \
                 - spelled: no claim here names `{token}` in `spells`\n  \
                 - read at: no claim here is read at that instant \
                 ({:?} in its own frame)\n  \
                 - shown:   it is in no `shows` string this sentence's claims assert\n  \
                 - setting: it is not the step length of any trajectory this sentence's \
                 claims read\n\
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
}

/// One phrase the lesson prose uses to print a number its scenario file decides.
///
/// This table is the ledger's honest cost. A generous arm — "the number appears somewhere
/// in the scenario file" — accounts for a third of the path's numbers and means nothing: a
/// scenario has enough integers in it that a `2` finds `series = 2` by accident. So the
/// arm has to name the field, and something has to say which field a sentence is talking
/// about. That something is the phrase around the number.
///
/// What is declared here is only the *vocabulary* — that "`{n}` in series" is the page's
/// way of saying `pack.series`. The number itself is never declared: it is read out of the
/// scenario file and compared, so a rule pointed at the wrong field fails on sight, the
/// same way a mis-pointed `spells` does. Keep the phrases specific for that reason. A bare
/// `"{n} mV"` would match any millivolt figure in any ledgered step and account it against
/// the sensor offset, which passes silently the day one of them happens to be 120.
struct ScenarioRule {
    /// The sentence shape, with `{n}` where each number sits. Matched literally against
    /// the prose, so it is written the way the prose writes it.
    phrase: &'static str,
    /// One dotted key path into the step's scenario TOML per `{n}`, in order. `*` walks an
    /// array, so `faults.*.at_s` does not care which order the file lists its faults in.
    ///
    /// **A wildcard is read strictly: *every* value it reaches must be the number.** It was
    /// existential first — some fault at 600 — and that is a fail-toward-green on the one
    /// relational thing this step's prose says. `lying-sensor` claims the short and the
    /// sensor lie land "in the same instant"; measured, with the second fault moved to
    /// 700 s, the existential arm stayed green and the sentence was false. Strict makes the
    /// arm say what the sentence says.
    ///
    /// The cost is stated rather than discovered: a *third* fault scheduled at some other
    /// time would fail this rule even though the sentence — which names two — would still
    /// be true. That is the fail-toward-red direction, and the answer then is a path that
    /// selects the fault the sentence names, which this walker cannot express.
    paths: &'static [&'static str],
    /// How many powers of ten larger the unit the *prose* writes is than the unit the
    /// *file* writes — 2 for a percentage against a fraction, 3 for mA against A. Same
    /// convention as a claim's `spells_pow10`.
    pow10: i32,
}

/// The vocabulary, one entry per way the ledgered steps name a scenario constant.
///
/// Every rule is required to match something ([`every_ledger_rule_is_a_phrase_and_is_used`]),
/// so a rule left behind by a prose edit fails here instead of sitting in the list looking
/// like coverage.
const SCENARIO_VOCABULARY: &[ScenarioRule] = &[
    // Step 3 — the pack's topology and its manufacturing spread.
    ScenarioRule {
        phrase: "{n} in series",
        paths: &["pack.series"],
        pow10: 0,
    },
    ScenarioRule {
        phrase: "{n} in parallel",
        paths: &["pack.parallel"],
        pow10: 0,
    },
    ScenarioRule {
        phrase: "with {n} % capacity",
        paths: &["pack.scatter.capacity_sigma"],
        pow10: 2,
    },
    ScenarioRule {
        phrase: "and {n} % resistance scatter",
        paths: &["pack.scatter.r0_sigma"],
        pow10: 2,
    },
    // Step 4 — what the BMS's own instrument is wrong by. The whole lesson is that these
    // are the errors it cannot know about, so they are the scenario's numbers and not
    // anything the engine produces.
    ScenarioRule {
        phrase: "current sensor reads {n} mA high",
        paths: &["pack.bms.current_offset_a"],
        pow10: 3,
    },
    ScenarioRule {
        phrase: "with {n} mA of noise",
        paths: &["pack.bms.current_noise_sigma_a"],
        pow10: 3,
    },
    ScenarioRule {
        phrase: "started {n} % wrong",
        paths: &["pack.bms.initial_soc_error"],
        pow10: 2,
    },
    // Step 5 — the two scheduled faults. The prose's whole first paragraph is a reading of
    // the `[[faults]]` tables, which is what makes this step free to ledger.
    ScenarioRule {
        phrase: "At t = {n} s this scenario springs",
        paths: &["faults.*.at_s"],
        pow10: 0,
    },
    ScenarioRule {
        phrase: "a {n} \u{3a9} internal short",
        paths: &["faults.*.fault.SoftInternalShort.ohms"],
        pow10: 0,
    },
    ScenarioRule {
        phrase: "on cell ({n},{n})",
        paths: &[
            "faults.*.fault.SoftInternalShort.s",
            "faults.*.fault.SoftInternalShort.p",
        ],
        pow10: 0,
    },
    ScenarioRule {
        phrase: "a +{n} mV offset",
        paths: &["faults.*.fault.SensorOffset.offset"],
        pow10: 3,
    },
    ScenarioRule {
        phrase: "Group {n}'s sensed voltage",
        paths: &["faults.*.fault.SensorOffset.sensor.GroupVoltage"],
        pow10: 0,
    },
    ScenarioRule {
        phrase: "by {n} mV and stays there",
        paths: &["faults.*.fault.SensorOffset.offset"],
        pow10: 3,
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

/// Every number at a dotted key path, with `*` walking an array.
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
    for (r, rule) in SCENARIO_VOCABULARY.iter().enumerate() {
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
                        SCENARIO_VOCABULARY[r0].phrase,
                        rule.phrase,
                    );
                }
                out[i] = Some((r, p));
            }
        }
    }
    out
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
/// So this scans a step's prose end to end. Three steps are ledgered today, chosen because
/// every *numeral* in them is a scenario constant: `docs/plans/path-prose-ledger.md` measured
/// all fourteen steps and found these three carry no measurement-shaped figure at all, so
/// they can be closed before a single number is measured. That is the whole of what this
/// check covers, and the rest of the design — arms for control settings, chemistry
/// constants, ordinals naming other steps, and figures derived from other figures in the
/// same sentence — is written up in that plan and not built.
///
/// **Numeral is the operative word, and it is a real limit rather than pedantry.**
/// [`written_numbers`] finds digits. A quantity spelled in English is invisible to it, and
/// two of the three ledgered steps state four of them: "about half a point across the whole
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
/// **There is still one arm, and a ledgered step may now carry claims.** This test used to
/// refuse the combination outright, on the grounds that a number a claim ties to the engine
/// has no accounting here. That fence came down when `belief-drifts` was given the two
/// claims on its estimator gap: the numbers those claims are about are spelled in *letters*
/// ("about three points"), the scan finds digits, and this step's three digits are the
/// scenario constants they always were. So the combination the fence forbade is now in the
/// tree and the scan is unchanged by it.
///
/// What the fence was really guarding is the first *numeral* in a ledgered step that only a
/// claim decides, and that is still not buildable-for-free: the accounting a claimed number
/// needs is check 6's [`accounting_for`], which is written and tested — but wiring it in
/// with nothing to account would be a second `CCCV_PERIOD_S`, pinned and consulted by
/// nothing. It is deferred until a number needs it, and the panic below already routes an
/// author there in the words they will need. See `docs/plans/path-estimator-gap.md`.
#[test]
fn every_numeral_in_a_ledgered_step_is_accounted_for() {
    let lessons = lessons();
    let ledger = ledger();

    for step in &ledger.steps {
        let lesson = lessons
            .iter()
            .find(|l| l.id == *step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));

        let text = ascii_minus(&lesson.text);
        let numbers = written_numbers(&text);
        assert!(
            !numbers.is_empty(),
            "step `{step}` is ledgered and prints no number at all. That is not a \
             failure of the prose, but it is not coverage either — drop it from `steps` \
             and say so in `unledgered`."
        );
        let scenario = scenario_toml(&lesson.scenario);
        let cover = cover_by_rule(&text, &numbers, step);

        for (w, covered) in numbers.iter().zip(&cover) {
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
            let Some((r, p)) = *covered else {
                panic!(
                    "step `{step}` prints `{}` and nothing accounts for it:\n  \
                     …{context}…\n\
                     No vocabulary rule spells that position, so this file cannot say \
                     what decides the number. If a scenario field does, add a rule to \
                     `SCENARIO_VOCABULARY` naming the field. If the engine does, it needs \
                     a claim in web/path-claims.toml and the `claimed` arm. If it is a \
                     control setting, a chemistry constant, an ordinal or an arithmetic \
                     consequence of another number in the sentence, it needs the arm for \
                     that — see docs/plans/path-prose-ledger.md. There is no waiver.",
                    w.token,
                );
            };
            let rule = &SCENARIO_VOCABULARY[r];
            let path = rule.paths[p];
            let found = numbers_at_path(&scenario, path);
            assert!(
                !found.is_empty(),
                "step `{step}`: the rule `{}` reads `{path}`, and scenarios/{} has no \
                 number there. The scenario was restructured; the rule has to follow it.",
                rule.phrase,
                lesson.scenario,
            );
            let spelled: f64 = w
                .token
                .parse()
                .unwrap_or_else(|_| panic!("`{}` scanned as a number", w.token));
            assert!(
                found
                    .iter()
                    .all(|v| tol_eq(v * 10f64.powi(rule.pow10), spelled)),
                "step `{step}` says `{}` where scenarios/{} says {found:?} at \
                 `{path}`:\n  …{context}…\n\
                 The prose and the scenario file have parted. One of them moved; the \
                 sentence is what a reader is shown, so fix whichever is wrong rather \
                 than the rule.\n\
                 Note that a `*` in the path is read strictly — EVERY value it reaches \
                 has to be this number, which is what lets `faults.*.at_s` carry \
                 \"in the same instant\". If the sentence really is about one of \
                 several, it needs a path that says which.",
                w.token,
                lesson.scenario,
            );
        }
    }
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

    let mut used = vec![0usize; SCENARIO_VOCABULARY.len()];
    for step in &ledger.steps {
        let lesson = lessons
            .iter()
            .find(|l| l.id == *step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));
        let text = ascii_minus(&lesson.text);
        let numbers = written_numbers(&text);
        for (r, rule) in SCENARIO_VOCABULARY.iter().enumerate() {
            used[r] += rule_matches(&text, &numbers, rule.phrase).len();
        }
    }

    for (r, rule) in SCENARIO_VOCABULARY.iter().enumerate() {
        let parts: Vec<&str> = rule.phrase.split("{n}").collect();
        assert!(
            parts.len() > 1,
            "vocabulary rule `{}` has no `{{n}}` — there is no number in it to account for.",
            rule.phrase
        );
        assert_eq!(
            parts.len() - 1,
            rule.paths.len(),
            "vocabulary rule `{}` has {} `{{n}}` and {} path(s). Each number in the \
             phrase needs the field that decides it.",
            rule.phrase,
            parts.len() - 1,
            rule.paths.len(),
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

        let earliest = arm.earliest_s(lesson);
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
        let end = arm.end_s(lesson);
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
        let r = run(lesson, arm);

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
                let Some(Accounted::ReadAt(at_s)) = accounting_for(&token, &group, lesson, &arms)
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
            let got = measure(&c.quantity, &r, c.read_at_s, c.probe);
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
