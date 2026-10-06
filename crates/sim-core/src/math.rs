//! Every transcendental function the engine calls, and the only place it may call one.
//!
//! `f64::exp`, `ln`, `powf` and the rest are not computed by Rust: `std` hands them to the
//! platform's maths library, and the platforms disagree. Measured on this engine, the
//! native Windows build and the wasm build the browser runs differ by one or two ULP on up
//! to a fifth of inputs, so 33 of 54 trajectories parted in the last bit
//! (`docs/plans/cross-platform-math.md`). Routed through the pure-Rust `libm` crate instead,
//! both builds compile the *same source* for these functions, and all 54 agree bit for bit,
//! including a snapshot taken in one and continued in the other.
//!
//! That is a property of this module being the only route, so `crates/sim-core/clippy.toml`
//! forbids the `f64` methods themselves in this crate.
//!
//! Two operations stay on `f64` and are not forbidden, because they cannot differ:
//! `sqrt`, which IEEE 754 requires to be correctly rounded and both targets compute in
//! hardware, and `powi`, which the engine uses only for a square and for powers of two,
//! and which lowers to multiplication.

/// `e^x`.
#[inline]
#[must_use]
pub(crate) fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// `e^x − 1`, accurate near `x = 0`.
#[inline]
#[must_use]
pub(crate) fn exp_m1(x: f64) -> f64 {
    libm::expm1(x)
}

/// Natural logarithm.
#[inline]
#[must_use]
pub(crate) fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// `x^p` for a real exponent.
#[inline]
#[must_use]
pub(crate) fn powf(x: f64, p: f64) -> f64 {
    libm::pow(x, p)
}

/// Hyperbolic sine.
#[inline]
#[must_use]
pub(crate) fn sinh(x: f64) -> f64 {
    libm::sinh(x)
}

/// Hyperbolic cosine.
#[inline]
#[must_use]
pub(crate) fn cosh(x: f64) -> f64 {
    libm::cosh(x)
}

/// Inverse hyperbolic sine.
#[inline]
#[must_use]
pub(crate) fn asinh(x: f64) -> f64 {
    libm::asinh(x)
}

/// Sine of an angle \[rad\].
#[inline]
#[must_use]
pub(crate) fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine of an angle \[rad\].
#[inline]
#[must_use]
pub(crate) fn cos(x: f64) -> f64 {
    libm::cos(x)
}
