//! What a steady-state `Pack::step` allocates — counted, not timed.
//!
//! `docs/plans/pack-step-perf.md` names the remaining per-step allocations as an
//! identified-and-not-taken lever, and this file is the instrument that lever needs.
//! It exists because the wall-clock alternative does not work on this machine: the
//! same unchanged binary has read 52 and 79 µs inside a single bench batch, so a
//! single-digit-percent change is well under the noise floor (see the measurement
//! sections of that document). An allocation count has no such problem — it is
//! **deterministic**, so a change either removes a heap block or it does not, and
//! this test says which without asking the clock anything.
//!
//! Steady state, not first step, is the quantity: several buffers are legitimately
//! allocated once and reused forever after (`SourceCache`, the BMS's own frame), so
//! the run below warms the pack first and then counts the steps after that. Every
//! measured step must allocate the *same* amount, which is a stronger statement than
//! a total: a total of zero over twenty steps hides nothing, but a total of twenty
//! could be one step doing all the work.
//!
//! # Why a whole `GlobalAlloc` rather than a crate
//!
//! `sim-core` keeps its dependency list short on purpose (`CLAUDE.md`), and this is
//! twenty lines. The counter is process-wide, so this file holds exactly **one**
//! `#[test]` function: two tests running concurrently in one binary would count each
//! other's allocations. Do not add a second one — extend the table inside it.
//!
//! `#![forbid(unsafe_code)]` in `src/lib.rs` does not reach here: an integration test
//! is its own crate. The engine itself is untouched by this file.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use sim_core::chem::{
    AgingParams, CellLimits, ChemMeta, ChemistryParams, ElectrodeParams, OcpTable, OcvTable,
    R0Table, RcPair, RegularSolutionParams, SpmParams, ThermalParams,
};
use sim_core::{
    AgingConfig, BalancingConfig, BmsConfig, CellModelConfig, Demand, EkfConfig, Env,
    EstimatorConfig, Pack, PackConfig, ProtectionConfig, Scatter, ThermalConfig,
};

// --- the instrument -------------------------------------------------------------

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// Deliberately *not* overriding `realloc` or `alloc_zeroed`: the trait's default
// bodies route both through `alloc`, so a `Vec` that grows is counted like the fresh
// allocation it effectively is. Forwarding them to `System` would hide exactly the
// case this file is looking for.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Allocations and bytes charged to `f`.
fn measure<T>(f: impl FnOnce() -> T) -> (usize, usize, T) {
    let a0 = ALLOCS.load(Ordering::Relaxed);
    let b0 = BYTES.load(Ordering::Relaxed);
    let out = f();
    let a1 = ALLOCS.load(Ordering::Relaxed);
    let b1 = BYTES.load(Ordering::Relaxed);
    (a1 - a0, b1 - b0, out)
}

// --- the pack under test ---------------------------------------------------------

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// The same shape `tests/thevenin_cache.rs` uses: a sloped OCV, a two-dimensional
/// `R0` grid, two RC pairs and an entropy coefficient, so no lookup collapses to a
/// constant and no branch is skipped for want of a table.
fn rich_chem() -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah: 0.0,
        },
        aging: Some(AgingParams {
            cal_pre_exp: 1.0e4,
            cal_ea_j_per_mol: 5.0e4,
            cal_soc_stress: vec![1.0, 1.0, 1.4],
            cyc_fade_per_ah: 2.0e-5,
            cyc_dod_stress_exp: 1.1,
            r_growth_per_capacity_loss: 1.5,
        }),
        safety: None,
        spm: None,
        dfn: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "alloc".into(),
            name: "Allocation-count synthetic cell".into(),
            provenance: "allocation-count test — not physical".into(),
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
            docv_dt_v_per_k: Some(vec![-1.0e-4, -0.8e-4, -0.5e-4, -0.2e-4, 0.3e-4]),
            t_ref_k: None,
            soc: vec![0.0, 0.2, 0.5, 0.8, 1.0],
            volts: vec![3.00, 3.20, 3.30, 3.40, 3.60],
        },
        r0: R0Table {
            soc: vec![0.0, 0.5, 1.0],
            temp_k: vec![283.15, 298.15, 318.15],
            ohms: vec![
                vec![0.030, 0.022, 0.018],
                vec![0.028, 0.020, 0.016],
                vec![0.029, 0.021, 0.017],
            ],
        },
        rc: vec![
            RcPair {
                r_ohms: 0.010,
                c_farad: 2000.0,
            },
            RcPair {
                r_ohms: 0.006,
                c_farad: 5000.0,
            },
        ],
    }
}

