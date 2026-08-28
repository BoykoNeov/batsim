//! **Phase 3 exit gate.** Overcharge a pack with no BMS until a cell vents, and watch
//! the fire spread to its neighbour.
//!
//! Every link in that chain is physics the engine already had, driven by nothing but a
//! client asking for a current it should not have asked for:
//!
//! 1. `bms: None`, so the demand passes through unclamped — a supported mode, not a
//!    broken pack (`CLAUDE.md` principle 7).
//! 2. Heating outruns what the pack can shed, so cells climb. Two sources, and on this
//!    fixture the second dominates by a factor of ~44: ohmic dissipation
//!    (`I²·R0 + I·ΣV_rc`, 0.24 W per cell here) and the side-reaction heat of charge
//!    pushed into a cell that is already full (`OCV(1.0)·I`, 10.8 W per cell). The
//!    interior climbs fastest because a cell with four neighbours keeps none of its
//!    convective conductance (`thermal::exposure`), which is the Phase 2 gradient.
//! 3. The hottest cell crosses `t_onset_k` and the exothermic reaction lights.
//! 4. It vents, and conducts enough heat along the ordinary `k_ij` links that a
//!    neighbour ignites too.
//!
//! Nothing is scripted and no fault is injected — unlike slice D's propagation test,
//! which used a soft short as a match. The ignition source here is the abuse itself.
//!
//! # The controls, and why there are two
//!
//! A test that only shows "abusive charging makes cells vent" would not distinguish the
//! reaction from a hot resistor, and one that only shows "a neighbour vented too" would
//! not distinguish propagation from every cell having been cooked in parallel. So:
//!
//! * **`runaway_power_w_at_onset = 0.0`** — identical pack, identical demand, no
//!   reaction. The fixture is tuned so charging heat alone plateaus *above* onset (it
//!   has to, or nothing would ever ignite) and *below* vent: measured 434.66 K at the
//!   centre against a 423.15 K onset and a 453.15 K vent. That arm must never vent any
//!   cell, which makes venting attributable to the reaction rather than to the charger.
//! * **The same abuse with a BMS** — protection clamps the demand to the chemistry's
//!   charge window, and the pack ends up 90 K short of onset instead of on fire. That is
//!   the pedagogical contrast the phase is built around, and it is also what proves the
//!   fixture is not simply unsurvivable by construction.
//!
//!   Worth noting what that arm does *not* do: the contactor never opens. The
//!   external-short scenario in `tests/faults.rs` remains the case where derating
//!   discovers it can do nothing and the contactor is the only move left.
//!
//! # The protected arm's history, because it has moved twice
//!
//! It originally settled 0.8 K above ambient. Closing the energy hole
//! (`docs/plans/energy-hole.md`) made it climb to 333.17 K — the chemistry's `t_max_k`,
//! where the over-temperature rung latched the charge off — and the test was renamed to
//! say so. That was not protection working: it was **chatter**. Over-voltage was a bare
//! comparator, so at the top of charge it ran a two-step limit cycle (admit the full
//! derated 6.91 A, land above `v_max`, derate to zero, the load comes off, the reading
//! falls back under, repeat), and once a refused-charge step cost 73.6 W instead of
//! 1.3 W a ~42 % duty cycle on that walked the pack to its limit.
//!
//! The rungs carry hysteresis now (`docs/plans/protection-chatter.md`), so this arm is
//! back where it started: **298.72 K, 0.57 K above ambient**, stopped by `OV` and never
//! near `OT`. The name is the original one again, and the reason is worth keeping in
//! view — the intermediate name was accurate about the engine and wrong about what the
//! scenario was for.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, SafetyParams, ThermalParams,
};
use sim_core::{
    BalancingConfig, BmsConfig, CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig,
    ProtectionConfig, Scatter, ThermalConfig,
};

/// Pack topology: 9 cells, so exactly one — `(1, 1)` — has four neighbours and
/// therefore no convective conductance of its own. That cell is the one the gradient
/// picks out, and its four neighbours are what "propagation" has to reach.
const SERIES: u16 = 3;
const PARALLEL: u16 = 3;

