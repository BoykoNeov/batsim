//! The diffusion overpotential, on a fixture chosen so each property can be isolated.
//!
//! `crates/sim-data/tests/lead_acid_rate.rs` is where this term is judged against Peukert
//! on the chemistry it was fitted for. This file is the other half: the mechanism's
//! *engine* properties, each against a named rival implementation that would pass the rate
//! sweep anyway.
//!
//! * the heat carries the same overpotential the voltage did, so a closed cycle balances;
//! * the depletion relaxes, so a rested cell recovers — and it relaxes on **its own** time
//!   constant, not the RC pair's;
//! * charge drives it negative, which is the one direction nothing has been fitted for;
//! * `soc = 0` saturates it, and the saturation lands exactly on the reversal floor;
//! * nothing anywhere in it produces a NaN;
//! * and the cell stays linear, so the pack's solve stays one closed-form pass.
//!
//! # The fixture separates the two time constants by 180x, on purpose
//! `τ_rc` is 20 s and `τ_d` is 1 h. Several tests below rest the cell for many RC time
//! constants and then read [`sim_core::CellView::overpotential_v`], which is the *sum* of
//! the two contributions: after such a rest the RC part is below a microvolt and what is
//! left is the diffusion term alone. Without that separation every assertion here would be
//! about a sum and could be satisfied by the wrong half of it.
//!
//! See `docs/plans/diffusion-overpotential.md`.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, DiffusionParams, OcvTable, R0Table, RcPair,
    ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig, SNAPSHOT_VERSION,
};

/// Cell capacity \[Ah\], so 1C is 2.5 A.
const CAP_AH: f64 = 2.5;

/// `OCV(0.0)` \[V\]. With `floor_v` at zero this is also `max_overpotential_v`, which is
/// the derivation [`DiffusionParams::max_overpotential_v`] states.
const OCV_EMPTY_V: f64 = 3.00;

/// Where the reversal ramp stops \[V\], and therefore where a saturated diffusion term
/// must put the cell's source too.
const FLOOR_V: f64 = 0.0;

/// The depletion's relaxation time \[s\]: 1 hour, against the RC pair's 20 seconds.
const TAU_D_S: f64 = 3600.0;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A cell with a sloped OCV, a flat `R0`, a fast RC pair and a slow depletion.
///
/// Nothing here is a physical claim about any real chemistry — the shipped lead-acid
/// numbers are fitted and live in `chemistries/pba_agm_2v_generic.toml`. These are chosen
/// so the two relaxations are unmistakably separable and so `max_overpotential_v` sits at
/// its derived value.
fn diffusive_chem() -> ChemistryParams {
    ChemistryParams {
        diffusion: Some(DiffusionParams {
            tau_s: TAU_D_S,
            limit_c_rate: 1.5,
            scale_v: 0.10,
            // The derivation: OCV(0) − floor_v. Asserted rather than assumed by
            // `saturation_lands_on_the_reversal_floor`.
            max_overpotential_v: OCV_EMPTY_V - FLOOR_V,
        }),
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: FLOOR_V,
            fade_per_ah: 0.0,
        },
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "diffusion".into(),
            name: "Diffusion test cell".into(),
            provenance: "engine test fixture — chosen to separate two time constants, not \
                         physical"
                .into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.00,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            soc: vec![0.0, 0.2, 0.5, 0.8, 1.0],
            volts: vec![OCV_EMPTY_V, 3.20, 3.30, 3.40, 3.60],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        // tau = 20 s, three orders below the depletion's hour.
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
    }
}

fn pack_at(soc0: f64) -> Pack {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: soc0,
        initial_temp_k: 298.15,
        seed: 1,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    };
    Pack::new(&config, diffusive_chem()).expect("fixture builds")
}

