//! The thermal integrator above the explicit sub-step cap.
//!
//! Below the cap the network is explicit Euler with automatic sub-stepping and these
//! tests do not reach it. Above it — `dt` past `MAX_SUBSTEPS · SUBSTEP_SAFETY ·
//! C_th / a_max`, about 1.7 hours for the shipped LFP parameters — the explicit
//! sub-step is longer than its own stability *ceiling*, and further up it is longer
//! than the true stability *limit* too, at which point the temperatures do not merely
//! lose accuracy: they overflow. The gap between those two is a factor of ~2.6, because
//! the ceiling is a safety factor on a conservative bound; it is measured in
//! `docs/plans/thermal-implicit-integrator.md`. This file pins both ends — the
//! integrator that runs just above the gate, and the day-long step that used to
//! overflow.
//!
//! Every test here uses the same flat-OCV, temperature-independent-`R0` synthetic cell as
//! `thermal.rs`, for the same reason stated there — heating cannot feed back into the
//! electrical solve, so the analytic checks stay exact.

use sim_core::chem::ThermalParams;
use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::thermal::exposure;
use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

const CAP_AH: f64 = 2.5;
const V0: f64 = 3.30;
const R0: f64 = 0.02;
const R_RC: f64 = 0.01;
const TAU_RC_S: f64 = 20.0;
const C_TH: f64 = 95.0;
const HA: f64 = 0.35;
const T_ENV: f64 = 298.15;

/// `SUBSTEP_SAFETY` and `MAX_SUBSTEPS`, mirrored from `sim_core::thermal` (both are
/// private). The gate these tests have to cross is `dt > MAX_SUBSTEPS ·
/// SUBSTEP_SAFETY · C_th / a_max`, and `a_max = max(4·k, hA)`.
const SUBSTEP_SAFETY: f64 = 0.5;
const MAX_SUBSTEPS: f64 = 512.0;

/// The `dt` above which a pack with this chemistry and this neighbour conductance
/// leaves the explicit path.
fn implicit_gate_s(k: f64) -> f64 {
    let a_max = (4.0 * k).max(HA);
    MAX_SUBSTEPS * SUBSTEP_SAFETY * C_TH / a_max
}

fn env() -> Env {
    Env {
        t_ambient: T_ENV,
        t_coolant: None,
    }
}

/// Flat-OCV, temperature-independent-`R0`, single-RC synthetic cell — the same one
/// `thermal.rs` builds, without the optional entropy coefficient.
fn chem() -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah: 0.0,
        },
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "thermal-implicit-test".into(),
            name: "Thermal implicit test cell".into(),
            provenance: "thermal integrator test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 5.0,
            v_min: 0.0,
            max_charge_c: 20.0,
            max_discharge_c: 20.0,
            t_charge_min_k: 250.0,
            t_max_k: 350.0,
        },
        ocv: OcvTable {
            soc: vec![0.0, 1.0],
            volts: vec![V0, V0],
            docv_dt_v_per_k: None,
            t_ref_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![R0], vec![R0]],
        },
        rc: vec![RcPair {
            r_ohms: R_RC,
            c_farad: TAU_RC_S / R_RC,
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: C_TH,
            h_area_w_per_k: HA,
        },
    }
}

fn config(series: u16, parallel: u16, k: f64) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        series,
        parallel,
        initial_soc: 0.9,
        initial_temp_k: T_ENV,
        seed: 0,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: k,
        },
        cell_model: CellModelConfig::Ecm,
    }
}

