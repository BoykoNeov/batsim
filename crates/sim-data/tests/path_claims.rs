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
//! read its header for the three-way check and why the literal is stored as a string
//! rather than formatted from the value.
//!
//! # The three checks, and why none of them is redundant
//!
//! * **Literal** — the claim's text appears verbatim in that step's prose. A prose edit
//!   that changes the number now fails here even though the engine never moved. This is
//!   the half that would have caught all four historical failures; a golden-value table
//!   would have caught none of them.
//! * **Value** — the engine, driven the way `applyStep` drives it, produces the number.
//! * **Reachable** — the claim is read at a time the step actually runs to. "Right but
//!   unreachable" is a defect class this repo has shipped twice.
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
//! * **Panel formatting.** A claim is "reachable" here if the simulation runs to its
//!   time. Whether the page's own formatter can *display* the number at that instant is
//!   a different question — `fmtTime` prints whole minutes above 120 s, and a SOC row
//!   prints one decimal — and both have produced true-but-unreadable claims before.
//!   Checking that needs the page's formatters, which are JavaScript. Not done here.
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
    /// The row whose end-of-step time is closest to `t`.
    fn at(&self, t: f64) -> &Telemetry {
        &self
            .rows
            .iter()
            .min_by(|a, b| (a.0 - t).abs().total_cmp(&(b.0 - t).abs()))
            .expect("run produced at least one row")
            .1
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
    #[allow(
        dead_code,
        reason = "authoring context for a human reader, not asserted"
    )]
    note: String,
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
        // The coupling CLAUDE.md refuses to let a chemistry model one half of: points
        // of resistance growth per point of capacity lost.
        "soh_ratio_at" => {
            let t = run.at(at_s);
            (t.soh_resistance - 1.0) / (1.0 - t.soh_capacity)
        }
        other => panic!(
            "path-claims.toml names a quantity this test cannot measure: `{other}`. \
             Known: v_at_mark, v_at, soc_at, i_at, t_max_at, soh_cap_at, soh_res_at, \
             soh_ratio_at, flag_first_s:<FLAG>, v_at_soc_below:<fraction>."
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
        }
    }
}
