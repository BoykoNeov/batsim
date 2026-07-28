//! The model-agnostic cell interface (Phase 6 slice A).
//!
//! `CellModel` is the only place the engine is allowed to know a cell is an
//! equivalent circuit (`CLAUDE.md`, Phase 6). These tests cover the two things
//! that refactor has to get right and that the trajectory gate cannot see: the
//! model-neutral overpotential reading, and the floating-point fact that forces
//! the interface's shape.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

/// A cell with `n_rc` RC pairs, so both `CellModel` arms are exercised. Nothing
/// here is physical; the OCV slope only has to be monotone and the RC pairs only
/// have to have distinct, short time constants.
fn chem(n_rc: usize) -> ChemistryParams {
    ChemistryParams {
        aging: None,
        safety: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "cm".into(),
            name: "Cell-model test cell".into(),
            provenance: "cell-model interface test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: 2.5,
            v_max: 3.65,
            v_min: 2.0,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            soc: vec![0.0, 1.0],
            volts: vec![3.0, 3.5],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: (0..n_rc)
            .map(|k| RcPair {
                r_ohms: 0.010 * (k + 1) as f64,
                c_farad: 2000.0 * (k + 1) as f64,
            })
            .collect(),
    }
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn cfg() -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: 0.8,
        initial_temp_k: 298.15,
        seed: 3,
        scatter: Scatter::default(),
    }
}

/// `CellView::v_rc_sum` is filled from `CellModel::overpotential_v`, which is the
/// model-neutral reading. It must be zero at rest, grow under load with the right
/// sign, and relax back — on **both** ECM arms, since `Ecm1Rc` and `Ecm2Rc` are
/// separate match arms in every method the refactor introduced.
#[test]
fn overpotential_reads_through_the_model_neutral_accessor_on_both_arms() {
    for (label, chem) in [("1 RC pair", chem(1)), ("2 RC pairs", chem(2))] {
        let mut pack = Pack::new(&cfg(), chem).expect("pack builds");

        let at_rest = pack.cell(0, 0).expect("cell exists").v_rc_sum;
        assert_eq!(
            at_rest, 0.0,
            "{label}: a cell that has never carried current has no polarization, got {at_rest}"
        );

        for _ in 0..60 {
            pack.step(1.0, Demand::Current(2.0), &env());
        }
        let loaded = pack.cell(0, 0).expect("cell exists").v_rc_sum;
        assert!(
            loaded > 0.0,
            "{label}: discharging is positive current, so the overpotential must be \
             positive (it subtracts from terminal voltage), got {loaded}"
        );

        // Long enough for any RC pair in the shipped chemistries to decay well past
        // one time constant, but not so long that it reaches exactly zero — the
        // claim is relaxation, not annihilation.
        for _ in 0..600 {
            pack.step(1.0, Demand::Rest, &env());
        }
        let relaxed = pack.cell(0, 0).expect("cell exists").v_rc_sum;
        assert!(
            relaxed < loaded && relaxed >= 0.0,
            "{label}: at rest the overpotential must decay toward zero, went \
             {loaded} -> {relaxed}"
        );
    }
}

/// The floating-point fact that decides the shape of `CellModel::source`.
///
/// Phase 6 slice D makes the pack solve iterative by having each cell report a
/// Thévenin **tangent**, and its bit-identity claim rests on "for a linear cell
/// the tangent is exact, so the first iteration is today's closed form". That is
/// true algebraically. It is **false bit-for-bit** if the generic path
/// reconstructs the source from an evaluated terminal voltage as
/// `(V(i*) + i*·r, r)`, because for an ECM `V(i*) = e − i*·r` and
/// `(e − i*·r) + i*·r` is not `e`.
///
/// So the ECM arm of `CellModel::source` answers with `cell_source`'s own
/// expression, unchanged. This test is the evidence that the distinction is real
/// rather than pedantic — without it, "simplify the ECM arm into the generic
/// path" looks like a safe cleanup and lands as unexplained golden drift.
///
/// # The rate is the dangerous part
/// Measured over 200 000 random pack-plausible operating points, the
/// reconstruction loses bits **3.2 % of the time**. That is the worst possible
/// frequency: rare enough that a hand-picked spot check round-trips exactly and
/// reads as proof the concern was imaginary (three plausible triples were tried
/// first here, and all three round-tripped), common enough that a 1000-cell pack
/// hits it on ~30 cells every step. So this test uses **measured** lossy triples
/// rather than chosen-looking ones, and sweeps besides.
#[test]
fn reconstructing_a_source_from_an_evaluated_voltage_loses_bits() {
    // Operating points found by search, not by taste. Each is an ordinary
    // (emf, resistance, current) an LFP-class cell could present.
    let known_lossy = [
        (
            3.498_663_837_262_354_6_f64,
            0.048_379_784_312_453_225_f64,
            -8.122_808_264_515_303_f64,
        ),
        (
            4.004_729_467_074_177_f64,
            0.054_482_304_829_912_96_f64,
            -9.638_140_327_190_566_f64,
        ),
        (
            3.810_133_726_188_756_f64,
            0.050_527_248_615_515_13_f64,
            -9.752_365_110_266_668_f64,
        ),
    ];
    for (e, r, i_star) in known_lossy {
        let v = e - i_star * r; // an evaluated terminal voltage
        let reconstructed = v + i_star * r; // the "algebraically identical" source
        assert_ne!(
            reconstructed.to_bits(),
            e.to_bits(),
            "e = {e:?} behind r = {r:?} at i = {i_star:?} was expected to lose bits \
             on the round trip but did not; if this target rounds differently the \
             constraint needs re-deriving rather than trusting"
        );
    }

    // A deterministic sweep, so the *rate* is pinned rather than the anecdote. If
    // this ever comes back zero, the generic reconstruction really is safe here
    // and slice A's interface constraint can be relaxed — deliberately, with this
    // number as the evidence.
    let mut lossy = 0;
    let mut total = 0;
    for a in 0..64 {
        for b in 0..64 {
            for c in 0..16 {
                let e = 2.0 + 2.3 * f64::from(a) / 64.0;
                let r = 0.005 + 0.055 * f64::from(b) / 64.0;
                let i_star = -10.0 + 20.0 * f64::from(c) / 16.0;
                total += 1;
                if ((e - i_star * r) + i_star * r).to_bits() != e.to_bits() {
                    lossy += 1;
                }
            }
        }
    }
    assert!(
        lossy > 0,
        "the whole {total}-point sweep round-tripped exactly — the constraint this \
         test documents does not hold on this target and must be re-derived"
    );

    // The identity *does* hold at the one point where it must, which is why the
    // hazard is easy to miss: a cold pack's first iterate is zero current, so a
    // test that only probes a resting cell would see nothing wrong.
    let (e, r) = (3.291_234_567_891_234_f64, 0.021_345_6_f64);
    assert_eq!(
        ((e - 0.0 * r) + 0.0 * r).to_bits(),
        e.to_bits(),
        "at i* = 0 the reconstruction must be exact"
    );
}
