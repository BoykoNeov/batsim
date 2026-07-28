//! The nonlinear pack solve, engaged (Phase 6 slice D).
//!
//! Slice C2 shipped a cell whose voltage curves within the step and left the pack
//! solve linear: every SPM cell reported a Thévenin tangent taken at the *previous*
//! step's current, and the pack aggregated those as if they were exact. Slice D makes
//! the solve iterate — each pass re-takes every tangent where the last pass put the
//! cell — and this file is what says it works.
//!
//! # Why here and not in `sim-core`
//! `sim-core` cannot read a file, so its SPM tests run against a fixture chemistry
//! with **decimated** OCP tables (see `sim-core/tests/spm_cell.rs`). Decimation makes
//! the voltage curve more piecewise-linear than the real one, which flatters exactly
//! the thing being measured here: a tangent iteration converges trivially on a
//! straight line. Every number below — iteration counts, the size of the error the
//! iteration removes — is only honest against a full parameter set, so these run on
//! the shipped `chemistries/nmc_21700_lgm50.toml`.
//!
//! The mirror half, that an all-equivalent-circuit pack still solves in exactly one
//! pass, is `sim-core/tests/nonlinear_solve_fast_path.rs`.
//!
//! # Mixed ECM/SPM packs
//! The plan asked slice D to decide whether they are supported. They are
//! **unrepresentable**: `PackConfig::cell_model` is one value for the whole pack, so
//! there is no configuration for a build check to reject and no test to write here.
//! The solve is already mixed-ready — it asks `is_linear` per cell — and what is
//! missing is only config surface. See `CellModelConfig`'s doc comment.

use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, Snapshot, Telemetry,
    ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// Ten shells, matching `sim-data/tests/spm_pack.rs`. Slice E picks a documented
/// default from an accuracy-vs-cost curve; this is not it.
const SHELLS: usize = 10;

/// Nominal capacity of the shipped LG M50 cell \[Ah\], for turning currents into
/// C-rates in the assertions below.
const CAP_AH: f64 = 5.153_198;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn config(series: u16, parallel: u16, scatter: Scatter) -> PackConfig {
    PackConfig {
        series,
        parallel,
        initial_soc: 0.5,
        initial_temp_k: 298.15,
        seed: 11,
        scatter,
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Spm { shells: SHELLS },
    }
}

fn pack(series: u16, parallel: u16, scatter: Scatter) -> Pack {
    Pack::new(
        &config(series, parallel, scatter),
        parse_chemistry(LGM50).expect("the shipped LG M50 chemistry parses"),
    )
    .expect("an SPM pack on a chemistry with an [spm] section builds")
}

// ---------------------------------------------------------------------------
// The error the iteration exists to remove
// ---------------------------------------------------------------------------

/// A step that *changes* the current lands on the same terminal voltage the cell
/// reports once its tangent has caught up.
///
/// # What this measures, and why it is the whole slice in one assertion
/// A tangent is exact only where it is taken. Slice C2 took every SPM cell's tangent
/// at [`sim_core::spm`]'s `i_last` — the previous step's current — so a step whose
/// current *jumps* solved a straight line extrapolated from somewhere the cell no
/// longer is. The error is the curvature of the Butler–Volmer branch, and on this
/// cell a rest-to-2C jump puts it in the tens of millivolts.
///
/// The comparison is against the same pack a moment later, when `i_last` *is* the
/// current being asked for and the tangent is therefore taken exactly where it is
/// evaluated. The intervening step is 1 µs long: `i_last` moves, the concentration
/// profile does not.
///
/// **Built to fail, and measured:** with the iteration disabled — `CellModel::is_linear`
/// forced to answer `true` for `Spm`, which is the smallest perturbation that leaves
/// the pack's homogeneity assertion satisfied — this gap is **1.229e-1 V**, four
/// orders of magnitude past the tolerance asserted here. Note what that number
/// contains: the terminal voltage of a 2 C step read off a tangent taken at rest,
/// which is 123 mV of pure linearization error on a cell whose whole operating window
/// is about 1.7 V. It is not a rounding detail.
#[test]
fn a_current_jump_lands_on_the_converged_voltage_not_the_stale_tangent() {
    let i = 2.0 * CAP_AH; // 2 C, straight from rest
    let mut p = pack(1, 1, Scatter::default());

    // The jump itself, as a zero-length probe: no state moves, so this is purely the
    // solve's answer for "what voltage does this cell hold at 2 C, right now".
    let jumped = p.step(0.0, Demand::Current(i), &env());
    assert!(
        jumped.solve_iterations > 1,
        "the solve took one pass on a current jump, which means the iteration never \
         engaged and this test is measuring nothing"
    );

    // Let `i_last` catch up without letting the particles move.
    p.step(1.0e-6, Demand::Current(i), &env());
    let settled = p.step(0.0, Demand::Current(i), &env());
    assert_eq!(
        settled.solve_iterations, 1,
        "with the tangent already taken at the current being demanded, the first \
         pass is the fixed point and the residual check should exit immediately"
    );

    let gap = (jumped.v_terminal - settled.v_terminal).abs();
    assert!(
        gap < 1.0e-5,
        "the jumped step reported {} V and the settled one {} V, a gap of {gap:.3e} V \
         — the iteration did not converge onto the cell's true curve",
        jumped.v_terminal,
        settled.v_terminal
    );
}

