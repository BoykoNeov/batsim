//! LTO: a chemistry added with zero lines of engine code, and what it can do that the
//! others cannot.
//!
//! `CLAUDE.md` principle 10 says a chemistry is data, not code. Until
//! `chemistries/lto_20ah_generic.toml` every chemistry in this repo shipped alongside Rust
//! that was being written anyway, so the principle had four non-examples behind it and no
//! example. This file is the behavioural half of the test; the loading half lives beside
//! the other chemistries in `load.rs`, and the reasoning is in
//! `docs/plans/phase-8-slice-a-lto.md`.
//!
//! **Every assertion here is one the shipped NMC file fails.** That is deliberate and it is
//! the point of the file: "terminal voltage falls under load" passes for every shipped
//! chemistry and is therefore evidence about none of them. The four claims below — rate
//! capability, immunity to cold-charge plating, depth-independent cycle life, and freedom
//! from over-discharge damage — are four things LTO is actually bought for, and each is
//! measured against NMC as a control arm rather than asserted alone. A fifth test guards the
//! voltage window itself, which is not a claim about the cell so much as a tripwire against
//! a future edit quietly turning this file back into a lithium cell of the usual shape.
//!
//! Run the tables with `cargo test -p sim-data --test lto_chemistry -- --nocapture`.

use sim_core::{
    aging::cycle_increment, AgingConfig, CellModelConfig, ChemistryParams, Demand, Env, EventFlags,
    Pack, PackConfig, Scatter, ThermalConfig,
};

fn lto() -> ChemistryParams {
    let text = include_str!("../../../chemistries/lto_20ah_generic.toml");
    sim_data::parse_chemistry(text).expect("LTO chemistry loads and validates")
}

/// The control arm throughout: a graphite/NMC cell, the chemistry LTO is usually compared
/// against and the one every claim below is measured relative to.
fn nmc() -> ChemistryParams {
    let text = include_str!("../../../chemistries/nmc_18650_generic.toml");
    sim_data::parse_chemistry(text).expect("NMC chemistry loads and validates")
}

/// A bare cell at `temp_k`: no BMS (so nothing derates or inhibits the demand — the
/// question is what the *physics* does, not what a protection policy allows), no aging (so
/// capacity does not move mid-run), and **isothermal**, so a rate comparison is not read off
/// a curve that is partly self-heating.
fn config(temp_k: f64, initial_soc: f64) -> PackConfig {
    PackConfig {
        aging: None,
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k: temp_k,
        seed: 0,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        cell_model: CellModelConfig::Ecm,
    }
}

fn env(temp_k: f64) -> Env {
    Env {
        t_ambient: temp_k,
        t_coolant: None,
    }
}

/// Amp-hours delivered discharging at `c_rate` until the terminal falls below `v_min`,
/// counting only steps that finished above the cutoff.
///
/// The conservative end of the bracket `lead_acid_rate.rs` documents: the step that carries
/// the terminal below `v_min` has already moved charge, but a real cutoff happens partway
/// through it, so this figure is short of the truth by at most one step's charge. Both arms
/// are measured the same way at the same `dt`, and every claim below is a *ratio* between
/// two runs of the same cell, so the bias cancels where it matters.
fn delivered_ah(chem: &ChemistryParams, c_rate: f64, dt: f64) -> f64 {
    let i = c_rate * chem.cell.capacity_ah;
    let v_min = chem.cell.v_min;
    let mut pack = Pack::new(&config(298.15, 1.0), chem.clone()).expect("pack builds");
    let mut ah = 0.0;
    // Two hours of headroom: the slowest run here is 1C, which is one hour.
    let max_steps = (2.0 * 3600.0 / dt) as u64;
    for _ in 0..max_steps {
        let t = pack.step(dt, Demand::Current(i), &env(298.15));
        if t.v_terminal < v_min {
            return ah;
        }
        ah += t.i_actual * dt / 3600.0;
    }
    panic!("discharge at {c_rate}C never reached the cutoff");
}

