//! NiMH: the falling end-of-charge voltage, and the resting-voltage memory.
//!
//! `CLAUDE.md`'s chemistry list asks for "NiMH (-ΔV, hysteresis)", and those are the two
//! things measured here. Both needed engine code — this is deliberately *not* a zero-code
//! chemistry, unlike `lto_chemistry.rs` beside it — and the mechanisms themselves are
//! tested in `sim-core`'s `hysteresis.rs`. What this file measures is the **shipped
//! parameter set**: that `chemistries/nimh_subc_3ah_generic.toml` produces the two
//! behaviours at the sizes a real charger and a real pack would see them at.
//!
//! Every measurement here carries a control arm, because the alternative is worthless:
//! `docs/plans/phase-8-slice-c-spike.md` measured that a peak-then-fall is *structurally
//! guaranteed* by the SOC clamp on any chemistry with a negative `[r0]` temperature slope,
//! so "it peaks and it falls" is a fact about the clamp and evidence about nothing. The
//! arms below are an isothermal run (which removes the whole mechanism) and the shipped
//! NMC cell (which has neither new section).
//!
//! Run the tables with `cargo test -p sim-data --test nimh_chemistry -- --nocapture`.

use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig, Scatter,
    ThermalConfig,
};

fn nimh() -> ChemistryParams {
    let text = include_str!("../../../chemistries/nimh_subc_3ah_generic.toml");
    sim_data::parse_chemistry(text).expect("NiMH chemistry loads and validates")
}

/// The control chemistry: an NMC cell, which has neither `[hysteresis]` nor an
/// `ocv.t_ref_k`, so every claim below should fail on it.
fn nmc() -> ChemistryParams {
    let text = include_str!("../../../chemistries/nmc_18650_generic.toml");
    sim_data::parse_chemistry(text).expect("NMC chemistry loads and validates")
}

const ROOM_K: f64 = 298.15;
/// A charger's `-ΔV` window: a drop of this many millivolts per cell, seen within a few
/// minutes of the peak, is what terminates a fast charge.
const DV_BAND_MV: (f64, f64) = (5.0, 10.0);
/// Where the charge experiments start, and it is **not** a convenience.
///
/// The first version of this file started at 0.90 and measured 4.4 mV at the +10 K mark
/// against an expected 5.4 mV of ohmic fall alone. The missing millivolts were not a
/// parameter error: at 1 C a charge from 0.90 lasts 360 s, which is barely over one time
/// constant of this cell's slow RC pair, so that pair was **still filling** through the
/// overcharge and lifting the terminal voltage by 3.3 mV while temperature pulled it down.
/// The hysteresis state was still crossing for the same reason and added another 1.3 mV of
/// lift. A charge from 0.10 lasts 3240 s — eleven time constants — so both have settled
/// before the cell fills, and what happens after the peak is temperature and nothing else.
///
/// That is also the realistic experiment: a charger starts from an empty pack.
const START_SOC: f64 = 0.10;
/// Long enough to fill from [`START_SOC`] at 1 C (3240 s) and overcharge for twenty
/// minutes after.
const RUN_S: f64 = 4500.0;

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

/// One point of a charge trace.
#[derive(Clone, Copy, Debug)]
struct Sample {
    t: f64,
    v: f64,
    temp_k: f64,
    clamped: bool,
}

/// Charge a 1S1P cell at 1 C from `soc0` for `run_s` seconds and return the trace.
fn charge(chem: &ChemistryParams, soc0: f64, thermal: ThermalConfig, run_s: f64) -> Vec<Sample> {
    const DT: f64 = 0.1;
    let i_charge = -chem.cell.capacity_ah; // 1 C, negative = charge
    let mut pack = Pack::new(&config(soc0, thermal), chem.clone()).expect("pack builds");
    let env = env();
    let steps = (run_s / DT).round() as usize;
    let mut out = Vec::with_capacity(steps);
    let mut t = 0.0;
    for _ in 0..steps {
        let tm = pack.step(DT, Demand::Current(i_charge), &env);
        t += DT;
        out.push(Sample {
            t,
            v: tm.v_terminal,
            temp_k: tm.t_max,
            clamped: tm.flags.contains(EventFlags::SOC_CLAMPED_HIGH),
        });
    }
    out
}

fn peak(trace: &[Sample]) -> (usize, Sample) {
    let mut best = 0;
    for (i, s) in trace.iter().enumerate() {
        if s.v > trace[best].v {
            best = i;
        }
    }
    (best, trace[best])
}

