//! Engine invariants of the single-particle cell model (Phase 6 slice C2).
//!
//! These are claims about the *integrator and the interface*, so they run against a
//! chemistry built in Rust rather than against a shipped file — `sim-core` cannot
//! read one, and none of these assertions is about a particular cell. The
//! assertions that are about `chemistries/nmc_21700_lgm50.toml` specifically (the
//! voltage window, the two aging multipliers, the build errors) live in
//! `sim-data/tests/spm_pack.rs`, where the file can be parsed.
//!
//! # What is covered here and why each one earns its place
//! * **`dt`-independence** — the reason the integrator is backward Euler rather than
//!   explicit. It is the property that lets one code path serve real-time stepping
//!   and a months-long aging fast-forward, and the spike measured it before this
//!   model was designed. Here it is a test rather than an observation.
//! * **Overpotential sign and relaxation** — the `Spm` arm of the same claim
//!   `cell_model.rs` makes for both ECM arms.
//! * **Voltage against equilibrium** — the property-table row that a wrong
//!   overpotential sign fails and that a voltage-RMS tolerance in slice E would
//!   absorb.
//! * **Round-trip energy** — the sharpest available check on the heat term.
//! * **Snapshot round-trip** — exit criterion 3, first half.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, ElectrodeParams, OcpTable, OcvTable, R0Table, RcPair,
    SpmParams, ThermalParams,
};
use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, Snapshot, ThermalConfig};

/// A single-particle chemistry whose numbers come from
/// `chemistries/nmc_21700_lgm50.toml` (LG M50, extracted from Chen2020), with both
/// OCP tables **decimated** to a handful of their points.
///
/// The decimation is deliberate and costs nothing here: no assertion in this file
/// is about a voltage the cell reaches, only about how the model behaves as the
/// state moves, and a coarse table is still monotone non-increasing and still spans
/// the window. The full tables are exercised against the shipped file in
/// `sim-data/tests/spm_pack.rs`.
///
/// The equivalent-circuit sections are present because `ChemistryParams` requires
/// them; nothing in this file runs them.
fn spm_chem() -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
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
        dfn: None,
        spm: Some(SpmParams {
            t_ref_k: 298.15,
            c_e_mol_per_m3: 1000.0,
            electrode_area_m2: 0.1027,
            contact_resistance_ohm: 0.0,
            negative: ElectrodeParams {
                particle_radius_m: 5.86e-6,
                diffusivity_m2_per_s: 3.3e-14,
                c_max_mol_per_m3: 33133.0,
                active_volume_fraction: 0.75,
                thickness_m: 8.52e-5,
                m_ref: 6.48e-7,
                reaction_ea_j_per_mol: 35000.0,
                diffusivity_ea_j_per_mol: 0.0,
                charge_transfer_alpha: 0.5,
                stoich_min: 0.026_345_790_270_645_77,
                stoich_max: 0.910_618_046_652_409,
                docp_dt_v_per_k: 0.0,
                ocp: OcpTable {
                    stoich: vec![
                        0.0, 0.026897, 0.082613, 0.182037, 0.360232, 0.562442, 0.720464, 0.840541,
                        0.960618,
                    ],
                    volts: vec![
                        2.383542, 1.090348, 0.466069, 0.224442, 0.139904, 0.130928, 0.092070,
                        0.092020, 0.092020,
                    ],
                },
            },
            positive: ElectrodeParams {
                particle_radius_m: 5.22e-6,
                diffusivity_m2_per_s: 4.0e-15,
                c_max_mol_per_m3: 63104.0,
                active_volume_fraction: 0.665,
                thickness_m: 7.56e-5,
                m_ref: 3.42e-6,
                reaction_ea_j_per_mol: 17800.0,
                diffusivity_ea_j_per_mol: 0.0,
                charge_transfer_alpha: 0.5,
                stoich_min: 0.263_845_224_591_330_1,
                stoich_max: 0.853_974_674_630_047,
                docp_dt_v_per_k: 0.0,
                ocp: OcpTable {
                    stoich: vec![0.213845, 0.300111, 0.428821, 0.558910, 0.731442, 0.903975],
                    volts: vec![4.439085, 4.205249, 4.060738, 3.884181, 3.704691, 3.564985],
                },
            },
        }),
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "spm_test".into(),
            name: "SPM interface test cell".into(),
            provenance: "geometry, transport and kinetics copied from \
                         chemistries/nmc_21700_lgm50.toml; OCP tables decimated \
                         from the same file — engine test fixture, not a chemistry"
                .into(),
        },
        cell: CellLimits {
            capacity_ah: 5.153198,
            v_max: 4.20,
            v_min: 2.50,
            max_charge_c: 1.0,
            max_discharge_c: 2.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            t_ref_k: None,
            soc: vec![0.0, 1.0],
            volts: vec![2.5, 4.2],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
    }
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn cfg(shells: usize, initial_soc: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k: 298.15,
        seed: 7,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Spm { shells },
    }
}

