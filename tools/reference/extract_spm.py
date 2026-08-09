"""Print a TOML-ready [spm] block for a batsim chemistry, extracted from PyBaMM.

The sibling of `fit_ocv.py`, and deliberately a *different verb*: the ECM tables are
**fitted** to a parameter set's output, while single-particle-model parameters are
**extracted** from the set directly. Every number this prints is a value PyBaMM
already holds (or a documented product of two of them), so each one carries a literal
citation rather than an order-of-magnitude apology.

    python tools/reference/extract_spm.py nmc_21700_lgm50

Paste the emitted block into the matching `chemistries/*.toml`. Not shipped; requires
PyBaMM (see requirements.txt).

## Where the numbers come from, and the one place they are not verbatim

Most fields are a direct key read. Three need a note:

* **`m_ref` and `reaction_ea_j_per_mol`** are *not* parameter-set keys. Chen2020
  supplies the exchange-current density as a Python function, and both the rate
  coefficient and its Arrhenius activation energy are literals inside that function's
  body. This script reads them out of the source text rather than hard-coding them, so
  a parameter-set upgrade that changes either one is caught here instead of silently
  leaving the TOML stale.
* **`diffusivity_ea_j_per_mol` is not emitted at all** for Chen2020, because the set
  fits solid diffusivity as a *constant* at 298.15 K and supplies no activation energy
  for it. Emitting a plausible one would be exactly the invented constant the
  provenance rule forbids. The schema field is optional and defaults to zero
  (= temperature-independent), which is the honest reading of this parameter set.
* **The OCP tables** are sampled from the set's OCP functions onto a grid chosen here,
  with the worst-case piecewise-linear interpolation error measured against a dense
  reference and printed — the same discipline `fit_chemistry` applies to `[ocv]`.
"""

from __future__ import annotations

import inspect
import re
import sys

import numpy as np
import pybamm

from common import PARAM_SETS

# Stoichiometry margin either side of the usable window that the OCP table's **core**
# covers.
#
# The tables cannot stop at the usable limits. Under load a particle's *surface*
# stoichiometry runs ahead of its bulk value — that lag is the entire point of the
# model — so a table clamped at the bulk limits would flatten the OCP exactly where
# the cell is working hardest. The margin is not free: graphite's OCP diverges as
# x -> 0, so widening far enough to be safe would spend most of the table's points on
# a region the cell never reaches.
#
# Phase 7 measured that 0.05 is not enough for a DFN, and *why* it looked like enough
# for an SPM. See EXTEND_TO_FULL_RANGE below: the margin is no longer where the table
# ends, only where its adaptively-refined core does.
STOICH_MARGIN = 0.05

# Extend each OCP table past the core margin to the full stoichiometry range [0, 1].
#
# ## Why, and why it is a Phase 7 finding rather than a Phase 6 bug
#
# A DFN resolves the reaction current through the electrode thickness, so it has a
# *local* surface stoichiometry at every x-node. Phase 7's spike measured the positive
# particle's local surface stoichiometry reaching **0.9998** at 3C, against a table
# topping at 0.9040 — and spending 27.6 % of the run above it, where `ocp_lookup`
# clamps flat exactly as the real OCP plunges. The x-AVERAGED value peaks at only
# 0.7065. An SPM has only the x-averaged quantity, so no SPM run at any rate can reach
# the table top: the 0.05 margin was right for the model it was sized for, and this
# failure mode was unreachable in Phase 6, not merely undetected.
#
# ## Why the core is still generated over the old window
#
# `_ocp_table` refines greedily from `linspace(lo, hi, 9)` against a dense reference on
# the same interval. Widening `lo`/`hi` and regenerating in one pass would move **every
# breakpoint**, redistributing interpolation error *inside* the old window — and the
# shipped SPM goldens and `spm_exact_bits.rs` are pinned to those breakpoints. So the
# extension is append-only **by construction**: the core call is byte-for-byte the one
# that produced the shipped table, and the two tails are tabulated separately and
# concatenated. Regenerating this file must reproduce every existing breakpoint exactly
# and must not insert a point strictly inside the old range.
EXTEND_TO_FULL_RANGE = True

# Seed points for a tail's adaptive grid, against `_ocp_table`'s 9 for a core.
#
# Lower because a tail is short and, at the top of both shipped electrodes, nearly or
# exactly flat: graphite's fit is constant at 0.092020 V above ~0.96, so nine seed
# points there would be nine ways of writing the same number.
TAIL_SEED_POINTS = 3