/// **The rate claim.** LTO keeps essentially all of its capacity at 10C; the graphite cell
/// loses most of its own.
///
/// This is the sharpest thing a parameter file alone can say about this chemistry, and it
/// falls out of two numbers that are not free: sub-milliohm resistances anchored on the
/// datasheet's 1 kHz impedance and cross-checked against its 10-second power ratings, and a
/// 1.5 V floor a long way below the loaded terminal voltage.
///
/// Both arms are driven at 10× *their own* capacity, so this is not a comparison of two
/// cells at the same amps — it is each cell asked for the same multiple of itself, which is
/// what a C-rate means and what makes the two figures comparable at all.
#[test]
fn ten_c_costs_lto_almost_nothing_and_nmc_most_of_its_capacity() {
    // 0.2 s resolves the fast RC pair (τ = 6 s on the LTO cell) many times over, and a 10C
    // discharge is six minutes, so the cutoff step is a small fraction of the total.
    const DT: f64 = 0.2;

    let rows: Vec<(&str, f64, f64)> = [("LTO 20 Ah", lto()), ("NMC 18650", nmc())]
        .into_iter()
        .map(|(name, chem)| {
            let one_c = delivered_ah(&chem, 1.0, DT);
            let ten_c = delivered_ah(&chem, 10.0, DT);
            (name, one_c, ten_c)
        })
        .collect();

    println!("\n10C capacity retention (isothermal 25 degC, no BMS)");
    println!(
        "{:<12} {:>10} {:>10} {:>10}",
        "cell", "1C [Ah]", "10C [Ah]", "retained"
    );
    for &(name, one_c, ten_c) in &rows {
        println!(
            "{name:<12} {one_c:>10.3} {ten_c:>10.3} {:>9.1}%",
            100.0 * ten_c / one_c
        );
    }

    let (_, lto_1c, lto_10c) = rows[0];
    let (_, nmc_1c, nmc_10c) = rows[1];
    let lto_retained = lto_10c / lto_1c;
    let nmc_retained = nmc_10c / nmc_1c;

    assert!(
        lto_retained >= 0.95,
        "LTO should keep >=95% of its capacity at 10C, kept {:.1}%",
        100.0 * lto_retained
    );
    assert!(
        nmc_retained <= 0.50,
        "the NMC control arm should lose more than half its capacity at 10C, kept {:.1}%",
        100.0 * nmc_retained
    );
}

/// **The plating claim, which is the pedagogical contrast this chemistry exists for.**
///
/// The same cold, fast charge: 4C at −30 °C, from a fifth full. The graphite cell plates
/// metallic lithium and says so; the LTO cell does not, because its anode plateau sits
/// ~1.55 V above the potential at which lithium deposits. Both outcomes come from
/// parameters — no engine code distinguishes the two cells.
///
/// Note what is *not* asserted: that the LTO file's `plating_fade_per_ah` is zero. A cell
/// whose plating flag never rises has no plating cost to check, and the difference between
/// "free" and "impossible" is exactly the distinction this test exists to draw.
///
/// **The LTO arm passes for a different reason than it used to, and that is why the test
/// below it exists.** Until `SNAPSHOT_VERSION` 19 the file carried a one-kelvin sentinel
/// and the silence here meant "the gate is shut". The gate is now *absent*, and the silence
/// means "there is no gate" — the same green from a different cause, which is exactly the
/// shape that stops a test discriminating. `the_lto_silence_is_caused_by_the_absent_gate`
/// splices a graphite gate into these same parameters and shows the flag rise, so the
/// absence is established by subtraction rather than by this test's word.
#[test]
fn cold_fast_charge_plates_the_nmc_cell_and_not_the_lto_cell() {
    // −30 °C: the bottom of the LTO cell's rated operating range, and the bottom row of its
    // R0 grid, so this is interpolated rather than clamped off the end of the table.
    const COLD_K: f64 = 243.15;
    const C_RATE: f64 = 4.0;
    const DT: f64 = 1.0;
    const STEPS: usize = 300;

    let mut seen: Vec<(&str, bool)> = Vec::new();
    for (name, chem) in [("LTO 20 Ah", lto()), ("NMC 18650", nmc())] {
        // Charging is negative current, per the discharge-positive convention.
        let i = -C_RATE * chem.cell.capacity_ah;
        let mut pack = Pack::new(&config(COLD_K, 0.2), chem).expect("pack builds");
        let mut plated = false;
        for _ in 0..STEPS {
            let t = pack.step(DT, Demand::Current(i), &env(COLD_K));
            plated |= t.flags.contains(EventFlags::PLATING_RISK);
        }
        seen.push((name, plated));
    }

    println!("\n4C charge at -30 degC, {STEPS} steps");
    for &(name, plated) in &seen {
        println!("{name:<12} PLATING_RISK raised: {plated}");
    }

    assert!(
        !seen[0].1,
        "the LTO cell must never raise PLATING_RISK: its anode cannot plate lithium"
    );
    assert!(
        seen[1].1,
        "the NMC control arm must raise PLATING_RISK, or this test is not measuring the \
         contrast it claims to"
    );
}

