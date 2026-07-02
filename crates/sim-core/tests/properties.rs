//! Phase 1 property tests: invariants that must hold across random topologies,
//! demands, timesteps, and scatter.
//!
//! Per the phase scope these cover charge conservation (not the heat-inclusive
//! energy balance, which needs the Phase 2 thermal model), SOC bounds, the
//! discharge/charge terminal-voltage sign relationship, per-cell currents summing
//! to the group current, and snapshot round-trip equality.

use proptest::prelude::*;

use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::ecm::ocv_lookup;
use sim_core::{Demand, Env, Pack, PackConfig, Scatter};

const CAP_AH: f64 = 2.5;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A sloped-OCV, temperature-varying-R0, single-RC chemistry used by every
/// property. Nothing here is chemistry-specific; it just needs to be non-trivial.
fn chem() -> ChemistryParams {
    ChemistryParams {
        meta: ChemMeta {
            id: "p".into(),
            name: "Property test cell".into(),
            provenance: "property test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.0,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            soc: vec![0.0, 0.2, 0.5, 0.8, 1.0],
            volts: vec![3.00, 3.20, 3.30, 3.40, 3.60],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.01,
            c_farad: 2000.0,
        }],
    }
}

fn cfg(series: u16, parallel: u16, soc0: f64, seed: u64, scatter: Scatter) -> PackConfig {
    PackConfig {
        series,
        parallel,
        initial_soc: soc0,
        initial_temp_k: 298.15,
        seed,
        scatter,
    }
}

