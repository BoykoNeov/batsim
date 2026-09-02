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
//! guaranteed* once a cell stops taking charge, on any chemistry with a negative `[r0]`
//! temperature slope, so "it peaks and it falls" is a fact about the charge stopping and
//! evidence about nothing. The arms below are an isothermal run (which removes the whole
//! mechanism), the shipped NMC cell (which has neither new section), and — since
//! `SNAPSHOT_VERSION` 21 — the same NiMH file with its `[charge_acceptance]` section
//! removed, which is the hard clamp this file used to run on and the corner the taper
//! exists to round. See `docs/plans/charge-acceptance.md`.
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
    i_rejected_a: f64,
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
            i_rejected_a: tm.i_rejected_a,
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
    let refusal = trace
        .iter()
        .position(|s| s.clamped)
        .expect("the cell passes its onset");

    println!("\n=== NiMH 1 C charge into overcharge (thermal live) ===");
    println!(
        "  refusal begins at t = {:.1} s; peak {:.6} V at t = {:.1} s ({:.2} K, {:.3} A refused)",
        trace[refusal].t, pk.v, pk.t, pk.temp_k, trace[pk_i].i_rejected_a
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
///
/// Since the taper, "exactly zero" has a sharper form than "it stops": the pinned cell
/// **never turns over at all**. It approaches full as an asymptote and a trickle of stored
/// charge keeps lifting its open-circuit voltage for as long as the run lasts, so the whole
/// trace is monotone non-decreasing and the peak is the last sample. Guided path step 28
/// says so in words, and this is what holds it to them.
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
    // The stronger statement the taper makes true: the pinned cell never comes down at
    // all. Every step is at or above the one before it, to the bit.
    let worst_down = trace
        .windows(2)
        .map(|w| w[0].v - w[1].v)
        .fold(0.0_f64, f64::max);
    assert_eq!(
        worst_down, 0.0,
        "the pinned cell fell {worst_down} V somewhere: with nothing warming, nothing can \
         pull the terminal down, so the trace must be monotone non-decreasing"
    );
    assert_eq!(
        pk_i,
        trace.len() - 1,
        "the pinned cell's peak must be its last sample — it never turns over"
    );
}

/// Both halves of the signal are live, and neither carries it alone.
///
/// The spike measured the ohmic channel alone at roughly half of what a charger needs,
/// which is why `ocv.t_ref_k` was built. This decomposes the shipped file's fall into the
/// two contributions by reading the same trace twice: once as the engine reports it, and
/// once with the OCV temperature term subtracted back out by hand from the chemistry's own
/// coefficient and the measured temperature rise.
///
/// Since the taper the "by difference" half is the ohmic channel **net of the storage
/// creep**: the cell is still storing a trickle past the peak, which lifts the open-circuit
/// voltage against the fall, and that lift lands in the remainder rather than in the OCV
/// term. It is why the fall is smaller than it was at a clamp. The split itself survives:
/// the OCV term is about 3.5 mV of the 5.9, the 60 % the scenario file's comment quotes.
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
    let hyst = chem
        .hysteresis
        .as_ref()
        .expect("NiMH declares [hysteresis]");
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

/// **A nickel cell being charged gets colder before it gets warmer**, which guided path
/// step 27 tells a reader to look for and which nothing else in this repo would notice.
///
/// The entropic term is `−I·T·∂U/∂T`, and with a negative `∂U/∂T` and a negative (charging)
/// current it is negative: the cell absorbs heat from the reaction rather than giving it
/// off. Over most of a 1 C charge on this file that term is larger than the irreversible
/// heating it is fighting, so `q_gen_w` is negative and the cell cools below ambient — and
/// then, as the overpotentials grow toward the top of the charge, it turns round.
///
/// It is also the observable the shipped `docv_dt_v_per_k` was *sized* against: the
/// chemistry file's own provenance note derives the coefficient from "a NiMH cell on a 1 C
/// charge stays roughly flat in temperature through the plateau and then heats sharply at
/// the end". A test that the cell cools first is the nearest thing in this repo to a check
/// on that derivation, so it is worth having for a second reason.
#[test]
fn a_charging_cell_cools_before_it_warms() {
    let chem = nimh();
    let full_trace = charge(
        &chem,
        START_SOC,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 0.0,
        },
        3240.0,
    );
    // Up to the onset, not to the old clamp instant: since the taper the cell starts
    // refusing — and heating — at 0.985 rather than at 1.0, so "the charge" this test is
    // about ends where the refusal begins. Everything after that is the next test's.
    let onset = full_trace
        .iter()
        .position(|s| s.clamped)
        .expect("the cell passes its onset inside the run");
    let trace = &full_trace[..onset];
    let coolest = trace
        .iter()
        .min_by(|a, b| a.temp_k.total_cmp(&b.temp_k))
        .expect("the charge produces samples");
    let last = trace.last().expect("the charge produces samples");
    println!("\n=== the cool part of a NiMH charge ===");
    println!(
        "  coolest {:.4} K at t = {:.1} s; full at {:.4} K ({:+.3} K on ambient)",
        coolest.temp_k,
        coolest.t,
        last.temp_k,
        last.temp_k - ROOM_K
    );
    assert!(
        coolest.temp_k < ROOM_K,
        "the cell never went below ambient: coolest {:.4} K against {ROOM_K}",
        coolest.temp_k
    );
    assert!(
        coolest.t < last.t,
        "the coolest instant is the end of the charge, so nothing turned round"
    );
    assert!(
        last.temp_k > coolest.temp_k,
        "the cell never warmed again after its minimum"
    );
    // And the whole excursion is small, which is the other half of what step 27 says: this
    // is a cell that has barely noticed a 53-minute charge, right up to the onset.
    assert!(
        (last.temp_k - ROOM_K).abs() < 2.0 && (ROOM_K - coolest.temp_k) < 2.0,
        "the charge is supposed to be nearly thermally neutral: {:.4} K coolest, {:.4} K \
         at the top, against an ambient of {ROOM_K}",
        coolest.temp_k,
        last.temp_k
    );
}