/// [`rich_chem`] with an LFP `[spm]` section on the regular-solution potential: the
/// many-particle cell's split, its sub-steps and its bracketed fallback are new code on the
/// step path, and this is what puts them under the counter. The numbers are the Phase 9
/// spike's (`sim-data/tests/ensemble.rs` has the full fixture).
fn ensemble_chem() -> ChemistryParams {
    let mut c = rich_chem();
    c.cell.capacity_ah = 2.303_324_557_209_257;
    let electrode =
        |r: f64, d: f64, c_max: f64, eps: f64, l: f64, m: f64, ea: f64| ElectrodeParams {
            particle_radius_m: r,
            diffusivity_m2_per_s: d,
            c_max_mol_per_m3: c_max,
            active_volume_fraction: eps,
            thickness_m: l,
            m_ref: m,
            reaction_ea_j_per_mol: ea,
            diffusivity_ea_j_per_mol: 0.0,
            charge_transfer_alpha: 0.5,
            stoich_min: 0.0,
            stoich_max: 1.0,
            docp_dt_v_per_k: 0.0,
            ocp: OcpTable {
                stoich: vec![0.0, 1.0],
                volts: vec![0.0, 0.0],
            },
            regular_solution: None,
        };
    let mut neg = electrode(5e-6, 3e-15, 30555.0, 0.58, 3.4e-5, 6.48e-7, 35000.0);
    neg.stoich_min = 0.017_617_931_791_027_094;
    neg.stoich_max = 0.81;
    neg.ocp = OcpTable {
        stoich: vec![0.0, 0.02, 0.1, 0.2, 0.5, 0.7, 1.0],
        volts: vec![
            2.383542, 1.304718, 0.406516, 0.216986, 0.133086, 0.092194, 0.09202,
        ],
    };
    let mut pos = electrode(5e-8, 5.9e-18, 22806.0, 0.374, 8e-5, 6e-7, 39570.0);
    pos.stoich_min = 0.0038;
    pos.stoich_max = 0.703_502_020_929_131_3;
    pos.ocp = OcpTable {
        stoich: vec![0.0, 1.0],
        volts: vec![3.6, 3.3],
    };
    pos.regular_solution = Some(RegularSolutionParams {
        u0_v: 3.42,
        omega_ev: 0.075_917_388_974_677_12,
    });
    c.spm = Some(SpmParams {
        t_ref_k: 298.15,
        c_e_mol_per_m3: 1200.0,
        electrode_area_m2: 0.18,
        contact_resistance_ohm: 0.0,
        negative: neg,
        positive: pos,
    });
    c
}

fn bms() -> BmsConfig {
    BmsConfig {
        balancing: Some(BalancingConfig {
            bleed_r_ohms: 47.0,
            v_threshold_v: 3.0, // below the resting voltage here, so bleeds close
            v_release_band_v: 0.010,
        }),
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.2,
            t_hard_margin_k: 10.0,
            v_release_band_v: 0.08,
            t_release_band_k: 2.0,
        }),
        current_offset_a: 0.01,
        current_noise_sigma_a: 0.05,
        temp_probes: vec![(0, 0), (2, 1)],
        initial_soc_error: 0.05,
        rest_current_threshold_a: 0.1,
        rest_time_for_ocv_s: 5.0,
        ocv_correction_gain: 0.5,
        min_ocv_slope_v_per_soc: 0.1,
        estimator: Default::default(),
    }
}

fn base_config() -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 4,
        parallel: 3,
        initial_soc: 0.7,
        initial_temp_k: 298.15,
        seed: 0xCAFE_F00D,
        scatter: Scatter {
            capacity_sigma: 0.03,
            r0_sigma: 0.05,
        },
        cell_model: CellModelConfig::Ecm,
    }
}

/// Discharge, rest, charge, power — every demand arm the linear path can take, so a
/// buffer that only one of them touches cannot hide behind a steady drain.
fn demand_at(step: usize) -> Demand {
    match step % 8 {
        0..=3 => Demand::Current(4.0),
        4 => Demand::Rest,
        5..=6 => Demand::Current(-3.0),
        _ => Demand::Power(12.0),
    }
}

/// Warm-up steps discarded before counting. Generous: the point is that whatever is
/// allocated once and kept is *not* what this file is measuring.
const WARM: usize = 12;
/// Counted steps. Each is reported separately, and they must agree.
const COUNTED: usize = 16;

