//! Sodium-ion: the cell whose voltage tells you where it is, and the floor on that.
//!
//! `chemistries/na_ion_18650_generic.toml` is the first chemistry added after Phase 8
//! closed, added with **zero lines of engine code changed** — so, like `lto_chemistry.rs`
//! beside it, nothing here tests a mechanism. Every mechanism these tests touch was already
//! shipped and is tested in `sim-core`. What is measured here is the **shipped parameter
//! set**: that this file produces the behaviours its own header claims, at the sizes it
//! claims them.
//!
//! The subject is the shape of the open-circuit curve. This cell climbs steadily from
//! 1.99 V empty to 4.08 V full with no plateau anywhere: its middle 60 % of charge moves
//! 910 mV, where the LFP file's moves 156 mV and only 31 mV of that lies across LFP's own
//! flat plateau. That is the difference between a voltage reading that constrains the
//! charge state and one that barely does — and the guided path already teaches the LFP
//! half. The second half of the lesson is the limit: `[hysteresis]` says the cell's resting
//! voltage depends on which way it was last driven, and on a steep curve that memory
//! converts directly into charge the voltage cannot resolve.
//!
//! Every measurement carries a control arm, for the reason `nimh_chemistry.rs` states at
//! length: a number with nothing beside it is usually a fact about the engine rather than
//! about the cell. The control here is the shipped LFP cell, which has the opposite curve
//! and no `[hysteresis]` section at all.
//!
//! Run the tables with `cargo test -p sim-data --test na_ion_chemistry -- --nocapture`.

use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig, Scatter,
    ThermalConfig,
};

fn na_ion() -> ChemistryParams {
    let text = include_str!("../../../chemistries/na_ion_18650_generic.toml");
    sim_data::parse_chemistry(text).expect("Na-ion chemistry loads and validates")
}

/// The control chemistry: the flat-curve cell, with no `[hysteresis]` section, whose
/// estimator problem the guided path already teaches.
fn lfp() -> ChemistryParams {
    let text = include_str!("../../../chemistries/lfp_26650_generic.toml");
    sim_data::parse_chemistry(text).expect("LFP chemistry loads and validates")
}

const ROOM_K: f64 = 298.15;

fn config(initial_soc: f64, thermal: ThermalConfig) -> PackConfig {
    PackConfig {
        aging: None,
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k: ROOM_K,
        seed: 0,
        scatter: Scatter::default(),
        thermal,
        bms: None,
        cell_model: CellModelConfig::Ecm,
    }
}

fn env() -> Env {
    Env {
        t_ambient: ROOM_K,
        t_coolant: None,
    }
}

/// The span the headline comparison is taken over: the working middle of the charge range,
/// away from both knees, which is where a fuel gauge spends its life.
///
/// **This is the conservative choice and it is made on purpose.** LFP is not uniformly flat
/// across it — the shipped table still climbs 130 mV through the knee between 15 % and 25 %
/// — so this span *understates* the contrast. See [`PLATEAU`].
const SPAN: (f64, f64) = (0.20, 0.80);

/// LFP's genuinely flat plateau, where its fuel-gauge problem actually lives.
///
/// Reported beside [`SPAN`] rather than instead of it. Picking the span that maximises a
/// ratio is how a true number becomes a misleading one, so the wider and less flattering
/// figure is the one this file's header and the lesson prose quote.
const PLATEAU: (f64, f64) = (0.45, 0.75);