/// The chemistry validates, and the fixture's ceiling really is the derived one.
#[test]
fn the_section_validates_and_the_ceiling_is_derived() {
    let chem = diffusive_chem();
    chem.validate().expect("the [diffusion] section validates");

    let d = chem.diffusion.expect("the fixture declares one");
    assert!(
        (d.max_overpotential_v - (chem.ocv.volts[0] - chem.reversal.floor_v)).abs() < 1e-12,
        "the ceiling must be OCV(0) − floor_v, or `saturation_lands_on_the_reversal_floor` \
         below is asserting an arbitrary number rather than the derivation"
    );

    // Each of the four is rejected on its own, so none of them is silently optional.
    for spoil in [
        |d: &mut DiffusionParams| d.tau_s = 0.0,
        |d: &mut DiffusionParams| d.limit_c_rate = -1.0,
        |d: &mut DiffusionParams| d.scale_v = f64::NAN,
        |d: &mut DiffusionParams| d.max_overpotential_v = f64::INFINITY,
    ] {
        let mut chem = diffusive_chem();
        spoil(chem.diffusion.as_mut().expect("present"));
        assert!(
            chem.validate().is_err(),
            "every [diffusion] constant must be required positive and finite"
        );
    }
}

/// **A closed cycle balances, which is where a heat term that forgot the new
/// overpotential would show up.**
///
/// Discharge, charge the same charge back, rest until both overpotentials have relaxed.
/// The cell returns to the state it started in, so the stored-energy change is zero *by
/// construction* and the assertion contains no state-function integral:
/// `∫V·I dt + ∫Q dt = 0`.
///
/// **What this can and cannot catch, stated because the distinction has bitten this repo
/// before.** Conservation here is structural — the terminal voltage subtracts exactly
/// what the heat adds — so this test is blind to whether `η` is the *right* physics, the
/// same way the reversal tests are blind to the ramp's slope. What it is not blind to is
/// the two sides disagreeing: had `heat_w` kept summing `Σ V_rc` alone, or recomputed the
/// diffusion term from end-of-step state, the ledger would miss by `∫i·η dt` — which on
/// this cycle is tens of joules against a few hundred, in one direction, and no other
/// test in the tree would have noticed.
#[test]
fn a_cycle_through_the_depletion_conserves_energy() {
    fn cycle(dt: f64) -> (f64, f64) {
        let mut pack = pack_at(0.6);
        let (mut elec_j, mut heat_j) = (0.0, 0.0);
        let mut weigh = |t: &sim_core::Telemetry, dt: f64| {
            elec_j += t.v_terminal * t.i_actual * dt;
            heat_j += t.q_gen_w * dt;
        };
        let legs = (600.0 / dt).round() as u32;
        for _ in 0..legs {
            weigh(&pack.step(dt, Demand::Current(5.0), &env()), dt);
        }
        for _ in 0..legs {
            weigh(&pack.step(dt, Demand::Current(-5.0), &env()), dt);
        }
        // 15 depletion time constants; the RC pair cleared 180 constants ago.
        for _ in 0..15 {
            weigh(&pack.step(TAU_D_S, Demand::Rest, &env()), TAU_D_S);
        }

        let view = pack.cell(0, 0).expect("cell exists");
        assert!(
            (view.soc - 0.6).abs() < 1e-12,
            "the cycle must return the cell to where it started, got soc {}",
            view.soc
        );
        assert!(
            view.overpotential_v.abs() < 1e-6,
            "the rest must relax BOTH overpotentials; {} V left",
            view.overpotential_v
        );
        (elec_j + heat_j, heat_j)
    }

    let (coarse, _) = cycle(1.0);
    let (fine, heat) = cycle(0.5);

    // Halving the step must halve the imbalance: that is what separates first-order
    // quadrature error from energy the model invented, which would not move with `dt`.
    assert!(
        fine.abs() < 0.6 * coarse.abs(),
        "imbalance must shrink with the step: {coarse} J at dt 1.0, {fine} J at dt 0.5"
    );
    assert!(
        fine.abs() < 0.02 * heat.abs(),
        "imbalance {fine} J is not small against the {heat} J of heat beside it"
    );
}