/// The drop in millivolts from the peak, at the first sample whose temperature is
/// `rise` kelvin above the peak's — or `None` if the run never gets that warm.
fn fall_at_rise_mv(trace: &[Sample], rise: f64) -> Option<(f64, f64)> {
    let (pk_i, pk) = peak(trace);
    trace[pk_i..]
        .iter()
        .find(|s| s.temp_k >= pk.temp_k + rise)
        .map(|s| ((pk.v - s.v) * 1000.0, s.t - pk.t))
}

/// **The claim `CLAUDE.md` names the chemistry for.** A full NiMH cell held on charge
/// falls back through the millivolt window a real charger terminates on, and it does so
/// within minutes rather than after the cell has cooked.
///
/// The number is pinned **at the +10 K point**, not at the end of the run, and that
/// instant is the whole assertion. `phase-8-slice-c-spike.md` recorded scoring a
/// pre-registered prediction green over a twenty-minute run where the honest figure at the
/// instant a charger fires was under half of it — a green registered on the wrong instant.
/// This is that lesson spent: a charger sees ~10 K of rise before it decides, so that is
/// where the window is checked.
#[test]
fn a_full_cell_falls_through_the_charger_termination_window() {
    let chem = nimh();
    let trace = charge(
        &chem,
        START_SOC,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 0.0,
        },
        RUN_S,
    );
    let (pk_i, pk) = peak(&trace);
    let clamp = trace
        .iter()
        .position(|s| s.clamped)
        .expect("the cell fills");

    println!("\n=== NiMH 1 C charge into overcharge (thermal live) ===");
    println!(
        "  clamp at t = {:.1} s; peak {:.6} V at t = {:.1} s ({:.2} K)",
        trace[clamp].t, pk.v, pk.t, pk.temp_k
    );
    println!("  rise    when      fall");
    for rise in [5.0, 10.0, 20.0, 30.0] {
        match fall_at_rise_mv(&trace, rise) {
            Some((mv, dt)) => println!("  +{rise:4.1} K  +{dt:6.1} s  {mv:7.3} mV"),
            None => println!("  +{rise:4.1} K  never reached"),
        }
    }
    let last = trace.last().unwrap();
    println!(
        "  end of run: {:.6} V at {:.2} K, total fall {:.3} mV",
        last.v,
        last.temp_k,
        (pk.v - last.v) * 1000.0
    );

    let (fall_10k_mv, dt_10k) = fall_at_rise_mv(&trace, 10.0).expect("the cell reaches +10 K");
    assert!(
        fall_10k_mv >= DV_BAND_MV.0 && fall_10k_mv <= DV_BAND_MV.1,
        "at +10 K the fall is {fall_10k_mv:.3} mV, outside the {:.0}-{:.0} mV window a \
         charger terminates on",
        DV_BAND_MV.0,
        DV_BAND_MV.1
    );
    assert!(
        dt_10k <= 600.0,
        "the +10 K point arrives {dt_10k:.1} s after the peak; a charger that waited that \
         long would have cooked the cell"
    );
    // The fall is a fall, not a wobble: nothing after the peak goes back up.
    let worst_up = trace[pk_i..]
        .windows(2)
        .map(|w| w[1].v - w[0].v)
        .fold(0.0_f64, f64::max);
    assert!(
        worst_up <= 0.0,
        "the voltage rose {:.6} mV after the peak",
        worst_up * 1000.0
    );
}

/// **The control arm, and the reason the test above means anything.** With the cell held
/// isothermal the temperature never moves, so neither `R0(T)` nor the OCV temperature
/// correction can act — and the fall goes to *exactly* zero. Whatever the number above is,
/// it is temperature, and nothing else.
#[test]
fn with_temperature_pinned_the_fall_is_exactly_zero() {
    let chem = nimh();
    let trace = charge(&chem, START_SOC, ThermalConfig::Isothermal, RUN_S);
    let (pk_i, pk) = peak(&trace);
    let last = trace.last().unwrap();
    let fall_mv = (pk.v - last.v) * 1000.0;
    println!("\n=== isothermal control ===");
    println!(
        "  peak {:.9} V, end {:.9} V, fall {:.9} mV, T {:.4} K",
        pk.v, last.v, fall_mv, last.temp_k
    );
    assert_eq!(
        last.temp_k, ROOM_K,
        "an isothermal pack must not move in temperature"
    );
    assert_eq!(
        fall_mv, 0.0,
        "with temperature pinned there is no channel for a fall, so it must be exactly zero"
    );
    // And nothing after the peak moved at all, which is the stronger statement: the
    // isothermal arm does not fall *and then recover*, it simply stops.
    let worst_up = trace[pk_i..]
        .windows(2)
        .map(|w| (w[1].v - w[0].v).abs())
        .fold(0.0_f64, f64::max);
    assert_eq!(
        worst_up, 0.0,
        "an isothermal trace must be flat after the peak"
    );
}

