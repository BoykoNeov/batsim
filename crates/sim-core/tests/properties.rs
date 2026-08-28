//! Property tests: invariants that must hold across random topologies, demands,
//! timesteps, and scatter.
//!
//! These cover charge conservation, the heat-inclusive energy balance, SOC bounds,
//! the discharge/charge terminal-voltage sign relationship, per-cell currents
//! summing to the group current, and snapshot round-trip equality.

use proptest::prelude::*;

use sim_core::bms::{BalancingConfig, BmsConfig, ProtectionConfig};
use sim_core::chem::{
    AgingParams, CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, SafetyParams,
    ThermalParams,
};
use sim_core::ecm::ocv_lookup;
use sim_core::{
    AgingConfig, CellModelConfig, Demand, Env, EventFlags, Fault, Pack, PackConfig, Scatter,
    ThermalConfig,
};

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
        diffusion: None,
        hysteresis: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            // Zero: this file's chemistry pays nothing for over-discharge, so its
            // trajectories are the ones this slice must not move. See
            // `docs/plans/reversal-damage.md`.
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
            t_ref_k: None,
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
        t_ref_k: None,
    };
    c
}

/// The same cell with aging coefficients and a `[safety]` section attached.
///
/// Phase 3 put four new pieces of per-cell state behind these two sections — the SOH
/// pair and its accumulators, the plating charge counter, the internal-short
/// conductance, and the exothermic budget — and the `chem()` above reaches none of
/// them. Properties that need Phase 3 state to exist use this one.
///
/// The coefficients are deliberately **faster than anything shipped**: a property runs
/// for tens of simulated seconds, and the shipped placeholders would fade a cell by
/// ~1e-9 over that, which is a number every monotonicity assertion would pass on
/// trivially. These are scaled to make the mechanisms visible in a short run, so
/// nothing here is a physical claim about any chemistry.
///
/// Scaled, but **not** scaled past the point of meaning anything. `cal_pre_exp` was
/// first set to `1e8`, which fades a cell by 2.46 over the longest trajectory a property
/// here generates — i.e. straight through [`sim_core::aging::MIN_SOH_CAPACITY`] on the
/// first few steps, after which `health_never_improves` was asserting monotonicity of a
/// clamped constant and would have passed against an engine that did no aging at all.
/// At `5e4` the same trajectory fades between 6e-5 and 1.2e-3, which is far above
/// rounding and nowhere near the floor. The coverage assertions in that property exist
/// to keep it that way.
fn aging_chem() -> ChemistryParams {
    let mut c = chem();
    c.aging = Some(AgingParams {
        cal_pre_exp: 5.0e4,
        cal_ea_j_per_mol: 5.0e4,
        cal_soc_stress: vec![1.0, 1.0, 1.4],
        cyc_fade_per_ah: 1.0e-2,
        cyc_dod_stress_exp: 1.1,
        r_growth_per_capacity_loss: 1.5,
    });
    c.safety = Some(SafetyParams {
        // Onset far above anything these packs reach: runaway has its own mid-burn
        // snapshot test in `tests/runaway.rs`, and an exponential source term inside a
        // property would make every shrunk counter-example a thermal investigation.
        t_onset_k: 1.0e4,
        t_vent_k: 1.1e4,
        runaway_energy_j: 24.0e3,
        runaway_power_w_at_onset: 0.0,
        runaway_ea_j_per_mol: 0.0,
        // Plating live, and reachable: the packs below run below this temperature so
        // that any charging current above the C-rate threshold plates.
        t_plating_min_k: Some(273.15),
        plating_c_threshold: Some(0.5),
        plating_fade_per_ah: 1.0e-2,
        // High enough that a short actually forms on a useful fraction of the generated
        // inputs. The *draw* happens on every tick with a positive hazard regardless, so
        // the RNG-phase half of the round-trip property is covered either way; this is
        // about also covering the branch where one fires.
        plating_short_hazard_per_ah: 5.0,
        plating_short_ohms: 50.0,
    });
    c
}

fn cfg(series: u16, parallel: u16, soc0: f64, seed: u64, scatter: Scatter) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series,
        parallel,
        initial_soc: soc0,
        initial_temp_k: 298.15,
        seed,
        scatter,
        cell_model: CellModelConfig::Ecm,
    }
}