/// **The depletion relaxes on its own hour, not on the RC pair's twenty seconds.**
///
/// The rival is a term wired to the existing RC time constant, which would reproduce a
/// rate sweep tolerably and get rest recovery badly wrong — recovering in a minute where a
/// lead-acid cell takes hours.
///
/// **The measurement inverts the overpotential rather than comparing volts**, and it has
/// to. `η = −k·ln(1 − x)` is not linear in the depletion, so "the voltage halved" says
/// nothing about how far the state moved. Inverting — `x = 1 − e^(−η/k)` — recovers the
/// depletion up to the factor `D_lim·soc`, and `soc` does not change at rest, so the ratio
/// of two `x` readings *is* the ratio of two depletions. One time constant apart it must
/// be `1/e`, and that is asserted to four figures rather than as a band.
///
/// The two-minute rest before the first reading is what makes the readings the diffusion
/// term alone: six RC constants clears that pair to below a microvolt, and two minutes is
/// a thirtieth of a depletion constant. (An earlier version of this test compared against
/// the *loaded* overpotential and failed, correctly: at 5 A the RC pair alone is 0.05 V of
/// the 0.108 V read under load, so nearly half of what "decayed" in two minutes was the
/// wrong term entirely.)
#[test]
fn the_depletion_relaxes_on_its_own_time_constant() {
    let k = diffusive_chem().diffusion.expect("present").scale_v;
    // η → the depletion, up to a factor that is constant across a rest.
    let depletion_from = |eta: f64| 1.0 - (-eta / k).exp();

    let mut pack = pack_at(0.8);
    for _ in 0..600 {
        pack.step(1.0, Demand::Current(5.0), &env());
    }
    // 30 RC constants, not 6: at six the pair is still 1.2e-4 V, which is 0.2 % of the
    // reading and biased the ratio below by six times the tolerance. Thirty puts it at
    // 5e-15 and the ratio becomes a measurement of the depletion alone.
    for _ in 0..600 {
        pack.step(1.0, Demand::Rest, &env());
    }
    let eta_0 = pack.cell(0, 0).expect("cell").overpotential_v;

    // Exactly one depletion time constant.
    pack.step(TAU_D_S, Demand::Rest, &env());
    let eta_1 = pack.cell(0, 0).expect("cell").overpotential_v;

    for _ in 0..3 {
        pack.step(TAU_D_S, Demand::Rest, &env());
    }
    let eta_4 = pack.cell(0, 0).expect("cell").overpotential_v;

    assert!(
        eta_0 > 0.04,
        "the fixture must leave a real diffusion overpotential to relax; got {eta_0} V"
    );
    let ratio = depletion_from(eta_1) / depletion_from(eta_0);
    assert!(
        (ratio - (-1.0f64).exp()).abs() < 1e-6,
        "one time constant of rest must decay the depletion by exactly 1/e = {:.6}; got \
         {ratio:.6} ({eta_0} V -> {eta_1} V). A term wired to the RC pair's 20 s would be \
         at zero here.",
        (-1.0f64).exp()
    );
    assert!(
        eta_4 < 0.05 * eta_0,
        "four time constants is four hours; {eta_4} V of {eta_0} V left"
    );
}

/// **Charge drives the depletion negative, and the cell sources above its OCV.**
///
/// The unmeasured direction, pinned so that it is at least bounded, correctly signed and
/// on the right timescale. The early return in `diffusion_overpotential_v` tests
/// `depletion == 0.0` rather than `<= 0.0` precisely so this exists; a `<= 0.0` return
/// would silently delete it and no rate sweep would notice.
#[test]
fn charge_drives_the_depletion_negative() {
    let mut pack = pack_at(0.4);
    for _ in 0..600 {
        pack.step(1.0, Demand::Current(-5.0), &env());
    }
    // Rest off the RC pair (20 s) without touching the depletion (1 h).
    for _ in 0..200 {
        pack.step(1.0, Demand::Rest, &env());
    }
    let view = pack.cell(0, 0).expect("cell exists");

    assert!(
        view.overpotential_v < -0.01,
        "after a charge and a short rest the surviving overpotential must be the \
         diffusion term, and it must be negative; got {} V",
        view.overpotential_v
    );
    assert!(
        view.overpotential_v > -1.0,
        "and it must stay a small correction, not a second battery; got {} V",
        view.overpotential_v
    );
}

