//! Host tests for the pure driver.
//!
//! Every assertion in this crate lives here rather than in a Godot process, for the
//! reason `crates/sim-godot/src/driver.rs` opens with: a `godot` type outside a running
//! engine is absent or a stub, so a test that touched one would be testing the stub.
//! Nothing below imports `godot`.
//!
//! The accumulator and the edge detector get the hardest testing in the file, because the
//! exit gate (slice D) drives the *explicit* step path and therefore cannot see inside
//! either of them. What slice C adds is one headless check that they are wired up at all;
//! their behaviour is pinned here.

use sim_core::{Demand, Env, EventFlags};
use sim_godot::driver::{Accumulator, Backlog, DriverError, Edges, FlagEdges, PackDriver};

const SCENARIO: &str = "../../scenarios/cc_discharge_lfp.toml";
const SHORT_SCENARIO: &str = "../../scenarios/soft_short_under_a_lying_sensor.toml";

/// Load a scenario and whatever chemistry it names, the way a game would.
fn driver_for(path: &str) -> PackDriver {
    let scenario = std::fs::read_to_string(path).expect("scenario file");
    let chem = PackDriver::chemistry_id_of(&scenario)
        .expect("scenario parses")
        .map(|id| {
            std::fs::read_to_string(format!("../../chemistries/{id}.toml")).expect("chemistry file")
        });
    PackDriver::new(&scenario, chem.as_deref()).expect("driver builds")
}

// ---------------------------------------------------------------------------
// Accumulator
// ---------------------------------------------------------------------------

#[test]
fn the_accumulator_carries_its_remainder_rather_than_losing_it() {
    let mut acc = Accumulator::default();
    // 2.5 steps' worth: two now, half carried.
    let first = acc.take(0.025, 0.01, 64, Backlog::Drop);
    assert_eq!(first.steps, 2);
    assert!(!first.capped);
    // The carried half plus another half is a whole step. If the remainder were dropped
    // this would be 0.
    let second = acc.take(0.005, 0.01, 64, Backlog::Drop);
    assert_eq!(second.steps, 1);
}

/// Over many uneven frames the accumulator must not drift: the steps taken should account
/// for essentially all the time fed in. This is the property that makes sim time track
/// wall time, and a naive "steps = round(delta/dt)" implementation fails it.
#[test]
fn the_accumulator_does_not_drift_over_many_uneven_frames() {
    let mut acc = Accumulator::default();
    let fixed_dt = 1.0 / 60.0;
    let deltas = [0.017, 0.016, 0.0161, 0.0173, 0.0159, 0.0166, 0.0168];
    let mut fed = 0.0;
    let mut steps = 0u32;
    for i in 0..700 {
        let delta = deltas[i % deltas.len()];
        fed += delta;
        steps += acc.take(delta, fixed_dt, 64, Backlog::Drop).steps;
    }
    let consumed = f64::from(steps) * fixed_dt;
    // Everything fed in is either consumed or still pending; nothing evaporates.
    assert!(
        (fed - consumed - acc.pending_s()).abs() < 1e-9,
        "fed {fed}, consumed {consumed}, pending {}",
        acc.pending_s()
    );
    // And the pending remainder never exceeds one step, which is what "no drift" means.
    assert!(acc.pending_s() < fixed_dt, "pending {}", acc.pending_s());
}

#[test]
fn the_per_frame_cap_binds_and_says_so() {
    let mut acc = Accumulator::default();
    // 1000 steps' worth of time arriving in one frame — a level load, or a breakpoint.
    let ticks = acc.take(10.0, 0.01, 8, Backlog::Drop);
    assert_eq!(ticks.steps, 8);
    assert!(ticks.capped, "the cap bound but did not report it");
    assert!(
        ticks.backlog_s > 0.0,
        "a capped frame must report its backlog, got {}",
        ticks.backlog_s
    );
}

#[test]
fn dropping_the_backlog_forgets_it_and_repaying_works_it_off() {
    let mut dropping = Accumulator::default();
    dropping.take(10.0, 0.01, 8, Backlog::Drop);
    assert_eq!(
        dropping.pending_s(),
        0.0,
        "Backlog::Drop kept time it said it discarded"
    );
    // The next ordinary frame is ordinary again.
    assert_eq!(dropping.take(0.01, 0.01, 8, Backlog::Drop).steps, 1);

    let mut repaying = Accumulator::default();
    repaying.take(10.0, 0.01, 8, Backlog::Repay);
    assert!(
        repaying.pending_s() > 0.0,
        "Backlog::Repay discarded what it owed"
    );
    // The next frame is still capped, because the debt is still there.
    let next = repaying.take(0.0, 0.01, 8, Backlog::Repay);
    assert_eq!(next.steps, 8);
    assert!(next.capped);
}