/// **The curve is steep where the LFP curve is flat** — by about 6x across the working
/// middle, and about 18x across LFP's own plateau.
///
/// This is the claim the whole lesson rests on, and it is a property of two shipped tables
/// rather than of any simulation, so it is measured directly off them. Both numbers are
/// millivolts of open-circuit voltage per *point* of charge (one hundredth of the cell's
/// capacity), which is the unit that makes them comparable across cells of different sizes
/// and different nominal voltages.
///
/// **The first version of this test asserted 13x and failed at 5.83x, which was the test
/// working.** The 13x came from the illustrative `[ocv]` block in `CLAUDE.md` rather than
/// from `chemistries/lfp_26650_generic.toml`, and that block is a *shape*, not the shipped
/// table — a trap this repo has fallen into before. The shipped LFP curve climbs 130 mV
/// through a knee between 15 % and 25 % that the sketch does not have.
///
/// The bands are deliberately wide. What is being asserted is that these two cells are not
/// in the same regime, not a fitted ratio, and a tight band would break every time either
/// OCV table is refitted for reasons having nothing to do with this comparison.
#[test]
fn the_open_circuit_curve_is_steep_where_the_lfp_curve_is_flat() {
    let (na, fe) = (mv_per_point(&na_ion(), SPAN), mv_per_point(&lfp(), SPAN));
    let (na_p, fe_p) = (
        mv_per_point(&na_ion(), PLATEAU),
        mv_per_point(&lfp(), PLATEAU),
    );

    println!("open-circuit slope, mV per point of charge:");
    println!(
        "  over {:.0}-{:.0} %   sodium-ion {na:6.2}   LFP {fe:6.2}   ratio {:5.2}x",
        SPAN.0 * 100.0,
        SPAN.1 * 100.0,
        na / fe
    );
    println!(
        "  over {:.0}-{:.0} %   sodium-ion {na_p:6.2}   LFP {fe_p:6.2}   ratio {:5.2}x",
        PLATEAU.0 * 100.0,
        PLATEAU.1 * 100.0,
        na_p / fe_p
    );

    assert!(
        na / fe > 4.0,
        "over the working middle the sodium-ion cell reads {na:.2} mV per point against \
         LFP's {fe:.2} — a ratio of {:.2}, below the 4x this file's lesson claims",
        na / fe
    );
    assert!(
        na_p / fe_p > 10.0,
        "over LFP's own flat plateau the ratio is {:.2}, below the 10x claimed",
        na_p / fe_p
    );
    // The direction matters as much as the size: LFP must be the flat one, and it is flat
    // on its plateau rather than across the whole working middle — which is exactly why
    // both spans are reported.
    assert!(
        fe_p < 1.5,
        "LFP is supposed to be flat on its plateau, and reads {fe_p:.2} mV per point"
    );
}

/// Millivolts of open-circuit voltage per *point* of charge (one hundredth of the cell's
/// capacity) across `span` — the unit that makes cells of different sizes and different
/// nominal voltages comparable.
fn mv_per_point(chem: &ChemistryParams, span: (f64, f64)) -> f64 {
    let lo = sim_core::ecm::ocv_lookup(&chem.ocv, span.0);
    let hi = sim_core::ecm::ocv_lookup(&chem.ocv, span.1);
    (hi - lo) * 1000.0 / ((span.1 - span.0) * 100.0)
}