/// **The control arm for the test above: the silence is caused by the absent gate, and by
/// nothing else.**
///
/// `cold_fast_charge_plates_the_nmc_cell_and_not_the_lto_cell` asserts that the LTO cell
/// stays quiet under a cold, fast charge. On its own that green has several possible
/// causes — the current might not be negative, the C-rate might land under a threshold, the
/// cell might never get cold — and after `SNAPSHOT_VERSION` 19 it has a *new* one: the
/// chemistry has no plating gate at all. This test removes the ambiguity the way the repo
/// removes it elsewhere, by subtraction: take the very same parameters and the very same
/// demand, splice a **graphite** gate in, and watch the flag rise.
///
/// What it therefore pins is a causal claim rather than an outcome: the LTO cell's silence
/// is the missing gate, not a demand that was never cold or hard enough to plate anything.
/// If some future edit made this trajectory unable to plate for an unrelated reason, this
/// test reddens and the one above it would not.
#[test]
fn the_lto_silence_is_caused_by_the_absent_gate() {
    // Identical to the test above, so the only difference between the two runs is the gate.
    const COLD_K: f64 = 243.15;
    const C_RATE: f64 = 4.0;
    const DT: f64 = 1.0;
    const STEPS: usize = 300;

    let mut spliced = lto();
    let safety = spliced
        .safety
        .as_mut()
        .expect("the LTO file ships a [safety] section for the runaway half");
    assert!(
        safety.t_plating_min_k.is_none() && safety.plating_c_threshold.is_none(),
        "this test splices a gate into a file that has none; if the shipped file has \
         grown one, the subtraction below measures nothing"
    );
    // The graphite gate, taken from the NMC file rather than invented here: 0 degC and
    // 0.4 C. Nothing else about the cell is touched.
    let graphite = nmc().safety.expect("the NMC file ships a [safety] section");
    safety.t_plating_min_k = graphite.t_plating_min_k;
    safety.plating_c_threshold = graphite.plating_c_threshold;

    let i = -C_RATE * spliced.cell.capacity_ah;
    let mut pack = Pack::new(&config(COLD_K, 0.2), spliced).expect("pack builds");
    let mut plated = false;
    for _ in 0..STEPS {
        let t = pack.step(DT, Demand::Current(i), &env(COLD_K));
        plated |= t.flags.contains(EventFlags::PLATING_RISK);
    }

    println!(
        "
4C charge at -30 degC on LTO parameters WITH a graphite plating gate"
    );
    println!("PLATING_RISK raised: {plated}");
    assert!(
        plated,
        "with a graphite gate spliced in, these parameters under this demand must \
         plate — otherwise the quiet LTO arm next door is quiet for some reason other \
         than the missing gate, and that test is measuring nothing"
    );
}

