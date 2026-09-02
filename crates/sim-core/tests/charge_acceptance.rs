//! The charge-acceptance taper: a cell that stops storing charge gradually as it fills.
//!
//! `docs/plans/phase-8-slice-c-spike.md` measured that the hard SOC clamp takes charge
//! acceptance from 100 % to 0 % in one timestep, so the top of every NiMH charge was a
//! corner rather than the dome a real cell makes, and named the fix as "a third mechanism"
//! that two slices then deliberately cut. This is that mechanism, and these are the
//! properties it has to have to be worth its snapshot bump:
//!
//! * a chemistry **without** the section must not move by a ULP (structural, and measured
//!   here on the one path that could break it — the ordinary count is the same call);
//! * a chemistry **with** it must not move either until the cell reaches the onset;
//! * the update must be **exact** — step-size invariant, the way the RC pairs are — so the
//!   same code serves real-time stepping and a fast-forward;
//! * the books must still close: what the charger delivered is what was stored plus what
//!   was refused, to the amp-second;
//! * the refusal must **ramp** rather than switch, which is the whole point;
//! * the cell must never reach full and never enter the hard clamp on a charge; and
//! * a snapshot taken mid-taper must continue bit-identically, because nothing was added
//!   to the cell state.
//!
//! The fixture is deliberately not a real chemistry: flat resistance, a straight OCV, and
//! an onset at 0.90 so the taper occupies a legible tenth of the range. What the shipped
//! NiMH file does with it is measured in `sim-data`'s `nimh_chemistry.rs`.

use sim_core::chem::{
    CellLimits, ChargeAcceptanceParams, ChemMeta, ChemistryError, ChemistryParams, OcvTable,
    R0Table, RcPair, ThermalParams,
};
use sim_core::ecm::{coulomb_step, coulomb_step_tapered};
use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};

const ONSET: f64 = 0.90;
const CAPACITY_AH: f64 = 3.0;

fn chem(charge_acceptance: Option<ChargeAcceptanceParams>) -> ChemistryParams {
    ChemistryParams {
        charge_acceptance,
        diffusion: None,
        hysteresis: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah: 0.0,
        },
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 55.0,
            h_area_w_per_k: 0.03,
        },
        meta: ChemMeta {
            id: "ca".into(),
            name: "Charge-acceptance test cell".into(),
            provenance: "charge-acceptance test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAPACITY_AH,
            v_max: 1.60,
            v_min: 1.00,
            max_charge_c: 1.0,
            max_discharge_c: 10.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            t_ref_k: None,
            soc: vec![0.0, 1.0],
            volts: vec![1.20, 1.40],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
    }
}

fn tapered() -> ChemistryParams {
    chem(Some(ChargeAcceptanceParams { soc_onset: ONSET }))
}

fn config(initial_soc: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k: 298.15,
        seed: 0,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Ecm,
    }
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// One step's worth of what a test compares: every telemetry field a cell model can move.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Row {
    v: f64,
    soc: f64,
    i_rejected: f64,
    q_gen: f64,
    flags: EventFlags,
}

fn run(chem: ChemistryParams, soc0: f64, i: f64, dt: f64, steps: usize) -> Vec<Row> {
    let mut pack = Pack::new(&config(soc0), chem).expect("the pack builds");
    let env = env();
    (0..steps)
        .map(|_| {
            let t = pack.step(dt, Demand::Current(i), &env);
            Row {
                v: t.v_terminal,
                soc: t.soc_true,
                i_rejected: t.i_rejected_a,
                q_gen: t.q_gen_w,
                flags: t.flags,
            }
        })
        .collect()
}

// --- the validator -------------------------------------------------------------------

#[test]
fn an_onset_outside_the_window_is_rejected() {
    for bad in [1.0, 1.5, -0.1, f64::NAN, f64::INFINITY] {
        let c = chem(Some(ChargeAcceptanceParams { soc_onset: bad }));
        assert!(
            matches!(c.validate(), Err(ChemistryError::BadRange { .. })),
            "soc_onset = {bad} must be refused"
        );
    }
    // Zero is a taper over the whole range: odd, but well-defined, and the closed form
    // holds for it. Refusing it would be a judgement the validator has no basis for.
    for good in [0.0, 0.5, ONSET, 0.999] {
        chem(Some(ChargeAcceptanceParams { soc_onset: good }))
            .validate()
            .unwrap_or_else(|e| panic!("soc_onset = {good} must validate: {e}"));
    }
}

// --- the pure function ---------------------------------------------------------------

