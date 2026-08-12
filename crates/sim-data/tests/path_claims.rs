//! Check the numbers the guided path's lesson prose claims, against the engine that
//! is supposed to produce them.
//!
//! # What this is for
//!
//! `web/app.js`'s `const LESSONS` is 21 teaching steps whose prose states hundreds of
//! specific quantities. Until this test existed, not one of them was checked by
//! anything in the repo. Four slices found numbers in that prose that had drifted, or
//! were never true, or were true about a quantity no reader can see — and every one of
//! those findings came from an instrument that lived outside the tree and never ran
//! again. `web/path-claims.toml` is that instrument's findings turned into assertions;
//! read its header for the four checks and why the literal is stored as a string
//! rather than formatted from the value.
//!
//! # The four checks, and why none of them is redundant
//!
//! * **Literal** — the claim's text appears verbatim in that step's prose. A prose edit
//!   that changes the number now fails here even though the engine never moved. This is
//!   the half that would have caught all four historical failures; a golden-value table
//!   would have caught none of them.
//! * **Value** — the engine, driven the way `applyStep` drives it, produces the number.
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
//!   quotes what that row prints", made checkable one claim at a time.
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
//! * **The two readout rows that read per-cell state.** `past empty` and `surface gap`
//!   are formatted from `Pack::cell` output rather than from telemetry, and `past empty`
//!   is additionally sampled on a *wall*-clock throttle, so what it shows at a given
//!   simulation time is not a function of that time at all. Neither row is mirrored: a
//!   claim naming one panics rather than passing. See [`render_row`].
//! * **Anything a reader has to change the demand box to reach.** Steps 20 and 21 both
//!   ask for a mid-run reversal to −2 A; this harness drives one demand program per step,
//!   so every number on those charge legs — including the two the formatter check exists
//!   because of — is outside what it can reproduce.
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
    // The two rows deliberately NOT mirrored, pinned so that a page change which turns
    // one of them into a telemetry-only row is noticed rather than silently leaving a
    // mirrorable row unmirrored.
    (
        "the `past empty` row reads per-cell state",
        "app.js",
        "const d = Math.max(...cs.map((c) => c.soc_deficit));",
    ),
    (
        "the `surface gap` row reads per-cell state",
        "app.js",
        "return `${gapPts(c.surface_gap_neg, 2)} / ${gapPts(c.surface_gap_pos, 2)} pts`;",
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

/// What a readout row prints for this frame — `READOUTS` in `web/app.js`, including the
/// placeholder a row falls back to when its formatter returns `null` on a running pack.
///
/// Two rows are missing on purpose and panic instead of returning something plausible:
///
/// * **`past empty`** is formatted from per-cell `soc_deficit`, which is not in
///   `Telemetry` at all, and the page samples it on a 250 ms *wall*-clock throttle rather
///   than per frame. There is therefore no such thing as "what that row shows at
///   simulation time t" — at speed it can be a dozen seconds of simulation behind, which
///   is a fact step 21's own prose has to warn a reader about. A mirror that answered
///   anyway would be asserting a number the page does not show.
/// * **`surface gap`** is per-cell for the same reason (`Pack::cell`), though without the
///   throttle.
///
/// Both are named in the module docs as uncovered rather than left to be inferred.
fn render_row(label: &str, t: &Telemetry, sim_time_s: f64) -> String {
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
        "past empty" | "surface gap" => panic!(
            "`{label}` is a readout row this test deliberately does not mirror: it is \
             formatted from per-cell state rather than telemetry, and `past empty` is \
             sampled on a wall-clock throttle, so it has no value at a given simulation \
             time. Claiming what it displays needs a different instrument — see the \
             module docs."
        ),
        other => panic!(
            "path-claims.toml names a readout row that is not in web/app.js's READOUTS: \
             `{other}`. Known: sim time, terminal, current, soc (true), soc (bms), \
             cell v, cell t, heat, soh cap, soh res, balancing, short (int), clamp."
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
    let body = &src[array_start..];

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

/// One step's trajectory, sampled every engine step.
struct Run {
    /// `(sim_time_s, telemetry)` after each step.
    rows: Vec<(f64, Telemetry)>,
}

impl Run {
    /// The row whose end-of-step time is closest to `t`, with that row's own time.
    ///
    /// The time is returned rather than `t` because the panel's clock renders the frame
    /// the reader is looking at, not the instant a claim was authored against.
    fn row_at(&self, t: f64) -> (f64, &Telemetry) {
        let row = self
            .rows
            .iter()
            .min_by(|a, b| (a.0 - t).abs().total_cmp(&(b.0 - t).abs()))
            .expect("run produced at least one row");
        (row.0, &row.1)
    }

    /// The row whose end-of-step time is closest to `t`.
    fn at(&self, t: f64) -> &Telemetry {
        self.row_at(t).1
    }

    /// First simulation time a flag was seen at.
    fn first_flag(&self, name: &str) -> Option<f64> {
        self.rows
            .iter()
            .find(|(_, t)| format!("{:?}", t.flags).contains(name))
            .map(|(t, _)| *t)
    }
}

fn run(lesson: &Lesson) -> Run {
    let mut pack = build(lesson);
    let env = Env {
        t_ambient: lesson.ambient_c + K,
        t_coolant: None,
    };
    let mut rows = Vec::new();
    let mut last: Option<Telemetry> = None;
    // `<=` because a mark that lands exactly on a step boundary is a mark the page
    // stops *at*, not one step before it.
    while pack.sim_time_s() < lesson.until_s - lesson.dt * 0.5 {
        let d = demand_now(lesson.demand, &pack, lesson.dt, last.as_ref());
        let t = pack.step(lesson.dt, d, &env);
        last = Some(t);
        rows.push((pack.sim_time_s(), t));
    }
    Run { rows }
}

// ---------------------------------------------------------------------------
// The claims
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct Claim {
    step: String,
    literal: String,
    quantity: String,
    value: f64,
    tol: f64,
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

#[derive(Debug, serde::Deserialize)]
struct Claims {
    claim: Vec<Claim>,
}

/// The quantities a claim may name, and how each is read off a run.
///
/// Kept as an explicit match rather than a registry so an unknown quantity is a
/// compile-time-shaped failure with a list in the message, not a silent skip.
fn measure(quantity: &str, run: &Run, at_s: f64) -> f64 {
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
            .find(|(_, t)| t.soc_true <= frac)
            .unwrap_or_else(|| panic!("the run never fell to soc <= {frac}"))
            .1
            .v_terminal;
    }
    match quantity {
        "v_at_mark" | "v_at" => run.at(at_s).v_terminal,
        "soc_at" => run.at(at_s).soc_true,
        "i_at" => run.at(at_s).i_actual,
        "t_max_at" => run.at(at_s).t_max,
        "soh_cap_at" => run.at(at_s).soh_capacity,
        "soh_res_at" => run.at(at_s).soh_resistance,
        // Heat, and the charge the pack would not take. Both exist because the `heat`
        // and `clamp` rows are the two the overcharge step tells a reader to read, and a
        // display claim with no value behind it would be pinning a rendering of a number
        // nothing checks.
        "q_gen_at" => run.at(at_s).q_gen_w,
        "i_rejected_at" => run.at(at_s).i_rejected_a,
        // The coupling CLAUDE.md refuses to let a chemistry model one half of: points
        // of resistance growth per point of capacity lost.
        "soh_ratio_at" => {
            let t = run.at(at_s);
            (t.soh_resistance - 1.0) / (1.0 - t.soh_capacity)
        }
        other => panic!(
            "path-claims.toml names a quantity this test cannot measure: `{other}`. \
             Known: v_at_mark, v_at, soc_at, i_at, t_max_at, soh_cap_at, soh_res_at, \
             soh_ratio_at, q_gen_at, i_rejected_at, flag_first_s:<FLAG>, \
             v_at_soc_below:<fraction>."
        ),
    }
}

fn claims() -> Vec<Claim> {
    let text = read(&repo_root().join("web").join("path-claims.toml"));
    let parsed: Claims = toml::from_str(&text).expect("web/path-claims.toml parses");
    assert!(
        !parsed.claim.is_empty(),
        "web/path-claims.toml has no claims — an empty claims file passes every check \
         and proves nothing"
    );
    parsed.claim
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
    for c in claims() {
        let lesson = lessons
            .iter()
            .find(|l| l.id == c.step)
            .unwrap_or_else(|| panic!("no lesson `{}`", c.step));
        assert!(
            c.read_at_s <= lesson.until_s,
            "step `{}` claims `{}` at t = {} s, but the step stops at its mark of {} s. \
             The number may well be true; a reader cannot get to it. This is the \
             'right but unreachable' defect this repo has shipped twice.",
            c.step,
            c.literal,
            c.read_at_s,
            lesson.until_s
        );
    }
}

/// Check 2 — the engine still produces the number.
#[test]
fn every_claim_matches_the_engine() {
    let lessons = lessons();
    let all = claims();

    // Group by step and run each lesson once. Not a micro-optimisation: step 8 is a
    // 200 000 s rest at dt = 0.5 — 400 000 engine steps — and it carries three claims.
    // Re-running it per claim tripled this test's cost for nothing.
    let mut steps: Vec<&str> = all.iter().map(|c| c.step.as_str()).collect();
    steps.sort_unstable();
    steps.dedup();

    for step in steps {
        let lesson = lessons
            .iter()
            .find(|l| l.id == step)
            .unwrap_or_else(|| panic!("no lesson `{step}`"));
        let r = run(lesson);
        for c in all.iter().filter(|c| c.step == step) {
            let got = measure(&c.quantity, &r, c.read_at_s);
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
            let (row_time_s, telemetry) = r.row_at(c.read_at_s);
            let printed = render_row(row, telemetry, row_time_s);
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