fn pack(shells: usize, initial_soc: f64) -> Pack {
    Pack::new(&cfg(shells, initial_soc), spm_chem()).expect("the SPM pack builds")
}

// ---------------------------------------------------------------------------
// The property the integrator was chosen for
// ---------------------------------------------------------------------------

/// One amp-hour drawn in one second-long step at a time and in one hour-long step
/// lands on the same state of charge.
///
/// # This is the whole argument for backward Euler
/// Explicit Euler on this grid is stable only to `dt ≈ dr²/(2·D)` — about 12 s at 10
/// shells for these parameters — so a scenario that fast-forwards a month at
/// `dt = 3600 s` would not merely lose accuracy, it would diverge. Backward Euler is
/// unconditionally stable, which is what lets `CLAUDE.md`'s "same code path serves
/// real-time stepping and months-long aging" hold for a model with a genuine PDE in
/// it.
///
/// The tolerance is on **SOC**, which is a bulk quantity and therefore the one the
/// scheme conserves exactly (see the mass-conservation note in `diffuse`). The
/// terminal *voltage* at the coarse step is a different matter and is deliberately
/// not asserted here: a 3600 s step evaluates the surface concentration once for the
/// whole hour, so the two runs differ by ~1 mV at the end. That is discretisation
/// error doing exactly what discretisation error does, and pinning it would be
/// pinning the grid rather than the property.
#[test]
fn state_of_charge_after_an_hour_does_not_depend_on_the_step_size() {
    let soc_after_an_hour = |dt: f64| {
        let mut p = pack(10, 0.9);
        let steps = (3600.0 / dt).round() as usize;
        let mut soc = f64::NAN;
        for _ in 0..steps {
            soc = p.step(dt, Demand::Current(1.0), &env()).soc_true;
        }
        soc
    };

    let fine = soc_after_an_hour(1.0);
    let coarse = soc_after_an_hour(60.0);
    let fast_forward = soc_after_an_hour(3600.0);
    for (label, value) in [("dt = 60 s", coarse), ("dt = 3600 s", fast_forward)] {
        assert!(
            (value - fine).abs() < 1e-9,
            "1 A for 1 h must reach the same SOC at any step size: dt = 1 s gives \
             {fine}, {label} gives {value}"
        );
    }
    // And the amount drawn is the amount asked for: 1 Ah out of a 5.153198 Ah cell.
    let expected = 0.9 - 1.0 / 5.153198;
    assert!(
        (fine - expected).abs() < 1e-6,
        "charge conservation: 1 Ah from soc 0.9 should reach {expected}, got {fine}"
    );
}