def _literal_from_source(fn, name: str) -> float:
    """Read a scalar literal assigned to `name` in `fn`'s source text.

    PyBaMM buries the kinetic rate coefficient and its activation energy inside the
    exchange-current-density function body, where no `ParameterValues` lookup can
    reach them. Parsing the source is ugly, and it is still better than transcribing
    the numbers into this file by hand: a transcription silently rots when the
    parameter set is upgraded, while this raises.
    """
    src = inspect.getsource(fn)
    match = re.search(rf"^\s*{name}\s*=\s*([0-9.eE+-]+)", src, re.MULTILINE)
    if match is None:
        raise RuntimeError(
            f"could not find literal '{name}' in {fn.__name__}; PyBaMM's parameter "
            f"set has changed shape and this extractor needs updating"
        )
    return float(match.group(1))


def _ocp_table(pv, ocp_fn, lo: float, hi: float, tol_v: float, max_points: int,
               n_seed: int = 9):
    """Tabulate `ocp_fn` over [lo, hi] to within `tol_v` of the true curve.

    Returns `(stoich, volts, max_lin_err_v, n)`.

    The grid is refined adaptively rather than laid down uniformly, because a uniform
    grid is the wrong instrument for these curves: graphite's OCP rises steeply as its
    stoichiometry approaches zero, and at 81 uniform points that one corner carries
    **43.9 mV** of interpolation error while the rest of the table sits under a
    millivolt. Spending points where the curvature is instead of where the axis is
    gets the same accuracy for a fraction of the table — and, more usefully, lets the
    tool state a *tolerance* it met rather than a point count it guessed.

    The rule is greedy bisection: repeatedly split whichever interval currently
    carries the largest error. `fit_chemistry`'s `[ocv]` grid is hand-tuned to LFP's
    knees instead; that one is load-bearing for committed goldens and is deliberately
    left alone.
    """
    dense = np.linspace(lo, hi, 2001)
    dense_v = np.array([float(pv.evaluate(ocp_fn(pybamm.Scalar(s)))) for s in dense])

    def err_of(grid):
        return np.abs(dense_v - np.interp(dense, grid, np.interp(grid, dense, dense_v)))

    grid = list(np.linspace(lo, hi, n_seed))
    while len(grid) < max_points:
        residual = err_of(np.array(grid))
        if float(np.max(residual)) <= tol_v:
            break
        # Split the interval containing the worst point, at that point's location:
        # bisecting the *error* converges faster than bisecting the interval.
        worst = dense[int(np.argmax(residual))]
        idx = int(np.searchsorted(grid, worst))
        idx = min(max(idx, 1), len(grid) - 1)
        mid = 0.5 * (grid[idx - 1] + grid[idx])
        grid.insert(idx, worst if grid[idx - 1] < worst < grid[idx] else mid)

    stoich = np.array(grid)
    volts = np.array([float(pv.evaluate(ocp_fn(pybamm.Scalar(s)))) for s in stoich])
    max_err = float(np.max(np.abs(dense_v - np.interp(dense, stoich, volts))))
    return stoich, volts, max_err, len(stoich)


def _fmt(xs, places: int) -> str:
    return ", ".join(f"{x:.{places}f}" for x in xs)


def _tail(pv, ocp_fn, lo: float, hi: float, tol_v: float, drop: str):
    """Tabulate the segment `[lo, hi]` that sits outside a core table, ready to append.

    Returns `(stoich, volts, max_lin_err_v)`, empty when the segment is degenerate —
    which the negative electrode's lower tail always is, because `lo - STOICH_MARGIN`
    already clamps at zero there.

    `drop` names the endpoint the core already owns (`"last"` for a segment below the
    core, `"first"` for one above), and dropping it is what makes the concatenation a
    valid strictly-ascending table rather than one with a repeated breakpoint. The
    junction value is therefore always the core's, never this segment's — which is the
    mechanical statement of "append-only": nothing this function returns can land on,
    or inside, the core's range.
    """
    if not hi - lo > 0.0:
        return np.array([]), np.array([]), 0.0
    stoich, volts, err, _ = _ocp_table(
        pv, ocp_fn, lo, hi, tol_v, 200, n_seed=TAIL_SEED_POINTS
    )
    cut = slice(None, -1) if drop == "last" else slice(1, None)
    return stoich[cut], volts[cut], err