/// `(label, config)` — every combination of the two components that own per-step
/// buffers, plus aging, which owns none but runs inside the same loop.
fn cases() -> Vec<(&'static str, PackConfig)> {
    let network = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let mut out = Vec::new();

    out.push(("bare (no thermal, no BMS)", base_config()));

    let mut c = base_config();
    c.thermal = network;
    out.push(("thermal network only", c));

    let mut c = base_config();
    c.bms = Some(bms());
    out.push(("BMS only", c));

    let mut c = base_config();
    c.thermal = network;
    c.bms = Some(bms());
    out.push(("thermal + BMS", c));

    let mut c = base_config();
    c.thermal = network;
    c.bms = Some(bms());
    c.aging = Some(AgingConfig {
        sub_clock_period_s: 0.0, // age on every step: the most expensive arm
    });
    out.push(("thermal + BMS + aging", c));

    // The model-based estimator: its state and covariance are fixed-size arrays, and
    // this is the arm that would catch one turning into a `Vec`.
    let mut c = base_config();
    c.thermal = network;
    let mut b = bms();
    b.estimator = EstimatorConfig::Ekf(EkfConfig {
        current_sigma_a: 0.05,
        voltage_sigma_v: 0.01,
        initial_soc_sigma: 0.05,
    });
    c.bms = Some(b);
    out.push(("thermal + BMS (EKF estimator)", c));

    // The size `CLAUDE.md`'s budget is stated at, so the bytes below are the ones that
    // matter rather than an extrapolation from a toy pack: three of these buffers are
    // one `f64` per cell, so they are 8 kB each here and 96 B each in the 4S3P cases.
    let mut c = base_config();
    c.thermal = network;
    c.bms = Some(bms());
    c.series = 100;
    c.parallel = 10;
    out.push(("100S10P, thermal + BMS", c));

    out
}

/// The many-particle cell: a scattered 1S2P of 20 particles each, from a third charged, at a
/// step of one sub-step and at one of four — `(label, config, dt)`.
fn ensemble_cases() -> Vec<(&'static str, PackConfig, f64)> {
    let mut c = base_config();
    c.thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    c.series = 1;
    c.parallel = 2;
    c.initial_soc = 0.35;
    c.cell_model = CellModelConfig::SpmEnsemble {
        shells: 20,
        particles: 20,
        radius_sigma: 0.2,
    };
    vec![
        ("SpmEnsemble 20 particles, 2 s", c.clone(), 2.0),
        ("SpmEnsemble 20 particles, 37 s", c, 37.0),
    ]
}

#[test]
fn a_warm_step_allocates_nothing() {
    let mut failures: Vec<String> = Vec::new();
    let mut porous: Vec<(&str, usize)> = Vec::new();

    let all = cases()
        .into_iter()
        .map(|(label, cfg)| (label, cfg, rich_chem(), 2.0))
        .chain(
            ensemble_cases()
                .into_iter()
                .map(|(label, cfg, dt)| (label, cfg, ensemble_chem(), dt)),
        )
        .chain(std::iter::once({
            // The single-particle model on the same cell, its positive on the table.
            let mut chem = ensemble_chem();
            if let Some(spm) = chem.spm.as_mut() {
                spm.positive.regular_solution = None;
            }
            let (_, mut cfg, _) = ensemble_cases().remove(0);
            cfg.cell_model = CellModelConfig::Spm { shells: 20 };
            ("Spm 20 shells, 2 s", cfg, chem, 2.0)
        }));
    for (label, cfg, chem, dt) in all {
        let mut pack = Pack::new(&cfg, chem).expect("valid config");
        for s in 0..WARM {
            pack.step(dt, demand_at(s), &env());
        }

        // Per step, so a single first-step allocation cannot average away.
        let mut per_step: Vec<(usize, usize)> = Vec::with_capacity(COUNTED);
        for s in 0..COUNTED {
            let d = demand_at(WARM + s);
            let e = env();
            let (allocs, bytes, _) = measure(|| pack.step(dt, d, &e));
            per_step.push((allocs, bytes));
        }

        let worst = per_step.iter().map(|&(a, _)| a).max().unwrap_or(0);
        let best = per_step.iter().map(|&(a, _)| a).min().unwrap_or(0);
        let bytes = per_step.iter().map(|&(_, b)| b).max().unwrap_or(0);
        println!("{label:26} allocs/step {best}..{worst}   bytes/step <= {bytes}");

        if label.starts_with("Spm") {
            porous.push((label, worst));
        } else if worst != 0 {
            failures.push(format!(
                "`{label}`: a warm step allocates {best}..{worst} times ({bytes} B at worst)"
            ));
        }
    }

    // The porous models' nonlinear path is **not** allocation-free, and was not before
    // Phase 9: four per-step scratch `Vec`s of the pack's pass loop (`tangent`, `probed`,
    // `i_cell`, `trial_src`/`report_src`), measured at 4 blocks and 224 B a step on this
    // 1S2P — the slice that brought `Pack::step` to zero measured equivalent-circuit packs
    // only. Recorded in `docs/ROADMAP.md` H9. What is pinned here is what Phase 9 slice B
    // owns: its split, sub-steps and fallback add **nothing** to the single-particle
    // model's count on the same pack.
    let spm = porous
        .iter()
        .find(|(l, _)| l.starts_with("Spm 20"))
        .map(|&(_, w)| w)
        .expect("the single-particle case ran");
    for &(label, worst) in &porous {
        if worst > spm {
            failures.push(format!(
                "`{label}`: {worst} allocations a step against the single-particle model's {spm}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "`Pack::step` allocates on the steady-state path:\n  {}\n\nEvery buffer a step \
         needs is either owned by the pack across the step boundary or written into one \
         that is. See `docs/plans/pack-step-perf.md`.",
        failures.join("\n  ")
    );
}