/// Refining the radial grid changes the answer by less and less — the discretisation
/// is converging rather than merely running.
///
/// Asserted on the *terminal voltage*, which is where the grid actually shows up:
/// SOC is a bulk quantity the scheme conserves at any shell count, so a SOC-based
/// convergence test would pass on a broken discretisation. Slice E turns this into a
/// measured accuracy-vs-cost curve and picks a default `N`; here it only has to
/// converge.
#[test]
fn refining_the_shell_grid_converges() {
    let voltage_after = |shells: usize| {
        let mut p = pack(shells, 0.9);
        let mut v = f64::NAN;
        for _ in 0..600 {
            v = p.step(1.0, Demand::Current(5.0), &env()).v_terminal;
        }
        v
    };

    let (v5, v10, v20, v40) = (
        voltage_after(5),
        voltage_after(10),
        voltage_after(20),
        voltage_after(40),
    );
    let (coarse_gap, fine_gap) = ((v10 - v5).abs(), (v40 - v20).abs());
    assert!(
        fine_gap < coarse_gap,
        "halving the shell size must shrink the difference it makes: 5 -> 10 moved \
         the voltage by {coarse_gap} V, 20 -> 40 by {fine_gap} V (v5 = {v5}, \
         v10 = {v10}, v20 = {v20}, v40 = {v40})"
    );
    assert!(
        fine_gap < 5e-3,
        "20 and 40 shells should agree to a few millivolts, differ by {fine_gap} V"
    );
}

// ---------------------------------------------------------------------------
// Signs — the failure mode a tolerance would absorb
// ---------------------------------------------------------------------------

/// Terminal voltage sits **below** the equilibrium voltage while discharging and
/// **above** it while charging, and the gap closes on rest.
///
/// Equilibrium here means the voltage at *mean* (bulk) stoichiometry, not at the
/// surface — the surface-vs-bulk gap is an overpotential the cell gets back on
/// relaxation, not energy it has spent. That distinction is why this test reads the
/// equilibrium voltage as `v_terminal + overpotential_v` rather than by resting the
/// cell: resting would change the state it is asking about.
///
/// A sign error in either Butler–Volmer overpotential inverts one of these two
/// assertions while leaving the open-circuit curve, the capacity and the `dt`
/// independence all intact — which is precisely the failure a slice E voltage
/// tolerance would absorb rather than reject.
#[test]
fn loaded_voltage_falls_below_equilibrium_and_charging_lifts_it_above() {
    for (label, i, discharging) in [("discharge", 5.0, true), ("charge", -5.0, false)] {
        let mut p = pack(10, 0.5);
        for _ in 0..120 {
            p.step(1.0, Demand::Current(i), &env());
        }
        let tele = p.step(0.0, Demand::Current(i), &env());
        let cell = p.cell(0, 0).expect("cell 0S0P exists");
        // v_terminal = U_eq − η − i·R_ohmic, and this chemistry's contact resistance
        // is zero, so the overpotential is the whole gap.
        let gap = cell.overpotential_v;
        assert!(
            gap.is_finite() && gap.abs() > 1e-6,
            "{label}: a loaded cell must be polarized, got {gap} V"
        );
        if discharging {
            assert!(
                gap > 0.0,
                "{label}: overpotential is discharge-positive and subtracts from the \
                 terminal, so it must be positive here, got {gap} V \
                 (v_terminal {})",
                tele.v_terminal
            );
        } else {
            assert!(
                gap < 0.0,
                "{label}: charging must lift the terminal above equilibrium, so the \
                 discharge-positive overpotential is negative, got {gap} V \
                 (v_terminal {})",
                tele.v_terminal
            );
        }

        // Relaxation: the concentration gradient decays and the kinetic term vanishes
        // the instant the current does, so the polarization shrinks toward zero
        // without reaching it.
        for _ in 0..600 {
            p.step(1.0, Demand::Rest, &env());
        }
        let relaxed = p.cell(0, 0).expect("cell 0S0P exists").overpotential_v;
        assert!(
            relaxed.abs() < gap.abs() && relaxed.abs() > 0.0,
            "{label}: at rest the polarization must decay toward zero, went {gap} V \
             -> {relaxed} V"
        );
    }
}

