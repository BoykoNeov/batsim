//! Phase 1 seeded-scatter tests.
//!
//! Scatter draws per-cell capacity/R0 factors from the single pack RNG at
//! construction. These check the three things that matter: no scatter means
//! exactly-nominal cells, the draw is deterministic per seed (a determinism-rule
//! requirement), and the distribution is sane (mean ≈ 1, spread ≈ σ, all positive).

use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::{Pack, PackConfig, Scatter};

fn chem() -> ChemistryParams {
    ChemistryParams {
        meta: ChemMeta {
            id: "s".into(),
            name: "Scatter test cell".into(),
            provenance: "scatter test — not physical".into(),
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
            soc: vec![0.0, 1.0],
            volts: vec![3.0, 3.5],
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

fn config(series: u16, parallel: u16, seed: u64, scatter: Scatter) -> PackConfig {
    PackConfig {
        series,
        parallel,
        initial_soc: 0.7,
        initial_temp_k: 298.15,
        seed,
        scatter,
    }
}

/// Collect `(capacity_factor, r0_factor)` for every cell, series-major.
fn factors(pack: &Pack, series: u16, parallel: u16) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for s in 0..series as usize {
        for p in 0..parallel as usize {
            let v = pack.cell(s, p).unwrap();
            out.push((v.capacity_factor, v.r0_factor));
        }
    }
    out
}

#[test]
fn no_scatter_gives_exactly_nominal_factors() {
    let pack = Pack::new(&config(4, 4, 42, Scatter::default()), chem()).unwrap();
    for (cap, r0) in factors(&pack, 4, 4) {
        assert_eq!(cap, 1.0);
        assert_eq!(r0, 1.0);
    }
}

#[test]
fn scatter_is_deterministic_per_seed() {
    let sc = Scatter {
        capacity_sigma: 0.03,
        r0_sigma: 0.05,
    };
    let a = Pack::new(&config(3, 3, 7, sc), chem()).unwrap();
    let b = Pack::new(&config(3, 3, 7, sc), chem()).unwrap();
    // Same seed + topology ⇒ identical per-cell factors, bit for bit.
    assert_eq!(factors(&a, 3, 3), factors(&b, 3, 3));

    // A different seed must change the draw (astronomically unlikely to collide).
    let c = Pack::new(&config(3, 3, 8, sc), chem()).unwrap();
    assert_ne!(factors(&a, 3, 3), factors(&c, 3, 3));
}

#[test]
fn scatter_distribution_is_sane() {
    let cap_sigma = 0.03;
    let r0_sigma = 0.08;
    let (series, parallel) = (100u16, 10u16); // 1000 cells
    let sc = Scatter {
        capacity_sigma: cap_sigma,
        r0_sigma,
    };
    let pack = Pack::new(&config(series, parallel, 12345, sc), chem()).unwrap();
    let fs = factors(&pack, series, parallel);
    let n = fs.len() as f64;

    let mean = |sel: fn(&(f64, f64)) -> f64| fs.iter().map(sel).sum::<f64>() / n;
    let std = |sel: fn(&(f64, f64)) -> f64, m: f64| {
        (fs.iter().map(|x| (sel(x) - m).powi(2)).sum::<f64>() / n).sqrt()
    };

    let cap_mean = mean(|x| x.0);
    let r0_mean = mean(|x| x.1);
    let cap_std = std(|x| x.0, cap_mean);
    let r0_std = std(|x| x.1, r0_mean);

    // Mean ≈ 1 (SE = σ/√N ≈ 1e-3, so 0.01 is very safe); spread ≈ σ within ±50 %.
    assert!((cap_mean - 1.0).abs() < 0.01, "cap mean {cap_mean}");
    assert!((r0_mean - 1.0).abs() < 0.01, "r0 mean {r0_mean}");
    assert!(
        cap_std > 0.5 * cap_sigma && cap_std < 1.5 * cap_sigma,
        "cap std {cap_std} vs σ {cap_sigma}"
    );
    assert!(
        r0_std > 0.5 * r0_sigma && r0_std < 1.5 * r0_sigma,
        "r0 std {r0_std} vs σ {r0_sigma}"
    );
    // Every factor must be strictly positive (the group solve depends on it).
    for (cap, r0) in fs {
        assert!(cap > 0.0 && r0 > 0.0, "non-positive factor: {cap}, {r0}");
    }
}
