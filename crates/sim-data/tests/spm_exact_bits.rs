//! Exact-bit pins for the SPM path (Phase 6, slice E).
//!
//! # Why this file exists
//! Every other SPM assertion in this repo is a **tolerance**, and slice C2 proved
//! twice over what that costs. `FARADAY_C_PER_MOL` was once "tidied" from the exact
//! defined product to the commonly quoted `96485.33212331` — a *different* `f64`,
//! one ULP away — and every SPM trajectory moved while all six shipped tolerances
//! (30 mV, 1 %, 2 %, 0.5 %) stayed green. Separately, a `(c·vol)/vol` round trip
//! that "cannot change anything" landed one ULP off 1600 times in 2000.
//!
//! The ECM path has an answer to this: an out-of-tree instrument that dumps every
//! reported `f64` as raw bits and is diffed across a change (see
//! `docs/plans/phase-6-porous-electrodes.md`). That instrument stays out of tree
//! because `CLAUDE.md` refuses to promise bit-exactness across libm implementations,
//! so a committed full-trajectory fixture would fail for anyone who clones on
//! another OS.
//!
//! **This file is the part of that defence that *can* be committed**, and the reason
//! is narrow and load-bearing: everything pinned below is **pure IEEE-754
//! arithmetic** — additions, multiplications, divisions and correctly-rounded
//! decimal-to-`f64` parsing. No `exp`, no `asinh`, no `powf`. Those operations are
//! exactly specified by the standard and give identical bits on every conforming
//! platform, so pinning them promises nothing `CLAUDE.md` declines to promise. The
//! moment an assertion here needs a transcendental it belongs in the out-of-tree
//! instrument instead.
//!
//! # What this catches, and what it does not
//! It catches a digit changing anywhere in the shipped `[spm]` section, a units
//! slip in the derived geometry, and a rewrite of either finite-volume helper that
//! is "algebraically the same". It does **not** catch a change to the kinetics, the
//! OCP interpolation or the diffusion solve, all of which route through
//! transcendentals; those are covered by `spm_golden.rs`'s tolerances and, across a
//! change, by the out-of-tree instrument.
//!
//! # If one of these fails
//! A failure is not automatically a bug — re-extracting the chemistry from a newer
//! parameter set *should* fail the first test. It means "a number moved; say which
//! and why in the commit message, then update the pin". What it must never mean is
//! "widen it".

use sim_core::chem::{ChemistryParams, ElectrodeParams, SpmParams};
use sim_core::spm::{c_surface, mean_concentration, FARADAY_C_PER_MOL};

fn lgm50() -> ChemistryParams {
    let text = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
    sim_data::parse_chemistry(text).expect("LG M50 chemistry loads")
}

fn spm() -> SpmParams {
    lgm50()
        .spm
        .expect("the shipped LG M50 file has an [spm] section")
}