// ---------------------------------------------------------------------------
// Kirchhoff, through the only currents the public API exposes
// ---------------------------------------------------------------------------

/// The cells of a parallel group carry currents that sum to the group's.
///
/// The property table in the phase plan says this "holds — it is Kirchhoff, and slice
/// D must not break it", and breaking it is exactly what a half-converged solve does:
/// the group node voltage comes from the aggregated tangents while each cell's own
/// current comes from its own, so if the two disagree the split stops adding up.
///
/// Per-cell currents are not public, so this reads them out of the *charge that
/// moved*: each cell's `ΔSOC · effective capacity · 3600 / dt` is the mean current it
/// carried. That is a stronger route than an accessor would be — it goes through
/// `spm::advance`'s own charge-to-stoichiometry mapping, so a solve that reported
/// consistent currents while moving the wrong amount of lithium would still fail.
///
/// The group is deliberately **imbalanced** by scatter; on identical cells the split
/// is symmetric and the assertion could not tell a correct solve from one that simply
/// halved the current.
///
/// **What it does not catch, stated rather than implied:** this passes with the
/// iteration disabled. Kirchhoff is a property of the *split*, and the split adds up
/// against whatever linearization it was built from — converged or not. It guards the
/// aggregation slice D restructured, not the convergence slice D added; the two tests
/// either side of it are what carry that.
#[test]
fn parallel_group_currents_still_sum_to_the_group_current() {
    let scatter = Scatter {
        capacity_sigma: 0.08,
        r0_sigma: 0.15,
    };
    let mut p = pack(1, 4, scatter);
    let dt = 10.0;
    let demand = 3.0 * CAP_AH; // ~0.7 C per cell across four of them

    // One step to leave rest, so the tangents are taken at a live operating point.
    p.step(dt, Demand::Current(demand), &env());

    let before: Vec<(f64, f64)> = (0..4)
        .map(|k| {
            let c = p.cell(0, k).expect("a 1S4P pack has cell 0S{k}P");
            (c.soc, CAP_AH * c.capacity_factor * c.soh_capacity)
        })
        .collect();
    let tele = p.step(dt, Demand::Current(demand), &env());
    let sum_i: f64 = (0..4)
        .map(|k| {
            let c = p.cell(0, k).expect("a 1S4P pack has cell 0S{k}P");
            let (soc0, cap) = before[k];
            (soc0 - c.soc) * cap * 3600.0 / dt
        })
        .sum();

    // 1e-9 relative is the solver tolerance's own scale, not a shrug: the node
    // voltage is converged to `SOLVE_TOL_V` and the currents are read off it.
    let rel = (sum_i - tele.i_actual).abs() / tele.i_actual.abs();
    assert!(
        rel < 1.0e-9,
        "the four cell currents summed to {sum_i} A against a group current of {} A \
         (relative {rel:.3e})",
        tele.i_actual
    );
}

// ---------------------------------------------------------------------------
// Exit criterion 3, through the nonlinear solve
// ---------------------------------------------------------------------------

/// Snapshot at the halfway mark, restore, and continue: the two telemetry streams are
/// bit-identical.
///
/// `sim-core/tests/spm_cell.rs` already makes this claim for a single SPM cell under a
/// linear solve. What is new here is that the solve *iterates*: if any part of the
/// iteration read state that the snapshot does not carry — an operating point cached
/// outside `SpmState`, a residual left over from the previous step — the restored run
/// would take a different number of passes and land somewhere else. That the pass
/// counts also match is asserted, because two runs can agree on a voltage while
/// disagreeing on how they got there.
#[test]
fn an_iterating_pack_survives_a_snapshot_bit_identically() {
    let scatter = Scatter {
        capacity_sigma: 0.05,
        r0_sigma: 0.10,
    };
    let legs = |p: &mut Pack| -> Vec<(u64, u64, u64, u32)> {
        schedule(2)
            .into_iter()
            .map(|(dt, demand)| {
                let t = p.step(dt, demand, &env());
                (
                    t.v_terminal.to_bits(),
                    t.i_actual.to_bits(),
                    t.q_gen_w.to_bits(),
                    t.solve_iterations,
                )
            })
            .collect()
    };

    let mut straight = pack(2, 2, scatter);
    for (dt, demand) in schedule(2) {
        straight.step(dt, demand, &env());
    }
    let snap: Snapshot = straight.snapshot();
    let tail_straight = legs(&mut straight);

    let mut restored = Pack::restore(&snap).expect("a v10 snapshot restores");
    let tail_restored = legs(&mut restored);

    assert_eq!(
        tail_straight, tail_restored,
        "the continued and restored streams diverged through the nonlinear solve"
    );
    assert!(
        tail_straight.iter().any(|&(_, _, _, n)| n > 1),
        "no step in the compared tail actually iterated, so this proved nothing \
         about the nonlinear solve"
    );
}