/// **At `soc = 0` the term saturates, and it saturates exactly onto the reversal floor.**
///
/// This is the one place the ceiling binds, and it is physics rather than a numerical
/// guard: the limit is `D_lim·soc`, so an empty cell can sustain nothing and any depletion
/// at all exhausts it. What the derived ceiling buys is that the two ways this engine
/// collapses a cell agree — a saturated cell with no deficit sources `OCV(0) − ceiling`,
/// which is `floor_v`, the same voltage the reversal ramp takes a fully over-discharged
/// cell to.
///
/// Read at open circuit, one step after arriving, so the source is the cell's own and not
/// a load line.
///
/// # The single long step is what keeps `soc_deficit` at zero
/// The cell has to be *at* empty and not past it, or the reversal ramp is in the voltage
/// too and the test measures both collapses at once. That state is one step wide, and
/// walking down to it in 1-second steps accumulates rounding and skips over it. So the
/// arrival is arranged in **one** step whose charge is the charge in the cell:
/// `1.25 A × 3600 s / 9000 As = 0.5`, from `soc = 0.5`.
///
/// It lands two femto-units short of zero rather than on it, and the reason is worth
/// recording because it is a standing property of this engine: **a 1S1P pack does not hand
/// its cell the current that was demanded.** The group solve settles a node voltage and
/// then reconstructs each cell's current as `(E − V)/R`, which for one cell is the
/// demanded value plus a rounding step. So the test asserts "at empty to within a
/// femto-unit of capacity, with no deficit" — which is the state it needs — rather than an
/// exactness the solve cannot deliver. What the step leaves behind is a depletion of
/// `0.5·(1 − e⁻¹)`, which is all this needs from it.
#[test]
fn saturation_lands_on_the_reversal_floor() {
    let mut pack = pack_at(0.5);
    let t = pack.step(3600.0, Demand::Current(1.25), &env());
    assert!(
        t.v_terminal.is_finite(),
        "no step may produce a non-finite voltage"
    );

    // Rest off the RC pair before reading. The ceiling bounds the *diffusion* term, not
    // the cell's whole overpotential, so a cell still carrying 12 mV of RC sits 12 mV
    // below the floor — correctly, and not what this test is about. Thirty RC constants,
    // during which the depletion decays by 1.7 % and stays saturated regardless, because
    // at `soc ≈ 0` the ratio is 1e14 and not 1.
    let mut t = pack.step(1.0, Demand::Rest, &env());
    for _ in 0..600 {
        t = pack.step(1.0, Demand::Rest, &env());
    }
    let view = pack.cell(0, 0).expect("cell exists");
    assert!(
        view.soc < 1e-12,
        "the arrival step must land on empty — see the note above; got soc {}",
        view.soc
    );
    assert_eq!(
        view.soc_deficit, 0.0,
        "this test is about the diffusion collapse alone, so the cell must be AT empty \
         rather than past it — otherwise the reversal ramp is also in the voltage"
    );
    assert!(
        (t.v_terminal - FLOOR_V).abs() < 1e-9,
        "a saturated cell at rest must source the reversal floor ({FLOOR_V} V), which is \
         what `max_overpotential_v = OCV(0) − floor_v` is derived to make true; got {} V",
        t.v_terminal
    );
}

/// **Nothing in the empty/rested/reversed region produces a NaN.**
///
/// The failure this exists for is silent: `depletion / (D_lim · soc)` at `soc = 0` is an
/// infinity for a loaded cell and `0/0` for a rested one, a bare `x >= 1.0` guard answers
/// *false* for the NaN, and the NaN travels from the cell's Thévenin source into the
/// parallel aggregation and out to every sibling. No panic, no flag, no failing
/// assertion anywhere else — which is why this walks the whole region rather than one
/// point of it, and on a 1S3P pack so that a poisoned cell would take its neighbours with
/// it.
#[test]
fn the_empty_region_never_produces_a_nan() {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 2,
        parallel: 3,
        initial_soc: 0.05,
        initial_temp_k: 298.15,
        seed: 7,
        // Scattered, so the six cells arrive at empty at six different times and the pack
        // spends real steps with some cells saturated and some not.
        scatter: Scatter {
            capacity_sigma: 0.10,
            r0_sigma: 0.05,
        },
        cell_model: CellModelConfig::Ecm,
    };
    let mut pack = Pack::new(&config, diffusive_chem()).expect("fixture builds");

    let check = |t: &sim_core::Telemetry, pack: &Pack, what: &str| {
        assert!(
            t.v_terminal.is_finite() && t.i_actual.is_finite() && t.q_gen_w.is_finite(),
            "{what}: telemetry went non-finite — v {} i {} q {}",
            t.v_terminal,
            t.i_actual,
            t.q_gen_w
        );
        for s in 0..2 {
            for p in 0..3 {
                let c = pack.cell(s, p).expect("cell exists");
                assert!(
                    c.overpotential_v.is_finite() && c.soc.is_finite(),
                    "{what}: cell ({s},{p}) went non-finite — overpotential {} V, soc {}",
                    c.overpotential_v,
                    c.soc
                );
            }
        }
    };

    // Down through empty and well past it, so every cell is in reversal.
    for _ in 0..600 {
        let t = pack.step(1.0, Demand::Current(5.0), &env());
        check(&t, &pack, "discharging past empty");
    }
    // Rest there: the depletion decays toward — but never reaches — zero while `soc`
    // stays exactly zero. This is the `0/0` corner.
    for _ in 0..500 {
        let t = pack.step(60.0, Demand::Rest, &env());
        check(&t, &pack, "resting at empty");
    }
    // And back out again, which is the step where the ratio collapses.
    let mut last = pack.step(1.0, Demand::Current(-5.0), &env());
    for _ in 0..999 {
        last = pack.step(1.0, Demand::Current(-5.0), &env());
        check(&last, &pack, "charging back out");
    }
    // The *pack's* charge, not one cell's. Inside a saturated parallel group the siblings
    // charge each other — a dead cell being pushed by its neighbours is real behaviour and
    // the closed-form group solve produces it — so no individual cell's SOC is a reliable
    // witness that the last leg did anything.
    assert!(
        last.soc_true > 0.0,
        "the pack must climb back out of empty, or the last leg proved nothing"
    );
}