const AMBIENT_K: f64 = 298.15;
const ONSET_K: f64 = 423.15;
const VENT_K: f64 = 453.15;

/// The chemistry's over-temperature limit \[K\], the rung that stops the protected arm.
/// Shared with [`lfp`]'s `t_max_k` so an edit to the fixture cannot silently decouple the
/// assertion from the limit it is about.
const T_MAX_K: f64 = 333.15;

/// Cell capacity \[Ah\], from the shipped LFP file.
const CAP_AH: f64 = 2.303451;

/// Simulation step \[s\]. Coarse enough that the run is cheap, fine enough that the
/// linear thermal path needs no sub-stepping (the ceiling for these parameters is
/// ≈ 12 s); the reaction, once lit, sub-steps adaptively regardless.
const DT: f64 = 1.0;

/// Neighbour conductance \[W/K\]. Well below the shipped benchmark's 1 W/K on purpose:
/// a strongly-coupled pack is nearly isothermal inside, and this scenario needs a
/// *gradient* — one cell has to reach onset first, with the others far enough behind
/// that "a neighbour followed" is a claim about heat flowing from the first one.
const K_NEIGHBOR_W_PER_K: f64 = 0.1;

fn env() -> Env {
    Env {
        t_ambient: AMBIENT_K,
        t_coolant: None,
    }
}

/// The shipped LFP chemistry, with the reaction amplitude left to the caller so the
/// control arm can zero it.
///
/// provenance: every value copied verbatim from `chemistries/lfp_26650_generic.toml`
/// (see that file for the per-number provenance). `sim-core` performs no file I/O, so
/// a scenario that wants the shipped numbers has to carry them inline; nothing here is
/// an independent physical claim.
fn lfp(runaway_power_w_at_onset: f64) -> ChemistryParams {
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
        spm: None,
        dfn: None,
        safety: Some(SafetyParams {
            t_onset_k: ONSET_K,
            t_vent_k: VENT_K,
            runaway_energy_j: 24.0e3,
            runaway_power_w_at_onset,
            runaway_ea_j_per_mol: 1.0e5,
            // Plating off. This pack is 150 K above the cold end and a runaway
            // scenario has no business also being a plating scenario.
            t_plating_min_k: Some(273.15),
            plating_c_threshold: Some(0.5),
            plating_fade_per_ah: 0.0,
            plating_short_hazard_per_ah: 0.0,
            plating_short_ohms: 0.0,
        }),
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "lfp_26650_generic".into(),
            name: "Generic LFP 26650".into(),
            provenance: "scenario copy of chemistries/lfp_26650_generic.toml".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.00,
            max_charge_c: 1.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: T_MAX_K,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            t_ref_k: None,
            soc: vec![
                0.0000, 0.0025, 0.0050, 0.0075, 0.0100, 0.0125, 0.0150, 0.0175, 0.0200, 0.0300,
                0.0400, 0.0500, 0.1000, 0.1500, 0.2500, 0.3500, 0.4500, 0.5500, 0.6500, 0.7500,
                0.8500, 0.9000, 0.9500, 0.9600, 0.9700, 0.9800, 0.9825, 0.9850, 0.9875, 0.9900,
                0.9925, 0.9950, 0.9975, 1.0000,
            ],
            volts: vec![
                2.0000, 2.0743, 2.1430, 2.2066, 2.2655, 2.3199, 2.3703, 2.4169, 2.4600, 2.6028,
                2.7077, 2.7853, 2.9781, 3.1080, 3.1857, 3.2324, 3.2621, 3.2678, 3.2700, 3.2926,
                3.3132, 3.3142, 3.3164, 3.3193, 3.3274, 3.3502, 3.3607, 3.3743, 3.3920, 3.4150,
                3.4449, 3.4838, 3.5343, 3.6000,
            ],
        },
        r0: R0Table {
            soc: vec![0.0, 0.5, 1.0],
            temp_k: vec![263.15, 298.15, 318.15],
            ohms: vec![
                vec![0.055, 0.022, 0.018],
                vec![0.048, 0.020, 0.016],
                vec![0.050, 0.021, 0.017],
            ],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
    }
}