/// A `NaN` frame delta must contribute nothing rather than poisoning the accumulator.
/// Godot will not normally produce one, but a `NaN` that got in would make *every*
/// subsequent frame `NaN` forever — a far worse failure than a dropped frame.
#[test]
fn a_hostile_frame_delta_cannot_poison_the_accumulator() {
    let mut acc = Accumulator::default();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        let ticks = acc.take(bad, 0.01, 64, Backlog::Drop);
        assert_eq!(ticks.steps, 0, "delta {bad} produced steps");
    }
    assert!(
        acc.pending_s().is_finite(),
        "pending became {}",
        acc.pending_s()
    );
    // And the accumulator still works afterwards.
    assert_eq!(acc.take(0.02, 0.01, 64, Backlog::Drop).steps, 2);
}

#[test]
fn a_reset_forgets_carried_time() {
    let mut acc = Accumulator::default();
    acc.take(0.019, 0.01, 64, Backlog::Drop);
    assert!(acc.pending_s() > 0.0);
    acc.reset();
    assert_eq!(acc.pending_s(), 0.0);
}

// ---------------------------------------------------------------------------
// Edge detection
// ---------------------------------------------------------------------------

#[test]
fn only_transitions_are_reported_not_the_standing_condition() {
    let mut edges = FlagEdges::default();
    let ov = EventFlags::OV;

    let first = edges.observe(ov);
    assert_eq!(
        first.rising, ov,
        "the flag turning on was not a rising edge"
    );
    assert!(first.falling.is_empty());

    // Still set. A 60 Hz signal storm is exactly what this prevents.
    let second = edges.observe(ov);
    assert!(
        second.is_empty(),
        "a standing condition re-reported as an edge: {second:?}"
    );

    let third = edges.observe(EventFlags::empty());
    assert_eq!(third.falling, ov);
    assert!(third.rising.is_empty());
}

#[test]
fn a_batch_unions_its_edges_including_a_flag_that_rose_and_fell() {
    let mut detector = FlagEdges::default();
    let mut batch = Edges::default();
    // A condition that appeared and cleared entirely inside one batch.
    for flags in [
        EventFlags::empty(),
        EventFlags::OV,
        EventFlags::empty(),
        EventFlags::UV,
    ] {
        batch = batch.union(detector.observe(flags));
    }
    assert!(
        batch.rising.contains(EventFlags::OV),
        "a flag that rose inside the batch was not reported"
    );
    assert!(
        batch.falling.contains(EventFlags::OV),
        "a flag that fell inside the batch was not reported"
    );
    assert!(batch.rising.contains(EventFlags::UV));
}

#[test]
fn a_reset_makes_the_detector_re_announce_everything_active() {
    let mut edges = FlagEdges::default();
    edges.observe(EventFlags::OV);
    assert!(edges.observe(EventFlags::OV).is_empty());

    edges.reset();
    assert_eq!(edges.last(), EventFlags::empty());
    // After a reset the standing condition is news again — the documented consequence of
    // keeping the previous mask out of the snapshot.
    assert_eq!(edges.observe(EventFlags::OV).rising, EventFlags::OV);
}

// ---------------------------------------------------------------------------
// PackDriver
// ---------------------------------------------------------------------------

