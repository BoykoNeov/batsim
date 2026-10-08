//! The many-particle LFP lesson's scenario files, run the way the guided path runs them
//! (Phase 9 slice D, `docs/plans/phase-9-slice-d-lesson.md`).
//!
//! `path_claims.rs` checks every number the lesson's prose prints against the engine. What
//! it cannot check is a sentence with no number in it — and the lesson's central ones are
//! of that kind: that the particles are either nearly empty or most of the way full and not
//! in between, that on the room-temperature charge they switch one at a time, that the
//! circuit cannot tell the two directions apart, and that a second draw in the cold moves
//! the discharge arm and leaves the charge arm where it was. Each is pinned here, on the
//! shipped files rather than on a fixture, so a change to a file, the chemistry or the cell
//! that falsifies a sentence reddens a named test.
//!
//! The page's shape: the Pulse mode at C/20 (`0.11517255 A`, signed by direction) on for
//! 36000 s and off after, `dt = 10 s`, every reading on a zero-length probe at its mark.
//! The numbers were measured out of tree first (`W:/temp/claude/phase9d/probe`).

use sim_core::{Demand, Env, EventFlags, Pack};
use sim_data::{parse_chemistry, parse_scenario};

const CHEM: &str = include_str!("../../../chemistries/lfp_26650_prada2013.toml");

/// C/20 on this cell, discharge-positive: what the lesson's pulse box says.
const C20_A: f64 = 0.115_172_55;
const DT: f64 = 10.0;
const ON_S: f64 = 36_000.0;
/// The lessons' marks: the first step's, the end of the current, the end of the rest.
const MARKS: [f64; 3] = [25_200.0, ON_S, 43_200.0];