/// Below the onset the taper *is* the ordinary count — the same function is called — so a
/// step that never reaches it agrees bit-for-bit, including a deficit being repaid.
#[test]
fn below_the_onset_the_taper_is_the_ordinary_count_to_the_bit() {
    for (soc, deficit) in [(0.10, 0.0), (0.0, 0.05), (0.85, 0.0)] {
        let a = coulomb_step(soc, deficit, -3.0, 1.0, CAPACITY_AH, 1.0);
        let b = coulomb_step_tapered(soc, deficit, -3.0, 1.0, CAPACITY_AH, 1.0, ONSET);
        assert_eq!(
            a.soc.to_bits(),
            b.soc.to_bits(),
            "soc from ({soc}, {deficit})"
        );
        assert_eq!(a.soc_deficit.to_bits(), b.soc_deficit.to_bits());
        assert_eq!(a.rejected_as.to_bits(), b.rejected_as.to_bits());
        assert_eq!(a.flags, b.flags);
    }
}

/// The update is exact for a piecewise-constant current, so one long step and many short
/// ones agree to rounding — the property the RC pairs have, and the one that lets the same
/// code serve a real-time client and a fast-forward. Checked across the onset crossing,
/// which is the one place the closed form has to be split.
#[test]
fn the_exact_update_is_step_size_invariant() {
    let i = -CAPACITY_AH; // 1 C charge
    let total_s = 1800.0;
    let one = coulomb_step_tapered(0.80, 0.0, i, total_s, CAPACITY_AH, 1.0, ONSET);

    for n in [2_usize, 30, 1800, 18_000] {
        let dt = total_s / n as f64;
        let mut soc = 0.80;
        let mut refused_as = 0.0;
        for _ in 0..n {
            let s = coulomb_step_tapered(soc, 0.0, i, dt, CAPACITY_AH, 1.0, ONSET);
            soc = s.soc;
            refused_as += s.rejected_as;
        }
        assert!(
            (soc - one.soc).abs() < 1e-12,
            "{n} steps land at {soc:.15}, one step at {:.15}",
            one.soc
        );
        assert!(
            (refused_as - one.rejected_as).abs() < 1e-8,
            "{n} steps refuse {refused_as:.9} As, one step {:.9} As",
            one.rejected_as
        );
    }
    // And the one-step answer is the closed form itself: 360 s to the onset at full
    // acceptance, then (1 − soc) decays with time constant (1 − onset)·3600 s = 360 s.
    let expected = 1.0 - (1.0 - ONSET) * (-(total_s - 360.0) / 360.0).exp();
    assert!(
        (one.soc - expected).abs() < 1e-12,
        "closed form {expected:.15} against {:.15}",
        one.soc
    );
}

/// What the charger delivered is what was stored plus what was refused, to the
/// amp-second, on every step — the invariant `Telemetry::i_rejected_a` exists to make
/// writable, checked here on the function that produces it.
#[test]
fn delivered_is_stored_plus_refused_on_every_step() {
    let i = -CAPACITY_AH;
    let dt = 7.3; // deliberately not a round number of anything
    let capacity_as = 3600.0 * CAPACITY_AH;
    let mut soc = 0.50;
    for _ in 0..2000 {
        let s = coulomb_step_tapered(soc, 0.0, i, dt, CAPACITY_AH, 1.0, ONSET);
        let delivered = -i * dt;
        let stored = (s.soc - soc) * capacity_as;
        let refused = -s.rejected_as;
        assert!(
            (delivered - stored - refused).abs() < 1e-9,
            "at soc {soc:.6}: delivered {delivered} = stored {stored} + refused {refused}"
        );
        assert!(
            refused >= 0.0,
            "a refusal can only be of charge that was offered"
        );
        assert!(
            s.soc < 1.0,
            "the taper approaches full and never reaches it"
        );
        assert_eq!(s.soc_deficit, 0.0);
        soc = s.soc;
    }
    assert!(
        1.0 - soc < 1e-12,
        "after 2000 × 7.3 s at 1 C the cell is full to rounding"
    );
}

// --- the pack --------------------------------------------------------------------------

/// A chemistry without the section takes the ordinary path. This is a structural claim —
/// the `None` arm is the unchanged call — and this test is the measurement beside it: a
/// pack driven into and through the hard clamp reports exactly what the fixture reported
/// before the section existed, which is a clamp in one step and total refusal after it.
#[test]
fn a_chemistry_without_the_section_still_clamps_in_one_step() {
    let rows = run(chem(None), 0.95, -CAPACITY_AH, 1.0, 400);
    let first = rows
        .iter()
        .position(|r| r.flags.contains(EventFlags::SOC_CLAMPED_HIGH))
        .expect("the cell fills");
    assert!(
        rows[first - 1].i_rejected == 0.0
            && (rows[first + 1].i_rejected - (-CAPACITY_AH)).abs() < 1e-9,
        "the clamp refuses nothing on the step before it ({}) and everything on the step \
         after ({})",
        rows[first - 1].i_rejected,
        rows[first + 1].i_rejected
    );
    assert_eq!(rows[first + 1].soc, 1.0);
}