fn config(bms: Option<BmsConfig>) -> PackConfig {
    PackConfig {
        aging: None,
        bms,
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: K_NEIGHBOR_W_PER_K,
        },
        series: SERIES,
        parallel: PARALLEL,
        // Nearly full, so the abuse is a genuine *over*charge: the pack reaches the
        // clamp within a minute and everything after that is charge with nowhere to go.
        initial_soc: 0.9,
        initial_temp_k: AMBIENT_K,
        seed: 0xB00,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    }
}

/// A conventional BMS: protection on, so this pack refuses the abuse.
fn protective_bms() -> BmsConfig {
    BmsConfig {
        balancing: Some(BalancingConfig {
            bleed_r_ohms: 33.0,
            v_threshold_v: 3.45,
            v_release_band_v: 0.010,
        }),
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.2,
            t_hard_margin_k: 10.0,
            v_release_band_v: 0.08,
            t_release_band_k: 2.0,
        }),
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: vec![(0, 0), (1, 1), (2, 2)],
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.1,
        rest_time_for_ocv_s: 600.0,
        ocv_correction_gain: 0.5,
        min_ocv_slope_v_per_soc: 0.5,
    }
}

/// Charge current \[A\] at the pack terminals, discharge-positive (so this is negative).
///
/// Per cell this is `CHARGE_A / PARALLEL`, and it is chosen so that **heating with the
/// reaction switched off** plateaus between onset and vent — see the module docs on why
/// both bounds matter.
///
/// **Re-derived when the energy hole was closed** (`docs/plans/energy-hole.md`), and the
/// old value is worth recording because the difference is the whole point. This was
/// `-60.0` when a clamped cell's refused charge vanished, so the only heat available was
/// ohmic: `Σ q = h·A·Σ exposure·ΔT` with `R0 + R_rc ≈ 27 mΩ` needed 20 A per cell to put
/// the plateau in the low 430s K. Rejected-charge heat is `OCV(1.0)·I` — 3.6 V per amp,
/// against ohmic's `I·27 mΩ` — so at 20 A/cell the same fixture now runs to 1321 K and
/// vents on the charger alone. **9 A puts the identical plateau back**: measured 434.66 K
/// at the centre, 407.05 K at its neighbours, 385.68 K at the corners.
///
/// That is 3 A/cell, i.e. 1.3 C against a chemistry whose `max_charge_c` is 1.0 — still
/// unambiguously abuse, and a far more plausible one than the 8.7 C this fixture used to
/// need.
const CHARGE_A: f64 = -9.0;

fn temp(pack: &Pack, s: usize, p: usize) -> f64 {
    pack.cell(s, p).expect("cell in range").temp_k
}

fn vented(pack: &Pack, s: usize, p: usize) -> bool {
    pack.cell(s, p).expect("cell in range").vented
}

fn budget_left(pack: &Pack, s: usize, p: usize) -> f64 {
    pack.cell(s, p)
        .expect("cell in range")
        .runaway_energy_remaining_j
}

/// Hottest cell in the pack, as `(s, p, T)`.
fn hottest(pack: &Pack) -> (usize, usize, f64) {
    let mut best = (0, 0, f64::NEG_INFINITY);
    for s in 0..SERIES as usize {
        for p in 0..PARALLEL as usize {
            let t = temp(pack, s, p);
            if t > best.2 {
                best = (s, p, t);
            }
        }
    }
    best
}

/// 4-connected grid neighbours of `(s, p)` — the cells the thermal network gives a
/// direct `k_ij` link, and therefore the only ones runaway can propagate to directly.
fn neighbours(s: usize, p: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if s > 0 {
        out.push((s - 1, p));
    }
    if s + 1 < SERIES as usize {
        out.push((s + 1, p));
    }
    if p > 0 {
        out.push((s, p - 1));
    }
    if p + 1 < PARALLEL as usize {
        out.push((s, p + 1));
    }
    out
}

