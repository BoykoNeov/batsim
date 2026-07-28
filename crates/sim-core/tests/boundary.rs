//! The finiteness rule on the engine's two step arguments.
//!
//! These checks are not called by [`sim_core::Pack::step`] — the engine is permissive by
//! contract and never returns `Err` from a step. They exist for adapters, and they live on
//! the engine's own types because the rule they encode is a fact about `Demand` and `Env`
//! rather than about any one protocol. See `Demand::check_finite` for the full argument,
//! and `docs/plans/phase-5-godot.md` for why three adapters share them instead of copying
//! four `is_finite` calls each.
//!
//! Every non-finite spelling is covered on every field, because the failure mode being
//! guarded against is one arm being forgotten — which is exactly what happened to
//! `t_coolant` in the hand-copied versions this replaced: it is the only field behind an
//! `Option`, so it is the only one a naive check misses.

use sim_core::{Demand, Env, NonFinite};

/// Every way a `f64` can fail to be finite. `-0.0` is deliberately *not* here: it is
/// finite, and rejecting it would be wrong.
const NON_FINITE: [f64; 3] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

#[test]
fn a_demand_carrying_a_number_rejects_every_non_finite_spelling() {
    for bad in NON_FINITE {
        for (what, demand) in [
            ("Current", Demand::Current(bad)),
            ("Power", Demand::Power(bad)),
            ("Voltage", Demand::Voltage(bad)),
        ] {
            let error = demand
                .check_finite()
                .expect_err(&format!("Demand::{what}({bad}) was accepted"));
            // The value is carried through verbatim so an adapter's message can name it.
            // `NaN != NaN`, so compare bits rather than values.
            assert_eq!(
                error.value.to_bits(),
                bad.to_bits(),
                "Demand::{what}({bad}) reported the wrong value"
            );
            assert!(
                error.field.contains(&what.to_lowercase()),
                "Demand::{what} reported field {:?}, which does not name the variant",
                error.field
            );
        }
    }
}

#[test]
fn a_finite_demand_and_rest_are_accepted() {
    for demand in [
        Demand::Current(-5.0),
        Demand::Power(0.0),
        // Finite, and a legitimate value: negative current is charging.
        Demand::Current(-0.0),
        Demand::Voltage(3.65),
        Demand::Rest,
    ] {
        assert_eq!(demand.check_finite(), Ok(()), "{demand:?} was rejected");
    }
}

/// `Rest` carries no number, so there is nothing it could fail on.
#[test]
fn rest_is_unconditionally_accepted() {
    assert_eq!(Demand::Rest.check_finite(), Ok(()));
}

#[test]
fn an_env_rejects_a_non_finite_ambient() {
    for bad in NON_FINITE {
        let error = Env {
            t_ambient: bad,
            t_coolant: None,
        }
        .check_finite()
        .expect_err(&format!("t_ambient = {bad} was accepted"));
        assert_eq!(error.value.to_bits(), bad.to_bits());
        assert!(
            error.field.contains("t_ambient"),
            "reported field {:?}",
            error.field
        );
    }
}

/// The arm a hand-written check misses, because it is the only one behind an `Option`.
#[test]
fn an_env_rejects_a_non_finite_coolant_behind_the_option() {
    for bad in NON_FINITE {
        let error = Env {
            t_ambient: 298.15,
            t_coolant: Some(bad),
        }
        .check_finite()
        .expect_err(&format!("t_coolant = Some({bad}) was accepted"));
        assert_eq!(error.value.to_bits(), bad.to_bits());
        assert!(
            error.field.contains("t_coolant"),
            "reported field {:?}",
            error.field
        );
    }
}

/// `None` means "no coolant", not "a coolant with no temperature". It must not be
/// confused with a missing value and rejected.
#[test]
fn an_absent_coolant_is_not_a_non_finite_one() {
    assert_eq!(
        Env {
            t_ambient: 298.15,
            t_coolant: None,
        }
        .check_finite(),
        Ok(())
    );
}

#[test]
fn a_finite_env_is_accepted() {
    assert_eq!(
        Env {
            t_ambient: 263.15,
            t_coolant: Some(288.15),
        }
        .check_finite(),
        Ok(())
    );
}

/// Ambient is checked before coolant, so a doubly-bad environment names the first field
/// rather than an arbitrary one. Pinned because an adapter's error message is the only
/// thing a user sees, and "fix t_coolant" would be misleading advice when both are wrong.
#[test]
fn ambient_is_reported_first_when_both_are_bad() {
    let error = Env {
        t_ambient: f64::NAN,
        t_coolant: Some(f64::INFINITY),
    }
    .check_finite()
    .expect_err("a doubly non-finite env was accepted");
    assert!(
        error.field.contains("t_ambient"),
        "reported field {:?}",
        error.field
    );
}

/// The error's `Display` is what every adapter forwards to a user, so it must name both
/// the field and the value without the adapter having to reformat it.
#[test]
fn the_error_displays_the_field_and_the_value() {
    let error = NonFinite {
        field: "env.t_ambient [K]",
        value: f64::NAN,
    };
    let text = error.to_string();
    assert!(text.contains("env.t_ambient [K]"), "{text}");
    assert!(text.contains("NaN"), "{text}");
}