/// With the section declared, nothing moves until the cell reaches the onset — bit for bit
/// against the same pack without it, over the whole approach. The taper is a path the
/// solve, the heat and the flags do not enter below its onset.
#[test]
fn nothing_moves_below_the_onset() {
    let i = -CAPACITY_AH;
    let dt = 0.5;
    // From 0.10 at 1 C the onset is 0.80 of capacity away: 2880 s, or 5760 steps. Stop
    // one step short, so no step's *end* state crosses it.
    let steps = 5759;
    let plain = run(chem(None), 0.10, i, dt, steps);
    let taper = run(tapered(), 0.10, i, dt, steps);
    assert_eq!(plain.len(), taper.len());
    for (k, (a, b)) in plain.iter().zip(&taper).enumerate() {
        assert_eq!(
            a, b,
            "step {k}: the taper moved a cell below its onset — {a:?} against {b:?}"
        );
    }
    assert!(
        taper.last().unwrap().soc < ONSET,
        "the run was supposed to stop short of the onset"
    );
}

/// Discharge takes the ordinary count whatever the file says: a cell above the onset
/// being *drained* is bit-identical with and without the section.
#[test]
fn discharge_above_the_onset_is_untouched() {
    let plain = run(chem(None), 0.98, CAPACITY_AH, 0.5, 600);
    let taper = run(tapered(), 0.98, CAPACITY_AH, 0.5, 600);
    assert_eq!(plain, taper);
}

/// **The point of the mechanism.** Under the clamp the refused current goes from nothing to
/// everything in one step; under the taper it ramps, and the largest change any one step
/// makes is a small fraction of the current. The heat, which is what warms the cell and
/// produces the falling voltage a charger stops on, ramps with it.
#[test]
fn refusal_ramps_instead_of_switching() {
    let i = -CAPACITY_AH;
    let dt = 0.5;
    let rows = run(tapered(), 0.85, i, dt, 7200);
    let worst_jump = rows
        .windows(2)
        .map(|w| (w[1].i_rejected - w[0].i_rejected).abs())
        .fold(0.0_f64, f64::max);
    // One step of 0.5 s at 1 C moves 1.39e-4 of capacity, which on a taper 0.10 wide is
    // 0.14 % of the current. The clamp's jump is 100 % of it.
    assert!(
        worst_jump < 0.01 * CAPACITY_AH,
        "the refused current jumped {worst_jump:.4} A in one step; the taper was supposed \
         to make that a ramp"
    );
    let worst_heat_jump = rows
        .windows(2)
        .map(|w| (w[1].q_gen - w[0].q_gen).abs())
        .fold(0.0_f64, f64::max);
    let full_refusal_w = 1.40 * CAPACITY_AH; // OCV(1.0) × 1 C
    assert!(
        worst_heat_jump < 0.01 * full_refusal_w,
        "the heat jumped {worst_heat_jump:.4} W in one step against {full_refusal_w:.2} W \
         at full refusal"
    );
    // And it does get there: by the end essentially the whole current is refused, the
    // cell is essentially full, and the flag has been up since the onset.
    let last = rows.last().unwrap();
    assert!(
        (last.i_rejected - i).abs() < 1e-3,
        "at the end {:.6} A is refused against {i} A offered",
        last.i_rejected
    );
    assert!(
        last.soc < 1.0 && 1.0 - last.soc < 1e-4,
        "soc {:.9}",
        last.soc
    );
    let first_flag = rows
        .iter()
        .position(|r| r.flags.contains(EventFlags::SOC_CLAMPED_HIGH))
        .expect("the flag rises");
    // The step that lands *on* the onset to rounding takes the ordinary count and refuses
    // nothing, so the flag rises on the first step that ends measurably above it — which
    // may be one step after the first that ends a ULP above it.
    assert!(
        rows[first_flag].soc > ONSET && rows[first_flag - 1].soc <= ONSET + 1e-9,
        "the flag rises on the first step that ends above the onset: {:.15} after {:.15}",
        rows[first_flag].soc,
        rows[first_flag - 1].soc
    );
    assert!(
        rows[first_flag..]
            .iter()
            .all(|r| r.flags.contains(EventFlags::SOC_CLAMPED_HIGH)),
        "and stays up: every step above the onset refuses something"
    );
}

