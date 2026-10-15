//! Phase 10 slice E: the shelf lesson's sentences that print no number.
//!
//! Steps 38 to 40 of the guided path (`web/app.js`, `drawn-on-the-shelf`,
//! `grown-on-the-shelf`, `stored-low`) age one LG M50 cell at rest for 365 days, by the drawn
//! calendar curve and by the SEI film, stored full and at 30 %. Every number they print is a
//! claim or a ledger rule in `web/path-claims.toml`. What is here is what the prose says in
//! words: the four files are one file with one field changed, the room is pinned, the film's
//! loss is near `√t` but not on it, the gap between full and 30 % narrows with time, and the
//! film does not tell the top of the charge range apart. See `docs/plans/phase-10-sei.md`.

use sim_core::{CalendarModel, Demand, Env, Pack, PackConfig, ThermalConfig};
use sim_data::{load_chemistry_file, load_scenario_file, Scenario};
use std::path::PathBuf;

const DAY_S: f64 = 86400.0;
/// The lesson's step length \[s\] (`dt: 3600` on all three steps).
const DT_S: f64 = 3600.0;
/// The lesson's mark \[s\]: 365 days.
const MARK_S: f64 = 365.0 * DAY_S;
/// The quarter mark the prose reads \[s\].
const QUARTER_S: f64 = MARK_S / 4.0;
/// The lesson's ambient \[K\]: 25 °C, the slider the step sets.
const ROOM_K: f64 = 298.15;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scenario(name: &str) -> Scenario {
    load_scenario_file(root().join("scenarios").join(format!("{name}.toml")))
        .unwrap_or_else(|e| panic!("scenarios/{name}.toml loads: {e}"))
}

fn pack_from(sc: &Scenario) -> Pack {
    let chem = load_chemistry_file(root().join("chemistries/nmc_21700_lgm50.toml"))
        .expect("the LG M50 file loads");
    sc.build_pack(chem).expect("the shelf pack builds")
}

/// Capacity lost \[fraction\] at each instant in `at_s`, stepping the page's way.
fn losses(mut pack: Pack, at_s: &[f64]) -> Vec<f64> {
    let env = Env {
        t_ambient: ROOM_K,
        t_coolant: None,
    };
    let mut t = 0.0;
    let mut out = Vec::new();
    for &mark in at_s {
        let mut cap = None;
        while t < mark - 1e-6 {
            cap = Some(pack.step(DT_S, Demand::Rest, &env).soh_capacity);
            t += DT_S;
        }
        out.push(1.0 - cap.expect("each mark lies past the last"));
    }
    out
}

fn with_soc(mut cfg: PackConfig, soc: f64) -> PackConfig {
    cfg.initial_soc = soc;
    cfg
}

fn with_calendar(mut cfg: PackConfig, calendar: CalendarModel) -> PackConfig {
    cfg.aging.as_mut().expect("the shelf files age").calendar = calendar;
    cfg
}

/// "One line of the scenario file is different" (step 39), "Nothing else in the file is
/// different" (step 40), and the control arm "the drawn curve stored at 30 %": the four
/// files are one `PackConfig` with one field changed, and the same chemistry.
#[test]
fn shelf_files_differ_only_where_their_headers_say() {
    let drawn_full = scenario("shelf_drawn_full");
    let film_full = scenario("shelf_film_full");
    let film_low = scenario("shelf_film_low");
    let drawn_low = scenario("shelf_drawn_low");
    for sc in [&drawn_full, &film_full, &film_low, &drawn_low] {
        assert_eq!(sc.chemistry.as_deref(), Some("nmc_21700_lgm50"));
        assert!(sc.faults.is_empty(), "a shelf has no faults");
    }
    let base = drawn_full.pack.clone();
    assert_eq!(
        base.aging.as_ref().map(|a| a.calendar),
        Some(CalendarModel::Curve)
    );
    assert_eq!(base.initial_soc, 1.0);
    assert_eq!(
        with_calendar(base.clone(), CalendarModel::Film),
        film_full.pack,
        "shelf_film_full is shelf_drawn_full with the calendar model changed and nothing else"
    );
    assert_eq!(
        with_soc(film_full.pack.clone(), 0.3),
        film_low.pack,
        "shelf_film_low is shelf_film_full stored at 30 % and nothing else"
    );
    assert_eq!(
        with_soc(base, 0.3),
        drawn_low.pack,
        "shelf_drawn_low is shelf_drawn_full stored at 30 % and nothing else"
    );
}