/// A cell that has never carried current is unpolarized, on the `Spm` arm as on
/// both ECM arms.
///
/// Exactly zero rather than small: the particles are built uniform, so the surface
/// and bulk stoichiometries are the same number, and at zero current the
/// Butler–Volmer term is `asinh(0)`.
#[test]
fn a_fresh_spm_cell_reports_no_polarization() {
    let p = pack(10, 0.7);
    let at_rest = p.cell(0, 0).expect("cell 0S0P exists").overpotential_v;
    assert_eq!(
        at_rest, 0.0,
        "a cell that has never carried current has no polarization, got {at_rest} V"
    );
}

/// Heat generation is non-negative in **both** current directions.
///
/// This is the energy check that has teeth without being a tautology. The
/// irreversible term is `I·(U_eq − V)`: on discharge both factors are positive, on
/// charge both are negative, and the product is positive either way — *if* the
/// overpotentials have the right sign. Flip one and charging becomes a refrigerator,
/// which is thermodynamically impossible and is the exact error a per-scenario
/// voltage tolerance cannot see. (The entropic term can legitimately make heat
/// negative; this chemistry publishes both entropy coefficients as literally zero,
/// which is why the assertion can be this blunt.)
#[test]
fn a_loaded_cell_generates_heat_in_either_direction() {
    for (label, i) in [("discharge", 6.0), ("charge", -6.0)] {
        let mut p = pack(10, 0.5);
        let mut minimum = f64::INFINITY;
        for _ in 0..300 {
            minimum = minimum.min(p.step(1.0, Demand::Current(i), &env()).q_gen_w);
        }
        assert!(
            minimum > 0.0,
            "{label}: an overpotential dissipates, whichever way the current runs — \
             the least heat over the run was {minimum} W. A negative value here means \
             a Butler-Volmer sign is inverted and the cell is being modelled as a \
             refrigerator."
        );
    }
}

/// Energy in equals energy out around a closed cycle: discharge, charge back to the
/// same state, and the electrical energy the terminals moved plus the heat the cell
/// made comes to zero.
///
/// # Why a closed loop rather than a one-way balance
/// The heat term *is* `I·(U_eq − V)`, so comparing it against `I·U_eq − I·V`
/// step-by-step would restate the definition. Around a loop that returns the state
/// to where it started, the stored chemical energy `∮U_eq·dq` is zero because
/// `U_eq` is a function of state — so the identity `∮V·I·dt + ∮Q·dt = 0` becomes a
/// claim about the *trajectory* rather than about one step's arithmetic, and it fails
/// if the heat term is systematically wrong in either direction.
///
/// The residual is not exact: both the electrical integral and the heat integral are
/// left-hand rectangle rules, so each step leaves an `O(dt²)` term and they do not
/// cancel between the two legs. 0.5 % of the energy that moved is comfortably inside
/// that and comfortably outside anything a wrong heat term could hide in — dropping
/// either overpotential entirely moves this by tens of percent.
#[test]
fn a_closed_cycle_conserves_energy() {
    let mut p = pack(10, 0.6);
    let (dt, i) = (1.0, 5.0);
    let mut electrical = 0.0;
    let mut heat = 0.0;
    let mut moved = 0.0;

    // Start-of-step voltage, from a zero-length probe: the electrical integral has to
    // pair each step's current with the voltage that drove it, which is the previous
    // step's end-of-step reading (see the ECM energy-balance property test).
    let mut v_start = p.step(0.0, Demand::Current(i), &env()).v_terminal;
    let mut leg = |p: &mut Pack, i: f64, steps: usize, v_start: &mut f64| {
        for _ in 0..steps {
            let tele = p.step(dt, Demand::Current(i), &env());
            electrical += *v_start * tele.i_actual * dt;
            heat += tele.q_gen_w * dt;
            moved += (*v_start * tele.i_actual * dt).abs();
            *v_start = tele.v_terminal;
        }
    };
    leg(&mut p, i, 900, &mut v_start);
    leg(&mut p, -i, 900, &mut v_start);

    let residual = electrical + heat;
    assert!(
        residual.abs() < 0.005 * moved,
        "a closed cycle must return the cell to its stored energy: electrical \
         {electrical} J + heat {heat} J = {residual} J, against {moved} J moved"
    );
}