/// The refused share is the taper, read off the telemetry: on a cell at charge state `s`
/// above the onset the refused fraction of the current is `(s − onset) / (1 − onset)`,
/// evaluated over the step — so it sits between the start-of-step and end-of-step values.
#[test]
fn the_refused_share_is_the_declared_taper() {
    let i = -CAPACITY_AH;
    let rows = run(tapered(), 0.85, i, 1.0, 3000);
    let refused_frac = |soc: f64| ((soc - ONSET) / (1.0 - ONSET)).clamp(0.0, 1.0);
    for w in rows.windows(2) {
        let (before, after) = (w[0].soc, w[1].soc);
        if before <= ONSET {
            continue;
        }
        let share = w[1].i_rejected / i;
        let (lo, hi) = (refused_frac(before), refused_frac(after));
        assert!(
            share >= lo - 1e-12 && share <= hi + 1e-12,
            "at soc {before:.6} -> {after:.6} the refused share is {share:.6}, outside \
             [{lo:.6}, {hi:.6}]"
        );
    }
}

/// The energy the refused charge carries is heat, at the cell's own open-circuit voltage,
/// on the same terms as the clamp — so the pack's heat is `I²R` plus `OCV·|i_rejected|`.
#[test]
fn refused_charge_is_billed_as_heat_at_the_cells_open_circuit_voltage() {
    let i = -CAPACITY_AH;
    let rows = run(tapered(), 0.85, i, 1.0, 3000);
    // The RC pair (τ = 20 s) has settled to `i·R` long before the onset at t = 180 s, so
    // from there the irreversible heat is exactly `I²·(R0 + R_rc)`; before it the pair is
    // still filling and the comparison would need a margin. Skip the approach.
    for r in rows.iter().skip(500).filter(|r| r.i_rejected != 0.0) {
        let ocv = 1.20 + 0.20 * r.soc; // the fixture's straight table, at end-of-step soc
        let ohmic = i * i * (0.02 + 0.010);
        let side = -ocv * r.i_rejected;
        assert!(
            (r.q_gen - ohmic - side).abs() < 1e-9,
            "q_gen {:.9} W against ohmic {ohmic:.9} + side {side:.9}",
            r.q_gen
        );
    }
}

/// Nothing was added to the cell state, so a snapshot taken mid-taper and restored
/// continues bit-identically — the regression `CLAUDE.md` requires of every state change,
/// run here on the one that claims not to be one.
#[test]
fn a_snapshot_mid_taper_continues_bit_identically() {
    let i = -CAPACITY_AH;
    let env = env();
    let mut a = Pack::new(&config(0.85), tapered()).expect("builds");
    for _ in 0..1200 {
        a.step(1.0, Demand::Current(i), &env);
    }
    let snap = a.snapshot();
    let mut b = Pack::restore(&snap).expect("restores");
    assert!(
        a.cell(0, 0).expect("cell (0, 0) exists").soc > ONSET,
        "the snapshot was supposed to be taken inside the taper"
    );
    for k in 0..1200 {
        let ta = a.step(1.0, Demand::Current(i), &env);
        let tb = b.step(1.0, Demand::Current(i), &env);
        assert_eq!(ta.v_terminal.to_bits(), tb.v_terminal.to_bits(), "step {k}");
        assert_eq!(ta.soc_true.to_bits(), tb.soc_true.to_bits(), "step {k}");
        assert_eq!(
            ta.i_rejected_a.to_bits(),
            tb.i_rejected_a.to_bits(),
            "step {k}"
        );
        assert_eq!(ta.q_gen_w.to_bits(), tb.q_gen_w.to_bits(), "step {k}");
        assert_eq!(ta.flags, tb.flags, "step {k}");
    }
}

/// A cell that starts *at* full — a pack built at `initial_soc = 1.0` — refuses everything
/// from the first step, which is the clamp's answer too: the taper's asymptote and the
/// clamp agree at the one point they share.
#[test]
fn a_full_cell_refuses_everything_from_the_first_step() {
    let i = -CAPACITY_AH;
    let rows = run(tapered(), 1.0, i, 1.0, 10);
    for r in &rows {
        assert_eq!(r.soc, 1.0);
        assert!(
            (r.i_rejected - i).abs() < 1e-12,
            "refused {} against {i}",
            r.i_rejected
        );
        assert!(r.flags.contains(EventFlags::SOC_CLAMPED_HIGH));
    }
}