/// Steps of the abusive charge each live arm takes.
///
/// The probe that re-sized this fixture puts the centre cell's vent at 2921 s, its four
/// neighbours at 3366 s and the corners at 3599 s, so 6000 s covers the whole
/// propagation with room to spare while staying cheap in a debug build.
///
/// Those are within half a percent of the timings the pre-energy-hole fixture had at
/// `-60.0` A (2918 / 3409 / 3652 s), which is the point of the re-derivation: the
/// scenario is the same fire on the same clock, reached with a sixth of the current.
const LIVE_STEPS: usize = 6_000;

/// Steps the control arm takes. Longer than [`LIVE_STEPS`] on purpose: its claim is
/// that ohmic heating *plateaus* below vent, and a plateau is only credible once the
/// pack has stopped moving (the thermal time constant here is â‰ˆ 800 s).
const CONTROL_STEPS: usize = 20_000;

/// Reaction amplitude at onset \[W\], from the shipped LFP file.
const REACTION_W_AT_ONSET: f64 = 5.0;

/// Per-cell exothermic budget \[J\], from the shipped LFP file. A cell that vented with
/// its budget still full was carried there by its neighbours rather than by its own
/// reaction, which is the distinction the propagation test turns on.
const BUDGET_J: f64 = 24.0e3;

/// The centre cell: the only one of the nine with four neighbours, hence the only one
/// with no convective conductance of its own, hence the one that gets hot first.
const CENTRE: (usize, usize) = (1, 1);

/// What one arm of the abuse did.
struct Run {
    /// Cells in the order they vented, with the simulation time \[s\] at which the
    /// engine first reported each one vented.
    vent_order: Vec<((usize, usize), f64)>,
    /// Simulation time \[s\] at which `THERMAL_RUNAWAY` was first raised.
    runaway_at_s: Option<f64>,
    /// Union of every flag raised over the run.
    flags_seen: EventFlags,
    /// The pack at the end, for per-cell ground truth.
    pack: Pack,
}

impl Run {
    fn vented_at(&self, cell: (usize, usize)) -> Option<f64> {
        self.vent_order
            .iter()
            .find(|(c, _)| *c == cell)
            .map(|(_, t)| *t)
    }
}

/// Charge `pack` at [`CHARGE_A`] for `steps`, recording the order cells vent in.
fn overcharge(pack: Pack, steps: usize) -> Run {
    let mut pack = pack;
    let mut vent_order: Vec<((usize, usize), f64)> = Vec::new();
    let mut runaway_at_s = None;
    let mut flags_seen = EventFlags::empty();
    for _ in 0..steps {
        let tele = pack.step(DT, Demand::Current(CHARGE_A), &env());
        flags_seen |= tele.flags;
        if runaway_at_s.is_none() && tele.flags.contains(EventFlags::THERMAL_RUNAWAY) {
            runaway_at_s = Some(pack.sim_time_s());
        }
        if tele.flags.contains(EventFlags::VENTED) {
            for s in 0..SERIES as usize {
                for p in 0..PARALLEL as usize {
                    if vented(&pack, s, p) && !vent_order.iter().any(|(c, _)| *c == (s, p)) {
                        vent_order.push(((s, p), pack.sim_time_s()));
                    }
                }
            }
        }
    }
    Run {
        vent_order,
        runaway_at_s,
        flags_seen,
        pack,
    }
}

/// The pack with no BMS at all, charged far past full.
fn abused() -> Run {
    overcharge(
        Pack::new(&config(None), lfp(REACTION_W_AT_ONSET)).expect("fixture builds"),
        LIVE_STEPS,
    )
}

/// The same pack and the same abuse with the exothermic reaction switched off.
fn control() -> Run {
    overcharge(
        Pack::new(&config(None), lfp(0.0)).expect("fixture builds"),
        CONTROL_STEPS,
    )
}

// --- the chain, link by link ----------------------------------------------