// ---------------------------------------------------------------------------
// Exit criterion 3, first half
// ---------------------------------------------------------------------------

/// Snapshot at t/2, restore, continue — and the two telemetry streams are
/// **bit-identical**.
///
/// # This is the leg `diffsol` failed
/// An adaptive multistep integrator's state is larger than its solution vector, and
/// the one Phase 6 evaluated does not expose the remainder through its public API: a
/// run restored through it diverges by 4.4e-8 relative. A fixed-step method whose
/// entire state *is* the concentration vector passes this trivially, which is the
/// whole reason the integrator in `spm.rs` is owned rather than imported. Compared on
/// bits rather than on a tolerance for exactly that reason — 4.4e-8 would sail
/// through any tolerance worth writing.
#[test]
fn an_spm_pack_survives_a_snapshot_bit_identically() {
    let schedule = |k: usize| match k % 4 {
        0 => Demand::Current(4.0),
        1 => Demand::Rest,
        2 => Demand::Current(-3.0),
        _ => Demand::Voltage(3.9),
    };

    let mut straight = pack(10, 0.55);
    let mut reference = Vec::new();
    for k in 0..400 {
        reference.push(straight.step(1.0, schedule(k), &env()).v_terminal.to_bits());
    }

    let mut split = pack(10, 0.55);
    for k in 0..200 {
        split.step(1.0, schedule(k), &env());
    }
    // Through a real serialization format, not just a clone: the claim is that the
    // *bytes* carry the state, which is what a client saving a run actually does.
    let bytes = bincode::serialize(&split.snapshot()).expect("the snapshot serializes");
    let restored: Snapshot = bincode::deserialize(&bytes).expect("and deserializes");
    let mut continued = Pack::restore(&restored).expect("a same-version snapshot restores");

    for (k, &expected) in reference.iter().enumerate().skip(200) {
        let bits = continued
            .step(1.0, schedule(k), &env())
            .v_terminal
            .to_bits();
        assert_eq!(
            bits,
            expected,
            "step {k} after a restore differs from the uninterrupted run: \
             {} vs {}",
            f64::from_bits(bits),
            f64::from_bits(expected)
        );
    }
}

/// A zero-length step does not move an SPM cell.
///
/// The diffusion solve is a no-op at `dt = 0` by arithmetic — every off-diagonal
/// carries a `dt` factor — but `i_last` is an assignment, not an integral, and it is
/// the one piece of this model's state that would have moved on a probe step. The
/// suite primes energy balances and start-of-step voltages with exactly such probes,
/// so an ungated write here would show up as a trajectory that depends on whether
/// anyone looked at it.
#[test]
fn a_zero_length_step_does_not_mutate_an_spm_cell() {
    let mut p = pack(10, 0.65);
    for _ in 0..50 {
        p.step(1.0, Demand::Current(3.0), &env());
    }
    let before = p.snapshot();
    p.step(0.0, Demand::Current(-9.0), &env());
    p.step(0.0, Demand::Voltage(4.1), &env());
    p.step(0.0, Demand::Rest, &env());
    assert_eq!(
        before,
        p.snapshot(),
        "three zero-length probe steps under three different demands must leave the \
         pack exactly as they found it"
    );
}

// ---------------------------------------------------------------------------
// The analytic golden: what Phase 0 did for the equivalent circuit
// ---------------------------------------------------------------------------