/// The claim `PackDriver::new`'s priming rests on. If a zero-length step ever stops being
/// free, this fails and the constructor has to change rather than the docs.
#[test]
fn priming_with_a_zero_length_step_is_unobservable() {
    let scenario_text = std::fs::read_to_string(SHORT_SCENARIO).expect("scenario");
    let scenario = sim_data::parse_scenario(&scenario_text).expect("parse");
    let id = match scenario.chemistry_source() {
        sim_data::ChemistrySource::Id(id) => id.to_owned(),
        sim_data::ChemistrySource::Inline(_) => panic!("this fixture names a chemistry"),
    };
    let chem_text = std::fs::read_to_string(format!("../../chemistries/{id}.toml")).expect("chem");
    let chem = sim_data::parse_chemistry(&chem_text).expect("chem parses");

    let env = Env {
        t_ambient: scenario.pack.initial_temp_k,
        t_coolant: None,
    };
    let demand = Demand::Current(2.0);

    // With a priming step, and without one.
    let mut primed = scenario.build_pack(chem.clone()).expect("build");
    primed.step(0.0, Demand::Rest, &env);
    let mut bare = scenario.build_pack(chem).expect("build");
    for _ in 0..200 {
        primed.step(0.1, demand, &env);
        bare.step(0.1, demand, &env);
    }

    // Snapshots, not telemetry: a snapshot carries the RNG's position, so this also
    // proves the zero-length step drew nothing. The fixture has faults *and* a noisy
    // sensor, so there is an RNG in play to draw from.
    assert_eq!(
        serde_json::to_string(&primed.snapshot()).unwrap(),
        serde_json::to_string(&bare.snapshot()).unwrap(),
        "a zero-length priming step changed the trajectory"
    );
}

#[test]
fn a_fresh_driver_already_has_a_real_reading() {
    let driver = driver_for(SCENARIO);
    let telemetry = driver.latest();
    assert_eq!(driver.sim_time_s(), 0.0, "priming moved the clock");
    // The point of priming: this is a measurement of the pack, not a zeroed struct.
    assert!(
        telemetry.v_terminal > 1.0,
        "v_terminal reads {} on a fresh pack — that is a default, not a measurement",
        telemetry.v_terminal
    );
    assert!(
        telemetry.soc_true > 0.0,
        "soc_true reads {} on a fresh pack",
        telemetry.soc_true
    );
}

#[test]
fn the_explicit_path_is_bit_identical_to_stepping_the_pack_directly() {
    let mut a = driver_for(SCENARIO);
    let mut b = driver_for(SCENARIO);

    // One batch of 100 against 100 batches of 1: decimation of *calls* must not change
    // the trajectory, which is what lets a game fast-forward without diverging.
    a.step_batch(0.5, 100, Demand::Current(2.0)).expect("batch");
    for _ in 0..100 {
        b.step_batch(0.5, 1, Demand::Current(2.0)).expect("step");
    }

    assert_eq!(
        a.latest().v_terminal.to_bits(),
        b.latest().v_terminal.to_bits(),
        "one batch of 100 diverged from 100 batches of 1"
    );
    assert_eq!(a.latest().soc_true.to_bits(), b.latest().soc_true.to_bits());
    assert_eq!(a.sim_time_s().to_bits(), b.sim_time_s().to_bits());
}

/// The accumulator path and the explicit path must produce the same trajectory for the
/// same *number of steps*. This is the true half of the determinism claim; the false half
/// (same wall-clock duration) is not testable and is not claimed.
#[test]
fn the_real_time_path_matches_the_explicit_path_step_for_step() {
    let fixed_dt = 0.25;
    let demand = Demand::Current(1.5);

    let mut explicit = driver_for(SCENARIO);
    explicit.step_batch(fixed_dt, 40, demand).expect("batch");

    let mut real_time = driver_for(SCENARIO);
    let mut taken = 0;
    // Feed lumpy frames until exactly 40 steps have been consumed.
    while taken < 40 {
        let advance = real_time
            .advance_real_time(0.4, fixed_dt, 8, demand, Backlog::Repay)
            .expect("tick");
        taken += advance.steps;
    }
    assert_eq!(taken, 40, "the frames did not land on a whole batch");

    assert_eq!(
        explicit.latest().v_terminal.to_bits(),
        real_time.latest().v_terminal.to_bits(),
        "the accumulator path diverged from the explicit path at equal step counts"
    );
    assert_eq!(
        explicit.sim_time_s().to_bits(),
        real_time.sim_time_s().to_bits()
    );
}

#[test]
fn a_real_time_tick_too_short_for_a_step_changes_nothing() {
    let mut driver = driver_for(SCENARIO);
    let before = driver.latest();
    let advance = driver
        .advance_real_time(0.001, 1.0, 8, Demand::Rest, Backlog::Drop)
        .expect("tick");
    assert_eq!(advance.steps, 0);
    assert_eq!(driver.sim_time_s(), 0.0);
    // The reported telemetry is the previous reading, not a synthesised one.
    assert_eq!(
        advance.telemetry.v_terminal.to_bits(),
        before.v_terminal.to_bits()
    );
    assert!(advance.edges.is_empty());
}