/// A lone cell above the cap follows the backward-Euler closed form exactly.
///
/// # Why `k` is 1000
///
/// The sub-step ceiling is built from `a_max = max(4·k, hA)`, a bound over neighbour
/// counts rather than a per-cell conductance — so on a **1S1P** pack, which has no
/// neighbour to conduct to, `k` changes no physics whatsoever and only decides how
/// conservative the bound is. Setting it to 1000 drops the gate from 6080 s to 6.08 s,
/// which is what lets this test enter the implicit path at a `dt` where the answer is
/// still *mid-transient*. Enter it at a `dt` of hours instead and the decay factor is
/// `e^(−35)`: every scheme that is stable at all returns ambient to the last bit, and
/// the test would assert nothing about the integrator.
///
/// # The closed form
///
/// A lone cell has `exposure = 1` and no neighbours, so its node conductance is exactly
/// `hA` and the sub-step is `T ← (T + (h/C)(q + hA·T_env)) / (1 + h·hA/C)`. Iterated `n`
/// times from `T₀` at constant `q` that is
///
/// ```text
/// T_n = T∞ + (T₀ − T∞)·(1 + h·hA/C)^(−n),   T∞ = T_env + q/hA
/// ```
///
/// which is asserted here against the engine, with `q = 0` (rest, so `T∞ = T_env`
/// exactly and no heat-model detail enters).
#[test]
fn a_lone_cell_above_the_cap_matches_the_backward_euler_closed_form() {
    const K: f64 = 1000.0;
    const DT: f64 = 10.0;
    let gate = implicit_gate_s(K);
    assert!(
        DT > gate,
        "this test must take the implicit path: dt = {DT} s, gate = {gate} s"
    );

    let mut pack = Pack::new(&config(1, 1, K), chem()).unwrap();
    // Warm the cell with an ordinary fine-`dt` load, so the coarse step below starts
    // from a temperature that is not ambient and the decay has something to decay.
    for _ in 0..600 {
        pack.step(1.0, Demand::Current(9.0), &env());
    }
    let t0 = pack.cell(0, 0).unwrap().temp_k;
    assert!(
        t0 > T_ENV + 1.0,
        "warm-up should heat the cell well clear of ambient: {t0} K"
    );

    // One coarse rest step, taken above the gate.
    pack.step(DT, Demand::Rest, &env());
    let got = pack.cell(0, 0).unwrap().temp_k;

    let h = DT / MAX_SUBSTEPS;
    let decay = (1.0 + h * HA / C_TH).powf(-MAX_SUBSTEPS);
    let want = T_ENV + (t0 - T_ENV) * decay;
    // The decay factor must be mid-transient, or the assertion below is satisfied by
    // any stable scheme at all. See the doc comment.
    assert!(
        (0.5..0.999).contains(&decay),
        "the arm has degenerated to steady state: decay = {decay}"
    );

    // Derived tolerance, not a picked one. The engine reaches `T_n` by iterating the
    // sub-step 512 times on an **absolute** temperature near 304 K, where one ULP is
    // ~5.7e-14 K, while the closed form gets there in one `powf`. Each sub-step commits
    // at most two roundings at that scale (the add and the divide), so the drift is
    // bounded by `2 · n · eps · T`, about 6.9e-11 K here. Comparing the *excursions*
    // does not escape that: the excursion is a small difference of large numbers, and
    // the rounding happened on the large ones. Anything materially over this bound is a
    // different scheme, not arithmetic — forward Euler, the pre-slice behaviour, misses
    // by 1.6e-5 K, five orders above it.
    let bound = 2.0 * MAX_SUBSTEPS * f64::EPSILON * t0;
    let drift = (got - want).abs();
    assert!(
        drift <= bound,
        "backward Euler closed form: got {got} K, want {want} K \
         (drift {drift:e} K, bound {bound:e} K, excursion {} vs {})",
        got - T_ENV,
        want - T_ENV
    );
}

/// A pack whose current is small enough to last a day, so a day-long step has a
/// non-trivial steady state to land on. 0.05 A per cell for 86 400 s is 1.2 A·h of the
/// cell's 2.5, which starting from 90 % never approaches either clamp.
const DAY_CELL_A: f64 = 0.05;

