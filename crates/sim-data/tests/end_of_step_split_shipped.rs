//! The one divergence `docs/plans/end-of-step-split.md` recorded that needs a real cell to
//! reproduce: a soft internal short draining a parallel pair of the shipped NMC 18650 at a
//! one-minute step.
//!
//! The rest of that slice's regression tests are in `sim-core/tests/end_of_step_split.rs`,
//! on a synthetic cell. This one was written there first and passed on the start-of-step
//! engine as well as on the fixed one, which made it a test of nothing: the synthetic
//! cell's coupling time never fell under the step. The shipped cell's does, near empty —
//! its first OCV segment is 8 V per unit of charge, its `R0` rises toward empty and with
//! temperature, and a 1 Ω short has it at 349 K by then — and on the start-of-step engine
//! the pair's branch currents swung ±284 A at t = 4860 s, raised `THERMAL_RUNAWAY` at
//! 3530 K, and climbed to 63 000 K. The same run at a 1 s step drains quietly to empty.
//! That was a runaway the physics did not produce, which `CLAUDE.md` forbids outright.

use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Fault, Pack, PackConfig, Scatter, ThermalConfig,
};

fn nmc() -> sim_core::ChemistryParams {
    sim_data::parse_chemistry(include_str!("../../../chemistries/nmc_18650_generic.toml"))
        .expect("the shipped NMC 18650 parses")
}

#[test]
fn a_shorted_pair_drains_without_a_runaway_the_physics_did_not_make() {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
        series: 1,
        parallel: 2,
        initial_soc: 0.8,
        initial_temp_k: 298.15,
        seed: 7,
        scatter: Scatter {
            capacity_sigma: 0.0,
            r0_sigma: 0.0,
        },
        cell_model: CellModelConfig::Ecm,
    };
    let mut pack = Pack::new(&config, nmc()).expect("builds");
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 1.0,
        },
    )
    .expect("valid fault");
    let env = Env {
        t_ambient: 298.15,
        t_coolant: None,
    };
    let mut t_peak: f64 = 0.0;
    // Two hours at a minute a step: past the 4860 s where the start-of-step engine came
    // apart, and to empty.
    for n in 0..120 {
        let tele = pack.step(60.0, Demand::Rest, &env);
        t_peak = t_peak.max(tele.t_max);
        assert!(
            !tele.flags.contains(EventFlags::THERMAL_RUNAWAY),
            "step {n}: runaway at {} K — the short heats this pack to about 350 K and no \
             further, as a 1 s step shows",
            tele.t_max
        );
        for k in 0..2 {
            let i = pack
                .cell(0, k)
                .expect("in range")
                .current_a
                .expect("stepped");
            assert!(
                i.abs() < 5.0,
                "step {n}: branch {k} carries {i} A; the short draws about 2 A per cell"
            );
        }
    }
    assert!(
        t_peak < 360.0,
        "peak {t_peak} K; the 1 s run peaks at 349.9 K"
    );
    let c = pack.cell(0, 1).expect("in range");
    assert!(
        c.soc < 0.01,
        "the run was meant to reach empty; the neighbour is at soc {}",
        c.soc
    );
}