#[test]
fn the_step_cap_and_the_frame_cap_are_different_knobs() {
    let mut driver = driver_for(SCENARIO);

    // The explicit call's footgun guard.
    let error = driver
        .step_batch(0.1, u32::MAX, Demand::Rest)
        .expect_err("an absurd batch was accepted");
    assert!(
        matches!(error, DriverError::TooManySteps { .. }),
        "got {error:?}"
    );

    // The per-frame policy, which is an argument rather than a constant — a game tunes it
    // and it must be allowed to be small without breaking anything.
    let advance = driver
        .advance_real_time(100.0, 0.1, 3, Demand::Rest, Backlog::Drop)
        .expect("tick");
    assert_eq!(advance.steps, 3);
    assert!(advance.capped);
}

#[test]
fn non_finite_arguments_are_refused_at_the_boundary() {
    let mut driver = driver_for(SCENARIO);

    for bad in [f64::NAN, f64::INFINITY] {
        assert!(
            matches!(
                driver.step_batch(bad, 1, Demand::Rest),
                Err(DriverError::OutOfRange(_))
            ),
            "dt = {bad} was accepted"
        );
        assert!(
            matches!(
                driver.step_batch(0.1, 1, Demand::Current(bad)),
                Err(DriverError::OutOfRange(_))
            ),
            "a demand of {bad} was accepted"
        );
        assert!(
            matches!(
                driver.set_env(Env {
                    t_ambient: bad,
                    t_coolant: None
                }),
                Err(DriverError::OutOfRange(_))
            ),
            "t_ambient = {bad} was accepted"
        );
        assert!(
            matches!(
                driver.advance_real_time(0.1, bad, 8, Demand::Rest, Backlog::Drop),
                Err(DriverError::OutOfRange(_))
            ),
            "fixed_dt = {bad} was accepted"
        );
    }

    assert!(matches!(
        driver.step_batch(-1.0, 1, Demand::Rest),
        Err(DriverError::OutOfRange(_))
    ));
    assert!(matches!(
        driver.step_batch(0.1, 0, Demand::Rest),
        Err(DriverError::OutOfRange(_))
    ));
}

/// `dt = 0` is legal on the explicit path (a deliberate telemetry read) but a zero
/// `fixed_dt` is not, because every frame would divide by zero and ask for infinitely
/// many steps. The two rules differ, and that is on purpose.
#[test]
fn a_zero_dt_is_legal_explicitly_but_a_zero_fixed_dt_is_not() {
    let mut driver = driver_for(SCENARIO);
    assert!(driver.step_batch(0.0, 1, Demand::Rest).is_ok());
    assert!(matches!(
        driver.advance_real_time(1.0, 0.0, 8, Demand::Rest, Backlog::Drop),
        Err(DriverError::OutOfRange(_))
    ));
    assert!(matches!(
        driver.advance_real_time(1.0, 0.1, 0, Demand::Rest, Backlog::Drop),
        Err(DriverError::OutOfRange(_))
    ));
}

#[test]
fn a_snapshot_round_trip_continues_the_same_trajectory() {
    let mut driver = driver_for(SCENARIO);
    driver.step_batch(0.5, 50, Demand::Current(2.0)).expect("a");
    let snapshot = driver.snapshot_json().expect("snapshot");

    // Continue uninterrupted.
    let mut uninterrupted = driver_for(SCENARIO);
    uninterrupted
        .step_batch(0.5, 100, Demand::Current(2.0))
        .expect("b");

    // Continue through a JSON round trip. This is the `float_roundtrip` guard: without
    // that feature the restored value can be one ULP off and this diverges.
    driver.restore_json(&snapshot).expect("restore");
    driver.step_batch(0.5, 50, Demand::Current(2.0)).expect("c");

    assert_eq!(
        driver.latest().v_terminal.to_bits(),
        uninterrupted.latest().v_terminal.to_bits(),
        "a JSON snapshot round trip changed the trajectory"
    );
    assert_eq!(
        driver.latest().soc_true.to_bits(),
        uninterrupted.latest().soc_true.to_bits()
    );
}