/// **The first half of the phase exit criterion.** A client with no BMS asks for a
/// current the chemistry never permitted, and the pack ends up on fire.
///
/// The ordering assertion is the substance: `THERMAL_RUNAWAY` is raised *before* the
/// first vent, so this is the reaction igniting and then carrying the cell the last
/// 30 K, not a cell cooked to `t_vent_k` by the charger with a flag attached afterwards.
#[test]
fn bms_off_overcharge_reaches_thermal_runaway_and_vents() {
    let run = abused();

    let runaway_at = run
        .runaway_at_s
        .expect("the abuse should have ignited at least one cell");
    let first_vent = *run
        .vent_order
        .first()
        .expect("at least one cell should have vented");
    assert!(
        runaway_at < first_vent.1,
        "the reaction must ignite before anything vents: runaway at {runaway_at} s, \
         first vent at {} s",
        first_vent.1
    );
    assert_eq!(
        first_vent.0, CENTRE,
        "the pack's thermal gradient should pick the centre cell first, got {:?}",
        first_vent.0
    );
    assert!(
        budget_left(&run.pack, CENTRE.0, CENTRE.1) < BUDGET_J,
        "the centre cell vented without consuming any exothermic budget, so it was \
         heated rather than ignited"
    );
    assert!(
        run.flags_seen
            .contains(EventFlags::SOC_CLAMPED_HIGH | EventFlags::THERMAL_RUNAWAY),
        "the run should show both the overcharge and the fire it caused: {:?}",
        run.flags_seen
    );
}

/// **The reaction is what vents the pack, not the charger.** The identical pack under
/// the identical demand with `runaway_power_w_at_onset = 0.0` never vents a single
/// cell, however long it is left.
///
/// The two bracketing assertions are what make this a control rather than a weaker
/// fixture. Ohmic heating alone has to reach *above* onset â€” otherwise nothing would
/// ever ignite and the live run above would be testing a pack that cannot burn â€” and it
/// has to stay *below* vent, or venting would prove nothing about the reaction. The
/// plateau sits between the two, and it is a steady state: the pack settles there
/// three-quarters of the way through the run and stays.
#[test]
fn without_the_reaction_the_same_abuse_never_vents() {
    let run = control();

    assert!(
        run.vent_order.is_empty(),
        "ohmic heating alone vented {:?}",
        run.vent_order
    );
    assert!(
        !run.flags_seen
            .intersects(EventFlags::VENTED | EventFlags::THERMAL_RUNAWAY),
        "a pack with no reaction should raise no safety flags: {:?}",
        run.flags_seen
    );

    let centre = temp(&run.pack, CENTRE.0, CENTRE.1);
    assert!(
        centre > ONSET_K,
        "ohmic heating must reach onset or the live pack could never ignite: centre \
         plateaus at {centre} K, onset {ONSET_K} K"
    );
    assert!(
        centre < VENT_K,
        "ohmic heating must stay below vent or venting proves nothing about the \
         reaction: centre plateaus at {centre} K, vent {VENT_K} K"
    );
}

/// **The second half of the phase exit criterion.** The centre cell's fire reaches its
/// neighbours through nothing but the `k_ij` links the thermal network has had since
/// Phase 2.
///
/// Three things have to hold together for this to be propagation rather than nine cells
/// cooking in parallel:
///
/// * the neighbour vents **after** the centre, not with it;
/// * the neighbour **burned** â€” its exothermic budget went down, so it ran its own
///   reaction rather than being conducted up to `t_vent_k` from outside;
/// * in the control arm the same neighbour never even reaches onset, so the heat that
///   ignited it came from the centre cell's reaction and not from the charger.
#[test]
fn the_fire_spreads_from_the_centre_to_its_neighbours() {
    let live = abused();
    let control = control();

    let centre_vent = live
        .vented_at(CENTRE)
        .expect("the centre cell should have vented");

    let mut followed = Vec::new();
    for n in neighbours(CENTRE.0, CENTRE.1) {
        let Some(t) = live.vented_at(n) else { continue };
        assert!(
            t > centre_vent,
            "neighbour {n:?} vented at {t} s, not after the centre's {centre_vent} s â€” \
             that is simultaneous cooking, not propagation"
        );
        assert!(
            budget_left(&live.pack, n.0, n.1) < BUDGET_J,
            "neighbour {n:?} vented without consuming exothermic budget, so it was \
             heated to the vent temperature rather than ignited"
        );
        let cold = temp(&control.pack, n.0, n.1);
        assert!(
            cold < ONSET_K,
            "neighbour {n:?} reaches {cold} K on the charger alone, so the live run \
             proves nothing about propagation"
        );
        followed.push(n);
    }
    assert!(
        !followed.is_empty(),
        "no neighbour of the centre followed it into runaway; vent order was {:?}",
        live.vent_order
    );
}