fn file(name: &str) -> String {
    let path = format!("{}/../../scenarios/{name}.toml", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// What the run read at each of [`MARKS`].
struct Reading {
    v: f64,
    soc: f64,
    particles: Option<Vec<f64>>,
}

impl Reading {
    fn full(&self) -> usize {
        self.particles
            .as_ref()
            .expect("a many-particle cell")
            .iter()
            .filter(|&&y| y > 0.5)
            .count()
    }
}

struct Run {
    at: Vec<Reading>,
    /// The most positive particles ever caught between 0.3 and 0.7 of full at once, after
    /// the first hour.
    most_switching_after_an_hour: usize,
}

/// One scenario file under the page's pulse, `sign` `+1` discharging and `-1` charging.
fn run(name: &str, sign: f64) -> Run {
    let sc = parse_scenario(&file(name)).expect("parses");
    let chem = parse_chemistry(CHEM).expect("chemistry parses");
    let mut pack: Pack = sc.build_pack(chem).expect("builds");
    let env = Env {
        t_ambient: sc.pack.initial_temp_k,
        t_coolant: None,
    };
    let demand = |t: f64| {
        if t < ON_S - 1e-9 {
            Demand::Current(sign * C20_A)
        } else {
            Demand::Rest
        }
    };
    let mut at = Vec::new();
    let mut most = 0;
    for mark in MARKS {
        while pack.sim_time_s() < mark - 1e-9 {
            let tel = pack.step(DT, demand(pack.sim_time_s()), &env);
            assert!(
                !tel.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "{name}: unsolved at {} s",
                pack.sim_time_s()
            );
            if let (true, Some(ps)) = (pack.sim_time_s() > 3600.0, pack.positive_particles(0, 0)) {
                most = most.max(ps.iter().filter(|&&y| y > 0.3 && y < 0.7).count());
            }
        }
        let tel = pack.step(0.0, demand(pack.sim_time_s()), &env);
        at.push(Reading {
            v: tel.v_terminal,
            soc: tel.soc_true,
            particles: pack.positive_particles(0, 0),
        });
    }
    Run {
        at,
        most_switching_after_an_hour: most,
    }
}

/// The first step's picture: seven hours into the charge from empty the cell is at 35 %, and
/// no particle is anywhere near 35 % of anything — eleven are most of the way full, nine are
/// nearly empty, none is between. And after the first hour of the charge no two particles are
/// ever caught halfway across at once.
///
/// **"Halfway across" is the window 0.3–0.7 of full, and the claim depends on it.** Measured
/// after the first hour: at most one particle in 0.3–0.7 at any step; up to four in 0.25–0.75
/// (34 900 s) and seventeen inside the whole unstable region, about 0.216–0.784 at 298 K
/// (3 610 s). That is because the FULL particles drift down together — 0.83 to about 0.73 —
/// into the edge of that region before each tips over, and the tipping, from about 0.7 to
/// 0.044, is what goes in turn. The prose says exactly that: the full ones give up a little
/// lithium together, then tip over in turn. Five cross at once near empty, at 270 s, which the
/// lesson does not claim; on the discharge and in the cold two or three cross together in
/// 0.3–0.7, which is why only this run's prose says it.
#[test]
fn halfway_through_the_charge_no_particle_is_halfway() {
    let r = run("lfp_particles_charged", -1.0);
    let mid = &r.at[0];
    assert!((mid.soc - 0.35).abs() < 1e-9, "soc {}", mid.soc);
    assert_eq!(mid.full(), 11);
    for y in mid.particles.as_ref().unwrap() {
        assert!(
            *y < 0.1 || *y > 0.7,
            "a particle at {y} of full: the lesson says none sits between"
        );
    }
    assert_eq!(
        r.most_switching_after_an_hour, 1,
        "after the first hour this charge switches its particles one at a time"
    );
    // And it arrives where the rest-gap lesson reads it.
    assert_eq!(r.at[2].full(), 8);
    assert!((r.at[2].v - 3.294_500).abs() < 1e-6, "{}", r.at[2].v);
}

/// The second step's contrast and its control: the many-particle pair rests 19.64 mV apart
/// with 8 and 4 particles full, and the equivalent circuit on the same file, seed and road
/// rests at the same voltage from both sides — so the gap is the particles'.
#[test]
fn the_direction_shows_on_the_particles_and_not_on_the_circuit() {
    let up = run("lfp_particles_charged", -1.0);
    let down = run("lfp_particles_discharged", 1.0);
    assert_eq!((up.at[2].full(), down.at[2].full()), (8, 4));
    let gap = up.at[2].v - down.at[2].v;
    assert!((gap - 0.019_638).abs() < 2e-6, "gap {gap} V");

    let c_up = run("lfp_circuit_charged", -1.0);
    let c_down = run("lfp_circuit_discharged", 1.0);
    assert!(c_up.at[2].particles.is_none(), "a circuit has no particles");
    assert!(
        (c_up.at[2].v - c_down.at[2].v).abs() < 1e-9,
        "the circuit remembers nothing: {} against {}",
        c_up.at[2].v,
        c_down.at[2].v
    );
    // The [ocv] table's own value at 0.5, relaxed.
    assert!((c_up.at[2].v - 3.264_95).abs() < 1e-6, "{}", c_up.at[2].v);
}

/// The cold step: the seed-9 pair is 27.44 mV apart, and the seed-3 draw moves only the
/// discharge arm — the charge arm leaves the same 8 particles full and rests on the same
/// voltage — while two of the seed-3 discharge arm's six full particles switch during the
/// rest, after the current has stopped.
#[test]
fn another_draw_in_the_cold_moves_only_the_discharge_arm() {
    let up = run("lfp_particles_cold_charged", -1.0);
    let down = run("lfp_particles_cold_discharged", 1.0);
    let up3 = run("lfp_particles_cold_charged_seed3", -1.0);
    let down3 = run("lfp_particles_cold_discharged_seed3", 1.0);

    assert_eq!((up.at[2].full(), down.at[2].full()), (8, 5));
    assert!((up.at[2].v - down.at[2].v - 0.027_444).abs() < 2e-6);

    assert_eq!(up3.at[2].full(), 8);
    assert!(
        // Measured 0.13 µV apart: the radii differ, so not to the bit.
        (up3.at[2].v - up.at[2].v).abs() < 1e-6,
        "the seed-3 charge arm rests where seed 9's does: {} against {}",
        up3.at[2].v,
        up.at[2].v
    );
    assert_eq!(
        (down3.at[1].full(), down3.at[2].full()),
        (4, 6),
        "on seed 3 two particles switch during the rest"
    );
    assert!((up3.at[2].v - down3.at[2].v - 0.023_316).abs() < 2e-6);
}
