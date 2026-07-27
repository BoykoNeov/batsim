//! Random draws from the single seeded simulation RNG.
//!
//! Every stochastic quantity in the engine comes through here, so there is exactly
//! one place where a draw can consume RNG state — which is what makes the
//! same-config-and-seed-gives-a-bit-identical-trajectory guarantee checkable rather
//! than hopeful. The RNG itself lives in [`crate::Pack`] and is part of the
//! snapshot; nothing in this module holds state.
//!
//! Two callers today: manufacturing scatter at construction, and current-sensor
//! noise per step.

use rand_chacha::ChaCha8Rng;

/// A uniform `f64` in `[0, 1)` with full 53-bit mantissa resolution.
fn next_unit(rng: &mut ChaCha8Rng) -> f64 {
    use rand_core::RngCore;
    // Top 53 bits of a u64 → an integer in [0, 2^53), scaled into [0, 1).
    (rng.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Two independent standard normals via the Box–Muller transform.
pub(crate) fn standard_normal_pair(rng: &mut ChaCha8Rng) -> (f64, f64) {
    // Guard the radius against u1 == 0 (ln(0) = −∞); MIN_POSITIVE keeps it finite.
    let u1 = next_unit(rng).max(f64::MIN_POSITIVE);
    let u2 = next_unit(rng);
    let mag = (-2.0 * u1.ln()).sqrt();
    let angle = core::f64::consts::TAU * u2;
    (mag * angle.cos(), mag * angle.sin())
}

/// One standard normal, discarding Box–Muller's second value.
///
/// The waste is deliberate. Caching the spare would put it in the snapshot and make
/// the RNG's observable behaviour depend on how many draws happened to be requested
/// in earlier steps — a determinism footgun in exchange for one `ln`, one `sqrt` and
/// one `cos` per call on a path that runs once per step, not once per cell.
pub(crate) fn standard_normal(rng: &mut ChaCha8Rng) -> f64 {
    standard_normal_pair(rng).0
}