/// **A rested cell remembers which way it was driven**, and on this curve that memory is
/// worth a measurable amount of charge.
///
/// Two arms reach the same true charge state from opposite directions and then rest until
/// nothing is left moving but the hysteresis state. The gap between their resting voltages
/// is the loop, and dividing it by the slope measured above converts it into the quantity
/// the lesson is actually about: **charge that no voltage reading can resolve, however good
/// the sensor.**
///
/// The rest is four hours, and the length is not padding. This cell's slow RC pair has a
/// time constant of **365 seconds** — six minutes, by far the longest of any shipped
/// chemistry — so a short rest reports that pair and not the open-circuit voltage. Four
/// hours is forty time constants. `docs/plans/phase-8-slice-c-spike.md` records the session
/// that measured a hysteresis figure before the slow pair had settled and got the sum of
/// the two; this is that lesson applied in advance.
///
/// The control arm is LFP, which has no `[hysteresis]` section: its two arms must land on
/// the same voltage, because for a chemistry without the section the state is never written.
#[test]
fn a_rested_cell_remembers_which_way_it_was_driven() {
    /// Seconds of 1 C current that move 10 % of a cell's capacity, whatever its size.
    const MOVE_S: usize = (0.10 * 3600.0) as usize;
    const REST_S: usize = 4 * 3600;

    // Both arms end at 45 % charge: one arrives from 55 % going down, one from 35 % going
    // up. Isothermal, so no temperature term can contribute, and 1 C in both directions so
    // the two arms move exactly the same charge.
    let loop_mv = |chem: &ChemistryParams| {
        let cap = chem.cell.capacity_ah;
        let mut down =
            Pack::new(&config(0.55, ThermalConfig::Isothermal), chem.clone()).expect("builds");
        let mut up =
            Pack::new(&config(0.35, ThermalConfig::Isothermal), chem.clone()).expect("builds");
        for _ in 0..MOVE_S {
            down.step(1.0, Demand::Current(cap), &env());
            up.step(1.0, Demand::Current(-cap), &env());
        }
        for _ in 0..REST_S {
            down.step(1.0, Demand::Rest, &env());
            up.step(1.0, Demand::Rest, &env());
        }
        let after_down = down.step(0.0, Demand::Rest, &env());
        let after_up = up.step(0.0, Demand::Rest, &env());
        // The two arms must actually have met, or the voltage gap is an SOC gap wearing a
        // disguise — the failure mode this whole test would otherwise be blind to.
        assert!(
            (after_down.soc_true - after_up.soc_true).abs() < 1e-12,
            "the two arms did not reach the same charge state: {} vs {}",
            after_down.soc_true,
            after_up.soc_true
        );
        (after_up.v_terminal - after_down.v_terminal) * 1000.0
    };

    let na = na_ion();
    let gap_mv = loop_mv(&na);
    let control_mv = loop_mv(&lfp());

    // What the file declares, and how far a 10 % excursion gets across it: the state
    // approaches its endpoint exponentially in charge moved, at rate `gamma`.
    let hyst = na
        .hysteresis
        .as_ref()
        .expect("Na-ion declares [hysteresis]");
    let crossed = 1.0 - (-hyst.gamma * 0.10).exp();
    let predicted_mv = 2.0 * hyst.scale_v * crossed * 1000.0;

    let slope_mv_per_point = mv_per_point(&na, SPAN);

    println!("resting-voltage gap after arriving at 45 % charge from either side:");
    println!("  sodium-ion  {gap_mv:6.2} mV   (predicted {predicted_mv:.2} mV)");
    println!("  LFP control {control_mv:6.2} mV");
    println!(
        "  the gap is worth {:.2} points of charge on this cell's curve",
        gap_mv / slope_mv_per_point
    );

    assert!(
        (gap_mv - predicted_mv).abs() < 0.5,
        "the measured loop is {gap_mv:.2} mV where the shipped scale_v and gamma predict \
         {predicted_mv:.2} mV — the parameters and the behaviour have parted"
    );
    assert!(
        control_mv.abs() < 1e-9,
        "LFP has no [hysteresis] section, so its two arms must rest on the same voltage, \
         and they differ by {control_mv} mV"
    );
    // The lesson's own number: the loop costs more than half a point of charge and less
    // than three. A band, not a fit — see the slope test above for why.
    let points = gap_mv / slope_mv_per_point;
    assert!(
        (0.5..3.0).contains(&points),
        "the loop is worth {points:.2} points of charge, outside the 0.5-3 band the lesson \
         is written around"
    );
}

/// **At 1 C this cell runs out of charge before it runs out of voltage; at 3 C it does
/// not.** Which of its two floors a discharge hits first is a property of the rate.
///
/// This matters beyond trivia. The cell's empty-endpoint open-circuit voltage is 1.99 V
/// against a 1.50 V cutoff, so at a gentle rate the charge state reaches zero with nearly
/// half a volt of headroom — and everything below empty, the reversal ramp and what it
/// costs, is reachable without the terminal voltage having tripped anything first. At the
/// datasheet's maximum 3 C the ohmic drop through this cell's ~150 mΩ at low charge eats
/// that headroom and the voltage floor arrives first instead.
///
/// No BMS in either arm, so nothing stops the discharge at either floor: both are observed
/// rather than enforced, which is what makes the crossing measurable at all.
#[test]
fn which_floor_a_discharge_hits_first_depends_on_the_rate() {
    let chem = na_ion();
    let cap = chem.cell.capacity_ah;
    let v_min = chem.cell.v_min;

    // Discharge a full cell at `c_rate` and report the terminal voltage at the instant the
    // charge state first reaches zero, plus whether the voltage floor was reached earlier.
    let run = |c_rate: f64| {
        let mut pack =
            Pack::new(&config(1.0, ThermalConfig::Isothermal), chem.clone()).expect("builds");
        let mut hit_v_min_first = false;
        for _ in 0..200_000 {
            let tel = pack.step(0.1, Demand::Current(c_rate * cap), &env());
            if tel.flags.contains(EventFlags::SOC_CLAMPED_LOW) {
                return (tel.v_terminal, hit_v_min_first);
            }
            if tel.v_terminal <= v_min {
                hit_v_min_first = true;
            }
        }
        panic!("a {c_rate} C discharge did not empty the cell within 20000 s");
    };

    let (v_at_empty_1c, v_min_first_1c) = run(1.0);
    let (v_at_empty_3c, v_min_first_3c) = run(3.0);

    println!("terminal voltage at the instant the charge state reaches zero:");
    println!("  1 C  {v_at_empty_1c:.4} V   voltage floor reached earlier: {v_min_first_1c}");
    println!("  3 C  {v_at_empty_3c:.4} V   voltage floor reached earlier: {v_min_first_3c}");
    println!("  cutoff {v_min:.2} V");

    assert!(
        !v_min_first_1c && v_at_empty_1c > v_min,
        "at 1 C the cell should empty at {v_at_empty_1c:.4} V, above its {v_min:.2} V cutoff"
    );
    assert!(
        v_min_first_3c,
        "at 3 C the ohmic drop should carry the terminal below {v_min:.2} V before the \
         charge state reaches zero, and it did not"
    );
}