/// Total remaining charge \[Ah\] summed over every cell (ground truth).
fn total_remaining_ah(pack: &Pack, series: u16, parallel: u16) -> f64 {
    let mut ah = 0.0;
    for s in 0..series as usize {
        for p in 0..parallel as usize {
            let c = pack.cell(s, p).unwrap();
            ah += c.soc * CAP_AH * c.capacity_factor;
        }
    }
    ah
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Every cell's SOC (and the pack estimate) stays within [0, 1] under any
    /// sequence of currents, timesteps, topology, and scatter.
    #[test]
    fn soc_stays_in_unit_interval(
        series in 1u16..=4,
        parallel in 1u16..=4,
        soc0 in 0.05f64..0.95,
        currents in prop::collection::vec(-6.0f64..6.0, 1..40),
        dt in 0.1f64..5.0,
        seed in any::<u64>(),
        cap_sigma in 0.0f64..0.1,
        r0_sigma in 0.0f64..0.1,
    ) {
        let scatter = Scatter { capacity_sigma: cap_sigma, r0_sigma };
        let mut pack = Pack::new(&cfg(series, parallel, soc0, seed, scatter), chem()).unwrap();
        for &i in &currents {
            let tele = pack.step(dt, Demand::Current(i), &env());
            prop_assert!((0.0..=1.0).contains(&tele.soc_true), "soc_true {}", tele.soc_true);
            for s in 0..series as usize {
                for p in 0..parallel as usize {
                    let soc = pack.cell(s, p).unwrap().soc;
                    prop_assert!((0.0..=1.0).contains(&soc), "cell soc {soc}");
                }
            }
        }
    }

    /// Charge conservation on a single series string (series = 1): the integral of
    /// the pack current equals the change in stored charge, when no SOC clamp fires.
    #[test]
    fn charge_conserved_without_clamp(
        parallel in 1u16..=4,
        i in -3.0f64..3.0,
        dt in 0.5f64..2.0,
        nsteps in 1usize..100,
        seed in any::<u64>(),
    ) {
        // Scatter off so effective capacity is exactly nominal; soc0 = 0.5 with a
        // bounded excursion keeps the run clear of the [0,1] clamps.
        let mut pack = Pack::new(&cfg(1, parallel, 0.5, seed, Scatter::default()), chem()).unwrap();
        let rem0 = total_remaining_ah(&pack, 1, parallel);
        let mut q_as = 0.0; // amp-seconds
        for _ in 0..nsteps {
            let tele = pack.step(dt, Demand::Current(i), &env());
            prop_assert!(tele.flags.is_empty(), "unexpected clamp: {:?}", tele.flags);
            q_as += tele.i_actual * dt;
        }
        let rem1 = total_remaining_ah(&pack, 1, parallel);
        // series = 1 ⇒ ∫I dt = 3600·Δ(stored Ah).
        let expected = 3600.0 * (rem0 - rem1);
        prop_assert!(
            (q_as - expected).abs() < 1e-6 + 1e-9 * q_as.abs(),
            "charge: ∫I dt = {q_as}, 3600·Δrem = {expected}"
        );
    }

    /// Under a sustained constant current, a uniform (no-scatter) pack's group
    /// voltage sits below OCV on discharge and above OCV on charge — the ohmic +
    /// RC overpotential always opposes the current.
    #[test]
    fn terminal_voltage_respects_ocv_sign(
        series in 1u16..=4,
        parallel in 1u16..=4,
        soc0 in 0.2f64..0.8,
        mag in 0.5f64..3.0,
        charging in any::<bool>(),
        dt in 0.2f64..3.0,
        nsteps in 1usize..40,
    ) {
        let i = if charging { -mag } else { mag };
        // No scatter → all cells identical → every group sits at the same SOC, so
        // cell (0,0) speaks for the whole pack.
        let mut pack = Pack::new(&cfg(series, parallel, soc0, 0, Scatter::default()), chem()).unwrap();
        for _ in 0..nsteps {
            let tele = pack.step(dt, Demand::Current(i), &env());
            let ocv = ocv_lookup(&chem().ocv, pack.cell(0, 0).unwrap().soc);
            if charging {
                prop_assert!(tele.v_cell_min >= ocv - 1e-9, "charge: {} < ocv {ocv}", tele.v_cell_min);
            } else {
                prop_assert!(tele.v_cell_max <= ocv + 1e-9, "discharge: {} > ocv {ocv}", tele.v_cell_max);
            }
        }
    }

    /// The per-cell currents in a parallel group sum to the group (pack) current,
    /// even with scatter making the split unequal. Reconstructed from the SOC change
    /// over one step from rest, where the split is exact.
    #[test]
    fn parallel_currents_sum_to_group_current(
        parallel in 1u16..=6,
        i in -5.0f64..5.0,
        seed in any::<u64>(),
        cap_sigma in 0.0f64..0.08,
        r0_sigma in 0.0f64..0.08,
    ) {
        let scatter = Scatter { capacity_sigma: cap_sigma, r0_sigma };
        let mut pack = Pack::new(&cfg(1, parallel, 0.5, seed, scatter), chem()).unwrap();
        let dt = 1.0;
        let tele = pack.step(dt, Demand::Current(i), &env());
        prop_assert!((tele.i_actual - i).abs() < 1e-12, "Current demand passes through");

        let mut sum = 0.0;
        for p in 0..parallel as usize {
            let c = pack.cell(0, p).unwrap();
            let cap_as = 3600.0 * CAP_AH * c.capacity_factor;
            sum += (0.5 - c.soc) * cap_as / dt; // I_k reconstructed from ΔSOC
        }
        prop_assert!((sum - tele.i_actual).abs() < 1e-6, "Σ I_k = {sum}, I_g = {}", tele.i_actual);
    }

    /// Snapshot round-trip equality: after any warm-up, a pack snapshotted through
    /// bincode bytes and restored continues bit-identically to the original.
    #[test]
    fn snapshot_roundtrip_continues_identically(
        series in 1u16..=3,
        parallel in 1u16..=3,
        soc0 in 0.2f64..0.9,
        seed in any::<u64>(),
        cap_sigma in 0.0f64..0.06,
        r0_sigma in 0.0f64..0.06,
        warmup in prop::collection::vec(-4.0f64..4.0, 0..20),
        tail in prop::collection::vec(-4.0f64..4.0, 1..20),
        dt in 0.2f64..2.0,
    ) {
        let scatter = Scatter { capacity_sigma: cap_sigma, r0_sigma };
        let mut original = Pack::new(&cfg(series, parallel, soc0, seed, scatter), chem()).unwrap();
        for &i in &warmup {
            original.step(dt, Demand::Current(i), &env());
        }
        let snap = original.snapshot();
        let bytes = bincode::serialize(&snap).unwrap();
        let snap2: sim_core::Snapshot = bincode::deserialize(&bytes).unwrap();
        let mut restored = Pack::restore(&snap2).unwrap();

        for &i in &tail {
            let a = original.step(dt, Demand::Current(i), &env());
            let b = restored.step(dt, Demand::Current(i), &env());
            prop_assert_eq!(a, b);
        }
    }
}
