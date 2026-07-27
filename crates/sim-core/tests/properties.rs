//! Property tests: invariants that must hold across random topologies, demands,
//! timesteps, and scatter.
//!
//! These cover charge conservation, the heat-inclusive energy balance, SOC bounds,
//! the discharge/charge terminal-voltage sign relationship, per-cell currents
//! summing to the group current, and snapshot round-trip equality.

use proptest::prelude::*;

use sim_core::bms::{BmsConfig, ProtectionConfig};
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::ecm::ocv_lookup;
use sim_core::{Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

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
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
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
            docv_dt_v_per_k: None,
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

/// Constant OCV of the [`flat_chem`] cell \[V\].
const FLAT_V0: f64 = 3.30;

/// Same cell, but with OCV constant in SOC. That makes the chemical energy drawn
/// over a run exactly `V0 · ∫I dt` with no path dependence, which is what turns the
/// energy balance into a closed-form check rather than a numerical integration of
/// `OCV(soc)` against the engine's own discretisation.
fn flat_chem() -> ChemistryParams {
    let mut c = chem();
    c.ocv = OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![FLAT_V0, FLAT_V0],
        docv_dt_v_per_k: None,
    };
    c
}

fn cfg(series: u16, parallel: u16, soc0: f64, seed: u64, scatter: Scatter) -> PackConfig {
    PackConfig {
        bms: None,
        thermal: ThermalConfig::Isothermal,
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

    /// Pack energy balance: the chemical energy the cells give up equals the
    /// electrical energy leaving the terminals plus the heat generated inside.
    ///
    /// With a flat OCV the chemical side is exactly `V0·S·∫I dt` (every cell sits at
    /// `V0` regardless of SOC or scatter, and each series group's per-cell currents
    /// sum to the pack current).
    ///
    /// The balance is exact — to floating-point rounding, not to a physical
    /// tolerance — because both heat and current are evaluated from **start-of-step**
    /// state. `Telemetry` reports the *end-of-step* voltage, so the electrical
    /// integral is accumulated one step behind: for a constant current, step `n`'s
    /// start-of-step node voltage is step `n−1`'s end-of-step value (same formula,
    /// same state). Pairing them naively instead leaves an O(dt²)-per-step residual
    /// that swamps a rounding-level tolerance.
    ///
    /// Exactness is what gives the test teeth: using `I²·(R0 + ΣR_rc)` (the
    /// steady-state heat form) instead of `I²·R0 + I·ΣV_rc` misstates the heat by
    /// double-digit percentages during an RC transient — a tolerance loose enough to
    /// absorb the discretisation residual would be within an order of magnitude of
    /// letting that through.
    #[test]
    fn electrical_and_heat_energy_balance(
        series in 1u16..=3,
        parallel in 1u16..=3,
        i in -4.0f64..4.0,
        dt in 0.01f64..0.5,
        nsteps in 5usize..200,
        seed in any::<u64>(),
        cap_sigma in 0.0f64..0.08,
        r0_sigma in 0.0f64..0.08,
    ) {
        let scatter = Scatter { capacity_sigma: cap_sigma, r0_sigma };
        let mut config = cfg(series, parallel, 0.5, seed, scatter);
        // A live thermal network, to prove the balance survives temperature moving
        // (R0 has a single temperature breakpoint, so heating cannot feed back into
        // the electrical solve and quietly rescue a broken balance).
        config.thermal = ThermalConfig::Network { k_neighbor_w_per_k: 1.0 };
        let mut pack = Pack::new(&config, flat_chem()).unwrap();

        let mut chemical = 0.0;   // V0·S·∫I dt
        let mut electrical = 0.0; // ∫V_terminal·I dt
        let mut heat = 0.0;       // ∫Q dt
        // The first step's start-of-step voltage, from a zero-length probe step:
        // dt = 0 mutates nothing (the RC update, the coulomb count and the thermal
        // integration all scale by dt), but telemetry still reports the pack solved
        // at this state under this current.
        let mut v_start = pack.step(0.0, Demand::Current(i), &env()).v_terminal;
        for _ in 0..nsteps {
            let tele = pack.step(dt, Demand::Current(i), &env());
            prop_assert!(tele.flags.is_empty(), "unexpected clamp: {:?}", tele.flags);
            chemical += FLAT_V0 * f64::from(series) * tele.i_actual * dt;
            electrical += v_start * tele.i_actual * dt;
            heat += tele.q_gen_w * dt;
            v_start = tele.v_terminal;
        }
        let imbalance = chemical - electrical - heat;
        let tol = 1e-12 * chemical.abs().max(1.0);
        prop_assert!(
            imbalance.abs() < tol,
            "chemical {chemical} J vs electrical {electrical} J + heat {heat} J \
             (imbalance {imbalance} J, tol {tol} J)"
        );
    }

    /// Whatever the BMS believes, it believes something *possible*: the SOC estimate
    /// stays in [0, 1] however badly its sensors lie or however long the drift runs.
    /// Downstream clients (a gauge, a charger policy) are entitled to that.
    #[test]
    fn bms_estimate_stays_in_unit_interval(
        series in 1u16..=3,
        parallel in 1u16..=3,
        soc0 in 0.05f64..0.95,
        soc_error in -1.5f64..1.5,
        offset in -0.5f64..0.5,
        sigma in 0.0f64..0.3,
        currents in prop::collection::vec(-6.0f64..6.0, 1..40),
        dt in 0.1f64..5.0,
        seed in any::<u64>(),
    ) {
        let mut config = cfg(series, parallel, soc0, seed, Scatter::default());
        config.bms = Some(BmsConfig {
            balancing: None,
            protection: None, // unprotected, so the pack really does run to its clamps
            current_offset_a: offset,
            current_noise_sigma_a: sigma,
            temp_probes: vec![(0, 0)],
            initial_soc_error: soc_error,
            rest_current_threshold_a: 0.05,
            rest_time_for_ocv_s: 60.0,
            ocv_correction_gain: 0.5,
            min_ocv_slope_v_per_soc: 0.05,
        });
        let mut pack = Pack::new(&config, chem()).unwrap();
        for &i in &currents {
            let tele = pack.step(dt, Demand::Current(i), &env());
            let est = tele.soc_bms.expect("a BMS is configured");
            prop_assert!((0.0..=1.0).contains(&est), "soc_bms {est}");
        }
    }

    /// With protection enabled the pack never carries more current than the
    /// chemistry's C-rate window allows — for *any* demand variant, including the
    /// Power and Voltage solves whose currents are not directly specified. The window
    /// does not depend on any sensor reading, so unlike the voltage and temperature
    /// limits this one binds from the very first step, with no lag.
    #[test]
    fn protection_never_exceeds_the_c_rate_window(
        series in 1u16..=3,
        parallel in 1u16..=3,
        soc0 in 0.1f64..0.9,
        dt in 0.1f64..2.0,
        seed in any::<u64>(),
        // A mix of wildly out-of-range demands of every variant.
        picks in prop::collection::vec(0usize..4, 1..30),
        mags in prop::collection::vec(-500.0f64..500.0, 1..30),
    ) {
        let mut config = cfg(series, parallel, soc0, seed, Scatter::default());
        config.bms = Some(BmsConfig {
            balancing: None,
            protection: Some(ProtectionConfig { v_hard_margin_v: 0.2, t_hard_margin_k: 10.0 }),
            current_offset_a: 0.0,
            current_noise_sigma_a: 0.0,
            temp_probes: vec![(0, 0)],
            initial_soc_error: 0.0,
            rest_current_threshold_a: 0.01,
            rest_time_for_ocv_s: 600.0,
            ocv_correction_gain: 1.0,
            min_ocv_slope_v_per_soc: 0.5,
        });
        let mut pack = Pack::new(&config, chem()).unwrap();
        let pack_ah = CAP_AH * f64::from(parallel);
        let c = chem();
        let i_dis_max = c.cell.max_discharge_c * pack_ah;
        let i_chg_max = c.cell.max_charge_c * pack_ah;

        for (idx, &pick) in picks.iter().enumerate() {
            let m = mags[idx % mags.len()];
            let demand = match pick {
                0 => Demand::Current(m),
                1 => Demand::Power(m),
                2 => Demand::Voltage(m.abs() * 0.01),
                _ => Demand::Rest,
            };
            let tele = pack.step(dt, demand, &env());
            prop_assert!(
                tele.i_actual.is_finite(),
                "current must stay finite under {demand:?}: {}", tele.i_actual
            );
            prop_assert!(
                tele.i_actual <= i_dis_max + 1e-9 && tele.i_actual >= -i_chg_max - 1e-9,
                "{demand:?} gave {} A, outside [{}, {}]",
                tele.i_actual, -i_chg_max, i_dis_max
            );
            // A latched contactor means exactly zero current, not merely a small one.
            if pack.bms().expect("bms configured").contactor_open() {
                prop_assert_eq!(tele.i_actual, 0.0, "open contactor must carry no current");
            }
        }
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