def _electrode_block(name: str, side: str, pv, lo: float, hi: float, tol_v: float):
    """Emit one `[spm.<name>]` table. `side` is PyBaMM's "Negative"/"Positive"."""
    ocp_fn = pv[f"{side} electrode OCP [V]"]
    exch_fn = pv[f"{side} electrode exchange-current density [A.m-2]"]
    m_ref = _literal_from_source(exch_fn, "m_ref")
    e_r = _literal_from_source(exch_fn, "E_r")

    lo_t = max(0.0, lo - STOICH_MARGIN)
    hi_t = min(1.0, hi + STOICH_MARGIN)
    stoich, volts, err, n_core = _ocp_table(pv, ocp_fn, lo_t, hi_t, tol_v, 200)

    tail_err = 0.0
    n_below = n_above = 0
    if EXTEND_TO_FULL_RANGE:
        below_s, below_v, err_below = _tail(pv, ocp_fn, 0.0, lo_t, tol_v, drop="last")
        above_s, above_v, err_above = _tail(pv, ocp_fn, hi_t, 1.0, tol_v, drop="first")
        n_below, n_above = len(below_s), len(above_s)
        tail_err = max(err_below, err_above)
        stoich = np.concatenate([below_s, stoich, above_s])
        volts = np.concatenate([below_v, volts, above_v])
    n_points = len(stoich)

    print(f"[spm.{name}]")
    print(f"particle_radius_m       = {pv[f'{side} particle radius [m]']:.6g}")
    print(f"diffusivity_m2_per_s    = {pv[f'{side} particle diffusivity [m2.s-1]']:.6g}")
    print(f"c_max_mol_per_m3        = {pv[f'Maximum concentration in {side.lower()} electrode [mol.m-3]']:.6g}")
    print(f"active_volume_fraction  = {pv[f'{side} electrode active material volume fraction']:.6g}")
    print(f"thickness_m             = {pv[f'{side} electrode thickness [m]']:.6g}")
    print(f"m_ref                   = {m_ref:.6g}   # from the exchange-current-density function body")
    print(f"reaction_ea_j_per_mol   = {e_r:.6g}   # ditto: E_r inside the same function")
    print(f"charge_transfer_alpha   = {pv[f'{side} electrode charge transfer coefficient']:.6g}")
    print(f"stoich_min              = {lo!r}")
    print(f"stoich_max              = {hi!r}")
    print(f"docp_dt_v_per_k         = {pv[f'{side} electrode OCP entropic change [V.K-1]']:.6g}")
    # ASCII only: the shipped chemistry files keep to it (they write "degC", not the
    # degree sign), and this text is pasted straight into one.
    print(f"# OCP table over the FULL stoichiometry range [{stoich[0]:.4f}, "
          f"{stoich[-1]:.4f}]: {n_core} adaptively-placed core points over "
          f"[{lo_t:.4f}, {hi_t:.4f}] (margin {STOICH_MARGIN} either side of the usable")
    print(f"# window), plus {n_below} appended below and {n_above} above. The core is "
          f"generated over the margin window ALONE and its breakpoints are unchanged "
          f"from Phase 6:")
    print(f"# regenerating it over [0, 1] in one pass would move every one of them and "
          f"shift the interpolation inside the window that the shipped SPM goldens "
          f"are pinned to.")
    print(f"# max piecewise-linear interpolation error = {err * 1e3:.2f} mV in the "
          f"core, {tail_err * 1e3:.2f} mV in the extension.")
    print(f"[spm.{name}.ocp]")
    print(f"stoich = [{_fmt(stoich, 6)}]")
    print(f"volts  = [{_fmt(volts, 6)}]")
    print()


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3) or argv[1] not in PARAM_SETS:
        ids = ", ".join(sorted(PARAM_SETS))
        print(f"usage: python extract_spm.py <chem_id> [ocp_tolerance_mV]   "
              f"(one of: {ids})", file=sys.stderr)
        return 2

    chem_id = argv[1]
    # 2 mV: below the resolution of any measurement these parameters came from, and
    # an order of magnitude under the ECM [ocv] table's own 13.9 mV fit error, so the
    # OCP tables are not the limiting approximation anywhere in this file.
    tol_v = (float(argv[2]) if len(argv) == 3 else 2.0) * 1e-3
    param_set = PARAM_SETS[chem_id]
    pv = pybamm.ParameterValues(param_set)
    xmin, xmax, ymin, ymax = pybamm.lithium_ion.get_min_max_stoichiometries(pv)

    print(f"# --- extracted from PyBaMM {param_set} (pybamm {pybamm.__version__}) "
          f"by tools/reference/extract_spm.py ---")
    print(f"# Every value below is a key of the {param_set} parameter set, a literal "
          f"inside one of its")
    print(f"# functions, or a stated product of two keys. Nothing here is fitted or "
          f"invented.")
    print()
    print("[spm]")
    print(f"t_ref_k                = {pv['Reference temperature [K]']:.6g}")
    print(f"c_e_mol_per_m3         = {pv['Initial concentration in electrolyte [mol.m-3]']:.6g}")
    # Electrode plate area: the set gives height and width separately and PyBaMM
    # multiplies them for the current-collector area, so this product is the set's
    # own definition rather than a geometric guess.
    area = float(pv["Electrode height [m]"]) * float(pv["Electrode width [m]"])
    print(f"electrode_area_m2      = {area:.6g}   # = height {pv['Electrode height [m]']} "
          f"x width {pv['Electrode width [m]']}")
    print(f"contact_resistance_ohm = {float(pv['Contact resistance [Ohm]']):.6g}")
    print()

    _electrode_block("negative", "Negative", pv, float(xmin), float(xmax), tol_v)
    _electrode_block("positive", "Positive", pv, float(ymin), float(ymax), tol_v)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