// ---------------------------------------------------------------------------
// The measurement the plan asks for
// ---------------------------------------------------------------------------

/// A schedule that crosses everything the solve can be asked to do: constant current
/// both ways, rests, current reversals, CV holds, and — the awkward one — a power
/// demand near the pack's maximum-power point.
///
/// `solve_current`'s `Power` arm selects a root through `disc <= 0`, and `disc`
/// depends on the aggregate the tangents produce. Near the knee that branch can flip
/// *between passes*, which is the one place this formulation could oscillate instead
/// of converge rather than merely converge slowly. It is included for that reason.
/// `series` scales the voltage and power legs: a CV target written for one cell is
/// not a CV hold on a four-series pack, it is a dead short, and a schedule that
/// silently turned into one would measure the solver's behaviour on an unreachable
/// operating point while claiming to measure a CV hold. (It does that too — see the
/// last leg — but deliberately and once.)
fn schedule(series: u16) -> Vec<(f64, Demand)> {
    let c = CAP_AH;
    let s = f64::from(series);
    vec![
        (1.0, Demand::Current(c)),
        (1.0, Demand::Current(2.0 * c)),
        (1.0, Demand::Rest),
        (1.0, Demand::Current(-c)), // reversal, straight from rest
        (1.0, Demand::Current(2.0 * c)),
        (5.0, Demand::Voltage(3.6 * s)), // CV hold, discharging
        (5.0, Demand::Voltage(4.0 * s)), // CV hold, charging
        (1.0, Demand::Power(20.0 * s)),
        (1.0, Demand::Power(-20.0 * s)),
        (1.0, Demand::Power(150.0 * s)), // at or past the max-power knee
        (0.0, Demand::Current(3.0 * c)), // probe step at a current never held
        (60.0, Demand::Current(c)),
        (600.0, Demand::Current(c)), // coarse step, as a fast-forward would take
    ]
}

/// Every step of that schedule converges, on a real pack, well inside the cap.
///
/// The bound is 8, against a `SOLVE_ITER_CAP` of 32 and a single-cell spike
/// measurement of 3. It is deliberately *not* set to whatever this run happens to
/// produce: a bound that tracks the measurement rejects an improvement as loudly as a
/// regression. What it pins is that the solve stays in the same league as the spike
/// rather than creeping toward its cap unnoticed.
#[test]
fn the_solve_converges_across_a_full_schedule() {
    let scatter = Scatter {
        capacity_sigma: 0.05,
        r0_sigma: 0.10,
    };
    for (label, series, mut p) in [
        ("1S1P", 1, pack(1, 1, Scatter::default())),
        ("4S3P scattered", 4, pack(4, 3, scatter)),
    ] {
        let mut worst = 0;
        for (n, (dt, demand)) in schedule(series).into_iter().enumerate() {
            let t: Telemetry = p.step(dt, demand, &env());
            assert!(
                !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "{label}: step {n} ({demand:?}) hit the iteration cap"
            );
            assert!(
                t.v_terminal.is_finite(),
                "{label}: step {n} ({demand:?}) produced a non-finite terminal voltage"
            );
            worst = worst.max(t.solve_iterations);
        }
        assert!(
            worst <= 8,
            "{label}: worst-case solver pass count was {worst}, which is drifting \
             toward the cap"
        );
        // Without this the whole test is satisfied by a solve that never iterates:
        // one pass never hits the cap and never exceeds the bound. The same
        // perturbation that `a_current_jump_lands_on_the_converged_voltage_not_the_stale_tangent`
        // documents leaves everything above green and only this line red.
        assert!(
            worst > 1,
            "{label}: no step in the schedule took more than one pass, so nothing \
             here exercised the nonlinear solve at all"
        );
    }
}