/// **Nothing turns it round.** Guided path step 28 closes by telling the reader that on
/// this file the voltage goes on falling and the cell goes on heating for as long as they
/// care to run it, and that there is nothing in the configuration that will stop it.
///
/// That sentence has no numeral in it, so neither the digit ledger nor the English ban can
/// see it — a shape `phase-8-slice-b-lto-client.md` recorded shipping *false* twice. This
/// is the instrument for it: over an hour and a half of overcharge, past the peak, the
/// terminal never rises by a single ULP and the cell never cools by one.
///
/// The run is far longer than the lesson's own, deliberately: the claim is about what
/// happens after a reader stops watching.
#[test]
fn past_the_peak_nothing_turns_round() {
    let chem = nimh();
    const LONG_S: f64 = 20_000.0;
    let trace = charge(
        &chem,
        START_SOC,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 0.0,
        },
        LONG_S,
    );
    let (pk_i, pk) = peak(&trace);
    let mut worst_rise = 0.0_f64;
    let mut worst_cool = 0.0_f64;
    for w in trace[pk_i..].windows(2) {
        worst_rise = worst_rise.max(w[1].v - w[0].v);
        worst_cool = worst_cool.max(w[0].temp_k - w[1].temp_k);
    }
    let last = trace.last().expect("the run produces samples");
    println!("\n=== {LONG_S} s of overcharge ===");
    println!(
        "  peak {:.6} V at {:.1} s; end {:.6} V at {:.2} K ({:.1} degC)",
        pk.v,
        pk.t,
        last.v,
        last.temp_k,
        last.temp_k - 273.15
    );
    assert_eq!(
        worst_rise, 0.0,
        "the terminal rose by {worst_rise} V somewhere after the peak"
    );
    assert_eq!(
        worst_cool, 0.0,
        "the cell cooled by {worst_cool} K somewhere after the peak"
    );
    // And it really does go somewhere a cell would not survive, which is the other half of
    // the sentence. 60 degC is this chemistry's own `t_max_k`.
    assert!(
        last.temp_k > chem.cell.t_max_k,
        "the run ends at {:.2} K, inside this cell's rated {:.2} K, so 'a long way past \
         anything the cell would survive' is not true of it",
        last.temp_k,
        chem.cell.t_max_k
    );
}