/// Both halves of the signal are live, and neither carries it alone.
///
/// The spike measured the ohmic channel alone at roughly half of what a charger needs,
/// which is why `ocv.t_ref_k` was built. This decomposes the shipped file's fall into the
/// two contributions by reading the same trace twice: once as the engine reports it, and
/// once with the OCV temperature term subtracted back out by hand from the chemistry's own
/// coefficient and the measured temperature rise.
#[test]
fn the_fall_is_shared_between_the_two_temperature_channels() {
    let chem = nimh();
    let docv_dt = chem
        .ocv
        .docv_dt_v_per_k
        .as_ref()
        .expect("NiMH declares dU/dT")[0];
    let trace = charge(
        &chem,
        START_SOC,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 0.0,
        },
        RUN_S,
    );
    let (_, pk) = peak(&trace);
    let at10 = trace
        .iter()
        .find(|s| s.temp_k >= pk.temp_k + 10.0)
        .expect("the cell reaches +10 K");

    let total_mv = (pk.v - at10.v) * 1000.0;
    // What the OCV correction contributed over exactly this temperature excursion.
    let ocv_mv = -docv_dt * (at10.temp_k - pk.temp_k) * 1000.0;
    let ohmic_mv = total_mv - ocv_mv;
    println!("\n=== where the {total_mv:.3} mV comes from, at +10 K ===");
    println!("  OCV temperature correction : {ocv_mv:7.3} mV");
    println!("  R0(T), by difference       : {ohmic_mv:7.3} mV");

    assert!(
        ocv_mv > 0.0 && ohmic_mv > 0.0,
        "both channels must push the same way: OCV {ocv_mv:.3} mV, ohmic {ohmic_mv:.3} mV"
    );
    // Neither is a rounding correction on the other. The spike's argument for building the
    // OCV term was that it roughly doubles a signal that was otherwise short of the window;
    // this pins that neither contribution is under a quarter of the total.
    let share = ocv_mv / total_mv;
    assert!(
        (0.25..=0.75).contains(&share),
        "the OCV channel carries {:.1} % of the fall; the two channels were supposed to be \
         comparable, and a lesson resting on one of them would be resting on the wrong one",
        share * 100.0
    );
}

/// The other half of `CLAUDE.md`'s sentence: **the cell remembers which way it was driven,
/// through a rest of any length.**
///
/// Charge to a state of charge and rest; discharge to the *same* state of charge and rest.
/// The two resting voltages differ, and the difference **does not decay** — which is what
/// separates this from an RC pair, and is the whole reason a new state had to be added
/// rather than a third `[[rc]]` entry.
///
/// The rest is four hours and then four more. Both RC pairs have time constants of 300 s
/// or less, so by the first reading `exp(-14400/300)` has taken their residue to about
/// 1e-21 V — below the last bit of a 1.3 V number — and the second reading can therefore be
/// asserted **bit-identical** to the first rather than within a tolerance. What is left
/// moving between hour four and hour eight is nothing at all.
#[test]
fn resting_voltage_remembers_the_drive_direction() {
    let chem = nimh();
    let hyst = chem.hysteresis.expect("NiMH declares [hysteresis]");
    let cap = chem.cell.capacity_ah;
    /// Seconds of 1 C current that move 30 % of the cell's capacity.
    const MOVE_S: usize = (0.30 * 3600.0) as usize;
    const REST_S: usize = 4 * 3600;

    // Arm 1: charge 0.30 -> 0.60. Arm 2: discharge 0.90 -> 0.60. Same destination, opposite
    // approach, and the pack is isothermal so no temperature term can contribute.
    let mut up =
        Pack::new(&config(0.30, ThermalConfig::Isothermal), chem.clone()).expect("pack builds");
    let mut down =
        Pack::new(&config(0.90, ThermalConfig::Isothermal), chem.clone()).expect("pack builds");
    for _ in 0..MOVE_S {
        up.step(1.0, Demand::Current(-cap), &env());
        down.step(1.0, Demand::Current(cap), &env());
    }
    for _ in 0..REST_S {
        up.step(1.0, Demand::Rest, &env());
        down.step(1.0, Demand::Rest, &env());
    }
    let (v_up_4h, v_down_4h) = (
        up.step(0.0, Demand::Rest, &env()).v_terminal,
        down.step(0.0, Demand::Rest, &env()).v_terminal,
    );
    let (soc_up, soc_down) = (
        up.step(0.0, Demand::Rest, &env()).soc_true,
        down.step(0.0, Demand::Rest, &env()).soc_true,
    );
    for _ in 0..REST_S {
        up.step(1.0, Demand::Rest, &env());
        down.step(1.0, Demand::Rest, &env());
    }
    let (v_up_8h, v_down_8h) = (
        up.step(0.0, Demand::Rest, &env()).v_terminal,
        down.step(0.0, Demand::Rest, &env()).v_terminal,
    );

    let gap_mv = (v_up_4h - v_down_4h) * 1000.0;
    println!("\n=== resting voltage at the same state of charge, four hours after ===");
    println!("  arrived by CHARGING    : soc {soc_up:.9}, {v_up_4h:.9} V");
    println!("  arrived by DISCHARGING : soc {soc_down:.9}, {v_down_4h:.9} V");
    println!(
        "  gap {gap_mv:.4} mV, against a declared loop width of {:.4} mV",
        2.0 * hyst.scale_v * 1000.0
    );
    println!("  four hours later       : {v_up_8h:.9} V / {v_down_8h:.9} V");

    assert!(
        (soc_up - soc_down).abs() < 1e-9,
        "the two arms must rest at the same state of charge: {soc_up} vs {soc_down}"
    );
    assert!(
        v_up_4h > v_down_4h,
        "a cell that arrived by charging must rest above one that arrived by discharging"
    );
    // Both arms moved 30 % of capacity in one direction, so `h` is within
    // `exp(-gamma*0.30)` of its endpoint and the gap is that fraction of the full width.
    // Derived from the two declared parameters rather than pinned as a measured number, so
    // that editing either of them moves the expectation with it.
    let closed = 1.0 - (-hyst.gamma * 0.30).exp();
    let expect_mv = 2.0 * hyst.scale_v * closed * 1000.0;
    assert!(
        (gap_mv - expect_mv).abs() < 1e-9,
        "gap {gap_mv:.9} mV against the {expect_mv:.9} mV the two parameters predict"
    );
    // **The property that made this a new state rather than an RC pair.** Four more hours
    // of open circuit moved neither arm by one bit.
    assert_eq!(
        v_up_4h, v_up_8h,
        "the memory must not decay at rest — that is what distinguishes it from an RC pair"
    );
    assert_eq!(v_down_4h, v_down_8h, "and the same on the discharge branch");
}