/// Under a constant surface flux a sphere settles into a **quasi-steady parabolic
/// profile** whose surface-to-mean concentration gap has a closed form,
/// `c_mean − c_surf = j·R_p / (5·D)`, and the discretisation reproduces it.
///
/// # Why this test and not the two above it
/// `state_of_charge_after_an_hour_does_not_depend_on_the_step_size` and
/// `refining_the_shell_grid_converges` are both satisfied by a scheme that converges
/// to the **wrong number**. A wrong face conductance, a wrong shell volume, or a
/// half-shell extrapolation off by a factor keeps SOC exact (it is a bulk quantity
/// the scheme conserves whatever the interior arithmetic does), keeps the grid
/// converging, and keeps the open-circuit window right (it is evaluated at zero
/// current, where there is no gradient at all). This is the assertion that pins the
/// interior against something outside the scheme, and it is the direct analogue of
/// what Phase 0's closed-form 1RC discharge did for the equivalent circuit.
///
/// # The profile, and why the run is this long
/// The quasi-steady solution of `∂c/∂t = D·∇²c` with a fixed surface flux is
/// `c(r) = c_mean + (j·R_p/D)·(r²/(2·R_p²) − 3/10)`, so at `r = R_p` the surface
/// sits `j·R_p/(5·D)` **below** the mean for an outgoing flux. It takes ~`R_p²/D` to
/// establish, which is ≈1040 s on the negative electrode and ≈**6800 s** on the
/// positive, whose diffusivity is an order of magnitude smaller. A run of a few
/// hundred seconds would never get there on the positive electrode and would read as
/// a discretisation error rather than as an unconverged transient — so both
/// electrodes run for several of their own time constants, and the flux is scaled to
/// each so the particle stays inside its concentration range.
#[test]
fn a_constant_flux_reaches_the_analytic_quasi_steady_profile() {
    // (label, particle radius [m], diffusivity [m²/s]) — the shipped LG M50
    // electrodes, which differ by an order of magnitude in D and therefore in how
    // long this takes.
    for (label, r_p, d_s) in [
        ("negative", 5.86e-6_f64, 3.3e-14_f64),
        ("positive", 5.22e-6_f64, 4.0e-15_f64),
    ] {
        // Run for four diffusion times, and pick a flux that moves the mean by a
        // usable fraction of the range over that span: dc_mean/dt = −3·j/R_p.
        let t_diffusion = r_p * r_p / d_s;
        let duration_s = 4.0 * t_diffusion;
        let j = 15_000.0 * r_p / (3.0 * duration_s);
        let expected = j * r_p / (5.0 * d_s);

        let gap_at = |n: usize| {
            let mut c = vec![30_000.0_f64; n];
            let dt = duration_s / 4000.0;
            for _ in 0..4000 {
                sim_core::spm::diffuse(&mut c, r_p, d_s, j, dt);
            }
            sim_core::spm::mean_concentration(&c) - sim_core::spm::c_surface(&c, r_p, d_s, j)
        };

        let (g10, g20, g40) = (gap_at(10), gap_at(20), gap_at(40));
        let err = |g: f64| (g - expected).abs() / expected;
        assert!(
            err(g10) < 0.02,
            "{label}: 10 shells should reach the analytic gap j·R_p/(5·D) = \
             {expected} mol/m³, got {g10} ({:.3} % off). This pins the face \
             conductances, the shell volumes, the surface extrapolation and the \
             flux together — a factor error in any one of them lands here.",
            100.0 * err(g10)
        );
        assert!(
            err(g40) < err(g20) && err(g20) < err(g10),
            "{label}: refining must move the gap *towards* the closed form, not \
             merely somewhere: 10 -> {:.4} %, 20 -> {:.4} %, 40 -> {:.4} %",
            100.0 * err(g10),
            100.0 * err(g20),
            100.0 * err(g40)
        );
    }
}