/// **The cycle-life claim, and a branch that waited for a chemistry nobody had written.**
///
/// `aging::cycle_increment` weights each amp-hour of throughput by `dod^(exp − 1)`, and
/// documents `exp = 1` as degenerating to pure throughput counting with the weight exactly
/// `1.0` *including at `dod = 0`*. Every chemistry shipped before this one sits above 1
/// (LFP 1.1, NMC 1.2, lead-acid 1.3), so that branch had never been reached by any shipped
/// parameter set. It is reached now, by data alone.
///
/// The claim it encodes is real rather than convenient: this cell's 20,000-cycle rating is
/// quoted at full depth with no shallow-cycle bonus, and LTO's zero-strain lattice is the
/// usual explanation for why depth barely matters to it.
#[test]
fn lto_is_the_first_chemistry_to_count_pure_throughput() {
    let lto_aging = lto()
        .aging
        .expect("the LTO file carries an [aging] section");
    let nmc_aging = nmc()
        .aging
        .expect("the NMC file carries an [aging] section");

    assert!(
        (lto_aging.cyc_dod_stress_exp - 1.0).abs() < 1e-12,
        "LTO counts pure throughput, got exp = {}",
        lto_aging.cyc_dod_stress_exp
    );
    assert!(
        nmc_aging.cyc_dod_stress_exp > 1.0,
        "the control arm must be depth-weighted, got exp = {}",
        nmc_aging.cyc_dod_stress_exp
    );

    // The defining property of the branch, at the one point where the two weightings
    // disagree completely: a micro-cycle of zero depth. LTO bills it in full; every
    // depth-weighted chemistry bills it at nothing.
    let ah = 1.0;
    assert!(
        (cycle_increment(&lto_aging, ah, 0.0) - lto_aging.cyc_fade_per_ah * ah).abs() < 1e-24,
        "at dod = 0 the pure-throughput weight must be exactly 1.0"
    );
    assert_eq!(
        cycle_increment(&nmc_aging, ah, 0.0),
        0.0,
        "at dod = 0 a depth-weighted chemistry bills nothing"
    );
}

/// **The window itself, which is what makes this file a strain test rather than a recolour.**
///
/// A fully charged LTO cell sits below a *fully discharged* NMC cell. Nothing else shipped
/// here overlaps like that except the lead-acid file, and that one needed a code change to
/// behave. Asserted rather than left as a comment because it is the cheapest possible check
/// that a future edit has not quietly turned this file back into a lithium cell of the usual
/// shape.
#[test]
fn the_whole_lto_window_sits_below_the_nmc_discharge_cutoff() {
    let lto = lto();
    let nmc = nmc();
    assert!(
        lto.cell.v_max < nmc.cell.v_min,
        "LTO v_max {} should be below NMC v_min {}",
        lto.cell.v_max,
        nmc.cell.v_min
    );
    // And the charge-inhibit limits genuinely differ, which no earlier pair of shipped
    // files does: this cell charges to −30 °C where a graphite cell is held at 0 °C.
    assert!(
        lto.cell.t_charge_min_k < nmc.cell.t_charge_min_k,
        "LTO should charge colder than NMC"
    );
}