#[test]
fn a_restore_refuses_a_differently_shaped_pack() {
    let mut small = driver_for(SCENARIO);
    // Same scenario, differently shaped. Derived from the committed file rather than
    // hand-written, so a new required field in `PackConfig` cannot leave this fixture
    // quietly testing a parse failure instead of a topology mismatch — which is exactly
    // what a hand-rolled `[pack]` table did on the first run of this test.
    let scenario_text = std::fs::read_to_string(SCENARIO).expect("scenario");
    let reshaped = scenario_text
        .replace("series = 1", "series = 9")
        .replace("parallel = 1", "parallel = 3");
    assert!(
        reshaped.contains("series = 9") && reshaped.contains("parallel = 3"),
        "the fixture's topology keys moved; this test is no longer reshaping anything"
    );
    let chem = std::fs::read_to_string("../../chemistries/lfp_26650_generic.toml").expect("chem");
    let big = PackDriver::new(&reshaped, Some(&chem)).expect("a 9S3P pack builds");

    let error = small
        .restore_json(&big.snapshot_json().expect("snapshot"))
        .expect_err("a mismatched snapshot was accepted");
    assert!(
        matches!(error, DriverError::TopologyMismatch { .. }),
        "got {error:?}"
    );
}

#[test]
fn a_restart_returns_the_run_to_its_beginning() {
    let mut driver = driver_for(SCENARIO);
    let fresh = driver.latest().soc_true;
    driver
        .step_batch(1.0, 200, Demand::Current(2.0))
        .expect("a");
    assert!(driver.sim_time_s() > 0.0);
    assert!(
        driver.latest().soc_true < fresh,
        "the pack did not discharge"
    );

    driver.restart(driver.bms_enabled()).expect("restart");
    assert_eq!(driver.sim_time_s(), 0.0);
    assert_eq!(
        driver.latest().soc_true.to_bits(),
        fresh.to_bits(),
        "a restart did not return the pack to its initial state"
    );
    assert_eq!(driver.pending_s(), 0.0, "a restart kept carried frame time");
}

/// Removing a BMS from a scenario whose faults target *sensors* has to drop those faults,
/// because sensors belong to the BMS — and it has to say how many, so a game can tell a
/// student which of the scenario's misfortunes it is no longer reproducing.
#[test]
fn turning_the_bms_off_drops_the_sensor_faults_and_counts_them() {
    let mut driver = driver_for(SHORT_SCENARIO);
    assert!(driver.facts().scenario_has_bms);
    assert!(driver.facts().has_bms);
    assert_eq!(driver.facts().sensor_faults_dropped, 0);

    driver.restart(false).expect("rebuild without the BMS");
    assert!(!driver.facts().has_bms);
    assert!(
        driver.facts().scenario_has_bms,
        "the scenario still configures a BMS; only this build lacks one"
    );
    assert!(
        driver.facts().sensor_faults_dropped > 0,
        "this fixture's sensor fault was silently kept or silently lost"
    );

    // And back again — the scenario was never mutated, so its BMS is still there.
    driver.restart(true).expect("rebuild with the BMS");
    assert!(driver.facts().has_bms);
    assert_eq!(driver.facts().sensor_faults_dropped, 0);
}

#[test]
fn a_scenario_naming_a_chemistry_says_so_before_it_fails() {
    let scenario = std::fs::read_to_string(SCENARIO).expect("scenario");
    let id = PackDriver::chemistry_id_of(&scenario)
        .expect("parses")
        .expect("this fixture names a chemistry");
    assert!(!id.is_empty());

    let error = PackDriver::new(&scenario, None).expect_err("built with no chemistry");
    match error {
        DriverError::ChemistryNotSupplied { id: named } => assert_eq!(named, id),
        other => panic!("got {other:?}"),
    }
    // An empty string is the same as nothing — GDScript has no null for a `GString`.
    assert!(matches!(
        PackDriver::new(&scenario, Some("   ")),
        Err(DriverError::ChemistryNotSupplied { .. })
    ));
}

#[test]
fn the_demand_dialect_is_the_one_every_other_client_speaks() {
    assert_eq!(
        PackDriver::demand_from_json("{\"Current\": -5.0}").unwrap(),
        Demand::Current(-5.0)
    );
    assert_eq!(
        PackDriver::demand_from_json("\"Rest\"").unwrap(),
        Demand::Rest
    );
    assert!(PackDriver::demand_from_json("{\"current\": -5.0}").is_err());
}

#[test]
fn cells_are_series_major_and_cover_the_whole_pack() {
    let driver = driver_for(SCENARIO);
    let facts = driver.facts();
    let cells = driver.cells();
    assert_eq!(
        cells.len(),
        usize::from(facts.series) * usize::from(facts.parallel)
    );
}