/// "this file holds the temperature where it is, so the ambient slider does nothing here"
/// (step 38), and the 25 °C every step prints is the file's as well as the slider's.
#[test]
fn the_shelf_is_held_at_the_lessons_room_temperature() {
    let sc = scenario("shelf_drawn_full");
    assert!(matches!(sc.pack.thermal, ThermalConfig::Isothermal));
    assert_eq!(sc.pack.initial_temp_k, ROOM_K);
    // The slider's half: a hot ambient moves nothing on an isothermal pack.
    let mut cool = pack_from(&sc);
    let mut hot = pack_from(&sc);
    for _ in 0..240 {
        let a = cool.step(
            DT_S,
            Demand::Rest,
            &Env {
                t_ambient: ROOM_K,
                t_coolant: None,
            },
        );
        let b = hot.step(
            DT_S,
            Demand::Rest,
            &Env {
                t_ambient: ROOM_K + 20.0,
                t_coolant: None,
            },
        );
        assert_eq!(a.soh_capacity.to_bits(), b.soh_capacity.to_bits());
        assert_eq!(a.t_max.to_bits(), b.t_max.to_bits());
    }
}

/// Step 39: "nearly `√t` … It is not exactly `√t` because the film began at a thickness of
/// its own, and while it is still close to that thickness it grows more nearly in a straight
/// line" — and step 40: stored low it "still grows nearly in a straight line".
///
/// Between the quarter mark and the mark, four times the time: `√t` would lose twice as
/// much and a straight line four times. The film stored full lies between and nearer `√t`;
/// stored low it lies nearer the line. The drawn curve is on `√t` to the digit, which the
/// claims on step 38 already hold.
#[test]
fn the_film_is_near_root_t_full_and_near_a_line_low() {
    let full = losses(
        pack_from(&scenario("shelf_film_full")),
        &[QUARTER_S, MARK_S],
    );
    let low = losses(pack_from(&scenario("shelf_film_low")), &[QUARTER_S, MARK_S]);
    let r_full = full[1] / full[0];
    let r_low = low[1] / low[0];
    assert!(
        r_full > 2.0 && r_full < 2.5,
        "stored full the film should be near sqrt(t) (2 for 4x the time) and above it, got {r_full}"
    );
    assert!(
        r_low > 2.75 && r_low < 4.0,
        "stored low the film should be nearer a straight line (4) than sqrt(t), got {r_low}"
    );
}

/// Step 40: "the 6.65 belongs to this span … So over a shorter span the gap between full and
/// 30 % is wider, and over a longer one narrower."
#[test]
fn the_gap_between_full_and_low_narrows_with_time() {
    let marks = [30.0 * DAY_S, QUARTER_S, MARK_S];
    let full = losses(pack_from(&scenario("shelf_film_full")), &marks);
    let low = losses(pack_from(&scenario("shelf_film_low")), &marks);
    let ratios: Vec<f64> = full.iter().zip(&low).map(|(f, l)| f / l).collect();
    assert!(
        ratios[0] > ratios[1] && ratios[1] > ratios[2],
        "full over low should fall as the span grows, got {ratios:?}"
    );
    // Measured 2026-10-15: 11.7 at 30 days, 8.8 at the quarter, 6.65 at the mark.
    assert!(ratios[0] > 10.0, "{ratios:?}");
}

/// Step 40: "this law grows the film at much the same rate anywhere near the top of the
/// charge range, where the graphite's potential is flat, so it cannot say that storing a
/// little below full is kinder than storing full."
///
/// Measured 2026-10-15: 1.4036 points full, 1.4027 at 85 %, over the 365 days.
#[test]
fn near_the_top_the_film_cannot_tell_charge_levels_apart() {
    let full = scenario("shelf_film_full");
    let mut below = full.clone();
    below.pack.initial_soc = 0.85;
    let a = losses(pack_from(&full), &[MARK_S])[0];
    let b = losses(pack_from(&below), &[MARK_S])[0];
    assert!(
        ((a - b) / a).abs() < 0.01,
        "full {a} and 85 % {b} should lose the same to 1 %"
    );
    // ...where 30 % is a different cell altogether.
    let low = losses(pack_from(&scenario("shelf_film_low")), &[MARK_S])[0];
    assert!(a / low > 5.0, "full {a} against 30 % {low}");
}