/// **The over-discharge claim — the one with a literature number behind it.**
///
/// Two runs per cell, and the answer is the difference between them. The control run
/// discharges at 1C for exactly an hour, which empties the cell precisely (coulomb counting
/// moves SOC by `1/3600` per second at 1C). The test run goes 72 seconds further: 2 % of the
/// cell's own capacity delivered *past empty*, the same excursion for both arms in units of
/// the cell they are taken from. Subtracting the control leaves the over-discharge damage
/// alone.
///
/// **The subtraction is not decoration, and the first version of this test did without it
/// and was wrong.** Asserting that the LTO cell simply loses nothing fails: it loses 0.038 %
/// over the hour, and every bit of that is ordinary calendar fade. Calendar loss goes as
/// `√t`, so the first hour of a cell's life is worth 1/93rd of its first year rather than
/// 1/8766th — the curve is steepest where nothing has happened yet. A control arm makes the
/// attribution a subtraction and the shelf time cancels.
///
/// What the difference then measures: the graphite cell's `[reversal] fade_per_ah` is sized
/// against dissolution of the anode's copper current collector, so a full reversal costs it
/// about 1 % of capacity permanently. The LTO cell pays nothing, because its anode sits above
/// aluminium's lithiation potential and both its electrodes are aluminium foil — there is no
/// copper to go into solution. That is the strongest-sourced claim in the parameter file (the
/// comparative in the over-discharge literature is ~84 % capacity loss for graphite-on-copper
/// against little or none for LTO-on-aluminium), and it is what this test keeps honest.
#[test]
fn over_discharge_scars_the_nmc_cell_and_costs_the_lto_cell_nothing() {
    const DT: f64 = 1.0;
    const TO_EMPTY_S: usize = 3600;
    const PAST_EMPTY_S: usize = 72;

    /// Capacity lost, and charge actually delivered, after `seconds` of 1C discharge with
    /// aging live.
    fn run_for(chem: &ChemistryParams, seconds: usize) -> (f64, f64) {
        let i = chem.cell.capacity_ah; // 1C, discharge-positive
        let cfg = PackConfig {
            aging: Some(AgingConfig {
                sub_clock_period_s: 10.0,
            }),
            ..config(298.15, 1.0)
        };
        let mut pack = Pack::new(&cfg, chem.clone()).expect("pack builds");
        let mut tele = pack.step(0.0, Demand::Rest, &env(298.15));
        let mut ah = 0.0;
        for _ in 0..seconds {
            tele = pack.step(DT, Demand::Current(i), &env(298.15));
            ah += tele.i_actual * DT / 3600.0;
        }
        (1.0 - tele.soh_capacity, ah)
    }

    let mut rows: Vec<(&str, f64, f64, f64, f64)> = Vec::new();
    for (name, chem) in [("LTO 20 Ah", lto()), ("NMC 18650", nmc())] {
        let (control, ah_control) = run_for(&chem, TO_EMPTY_S);
        let (past, ah_past) = run_for(&chem, TO_EMPTY_S + PAST_EMPTY_S);
        // What the extra 72 seconds were *asked* to deliver, in this cell's own amp-hours.
        let demanded = chem.cell.capacity_ah * PAST_EMPTY_S as f64 / 3600.0;
        rows.push((
            name,
            control,
            past - control,
            ah_past - ah_control,
            demanded,
        ));
    }

    println!(
        "
2 % of capacity delivered past empty, at 1C"
    );
    println!(
        "{:<12} {:>18} {:>18}",
        "cell", "to empty [%]", "past empty [%]"
    );
    for &(name, control, delta, ..) in &rows {
        println!(
            "{name:<12} {:>17.4} {:>18.4}",
            control * 100.0,
            delta * 100.0
        );
    }

    let lto_delta = rows[0].2;
    let nmc_delta = rows[1].2;
    assert!(
        lto_delta < 1.0e-5,
        "over-discharge must cost the LTO cell essentially nothing beyond the shelf time it shares with the control, cost it {:.5} %",
        lto_delta * 100.0
    );
    assert!(
        nmc_delta > 5.0e-3,
        "the graphite control arm must be scarred by the same excursion, lost only {:.5} %",
        nmc_delta * 100.0
    );
    // **Both bounds above are satisfied by a cell that simply refuses to discharge past
    // empty**, which would be a different bug wearing this test's green. So each arm must
    // also have *delivered* the charge it was asked for: the extra 72 seconds moved 2 % of
    // the cell's own capacity out of it, damage or no damage.
    //
    // A ratio between the two fade figures — which is what this check was at first — cannot
    // do that job, and is anti-correlated with it: a refusing cell drives its own fade to
    // zero and the ratio to infinity, so the check would pass hardest exactly where it
    // should fail. It was also implied by the two bounds above and could not fail on its
    // own. No perturbation of the chemistry file can expose an assertion its siblings
    // already imply, which is why this one had to be found by reading.
    for &(name, _, _, delivered, demanded) in &rows {
        assert!(
            (delivered - demanded).abs() < 0.01 * demanded,
            "{name}: the past-empty leg delivered {delivered:.4} Ah against {demanded:.4} Ah demanded — the cell did not carry the excursion, so the fade figures above are not measuring over-discharge at all"
        );
    }
}

/// **The id a client asks for has to be the name of the file on disk.**
///
/// `sim-server` resolves a scenario's `chemistry = "..."` key by joining the id onto the
/// chemistry directory as `{id}.toml`, after validating it against `[a-z0-9_]+`. A file whose
/// `[meta] id` disagrees with its own filename, or whose id carries a character that charset
/// rejects, parses and validates perfectly and is then unreachable by every client.
///
/// Checked rather than assumed, because "reachable through the existing mechanism" is a claim
/// this repo has been wrong about before — a feature has landed reachable by no client at
/// all. This does not exercise the server; it pins the one property the server's lookup
/// depends on.
#[test]
fn the_lto_id_is_the_name_of_its_own_file() {
    let id = lto().meta.id;
    assert_eq!(
        id, "lto_20ah_generic",
        "the id must match chemistries/lto_20ah_generic.toml"
    );
    assert!(
        id.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
        "the id must satisfy the [a-z0-9_]+ charset the server validates before the path join"
    );
}