/// Run a day-long step against a fine-`dt` reference, assert both the steady state and
/// the agreement, and hand back the coarse arm's temperatures for a shape assertion.
///
/// This is the regime the slice exists for. At `dt` = 86 400 s the explicit path clamps
/// to 512 sub-steps of **168.75 s**, and the fastest mode of a 3×3 grid at `k` = 1 has
/// `λ ≈ 6.06 W/K`, so its explicit amplification factor is `1 − 168.75·6.06/95 ≈ −9.8`:
/// raised to the 512th power that overflows to a non-finite temperature. Backward Euler
/// has no such bound.
///
/// # What "the steady state" is asserted against
///
/// Two independent things, neither of which is a second integrator:
///
/// * **The per-cell heat balance closes.** At a true steady state
///   `q + Σk·(T_j − T_i) + exposure_i·hA·(T_env − T_i)` is zero for every cell. 86 400 s
///   is over a hundred of this pack's slow-mode time constants (measured at ≈ 820 s; see
///   `docs/plans/thermal-implicit-integrator.md`), so the physical residual is nil and the
///   bound is set by floating-point cancellation instead: the temperature *differences*
///   are ~1e-4 K on numbers of magnitude 298, so each term carries ~3e-14 K of rounding,
///   and 1e-11 W is that with three orders of margin.
/// * **It is where a fine-`dt` run arrives.** Both arms are a hundred time constants in,
///   so this is an agreement between two converged answers, not a race between two
///   transients.
///
/// The caller adds the gradient, so a solver that returned a uniform "everything is
/// ambient" cannot pass on the residual alone.
fn day_long_steady_state(series: usize, parallel: usize) -> Vec<f64> {
    const K: f64 = 1.0;
    const DT_COARSE: f64 = 86_400.0;
    const WARMUP_S: usize = 400;
    #[allow(clippy::cast_precision_loss)]
    let i_pack = DAY_CELL_A * parallel as f64;

    let gate = implicit_gate_s(K);
    assert!(
        DT_COARSE > gate,
        "this test must take the implicit path: dt = {DT_COARSE} s, gate = {gate} s"
    );

    #[allow(clippy::cast_possible_truncation)]
    let build = || Pack::new(&config(series as u16, parallel as u16, K), chem()).unwrap();
    let mut coarse = build();
    let mut fine = build();

    // Settle the RC pair in both arms before their `dt` diverges. A step holds its heat
    // constant across the whole step, and `ecm::cell_heat_w` charges `i·(i·R0 + V_rc)`
    // with the *actual* overpotential — so a day-long step from a fresh pack would burn
    // the unsettled `i²·R0` for a day while the fine arm settled onto `i²·(R0 + R_rc)`,
    // and the two arms would differ by a third for a reason that is not the integrator.
    // 400 s is 20 RC time constants.
    for _ in 0..WARMUP_S {
        coarse.step(1.0, Demand::Current(i_pack), &env());
        fine.step(1.0, Demand::Current(i_pack), &env());
    }

    let tele = coarse.step(DT_COARSE, Demand::Current(i_pack), &env());
    #[allow(clippy::cast_possible_truncation)]
    let n_fine = DT_COARSE as usize;
    for _ in 0..n_fine {
        fine.step(1.0, Demand::Current(i_pack), &env());
    }

    // The cells are identical and unscattered, so the pack total splits evenly. Read
    // from the coarse step, because that is the heat it actually held constant.
    #[allow(clippy::cast_precision_loss)]
    let q_cell = tele.q_gen_w / (series * parallel) as f64;
    assert!(
        q_cell > 0.0,
        "the arm needs live heat to have a non-trivial steady state: {q_cell} W"
    );

    let temp = |pack: &Pack, s: usize, p: usize| pack.cell(s, p).unwrap().temp_k;
    let mut out = Vec::with_capacity(series * parallel);
    for s in 0..series {
        for p in 0..parallel {
            let t = temp(&coarse, s, p);
            assert!(
                t.is_finite(),
                "the day-long step diverged at {s},{p}: {t} K"
            );

            let mut flow = 0.0;
            if s > 0 {
                flow += K * (temp(&coarse, s - 1, p) - t);
            }
            if s + 1 < series {
                flow += K * (temp(&coarse, s + 1, p) - t);
            }
            if p > 0 {
                flow += K * (temp(&coarse, s, p - 1) - t);
            }
            if p + 1 < parallel {
                flow += K * (temp(&coarse, s, p + 1) - t);
            }
            flow += exposure(s, p, series, parallel) * HA * (T_ENV - t);
            let residual = q_cell + flow;
            assert!(
                residual.abs() < 1e-11,
                "cell {s},{p} of {series}S{parallel}P is not at steady state: \
                 residual {residual} W (q {q_cell} W, T {t} K)"
            );

            let f = temp(&fine, s, p);
            assert!(
                (t - f).abs() < 1e-9,
                "cell {s},{p} of {series}S{parallel}P: coarse {t} K vs fine {f} K"
            );
            out.push(t);
        }
    }
    out
}

/// A day-long step on a 3S3P block is stable, and lands on the steady state.
///
/// Before this slice the same call returned `NaN`.
#[test]
fn a_day_long_step_on_a_coupled_pack_is_stable_and_lands_on_the_steady_state() {
    let t = day_long_steady_state(3, 3);
    let at = |s: usize, p: usize| t[s * 3 + p];
    let (centre, edge, corner) = (at(1, 1), at(0, 1), at(0, 0));
    assert!(
        centre > edge && edge > corner && corner > T_ENV + 1e-6,
        "gradient should be centre > edge > corner > ambient: {centre} / {edge} / {corner}"
    );
}