/// **The over-voltage limit sits inside the protection layer's release band**, which is what
/// keeps a full cell from chattering against it.
///
/// `docs/plans/protection-chatter.md` measured the rule: the over-voltage rung releases on a
/// *rested* reading and a saturated cell rests at its own full-charge open-circuit voltage,
/// so the quantity a release band has to clear is `v_max − OCV(1.0)` — a property of the
/// chemistry file, not of the operating current. While the band is wider than that gap the
/// rung holds; once the gap exceeds the band, the reading falls back through the release
/// threshold every time the load comes off.
///
/// **This is deliberately a test of one file and not a rule over all of them.** The NiMH
/// file sits at 175 mV, far outside the band, and does so on purpose: a NiMH cell under 1 C
/// charge legitimately reaches ~1.50 V at the terminals, so a tighter limit would trip on
/// ordinary charging. Do not turn this into a loop over the shipped chemistries; it would be
/// asserting a resolution of that trade-off that each file is entitled to make for itself.
#[test]
fn the_over_voltage_limit_sits_inside_the_release_band() {
    let chem = na_ion();
    let hyst = chem.hysteresis.expect("Na-ion declares [hysteresis]");
    let full_ocv = sim_core::ecm::ocv_lookup(&chem.ocv, 1.0);
    // A cell that arrives at full by charging sits a full half-loop above the midline.
    let rested_full = full_ocv + hyst.scale_v;
    let gap = chem.cell.v_max - rested_full;
    // The band is read from the engine's own serde default rather than written down here:
    // a copy of `0.08` in this file would keep passing after the default moved.
    let band = serde_json::from_str::<sim_core::ProtectionConfig>(
        r#"{"v_hard_margin_v": 0.0, "t_hard_margin_k": 0.0}"#,
    )
    .expect("a ProtectionConfig with only its non-defaulted fields")
    .v_release_band_v;

    println!(
        "v_max {:.4} V, rested full cell {rested_full:.4} V, gap {:.1} mV, band {:.1} mV",
        chem.cell.v_max,
        gap * 1000.0,
        band * 1000.0
    );

    assert!(
        gap > 0.0,
        "v_max {:.4} V is below a rested full cell at {rested_full:.4} V, so this cell trips \
         over-voltage merely by being charged",
        chem.cell.v_max
    );
    assert!(
        gap < band,
        "v_max sits {:.1} mV above a rested full cell against a {:.1} mV release band, so the \
         over-voltage rung would release on every load removal and the pack would chatter",
        gap * 1000.0,
        band * 1000.0
    );
}