/// **The contrast the phase is built around.** The same abusive demand into the same
/// pack, with a conventional BMS in front of it, never gets near the fire: protection
/// clamps the current to the chemistry's charge window, and when the pack warms anyway
/// the over-temperature rung latches the charge off at `t_max_k` — 90 K short of onset.
///
/// This is also what stops the fixture above being unfalsifiable. A pack that burned
/// under every configuration would show only that the scenario was built to burn; this
/// one burns exactly when the protection that exists to prevent it is absent.
///
/// # This assertion has been rewritten twice; see the module docs
/// Closing the energy hole made the protected pack climb to its `t_max_k` and be saved
/// by `OT`, and this test was renamed and re-pointed at that rung. Hysteresis on the
/// over-voltage comparator removed the limit cycle that was doing the heating, so the
/// pack is back to **298.72 K — 0.57 K above the 298.15 K ambient** — and `OT` is never
/// reached at all. The rung named here is `OV`, which is the one that should always have
/// stopped an overcharge, and the temperature bound is written against **ambient**,
/// because "never gets warm" is the actual claim.
///
/// What is deliberately *not* asserted: `SOC_CLAMPED_HIGH`. Whether the pack stops just
/// short of the clamp or just past it moves with the release band (absent at 0.08 V,
/// present at 0.10 V, absent again at 0.15 V), so a claim either way would be pinning a
/// knife edge rather than the physics.
#[test]
fn the_same_abuse_through_a_bms_never_gets_warm() {
    let run = overcharge(
        Pack::new(&config(Some(protective_bms())), lfp(REACTION_W_AT_ONSET))
            .expect("fixture builds"),
        LIVE_STEPS,
    );

    assert!(
        !run.flags_seen
            .intersects(EventFlags::VENTED | EventFlags::THERMAL_RUNAWAY),
        "a protected pack should never approach a safety event: {:?}",
        run.flags_seen
    );
    assert!(
        run.flags_seen.contains(EventFlags::OC),
        "protection should have reported the over-current it clamped: {:?}",
        run.flags_seen
    );
    assert!(
        run.flags_seen.contains(EventFlags::OV),
        "the rung that stops this charge is over-voltage: {:?}",
        run.flags_seen
    );
    assert!(
        !run.flags_seen.contains(EventFlags::OT),
        "with the over-voltage rung holding, the pack never approaches its {T_MAX_K} K \
         temperature limit — reaching it would mean the limit cycle is back: {:?}",
        run.flags_seen
    );
    assert!(
        !run.flags_seen.contains(EventFlags::CONTACTOR_OPEN),
        "derating alone should have been enough; the contactor is for the cases it is \
         not: {:?}",
        run.flags_seen
    );

    // "Never gets warm" is the claim, so the bound is ambient plus a degree rather than
    // a fraction of the distance to onset. Measured: 298.7206 K, i.e. 0.5706 K up.
    let (s, p, hot) = hottest(&run.pack);
    assert!(
        hot < AMBIENT_K + 1.0,
        "the protected pack reached {hot} K at {s}S{p}P; a charge the BMS refuses \
         should leave it within a degree of the {AMBIENT_K} K ambient"
    );
    assert!(
        hot < ONSET_K - 80.0,
        "the protected pack reached {hot} K at {s}S{p}P, far closer to the {ONSET_K} K \
         onset than a protected charge should take it"
    );
}