/// The same, on a pack with a single series element — the one topology where the band's
/// width is decided by the other arm of its branch.
///
/// `Banded::assemble` sets the half-bandwidth to `parallel`, *except* when `series` is 1,
/// where there is no series neighbour and the band is tridiagonal however wide the pack
/// is. 1S1P cannot tell those two arms apart (`parallel` is 1 either way) and 3S3P only
/// exercises the first, so without this case the branch ships unmeasured: a `1S3P` pack
/// is the only shape where the arms disagree. It is also a real topology — cells in
/// parallel with no series string is how a single-voltage pack is built.
#[test]
fn a_day_long_step_on_a_single_series_pack_is_stable_and_lands_on_the_steady_state() {
    let t = day_long_steady_state(1, 3);
    // A 1×3 chain: the middle cell has two neighbours and keeps half its ambient
    // coupling, the ends have one each and keep three quarters. So the middle runs
    // hottest and the two ends are equal by symmetry.
    let (middle, left, right) = (t[1], t[0], t[2]);
    assert!(
        middle > left && left > T_ENV + 1e-6,
        "the middle of a 1S3P chain should be hottest: {left} / {middle} / {right}"
    );
    // The two ends are equal by symmetry — but only to within a rounding, and that is a
    // property of the implicit path worth pinning rather than hiding. `euler_substep` is
    // a Jacobi sweep, so symmetric positions come out bit-identical; a banded solve is
    // forward-then-back substitution, which visits the chain in an order, so cell 0 and
    // cell 2 reach the same answer through different arithmetic and can differ in the
    // last bit (measured: exactly one ULP). Determinism is untouched — the order is
    // fixed, so the same binary gives the same bits — but bit-exact spatial symmetry is
    // not something a direct solve promises.
    assert!(
        (left - right).abs() <= 2.0 * f64::EPSILON * left,
        "the ends of a chain should agree to a rounding: {left} vs {right}"
    );
}

/// Crossing the gate does not change the answer.
///
/// `dt` = 7000 s is just above the 6080 s gate, and it is a regime where the explicit
/// path is still *stable* — measured: its sub-step is 13.67 s against a true stability
/// limit near 31 s, which is why the cap binding and actual divergence are a factor of
/// ~2.6 apart (see the note). So this test passes on the pre-slice code as well, by
/// design: what it guards is that the new path is *right* near the gate, not that it is
/// reachable. A broken solve — a dropped conduction term, a wrong sign, a factorisation
/// that does not invert the matrix — fails here even though it is stable.
///
/// The tolerance is derived: both arms sit ~8e-6 K short of steady state at this `dt`
/// (nine slow-mode time constants), and the two schemes' remaining excursions differ by
/// second-order terms worth ~3e-7 K, so 1e-6 K separates "the same answer" from "8× the
/// convergence shortfall".
#[test]
fn crossing_the_gate_does_not_change_the_answer() {
    const K: f64 = 1.0;
    const DT_COARSE: f64 = 7000.0;
    const I_A: f64 = 1.5;
    const WARMUP_S: usize = 400;

    let gate = implicit_gate_s(K);
    assert!(
        DT_COARSE > gate,
        "this test must take the implicit path: dt = {DT_COARSE} s, gate = {gate} s"
    );

    let build = || Pack::new(&config(3, 3, K), chem()).unwrap();
    let mut coarse = build();
    let mut fine = build();
    for _ in 0..WARMUP_S {
        coarse.step(1.0, Demand::Current(I_A), &env());
        fine.step(1.0, Demand::Current(I_A), &env());
    }

    coarse.step(DT_COARSE, Demand::Current(I_A), &env());
    #[allow(clippy::cast_possible_truncation)]
    let n_fine = DT_COARSE as usize;
    for _ in 0..n_fine {
        fine.step(1.0, Demand::Current(I_A), &env());
    }

    for s in 0..3 {
        for p in 0..3 {
            let c = coarse.cell(s, p).unwrap().temp_k;
            let f = fine.cell(s, p).unwrap().temp_k;
            assert!(c.is_finite(), "coarse step diverged at {s},{p}: {c}");
            assert!(
                (c - f).abs() < 1e-6,
                "cell {s},{p}: coarse {c} K vs fine {f} K"
            );
        }
    }
}