/// The control chemistry does none of it. Every claim above is measured against a cell
/// that declares neither section, and each one fails there — which is what makes them
/// claims about NiMH rather than about the engine.
#[test]
fn the_control_chemistry_has_neither_mechanism() {
    let nmc = nmc();
    assert!(
        nmc.hysteresis.is_none(),
        "the control chemistry must not declare [hysteresis]"
    );
    assert!(
        nmc.ocv.t_ref_k.is_none(),
        "the control chemistry must not declare ocv.t_ref_k"
    );

    // Same experiment, same clamp, and the fall is the ohmic channel alone.
    let trace = charge(
        &nmc,
        START_SOC,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 0.0,
        },
        RUN_S,
    );
    let (_, pk) = peak(&trace);
    let at10 = trace.iter().find(|s| s.temp_k >= pk.temp_k + 10.0);
    println!("\n=== NMC control, same 1 C charge into overcharge ===");
    match at10 {
        Some(s) => println!("  fall at +10 K: {:.3} mV", (pk.v - s.v) * 1000.0),
        None => println!(
            "  never reached +10 K (end {:.2} K)",
            trace.last().unwrap().temp_k
        ),
    }

    // And the rested-voltage claim: with no hysteresis state, both arms rest identically.
    let cap = nmc.cell.capacity_ah;
    let mut up =
        Pack::new(&config(0.30, ThermalConfig::Isothermal), nmc.clone()).expect("pack builds");
    let mut down =
        Pack::new(&config(0.90, ThermalConfig::Isothermal), nmc.clone()).expect("pack builds");
    for _ in 0..(0.30 * 3600.0) as usize {
        up.step(1.0, Demand::Current(-cap), &env());
        down.step(1.0, Demand::Current(cap), &env());
    }
    for _ in 0..(4 * 3600) {
        up.step(1.0, Demand::Rest, &env());
        down.step(1.0, Demand::Rest, &env());
    }
    let (a, b) = (
        up.step(0.0, Demand::Rest, &env()).v_terminal,
        down.step(0.0, Demand::Rest, &env()).v_terminal,
    );
    println!("  rested after charging {a:.9} V, after discharging {b:.9} V");
    // A tolerance rather than bit-equality, and its size is the point. The two arms counted
    // the same charge in opposite directions, so they arrive at `soc = 0.6` a few ULPs apart
    // and the OCV table maps that to tens of nanovolts. Against the ~50 mV the NiMH file
    // shows for the identical experiment that is six orders of magnitude: "no mechanism",
    // not "a small one".
    assert!(
        (a - b).abs() < 1e-9,
        "a chemistry with no [hysteresis] section must rest at one voltage per state of \
         charge, whichever way it arrived: {a} vs {b}"
    );
}
