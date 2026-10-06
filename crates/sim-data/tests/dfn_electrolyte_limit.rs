//! Where the Doyle–Fuller–Newman cell stops being able to answer, and the flag that says so
//! — `docs/plans/dfn-electrolyte-limit.md`, against the **shipped** LG M50.
//!
//! Driven at 3 C and above the positive electrode is pinched: its particles next to the
//! separator fill to their surface, while deeper in, beside particles with room, the
//! electrolyte has run out. Past that the model has no physical answer — the solve either
//! fails or converges on a particle surface past full, handed lithium it cannot hold.
//! [`EventFlags::SURFACE_OUT_OF_RANGE`] reports the second, and changes no number.
//!
//! A solve that refuses the over-full answer was built and measured instead (the note): it
//! finds nothing past the limit, and loses the current's lithium from the cell's books.

use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig, Scatter,
    ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// The shipped cell's nameplate \[Ah\].
const Q_AH: f64 = 5.153198;

/// The chemistry's discharge cut-off \[V\].
const V_MIN: f64 = 2.5;

fn chem() -> ChemistryParams {
    sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses")
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A 1S1P isothermal unprotected `Dfn` at 10/5/10 — `cc_discharge_3c_dfn.toml`'s pack, with
/// the shell count as a knob.
fn pack(soc: f64, shells: usize) -> Pack {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: soc,
        initial_temp_k: 298.15,
        seed: 0,
        scatter: Scatter {
            capacity_sigma: 0.0,
            r0_sigma: 0.0,
        },
        cell_model: CellModelConfig::Dfn {
            shells,
            nodes_negative: 10,
            nodes_separator: 5,
            nodes_positive: 10,
        },
    };
    Pack::new(&config, chem()).expect("builds")
}

/// The 3 C scenario at its own 2 s step: the cut-off step is the first flagged, at the
/// voltage guided-path step `the-electrolyte-starves` quotes, and no step before it is.
///
/// That step **converged**: [`EventFlags::SOLVE_UNCONVERGED`] first arrives two steps later,
/// so before this flag nothing said the 2.42 V was read off a surface past full.
#[test]
fn the_3c_cut_off_step_is_the_first_one_flagged() {
    let mut p = pack(1.0, 20);
    let i = 3.0 * Q_AH;
    let mut t = 0.0;
    loop {
        t += 2.0;
        let tele = p.step(2.0, Demand::Current(i), &env());
        let flagged = tele.flags.contains(EventFlags::SURFACE_OUT_OF_RANGE);
        if t < 464.0 {
            assert!(
                !flagged,
                "flagged at {t} s, before the cut-off: V = {}",
                tele.v_terminal
            );
            assert!(tele.v_terminal > V_MIN, "crossed early, at {t} s");
            continue;
        }
        assert!(
            flagged,
            "the cut-off step must be flagged: {:?}",
            tele.flags
        );
        assert!(
            !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
            "and it converged — which is why this flag is the only report"
        );
        assert!(tele.v_terminal < V_MIN, "464 s is the cut-off step");
        assert!(
            (tele.v_terminal - 2.421753).abs() < 5.0e-7,
            "the flag changes no number: V = {}",
            tele.v_terminal
        );
        break;
    }
    // And it stays up: the model does not come back from the pinch under the same current.
    for _ in 0..10 {
        let tele = p.step(2.0, Demand::Current(i), &env());
        assert!(tele.flags.contains(EventFlags::SURFACE_OUT_OF_RANGE));
    }
}

/// At 1 C the cell reaches its cut-off with its electrolyte intact and every surface inside
/// its range, and a minute past it the flag has still not been raised.
#[test]
fn a_1c_discharge_through_its_cut_off_never_raises_it() {
    let mut p = pack(1.0, 10);
    let mut cut = None;
    let mut t = 0.0;
    while cut.is_none_or(|c| t < c + 60.0) {
        t += 10.0;
        let tele = p.step(10.0, Demand::Current(Q_AH), &env());
        assert!(
            !tele.flags.contains(EventFlags::SURFACE_OUT_OF_RANGE),
            "flagged at {t} s at 1 C"
        );
        if cut.is_none() && tele.v_terminal < V_MIN {
            cut = Some(t);
        }
        assert!(t < 4000.0, "never reached the cut-off");
    }
}

/// The other end of the range: a charge driven on past the 4.2 V ceiling fills a surface
/// past full too, and how long that takes is set by the rate. From half charge at 10 s
/// steps: 3 C crosses the ceiling at 30 s and is flagged from 250 s; 1 C crosses at 670 s and
/// is flagged from 2090 s, nearly twenty-four minutes later.
#[test]
fn a_charge_driven_on_past_full_raises_it_sooner_the_faster_it_is() {
    let first_flagged = |c_rate: f64, steps: usize| {
        let mut p = pack(0.5, 10);
        (1..=steps).find_map(|k| {
            p.step(10.0, Demand::Current(-c_rate * Q_AH), &env())
                .flags
                .contains(EventFlags::SURFACE_OUT_OF_RANGE)
                .then_some(10.0 * k as f64)
        })
    };
    let fast = first_flagged(3.0, 30).expect("3 C is flagged within 300 s");
    let slow = first_flagged(1.0, 220).expect("1 C is flagged within 2200 s");
    assert!(
        fast <= 300.0 && slow >= 2000.0,
        "3 C at {fast} s, 1 C at {slow} s"
    );
}