/// **The peak is a dome, and the clamp it replaced is the corner.** The spike measured the
/// slope of the terminal reversing 29-fold in one 0.1 s step at the top of a NiMH charge and
/// named the fix as a third mechanism; two slices cut it and wrote the lesson about a number
/// rather than a shape. `[charge_acceptance]` is that mechanism, and this is the shape,
/// measured against the one control arm that isolates it: the same file with the section
/// removed — one field mutated in Rust, so the two chemistries are provably identical
/// otherwise — which is exactly the cell this file used to ship.
///
/// The instrument is the largest one-step change in `dV/dt` within two minutes of the peak,
/// which is a curvature and not a slope, so a smooth dome scores near zero and a corner
/// scores its whole slope reversal. It is read at the page's own `dt = 0.5`, because the
/// corner's size is one step's worth of slope by construction and a finer step makes it
/// look sharper rather than rounder.
#[test]
fn the_peak_is_a_dome_and_the_clamp_it_replaced_is_a_corner() {
    const DT: f64 = 0.5;
    let tapered = nimh();
    let mut clamped = tapered.clone();
    clamped.charge_acceptance = None;
    assert!(
        tapered.charge_acceptance.is_some(),
        "the shipped NiMH file declares [charge_acceptance]; without it this test measures \
         two copies of the corner"
    );

    let run = |chem: &ChemistryParams| -> Vec<Sample> {
        let mut pack = Pack::new(
            &config(
                START_SOC,
                ThermalConfig::Network {
                    k_neighbor_w_per_k: 0.0,
                },
            ),
            chem.clone(),
        )
        .expect("pack builds");
        let env = env();
        let steps = (RUN_S / DT).round() as usize;
        let mut t = 0.0;
        (0..steps)
            .map(|_| {
                let tm = pack.step(DT, Demand::Current(-chem.cell.capacity_ah), &env);
                t += DT;
                Sample {
                    t,
                    v: tm.v_terminal,
                    temp_k: tm.t_max,
                    clamped: tm.flags.contains(EventFlags::SOC_CLAMPED_HIGH),
                    i_rejected_a: tm.i_rejected_a,
                }
            })
            .collect()
    };
    let kink = |trace: &[Sample]| -> f64 {
        let (_, pk) = peak(trace);
        let window: Vec<&Sample> = trace
            .iter()
            .filter(|s| (s.t - pk.t).abs() < 120.0)
            .collect();
        window
            .windows(3)
            .map(|w| ((w[2].v - w[1].v) - (w[1].v - w[0].v)) / DT)
            .fold(0.0_f64, |m, k| m.max(k.abs()))
    };

    let dome = run(&tapered);
    let corner = run(&clamped);
    let (dome_i, dome_pk) = peak(&dome);
    let (corner_i, corner_pk) = peak(&corner);
    let (k_dome, k_corner) = (kink(&dome), kink(&corner));
    println!("\n=== the shape of the peak, at dt = {DT} ===");
    println!(
        "  taper: peak {:.6} V at {:.1} s, {:.3} A refused there; worst kink {:.5} mV/s per step",
        dome_pk.v,
        dome_pk.t,
        -dome[dome_i].i_rejected_a,
        k_dome * 1e3
    );
    println!(
        "  clamp: peak {:.6} V at {:.1} s, {:.3} A refused one step later; worst kink {:.5} mV/s per step",
        corner_pk.v,
        corner_pk.t,
        -corner[corner_i + 1].i_rejected_a,
        k_corner * 1e3
    );

    // The corner is the whole slope reversal in one step; the dome is two orders of
    // magnitude rounder. Fifty is the bar, against a measured ratio near a hundred.
    assert!(
        k_dome * 50.0 < k_corner,
        "the taper's peak is not rounder than the clamp's by 50x: {:.5} against {:.5} mV/s \
         per step",
        k_dome * 1e3,
        k_corner * 1e3
    );
    // And the refusal is partial at the taper's peak and total one step past the clamp's,
    // which is the mechanism behind the shape: a corner is what a refusal that goes from
    // nothing to everything in one step looks like.
    let refused_at_peak = -dome[dome_i].i_rejected_a;
    assert!(
        refused_at_peak > 0.5 * tapered.cell.capacity_ah
            && refused_at_peak < 0.95 * tapered.cell.capacity_ah,
        "at the taper's peak the cell refuses {refused_at_peak:.3} A of {}: it should be \
         most of the current and not all of it",
        tapered.cell.capacity_ah
    );
    assert!(
        (-corner[corner_i + 1].i_rejected_a - clamped.cell.capacity_ah).abs() < 1e-6,
        "one step past the clamp the control refuses everything"
    );
    // The dome costs signal, and the file's onset is bounded by exactly that: the fall at
    // +10 K must still land inside the charger's window on the tapered file.
    let (fall_dome, _) = fall_at_rise_mv(&dome, 10.0).expect("the tapered cell reaches +10 K");
    let (fall_corner, _) = fall_at_rise_mv(&corner, 10.0).expect("the clamped cell reaches +10 K");
    println!("  fall at +10 K: taper {fall_dome:.3} mV, clamp {fall_corner:.3} mV");
    assert!(
        fall_dome < fall_corner,
        "the taper was measured to shrink the fall (a cell still storing a trickle is still \
         lifting its own open-circuit voltage); {fall_dome:.3} against {fall_corner:.3} mV"
    );
    assert!(
        fall_dome >= DV_BAND_MV.0,
        "the tapered fall {fall_dome:.3} mV is below the {:.0} mV a charger fires on — the \
         onset in the chemistry file has moved below the bound its provenance states",
        DV_BAND_MV.0
    );
}