/// The Faraday constant is the exact defined product, to the nearest `f64`.
///
/// Pinned because it was briefly wrong by **one ULP** during this slice — "tidied"
/// from the full product to the commonly quoted `96485.33212331`, which is a
/// different `f64` and moves every SPM trajectory. Every other test in the slice is
/// tolerance-based and all of them stayed green through it. Since 2019 both factors
/// are SI *definitions*, so there is a right answer here and it can simply be
/// asserted.
#[test]
fn faraday_is_the_exact_defined_product() {
    // The exact product is 96485.3321233100184; this is its nearest f64, spelled as
    // the shortest decimal that round-trips to it.
    assert_eq!(
        sim_core::spm::FARADAY_C_PER_MOL.to_bits(),
        96_485.332_123_310_01_f64.to_bits(),
        "the constant is not N_A x e = 6.02214076e23 x 1.602176634e-19 rounded to f64"
    );
    // And the truncation that would look like a harmless tidy-up is a different f64.
    // This is the assertion with teeth: it is the mistake that was actually made, and
    // every tolerance-based test in this slice stayed green through it.
    assert_ne!(
        sim_core::spm::FARADAY_C_PER_MOL.to_bits(),
        96_485.332_123_31_f64.to_bits(),
        "the commonly quoted truncation is one ULP away and must not be what ships"
    );
}

/// The diffusion solve leaves a profile **bit-identically** alone at `dt = 0`, for
/// every profile rather than for the lucky ones.
///
/// # Why this is not covered by the pack-level probe-step test
/// `a_zero_length_step_does_not_mutate_an_spm_cell` exercises one trajectory's worth
/// of concentrations, and it passed for a while on a version of `diffuse` that had no
/// zero-`dt` guard at all. Without one the scheme collapses to a diagonal system and
/// still evaluates `(c·vol)/vol` — and multiplying and dividing an `f64` by the same
/// number is *not* the identity, it lands one ULP off for a good fraction of inputs.
/// Whether a probe step moved the pack therefore depended on which concentrations it
/// happened to find. This sweeps enough profiles that the answer cannot be luck.
#[test]
fn a_zero_length_diffusion_step_is_exactly_the_identity() {
    let (r_p, d_s) = (5.86e-6_f64, 3.3e-14_f64);
    let mut moved = 0;
    for k in 0..500 {
        // Concentrations with full mantissas — the ones a real run produces, and the
        // ones a round trip through a multiply and a divide loses bits on.
        let base = 1_000.0 + 31_000.0 * f64::from(k) / 500.0;
        let profile: Vec<f64> = (0..10)
            .map(|s| base * (1.0 + f64::from(s) / 97.310_527_1))
            .collect();
        for &dt in &[0.0, -0.0, -1.0, f64::NAN] {
            let mut c = profile.clone();
            sim_core::spm::diffuse(&mut c, r_p, d_s, 3.0e-6, dt);
            if c.iter()
                .zip(&profile)
                .any(|(a, b)| a.to_bits() != b.to_bits())
            {
                moved += 1;
            }
        }
    }
    assert_eq!(
        moved, 0,
        "{moved} of 2000 zero-or-negative-length diffusion steps moved the profile; \
         a probe step must be an observation, not an edit"
    );
}

/// **The same flag, a different meaning.** A single-particle cell driven past the top of
/// its window raises `SOC_CLAMPED_HIGH` and rejects **nothing**, because nothing was
/// discarded: the lithium is in the particle and a discharge gets it back out.
///
/// `Telemetry::i_rejected_a` is the equivalent circuit's conservation defect made
/// visible (`docs/plans/energy-hole.md`), and the reason this test exists is that the
/// flag alone does not say which model raised it. A future refactor that made the
/// porous-electrode arms "consistent" by rejecting charge here would be inventing a
/// truncation that this model does not perform — and would burn the invented charge as
/// heat, on a cell that is still holding it.
#[test]
fn a_single_particle_clamps_its_readout_without_rejecting_charge() {
    let mut p = pack(8, 0.98);
    let mut clamped_steps = 0;

    for _ in 0..400 {
        let tele = p.step(1.0, Demand::Current(-20.0), &env());
        if tele.flags.contains(sim_core::EventFlags::SOC_CLAMPED_HIGH) {
            clamped_steps += 1;
        }
        assert_eq!(
            tele.i_rejected_a, 0.0,
            "a single-particle cell keeps the lithium it is pushed; it reported {} A \
             rejected",
            tele.i_rejected_a
        );
    }

    assert!(
        clamped_steps > 0,
        "the run never pushed the readout past its window, so this proves nothing"
    );
}