/// FNV-1a over the raw bits of a sequence of `f64`s.
///
/// A hash rather than one assertion per value because the `[spm]` section carries
/// 65 OCP breakpoints per electrode plus their potentials, and a per-number pin
/// would be 150 lines of noise that nobody would keep current. FNV-1a is chosen for
/// the same reason the out-of-tree instrument uses it: it is four lines, it has no
/// dependency, and every input bit reaches the output.
fn fnv1a(values: impl IntoIterator<Item = f64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// Every `f64` in one electrode's parameters, in a fixed order.
fn electrode_values(e: &ElectrodeParams) -> Vec<f64> {
    let mut v = vec![
        e.particle_radius_m,
        e.diffusivity_m2_per_s,
        e.c_max_mol_per_m3,
        e.active_volume_fraction,
        e.thickness_m,
        e.m_ref,
        e.reaction_ea_j_per_mol,
        e.diffusivity_ea_j_per_mol,
        e.charge_transfer_alpha,
        e.stoich_min,
        e.stoich_max,
        e.docp_dt_v_per_k,
    ];
    v.extend(e.ocp.stoich.iter().copied());
    v.extend(e.ocp.volts.iter().copied());
    v
}

#[test]
fn the_shipped_spm_parameters_are_pinned_bit_for_bit() {
    let s = spm();
    let mut values = vec![
        s.t_ref_k,
        s.c_e_mol_per_m3,
        s.electrode_area_m2,
        s.contact_resistance_ohm,
    ];
    values.extend(electrode_values(&s.negative));
    values.extend(electrode_values(&s.positive));

    // Sanity on the hash's own coverage before trusting it: 4 shared scalars plus
    // 12 scalars and two tables per electrode. If the struct grows a field, this
    // count moves and the reader is told to extend `electrode_values` rather than
    // silently pinning a subset.
    //
    // The table counts went 45 -> 47 and 20 -> 27 in Phase 7 slice A, which extended
    // both OCP tables to the full stoichiometry range [0, 1]. That is an **append**:
    // every pre-existing breakpoint and potential is bit-identical and no point was
    // inserted inside the old spans, verified mechanically rather than assumed. The
    // hash below moves anyway, because it hashes the whole table.
    assert_eq!(
        values.len(),
        4 + 2 * 12 + (47 + 47) + (27 + 27),
        "the [spm] value count changed — extend electrode_values() before repinning"
    );

    assert_eq!(
        fnv1a(values),
        0x2410_32c7_99f2_e883,
        "a number in chemistries/nmc_21700_lgm50.toml's [spm] section changed. \
         That is allowed — re-extraction from a newer parameter set is exactly the \
         case — but it must be deliberate: say which number and why, then update \
         this pin. Do not delete the assertion."
    );
}

#[test]
fn the_shipped_spm_geometry_derives_to_exact_bits() {
    // The three quantities every step is built on, derived here the way
    // `spm::Geometry` and `spm::geometric_capacity_ah` derive them. Pure arithmetic,
    // so these are exact `f64`s rather than approximations, and pinning them catches
    // a ULP moving in `FARADAY_C_PER_MOL`, in either electrode's geometry, or in the
    // stoichiometry limits — a class of change no tolerance in `spm_golden.rs` can
    // see (its tightest is 2 mV, and one ULP of Faraday is ~1e-11 relative).
    let s = spm();
    let derive = |e: &ElectrodeParams| {
        let volume_m3 = e.active_volume_fraction * s.electrode_area_m2 * e.thickness_m;
        let area_m2 = 3.0 * volume_m3 / e.particle_radius_m;
        let capacity_ah =
            (e.stoich_max - e.stoich_min) * e.c_max_mol_per_m3 * volume_m3 * FARADAY_C_PER_MOL
                / 3600.0;
        (volume_m3, area_m2, capacity_ah)
    };

    let (v_n, a_n, q_n) = derive(&s.negative);
    let (v_p, a_p, q_p) = derive(&s.positive);

    assert_eq!(
        v_n.to_bits(),
        0x3edb_8676_82ba_6e49,
        "negative active volume [m3]"
    );
    assert_eq!(
        a_n.to_bits(),
        0x400a_e093_d8f0_f378,
        "negative interfacial area [m2]"
    );
    assert_eq!(
        v_p.to_bits(),
        0x3ed5_a7e1_0fbd_5dfb,
        "positive active volume [m3]"
    );
    assert_eq!(
        a_p.to_bits(),
        0x4007_bd13_2c0f_0fde,
        "positive interfacial area [m2]"
    );
    assert_eq!(
        q_n.to_bits(),
        0x4014_9ce0_05a2_278a,
        "negative geometric capacity [Ah]"
    );

    // And the claims those bits are *about*, stated in physical terms so whoever has
    // to repin them knows what the new numbers should look like.
    //
    // The electrodes are balanced — and on this parameter set they are balanced to
    // the *bit*, which is not a coincidence worth hiding: Chen2020's stoichiometry
    // limits come from `get_min_max_stoichiometries`, which solves for the windows
    // that hold the same charge at both electrodes. The assertion is written as a
    // relative tolerance anyway, because a future re-extraction is entitled to land
    // a ULP apart and that would be balance, not breakage.
    assert!(
        (q_n / q_p - 1.0).abs() < 1e-9,
        "the two electrodes are not balanced: {q_n} vs {q_p} A.h"
    );
    // And the geometry agrees with what the [cell] section configures, which is what
    // makes `Working::kappa` ≈ 1 — i.e. the flux the particles see is the flux the
    // terminal current implies, with no hidden rescale. The residual 6.3e-8 is the
    // six-decimal rounding of `capacity_ah` in the TOML against the geometry's own
    // 5.153198326 A.h, and `spm_golden.rs`'s SOC-readout test is where that same
    // 6.3e-8 shows up as the drift it causes over a full discharge.
    assert!(
        (q_n / lgm50().cell.capacity_ah - 1.0).abs() < 1e-6,
        "geometric capacity {q_n} A.h has drifted from the configured capacity_ah"
    );
}

#[test]
fn the_finite_volume_helpers_are_pinned_bit_for_bit() {
    // `mean_concentration` is the quantity the scheme conserves and the one SOC is
    // read from; `c_surface` is the half-shell extrapolation the whole concentration
    // overpotential rests on. Both are pure arithmetic, and both have already been
    // rewritten once during this phase — C2's `(c·vol)/vol` round trip is the exact
    // shape of edit these two invite, and it is not the identity.
    //
    // The profile is deliberately not uniform: a uniform one makes
    // `mean_concentration` return its input whatever the volume weights are, which
    // is the version of this test that pins nothing.
    let profile: Vec<f64> = (0..20)
        .map(|i| 12_345.678_9 + 137.0 * (i as f64) - 3.5 * (i as f64) * (i as f64))
        .collect();

    let mean = mean_concentration(&profile);
    assert_eq!(mean.to_bits(), 0x40ca_73c0_b0f2_7bb2, "mean_concentration");

    // Chen2020's negative electrode at a plausible discharge flux.
    let surf = c_surface(&profile, 5.86e-06, 3.3e-14, 3.1794e-06);
    assert_eq!(surf.to_bits(), 0x40ca_b388_3aaf_3bfd, "c_surface");

    // Zero flux is the one case with an answer that can be stated without a bit
    // pattern, so it is stated: the surface *is* the outermost shell average.
    assert_eq!(
        c_surface(&profile, 5.86e-06, 3.3e-14, 0.0),
        profile[profile.len() - 1]
    );
}