/// **The cell is still a straight line within a step, so the pack solve is still one
/// closed-form pass.**
///
/// The whole reason `η` is read from the *previous* step's depletion. Were it computed
/// from the in-flight current the cell would be nonlinear, `CellModel::is_linear` would
/// have to answer `false`, and every equivalent-circuit pack in the repo would start
/// iterating. `SOLVE_UNCONVERGED` is the observable: the fast path exits on its first pass
/// having done exactly the arithmetic Phase 1 did, and it never raises the flag.
#[test]
fn the_solve_stays_one_pass_with_a_depletion() {
    let mut pack = pack_at(0.5);
    for step in 0..400 {
        let demand = match step % 4 {
            0 => Demand::Current(7.5),
            1 => Demand::Rest,
            2 => Demand::Power(20.0),
            _ => Demand::Voltage(3.1),
        };
        let t = pack.step(1.0, demand, &env());
        assert!(
            !t.flags.contains(sim_core::EventFlags::SOLVE_UNCONVERGED),
            "step {step}: an equivalent-circuit pack must never iterate"
        );
    }
}

/// **The depletion survives a real serialize/deserialize round trip, and the trajectory
/// after it is identical.**
///
/// Through `bincode`, not through `restore(&snapshot())` — that pair is a `Clone` and
/// exercises no serde attribute at all, which this repo has recorded as a trap once
/// already. The cell is left mid-discharge with a live depletion, because a rested one
/// would restore correctly even if the field were dropped entirely: `#[serde(default)]`
/// supplies zero, and zero is right for a rested pack and wrong for this one. That
/// asymmetry is the whole argument for [`SNAPSHOT_VERSION`] 17 being semantic.
#[test]
fn a_loaded_pack_survives_a_serde_round_trip() {
    let mut pack = pack_at(0.7);
    for _ in 0..300 {
        pack.step(1.0, Demand::Current(5.0), &env());
    }
    let loaded = pack.cell(0, 0).expect("cell").overpotential_v;
    assert!(
        loaded > 0.05,
        "the fixture must be saved with a live depletion; got {loaded} V"
    );

    let snapshot = pack.snapshot();
    assert_eq!(snapshot.version, SNAPSHOT_VERSION);
    let bytes = bincode::serialize(&snapshot).expect("the snapshot serializes");
    let parsed = bincode::deserialize(&bytes).expect("the snapshot deserializes");
    let mut restored = Pack::restore(&parsed).expect("the snapshot restores");

    assert_eq!(
        restored.cell(0, 0).expect("cell").overpotential_v,
        loaded,
        "the depletion must cross the wire exactly, not be defaulted to zero"
    );
    for _ in 0..300 {
        let a = pack.step(1.0, Demand::Current(5.0), &env());
        let b = restored.step(1.0, Demand::Current(5.0), &env());
        assert_eq!(
            a.v_terminal, b.v_terminal,
            "the restored trajectory must be bit-identical"
        );
        assert_eq!(a.q_gen_w, b.q_gen_w);
    }
}