/// Total remaining charge \[Ah\] summed over every cell (ground truth).
///
/// **`soc − soc_deficit`, not `soc`.** A cell driven past empty reports `soc == 0.0`
/// while genuinely holding *less* than nothing, and the deficit is where that goes; using
/// `soc` alone would make this quantity stop moving at the bottom of the window and every
/// charge ledger built on it would go quietly non-conserving there. See
/// [`sim_core::CellView::soc_deficit`].
fn total_remaining_ah(pack: &Pack, series: u16, parallel: u16) -> f64 {
    let mut ah = 0.0;
    for s in 0..series as usize {
        for p in 0..parallel as usize {
            let c = pack.cell(s, p).unwrap();
            ah += (c.soc - c.soc_deficit) * CAP_AH * c.capacity_factor;
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
    ///
    /// **Fault-free by construction.** A soft internal short moves charge out of a
    /// cell without it ever crossing the pack terminals, so `∫ i_actual dt` would
    /// legitimately fall short of the stored-charge change; the fault-aware statement
    /// adds `∫ i_internal_short_a dt` to the terminal integral.
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
    ///
    /// **Fault-free by construction.** The currents reconstructed here are the cells'
    /// *internal* branch currents, and with a shunt on any cell those sum to
    /// `i_actual + Σ v_node·G_s` — the shunted share never reaches the terminals. No
    /// balancing either, for the same reason (a bleed is the group-level version of
    /// the same term).
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
    /// With balancing active the balance has **four** terms, not three: charge bled to
    /// a resistor leaves the cells (so it belongs on the chemical side) and its energy
    /// lands in the resistor (so it belongs on the loss side). Balancing is switched on
    /// here for exactly that reason — the three-term version would look like a physics
    /// failure the moment anyone enabled a bleed switch.
    ///
    /// With a flat OCV the chemical side is exactly `V0·(S·I + I_bleed)·dt` (every cell
    /// sits at `V0` regardless of SOC or scatter, and each series group's per-cell
    /// currents sum to the pack current *plus* that group's bleed current).
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
        // Balancing on, with a threshold well below the flat OCV so every group bleeds
        // for the whole run: the bleed path is then part of what this test covers.
        config.bms = Some(BmsConfig {
            balancing: Some(BalancingConfig {
                bleed_r_ohms: 47.0,
                v_threshold_v: FLAT_V0 - 0.5,
                v_release_band_v: 0.010,
            }),
            protection: None, // clamping the current would end the run early
            current_offset_a: 0.0,
            current_noise_sigma_a: 0.0,
            temp_probes: Vec::new(),
            initial_soc_error: 0.0,
            rest_current_threshold_a: 0.0,
            rest_time_for_ocv_s: 1.0e9,
            ocv_correction_gain: 0.0,
            min_ocv_slope_v_per_soc: 0.0,
        });
        let mut pack = Pack::new(&config, flat_chem()).unwrap();

        let mut chemical = 0.0;   // V0·(S·I + I_bleed)·dt
        let mut electrical = 0.0; // ∫V_terminal·I dt
        let mut heat = 0.0;       // ∫Q dt
        let mut bled = 0.0;       // ∫Q_balancing dt
        // The first step's start-of-step voltage, from a zero-length probe step:
        // dt = 0 mutates nothing (the RC update, the coulomb count and the thermal
        // integration all scale by dt), but telemetry still reports the pack solved
        // at this state under this current.
        let mut v_start = pack.step(0.0, Demand::Current(i), &env()).v_terminal;
        for _ in 0..nsteps {
            let tele = pack.step(dt, Demand::Current(i), &env());
            // BALANCING is expected here; a SOC clamp is not. Note what this
            // exclusion is and is not: it keeps the property to the regime it was
            // written for, but it is **not** evidence that a clamp would break the
            // balance. Measured, it does not — this accounting is an identity in the
            // reported currents and survives a clamp to 4.5e-13 J, which is why
            // `overcharge_heat_closes_the_energy_ledger` below takes its chemical side
            // from ground-truth state instead. This strategy cannot reach a clamp
            // anyway (400 As of a 9000 As cell), so the assertion has never fired.
            prop_assert!(
                !tele.flags.intersects(EventFlags::SOC_CLAMPED_HIGH | EventFlags::SOC_CLAMPED_LOW),
                "unexpected SOC clamp: {:?}", tele.flags
            );
            chemical +=
                FLAT_V0 * (f64::from(series) * tele.i_actual + tele.i_balancing_a) * dt;
            electrical += v_start * tele.i_actual * dt;
            heat += tele.q_gen_w * dt;
            bled += tele.q_balancing_w * dt;
            v_start = tele.v_terminal;
        }
        prop_assert!(bled > 0.0, "the bleed path should have been exercised");
        let imbalance = chemical - electrical - heat - bled;
        let tol = 1e-12 * chemical.abs().max(1.0);
        prop_assert!(
            imbalance.abs() < tol,
            "chemical {chemical} J vs electrical {electrical} J + heat {heat} J \
             + bled {bled} J (imbalance {imbalance} J, tol {tol} J)"
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
    ///
    /// **Fault-free by construction, and an external short is the counter-example on
    /// purpose.** Protection clamps the current the *load* is allowed; a short
    /// downstream of the contactor draws whatever the terminal voltage gives it, so
    /// `i_actual` exceeds the window until the sag latches the contactor open. That
    /// is the fault's whole pedagogical point, not a protection bug.
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
            protection: Some(ProtectionConfig { v_hard_margin_v: 0.2, t_hard_margin_k: 10.0 , v_release_band_v: 0.08, t_release_band_k: 2.0 }),
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
                2 => Demand::Voltage(m.abs() * 0.03),
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

    /// Charge conservation, **with a soft internal short draining a cell**. This is the
    /// fault-aware form that `charge_conserved_without_clamp` documents and excludes:
    /// leakage moves charge out of a cell without it ever crossing the pack terminals,
    /// so the terminal integral alone falls short by exactly the short's throughput.
    ///
    /// Adding `∫ i_internal_short_a dt` closes it again. That is a stronger statement
    /// than it looks: it says the current the engine *reports* through the shunt is the
    /// same current it *charges* against the cells' coulomb counters. A shunt that
    /// heated the cell but did not drain it, or drained it at the terminal voltage
    /// instead of the node voltage, would pass every voltage assertion in the suite and
    /// fail here.
    ///
    /// Aging is off: with SOH moving, stored charge is no longer `soc · capacity ·
    /// factor` at a fixed capacity, and this property would be measuring two things.
    #[test]
    fn charge_conserved_through_an_internal_short(
        parallel in 1u16..=4,
        shorted in 0usize..4,
        ohms in 5.0f64..200.0,
        i in -2.0f64..2.0,
        dt in 0.5f64..2.0,
        nsteps in 1usize..80,
        seed in any::<u64>(),
    ) {
        let p_short = shorted % parallel as usize;
        let mut pack = Pack::new(&cfg(1, parallel, 0.6, seed, Scatter::default()), chem()).unwrap();
        pack.schedule_fault(0.0, Fault::SoftInternalShort {
            s: 0,
            p: p_short as u16,
            ohms,
        }).unwrap();

        let rem0 = total_remaining_ah(&pack, 1, parallel);
        let mut q_as = 0.0; // amp-seconds out of the cells, terminals + leakage
        for _ in 0..nsteps {
            let tele = pack.step(dt, Demand::Current(i), &env());
            // Unlike the energy balance above, this ledger genuinely does diverge at
            // a clamp — by the whole rejected amount — so the exclusion is load-bearing
            // here rather than defensive. The clamped case is
            // `charge_conserved_through_a_soc_clamp` below, which carries the
            // `i_rejected_a` term that closes it.
            prop_assert!(
                !tele.flags.intersects(EventFlags::SOC_CLAMPED_HIGH | EventFlags::SOC_CLAMPED_LOW),
                "unexpected clamp: {:?}", tele.flags
            );
            prop_assert!(
                tele.i_internal_short_a > 0.0,
                "an injected short should always be leaking, got {}", tele.i_internal_short_a
            );
            q_as += (tele.i_actual + tele.i_internal_short_a) * dt;
        }
        let rem1 = total_remaining_ah(&pack, 1, parallel);
        let expected = 3600.0 * (rem0 - rem1);
        prop_assert!(
            (q_as - expected).abs() < 1e-6 + 1e-9 * q_as.abs(),
            "charge: ∫(I + I_short) dt = {q_as}, 3600·Δrem = {expected}"
        );
    }

    /// **Charge conservation through a clamp**, which is the invariant the SOC clamp
    /// used to break silently.
    ///
    /// The two properties above both open by asserting no clamp occurred. Neither
    /// exclusion was ever load-bearing — their strategies cannot reach a clamp
    /// (`CAP_AH` is 9000 As and the widest case moves 400 of them) — so this is not
    /// those tests with a restriction lifted. It is the case they could not generate,
    /// driven deliberately, with a coverage assertion so it cannot quietly stop
    /// clamping.
    ///
    /// The ledger is the one on [`sim_core::Telemetry::i_rejected_a`]. Each of the `S`
    /// series groups carries the terminal current, so the charge that crosses cell
    /// boundaries is `S·i_actual`, of which `i_rejected_a` never reached the stored
    /// charge:
    ///
    /// ```text
    /// ∫(S·i_actual − i_rejected_a) dt = 3600 · Δ(stored charge over every cell)
    /// ```
    ///
    /// Exact to rounding, not to a physical tolerance: every term is a sum of the same
    /// per-cell products the engine itself formed.
    #[test]
    fn charge_conserved_through_a_soc_clamp(
        series in 1u16..=3,
        parallel in 1u16..=3,
        // Either end of the window, approached fast enough to arrive inside the run.
        high in any::<bool>(),
        // Sized so the clamp is *guaranteed*, not likely: the weakest corner
        // (40 A over 3 parallel, dt = 0.5, 60 steps) still moves 400 As per cell
        // against the 270 As that separates `soc0` from the bound.
        amps in 40.0f64..80.0,
        dt in 0.5f64..2.0,
        nsteps in 60usize..200,
        seed in any::<u64>(),
        cap_sigma in 0.0f64..0.08,
    ) {
        let soc0 = if high { 0.97 } else { 0.03 };
        let i = if high { -amps } else { amps };
        let scatter = Scatter { capacity_sigma: cap_sigma, r0_sigma: 0.0 };
        let mut pack = Pack::new(&cfg(series, parallel, soc0, seed, scatter), chem()).unwrap();

        let rem0 = total_remaining_ah(&pack, series, parallel);
        let mut q_as = 0.0;
        let mut clamped_steps = 0usize;
        for _ in 0..nsteps {
            let tele = pack.step(dt, Demand::Current(i), &env());
            if tele.flags.intersects(EventFlags::SOC_CLAMPED_HIGH | EventFlags::SOC_CLAMPED_LOW) {
                clamped_steps += 1;
            } else {
                prop_assert_eq!(
                    tele.i_rejected_a, 0.0,
                    "an unclamped step rejected {} A", tele.i_rejected_a
                );
            }
            q_as += (f64::from(series) * tele.i_actual - tele.i_rejected_a) * dt;
        }
        let rem1 = total_remaining_ah(&pack, series, parallel);

        // Coverage: without this the property passes on a run that never clamped, and
        // would be re-proving the two above.
        prop_assert!(clamped_steps > 0, "the run never reached the clamp it was aimed at");

        let expected = 3600.0 * (rem0 - rem1);
        prop_assert!(
            (q_as - expected).abs() < 1e-6 + 1e-9 * q_as.abs(),
            "charge: ∫(S·I − I_rejected) dt = {q_as}, 3600·Δrem = {expected} \
             ({clamped_steps} clamped steps)"
        );
    }

    /// **The energy ledger at the top of the window closes exactly**, because the charge
    /// the clamp refused is dissipated rather than destroyed.
    ///
    /// Same accounting as `electrical_and_heat_energy_balance` and the same
    /// one-step-behind pairing, with one change that is the whole point: **the chemical
    /// side is measured from ground-truth state**, `V0·3600·Δ(remaining Ah)`, not
    /// assembled from reported currents.
    ///
    /// That is not a stylistic preference, and the first draft of this property got it
    /// wrong. Writing the chemical side as `V0·(S·i_actual − i_rejected_a)·dt` makes the
    /// assertion **tautological in `i_rejected_a`**: the field then appears on the
    /// chemical side and inside `q_gen_w`'s rejection term with opposite signs, they
    /// cancel, and the residual is identically zero for *any* rejected amount. The
    /// perturbation table proved it — doubling `rejected_as` in `coulomb_step` left this
    /// property green while four other tests failed. Sourcing the left-hand side from
    /// state puts `i_rejected_a` on one side only, and a wrong magnitude now shows up as
    /// heat that does not match the charge the cells actually stored.
    ///
    /// A flat OCV keeps the chemical side closed-form; `FLAT_V0` is therefore also
    /// `OCV(1.0)`. The tolerance is relative rather than at rounding because the two
    /// sides accumulate differently — one telescopes through the SOC state, the other
    /// sums a few hundred per-step products — so they agree to floating-point
    /// *accumulation* error, not to a single rounding.
    #[test]
    fn overcharge_heat_closes_the_energy_ledger(
        series in 1u16..=3,
        parallel in 1u16..=3,
        // See `charge_conserved_through_a_soc_clamp` on why these bounds and not
        // wider ones: the coverage assertion below is only meaningful if every
        // generated case can actually arrive.
        amps in 40.0f64..80.0,
        dt in 0.5f64..2.0,
        nsteps in 60usize..200,
        seed in any::<u64>(),
    ) {
        let mut pack = Pack::new(
            &cfg(series, parallel, 0.97, seed, Scatter::default()),
            flat_chem(),
        ).unwrap();

        let rem0 = total_remaining_ah(&pack, series, parallel);
        let mut electrical = 0.0;
        let mut heat = 0.0;
        let mut clamped_steps = 0usize;
        let mut v_start = pack.step(0.0, Demand::Current(-amps), &env()).v_terminal;
        for _ in 0..nsteps {
            let tele = pack.step(dt, Demand::Current(-amps), &env());
            if tele.flags.contains(EventFlags::SOC_CLAMPED_HIGH) {
                clamped_steps += 1;
            }
            prop_assert!(
                !tele.flags.contains(EventFlags::SOC_CLAMPED_LOW),
                "a charge should not have reached the bottom of the window"
            );
            electrical += v_start * tele.i_actual * dt;
            heat += tele.q_gen_w * dt;
            v_start = tele.v_terminal;
        }
        prop_assert!(clamped_steps > 0, "the run never reached the clamp it was aimed at");
        let rem1 = total_remaining_ah(&pack, series, parallel);

        // Ground truth, summed over cells: what the pack actually holds now against what
        // it held. Negative here, because this run charges.
        let chemical = FLAT_V0 * 3600.0 * (rem0 - rem1);
        let imbalance = chemical - electrical - heat;
        let tol = 1e-9 * chemical.abs().max(1.0);
        prop_assert!(
            imbalance.abs() < tol,
            "chemical {chemical} J vs electrical {electrical} J + heat {heat} J \
             (imbalance {imbalance} J, tol {tol} J, {clamped_steps} clamped steps)"
        );
    }

    /// **The bottom of the window fabricates nothing, and this is the property that used
    /// to pin how much it did.**
    ///
    /// It was written to fail when the defect was fixed — "a solve-side fix makes both
    /// sides zero and the `fabricated > 0.0` coverage assertion is where it announces
    /// itself, rather than a golden shifting by an amount nobody can attribute" — and
    /// that is exactly how the fix announced itself. The fix was not the solve-side one
    /// it predicted (no cell model can refuse a demanded current; see
    /// `docs/plans/low-clamp-solve-side.md`), but the outcome it described is the one
    /// that arrived: the cell now goes into voltage reversal, the charge is carried as a
    /// deficit rather than invented, and `i_rejected_a` is zero throughout.
    ///
    /// What replaces the old equation is a **closed cycle**: drive the pack far past
    /// empty, put exactly the same charge back, and rest until the overpotentials have
    /// relaxed. The chemical term is then zero by construction rather than by an
    /// integral over a curve this test would have to re-derive, and the whole ledger
    /// reduces to "whatever the circuit put in came back out as heat".
    ///
    /// It discriminates twice, and the first failure is in *state* rather than in energy.
    /// Charge pushed back into a cell the old engine had clamped at `soc = 0` repaid
    /// nothing, so an equal-and-opposite pair of legs left it *above* where it started —
    /// which is why [`total_remaining_ah`] reads `soc − soc_deficit` and why the
    /// round-trip assertion below comes first.
    ///
    /// **The energy tolerance is relative and loose where the other ledger properties are
    /// exact**, and the difference is physics rather than slack. Those run inside the
    /// window, where a flat OCV makes every term a closed form. This one crosses the
    /// reversal ramp, where the source moves *within* a step, so the residue is
    /// first-order in `dt` — real quadrature error, shrinking with the step. The rival it
    /// excludes is not a rounding difference: the pre-reversal engine fabricates
    /// kilojoules here and does not move with `dt` at all.
    #[test]
    fn the_bottom_of_the_window_fabricates_nothing(
        series in 1u16..=3,
        parallel in 1u16..=3,
        // See `charge_conserved_through_a_soc_clamp` on why these bounds and not
        // wider ones: the coverage assertion below is only meaningful if every
        // generated case can actually arrive.
        amps in 40.0f64..80.0,
        dt in 0.5f64..2.0,
        nsteps in 60usize..200,
        seed in any::<u64>(),
    ) {
        let mut pack = Pack::new(
            &cfg(series, parallel, 0.03, seed, Scatter::default()),
            flat_chem(),
        ).unwrap();

        let rem0 = total_remaining_ah(&pack, series, parallel);
        let mut electrical = 0.0;
        let mut heat = 0.0;
        let mut clamped_steps = 0usize;
        let mut v_start = pack.step(0.0, Demand::Current(amps), &env()).v_terminal;
        let mut leg = |pack: &mut Pack, i: f64, clamped: &mut usize| -> Result<(), TestCaseError> {
            for _ in 0..nsteps {
                let tele = pack.step(dt, Demand::Current(i), &env());
                if tele.flags.contains(EventFlags::SOC_CLAMPED_LOW) {
                    *clamped += 1;
                }
                prop_assert_eq!(
                    tele.i_rejected_a, 0.0,
                    "nothing is rejected at the bottom of the window any more"
                );
                electrical += v_start * tele.i_actual * dt;
                heat += tele.q_gen_w * dt;
                v_start = tele.v_terminal;
            }
            Ok(())
        };
        leg(&mut pack, amps, &mut clamped_steps)?;
        let deepest = total_remaining_ah(&pack, series, parallel);
        leg(&mut pack, -amps, &mut clamped_steps)?;
        // 20 RC time constants on this chemistry, so the overpotential left is ~1e-9 V.
        for _ in 0..(400.0 / dt).round() as usize {
            let tele = pack.step(dt, Demand::Rest, &env());
            electrical += v_start * tele.i_actual * dt;
            heat += tele.q_gen_w * dt;
            v_start = tele.v_terminal;
        }

        // Coverage, and the two halves are different claims: that the run reached the
        // clamp at all, and that it went meaningfully *past* it rather than grazing it.
        prop_assert!(clamped_steps > 0, "the run never reached the clamp it was aimed at");
        prop_assert!(
            deepest < 0.0,
            "the discharge leg is meant to end below empty, not at it: {deepest} Ah"
        );

        let rem1 = total_remaining_ah(&pack, series, parallel);
        prop_assert!(
            (rem1 - rem0).abs() < 1e-9 * rem0.abs().max(1.0),
            "the cycle must return the pack to where it started: {rem0} Ah then {rem1} Ah"
        );
        // Δstored = 0, so electrical-out + heat = 0.
        //
        // The tolerance is derived rather than tuned, because the residue here has a
        // known shape. Each step closes *exactly* against the OCV at the state it
        // started from, so a loop's residue is the rectangle rule's error going round
        // it: on the ramp that is `v_per_soc · Δx · Δq` per step, and summing it over
        // the `FLAT_V0 / v_per_soc` width of the ramp telescopes to `S · FLAT_V0 · I ·
        // dt` — independent of the slope, of the capacity, and of how many steps it
        // took. Measured at 1.5× that scale, so the factor below is margin over a
        // derivation, not a number picked to make a case pass.
        //
        // It is loose in absolute terms and still nowhere near the rival: the
        // pre-reversal engine fabricates `FLAT_V0` times the charge delivered past
        // empty, which on the smallest case here is ~6 kJ against a ~200 J bound.
        let residual = electrical + heat;
        let tol = 3.0 * f64::from(series) * FLAT_V0 * amps * dt;
        prop_assert!(
            residual.abs() < tol,
            "cycle imbalance {residual} J against {heat} J of heat (tol {tol} J, \
             {clamped_steps} clamped steps)"
        );
    }

    /// Health only ever gets worse. Capacity SOH never rises, resistance SOH never
    /// falls, and neither leaves its stated range however the pack is driven.
    ///
    /// `CLAUDE.md` forbids modelling capacity fade without the matching resistance
    /// growth, and this is that rule as an invariant rather than as a review comment:
    /// the two are asserted to move together on every step of every trajectory, not
    /// merely to exist.
    ///
    /// **Scatter is off**, and that is a real restriction rather than convenience.
    /// `Telemetry::soh_capacity` aggregates with constant weights (each cell's nominal
    /// capacity), so it is monotone on any pack. `soh_resistance` is a ratio of pack
    /// resistances, and its per-cell weights are conductances that move with SOC and
    /// temperature — on a *scattered* pack those weights re-sort as the trajectory runs,
    /// so the aggregate can dip while every underlying cell has only got worse. On a
    /// uniform pack the weights are equal and the ratio is exactly the common SOH.
    #[test]
    fn health_never_improves(
        series in 1u16..=3,
        parallel in 1u16..=3,
        soc0 in 0.2f64..0.8,
        currents in prop::collection::vec(-5.0f64..5.0, 1..40),
        dt in 0.5f64..5.0,
        seed in any::<u64>(),
    ) {
        let mut config = cfg(series, parallel, soc0, seed, Scatter::default());
        config.aging = Some(AgingConfig { sub_clock_period_s: 0.0 });
        // Warm, so this stays a pure aging property: below `t_plating_min_k` a charging
        // step would also plate, and a plating short would drain one series group faster
        // than the others and re-sort the resistance weights the doc comment warns about.
        config.initial_temp_k = 298.15;
        let mut pack = Pack::new(&config, aging_chem()).unwrap();

        let mut prev = pack.step(0.0, Demand::Rest, &env());
        for &i in &currents {
            let tele = pack.step(dt, Demand::Current(i), &env());
            prop_assert!(
                tele.soh_capacity <= prev.soh_capacity,
                "capacity SOH rose: {} -> {}", prev.soh_capacity, tele.soh_capacity
            );
            prop_assert!(
                tele.soh_resistance >= prev.soh_resistance,
                "resistance SOH fell: {} -> {}", prev.soh_resistance, tele.soh_resistance
            );
            prop_assert!(
                tele.soh_capacity > 0.0 && tele.soh_capacity <= 1.0,
                "soh_capacity {} outside (0, 1]", tele.soh_capacity
            );
            prop_assert!(
                tele.soh_resistance >= 1.0,
                "soh_resistance {} below 1", tele.soh_resistance
            );
            prev = tele;
        }

        // Coverage, not physics: a monotonicity assertion is satisfied by a constant, so
        // without these the property would still pass against an engine that never aged
        // anything — and did, on the first draft of this fixture (see `aging_chem`).
        prop_assert!(
            prev.soh_capacity < 1.0 && prev.soh_resistance > 1.0,
            "nothing aged over the run, so monotonicity was asserted of a constant: \
             capacity {}, resistance {}", prev.soh_capacity, prev.soh_resistance
        );
        prop_assert!(
            prev.soh_capacity > 0.5,
            "the fixture faded the pack to {} — it is sitting on the SOH floor, where \
             monotonicity is again trivial", prev.soh_capacity
        );
    }

    /// Snapshot round-trip with **every Phase 3 mechanism live at once**: aging,
    /// plating (including its seeded soft-short draw), an injected internal short, an
    /// external short, and a BMS whose current sensor is drawing noise from the same
    /// RNG.
    ///
    /// `snapshot_roundtrip_continues_identically` above covers the electrical and
    /// thermal state, but it runs an unaged, fault-free, `[safety]`-less pack — so four
    /// consecutive slices added per-cell state (the SOH pair and its accumulators, the
    /// plating charge counter, the shunt conductance, the exothermic budget and vent
    /// latch) without any of it ever crossing a serde boundary under proptest. Design
    /// principle 5 says the *entire* engine state round-trips; this is the version of
    /// that claim with Phase 3 in it.
    ///
    /// The pack runs **cold** on purpose. Below `t_plating_min_k` every charging step
    /// above the C-rate threshold plates, which both accumulates `q_plating` and rolls
    /// the seeded hazard — so the RNG stream, not just the float state, has to survive
    /// the round trip. A restored pack that resumed its draws one step out of phase
    /// would agree on the first step and diverge on a later one, which is why the tail
    /// is compared step by step rather than only at the end.
    ///
    /// Runaway state is excluded (the `[safety]` fixture puts onset out of reach); its
    /// mid-burn snapshot is pinned directly in `tests/runaway.rs`.
    #[test]
    fn snapshot_roundtrip_survives_aging_faults_and_plating(
        series in 1u16..=3,
        parallel in 1u16..=3,
        soc0 in 0.3f64..0.8,
        seed in any::<u64>(),
        short_ohms in 20.0f64..400.0,
        ext_ohms in 5.0f64..50.0,
        warmup in prop::collection::vec(-6.0f64..6.0, 1..15),
        tail in prop::collection::vec(-6.0f64..6.0, 1..15),
        dt in 0.5f64..3.0,
    ) {
        let mut config = cfg(series, parallel, soc0, seed, Scatter { capacity_sigma: 0.03, r0_sigma: 0.03 });
        config.aging = Some(AgingConfig { sub_clock_period_s: 10.0 });
        config.initial_temp_k = 263.15; // below t_plating_min_k, so charging plates
        config.bms = Some(BmsConfig {
            balancing: None,
            protection: None, // an open contactor would end the trajectory early
            current_offset_a: 0.01,
            current_noise_sigma_a: 0.05,
            temp_probes: vec![(0, 0)],
            initial_soc_error: 0.02,
            rest_current_threshold_a: 0.1,
            rest_time_for_ocv_s: 600.0,
            ocv_correction_gain: 0.5,
            min_ocv_slope_v_per_soc: 0.05,
        });
        let mut original = Pack::new(&config, aging_chem()).unwrap();
        original.schedule_fault(0.0, Fault::SoftInternalShort { s: 0, p: 0, ohms: short_ohms }).unwrap();
        // Part-way through the warm-up, so the queue itself is mid-flight at the
        // snapshot on some inputs and already drained on others.
        original.schedule_fault(dt * warmup.len() as f64 * 0.5, Fault::ExternalShort { ohms: ext_ohms }).unwrap();

        for &i in &warmup {
            original.step(dt, Demand::Current(i), &env());
        }

        let bytes = bincode::serialize(&original.snapshot()).unwrap();
        let snap: sim_core::Snapshot = bincode::deserialize(&bytes).unwrap();
        let mut restored = Pack::restore(&snap).unwrap();

        for &i in &tail {
            let a = original.step(dt, Demand::Current(i), &env());
            let b = restored.step(dt, Demand::Current(i), &env());
            prop_assert_eq!(a, b);
        }
        // Telemetry is a summary; the per-cell state Phase 3 added is mostly not in it.
        // Compare ground truth cell by cell, which is what actually pins `q_plating`,
        // the shunt conductance and the SOH pair.
        for s in 0..series as usize {
            for p in 0..parallel as usize {
                prop_assert_eq!(
                    original.cell(s, p).unwrap(),
                    restored.cell(s, p).unwrap(),
                    "cell {}S{}P diverged", s, p
                );
            }
        }

        // Coverage: a round-trip property is happy to pass over state that never moved,
        // so check the two mechanisms that are supposed to have written some.
        //
        // The aging check is conditional on the sub-clock having actually ticked. Short
        // trajectories that never reach `sub_clock_period_s` are deliberately *kept* in
        // the input space rather than tuned out — a pack snapshotted mid-period is
        // precisely the case where the accumulator has to survive the round trip — so
        // the assertion has to allow for them instead of demanding fade that the engine
        // correctly has not applied yet.
        let c0 = original.cell(0, 0).unwrap();
        let period = original.aging().expect("aging configured").sub_clock_period_s();
        prop_assert!(
            original.sim_time_s() < period || c0.soh_capacity < 1.0,
            "the sub-clock ticked ({} s elapsed, period {period} s) but aging never \
             moved, so the SOH fields round-tripped their initial values",
            original.sim_time_s()
        );
        prop_assert!(
            c0.internal_short_conductance_s > 0.0,
            "the injected short never fired, so the shunt round-tripped a zero"
        );
    }
}