/// **The loop is not one width, and the file now says where it is wider.**
///
/// `[hysteresis.width_over_soc]` is what `SNAPSHOT_VERSION` 20 added, and this file is the
/// reason it exists: the source measures roughly 20 mV of loop above 35 % charge and up to
/// 80 mV below it, and until v20 one scalar had to stand for both. It stood for the first,
/// so the shipped file understated its own source over the bottom third of the range. See
/// `docs/plans/hysteresis-width-over-soc.md`.
///
/// The measurement is the one above, run twice: two arms meeting at the same charge from
/// opposite directions, each having moved the **same** charge, so the fraction of the loop
/// each has crossed is identical at both meeting points and divides out. What is left in the
/// ratio is the multiplier the file declares, and nothing else.
///
/// Two things this deliberately does not measure. The arms below the breakpoint travel
/// through a *varying* half-width on the way, and that must not show up in the answer — the
/// resting voltage is `OCV(soc) ∓ M(soc)·h`, and neither term depends on the path. And the
/// absolute widths are asserted against the file's own fields rather than against 20 and
/// 80 mV, because those two are the *source's* figures and this test is about whether the
/// engine does what the file says, not about whether the file is right about the cell.
///
/// Four hours of rest, forty time constants of this cell's 365 s pair, for the reason the
/// test above states at length.
#[test]
fn the_loop_is_wider_below_the_breakpoint_than_above_it() {
    const MOVE_S: usize = (0.10 * 3600.0) as usize;
    const REST_S: usize = 4 * 3600;

    let chem = na_ion();
    let cap = chem.cell.capacity_ah;
    let loop_v = |meet: f64| {
        let mut down = Pack::new(
            &config(meet + 0.10, ThermalConfig::Isothermal),
            chem.clone(),
        )
        .expect("builds");
        let mut up = Pack::new(
            &config(meet - 0.10, ThermalConfig::Isothermal),
            chem.clone(),
        )
        .expect("builds");
        for _ in 0..MOVE_S {
            down.step(1.0, Demand::Current(cap), &env());
            up.step(1.0, Demand::Current(-cap), &env());
        }
        for _ in 0..REST_S {
            down.step(1.0, Demand::Rest, &env());
            up.step(1.0, Demand::Rest, &env());
        }
        let after_down = down.step(0.0, Demand::Rest, &env());
        let after_up = up.step(0.0, Demand::Rest, &env());
        assert!(
            (after_down.soc_true - after_up.soc_true).abs() < 1e-12
                && (after_down.soc_true - meet).abs() < 1e-12,
            "the two arms did not meet at {meet}: {} vs {}",
            after_down.soc_true,
            after_up.soc_true
        );
        after_up.v_terminal - after_down.v_terminal
    };

    // Well clear of the 35 % breakpoint on either side, and far enough from both ends that
    // neither arm approaches a clamp.
    const ABOVE: f64 = 0.70;
    const BELOW: f64 = 0.15;
    let hyst = chem
        .hysteresis
        .as_ref()
        .expect("Na-ion declares [hysteresis]");
    let crossed = 1.0 - (-hyst.gamma * 0.10).exp();
    let predict = |soc: f64| 2.0 * sim_core::ecm::hysteresis_half_width_v(hyst, soc) * crossed;

    let above = loop_v(ABOVE);
    let below = loop_v(BELOW);
    let ratio = below / above;
    println!("resting-voltage loop, measured against what the file declares:");
    println!(
        "  at {:.0} % charge  {:6.2} mV   (predicted {:.2})",
        ABOVE * 100.0,
        above * 1000.0,
        predict(ABOVE) * 1000.0
    );
    println!(
        "  at {:.0} % charge  {:6.2} mV   (predicted {:.2})",
        BELOW * 100.0,
        below * 1000.0,
        predict(BELOW) * 1000.0
    );
    println!("  ratio {ratio:.6}");

    // Tolerance: what is left in either reading after four hours is the slow RC pair, whose
    // 31.5 mV at 1 C is down by exp(-14400/365) = 8e-18 V by then, so the bound below is
    // dominated by nothing physical at all and is set at a hundredth of a millivolt purely
    // so a real regression cannot hide under it.
    for (what, measured, soc) in [("above", above, ABOVE), ("below", below, BELOW)] {
        assert!(
            (measured - predict(soc)).abs() < 1.0e-5,
            "{what} the breakpoint the loop measures {:.4} mV where the shipped fields \
             predict {:.4} mV",
            measured * 1000.0,
            predict(soc) * 1000.0
        );
    }
    // And the ratio is the multiplier itself, with the crossing fraction divided out.
    let mult = sim_core::ecm::hysteresis_half_width_v(hyst, BELOW)
        / sim_core::ecm::hysteresis_half_width_v(hyst, ABOVE);
    assert!(
        (ratio - mult).abs() < 1.0e-6,
        "the ratio of the two loops is {ratio:.6} where the table says {mult:.6}"
    );
    // The claim that made the slice worth paying a snapshot bump for: one scalar could not
    // have produced both of these, so the file is no longer understating its own source.
    assert!(
        ratio > 2.0,
        "the point of the table is that the two ends differ by more than rounding, and \
         they differ by {ratio:.3}"
    );
}
