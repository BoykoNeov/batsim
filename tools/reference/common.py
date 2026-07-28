"""Shared PyBaMM helpers for batsim's golden-reference pipeline.

This module is **not shipped** and is **never** on the Rust build or CI path
(see CLAUDE.md, "Testing strategy"). It requires PyBaMM (see requirements.txt)
and is run by hand to (a) fit a chemistry's OCV table + usable capacity from a
PyBaMM parameter set and (b) generate golden CSV trajectories that the Rust
integration tests replay through `sim-core`.

Conventions matched to `sim-core`:
  * SI units (seconds, amperes, volts).
  * **Positive current = discharge** (batsim's sign convention). PyBaMM already
    uses discharge-positive "Current [A]", so no flip is needed, but every
    current we emit is asserted/derived as discharge-positive.
  * SOC is coulomb-counted against the *usable capacity between the parameter
    set's stoichiometry limits* `Q_use`, so that batsim (whose `capacity_ah` we
    set to the same `Q_use`) tracks PyBaMM's internal stoichiometry-SOC exactly
    under constant current. This is what makes the low-C golden a tight test of
    the OCV table rather than a test of a capacity mismatch.
"""

from __future__ import annotations

import warnings
from dataclasses import dataclass
from typing import Optional

import numpy as np
import pybamm

# PyBaMM is noisy with deprecation/solver chatter that would drown the useful
# output; silence it for these batch scripts.
warnings.filterwarnings("ignore")

# batsim chemistry id -> the PyBaMM parameter set it is fitted against.
# Adding a chemistry here is all that is needed to fit/generate for it.
#
# `nmc_18650_generic` is deliberately ABSENT. It used to map to Chen2020, which was
# never true: Chen2020 parameterizes the LG M50, a 21700 / 5 A.h cell, while that
# file declares an 18650 / 3.0 A.h identity and its values are hand-fit to datasheet
# curves. The map's contract is "the set this chemistry is fitted against", so an
# entry that no fit ever used made the file's provenance false the moment anyone ran
# fit_ocv.py against it. PyBaMM ships no 18650-class NMC set at all (Chen2020,
# OKane2022 and ORegan2022 are all LG M50 21700; Mohtat2020 and Xu2019 are NMC532
# pouch cells), so the honest resolution is that this chemistry has no PyBaMM source
# — see chemistries/nmc_18650_generic.toml's provenance line, and
# docs/plans/phase-6-porous-electrodes.md for the decision.
PARAM_SETS = {
    "lfp_26650_generic": "Prada2013",
    "nmc_21700_lgm50": "Chen2020",
}

# Isothermal reference temperature for every scenario [K] (25 degC). batsim holds
# cell temperature constant in Phase 1, so the DFN reference is run isothermal at
# the same temperature to isolate the electrical comparison.
T_REF_K = 298.15


@dataclass(frozen=True)
class Reference:
    """Which PyBaMM model generates a golden, and how finely it is solved.

    Phase 1 had one answer (a DFN on PyBaMM's defaults) and did not need this type.
    Phase 6 needs three, because batsim now has two cell models and they are
    validated against different references:

      * `DFN_DEFAULT` — what the committed LFP goldens were generated with. It is
        spelled out here as a value rather than left implicit so that the LFP path
        is *provably* unchanged: `kind="DFN"` with no grid and no solver override
        means `Simulation(model, parameter_values=pv)`, exactly the call Phase 1
        made. Regenerating the LFP CSVs must produce byte-identical files.
      * `SPM_CONVERGED` — batsim's SPM against PyBaMM's SPM on the same parameter
        set. Same model form, so this is a far tighter comparison than ECM-vs-DFN,
        and it is only meaningful if the reference is converged in **both**
        discretisations; see the two fields below.
      * `DFN_CONVERGED` — the same scenario against a DFN, so the SPM-vs-DFN gap
        can be shown as the physics result it is rather than read as a test
        failure.

    # Why `rtol`/`atol` are here and not left at PyBaMM's defaults
    Measured on this parameter set at 1C: between the default `1e-6` and `1e-10`
    the terminal voltage moves by **18-75 mV** near the end-of-discharge knee, and
    *non-monotonically in the spatial grid* — a finer particle grid at loose
    tolerance was sometimes further from the converged answer than a coarser one.
    That is a time-integration error masquerading as a discretisation error, and a
    golden carrying it would tolerance batsim against solver noise. At `1e-10` the
    spatial convergence is clean and monotone: r_pts 50 -> 100 -> 200 sits 7.99 ->
    4.04 -> 1.90 mV from r_pts = 400 at the knee, and 0.34 -> 0.35 -> 0.21 mV away
    from it.
    """

    # Why `dense_output` exists, and what it cost to find out
    # -------------------------------------------------------
    # `sim.solve(t_eval=[t0, t1])` asks for an interval, and the returned solution
    # carries the solver's OWN steps. An adaptive integrator takes enormous steps
    # wherever the solution is smooth, so resampling that output onto a uniform grid
    # with `np.interp` draws a straight CHORD across each step. On the C/5 SPM run
    # that chord spanned 8_700 s of a 17_850 s discharge, and the committed CSV would
    # have carried a perfectly linear voltage from soc 0.107 down to the cut-off --
    # off by up to 346 mV, and *independent of every knob batsim has*, which is
    # exactly what makes it lethal: it reads as a converged disagreement rather than
    # as a sampling artifact. It was caught only because batsim's error did not move
    # with shell count.
    #
    # `t_interp` asks the solver to evaluate its own interpolant at given times. It
    # does not change the integration, so it costs accuracy nothing and it makes the
    # later `np.interp` an identity on the points it is given.
    #
    # Phase 1's LFP goldens keep `dense_output=False` and are therefore untouched by
    # this. They are not wrong in the same way -- their DFN ran on the default CasADi
    # solver, whose returned grid is dense enough that the chords are short -- but
    # they are the same *shape* of risk, and the flag is where that is written down.

    kind: str  # "DFN" or "SPM"
    r_pts: Optional[int] = None  # particle-grid points; None = the model's default
    rtol: Optional[float] = None  # None = PyBaMM's default solver and tolerances
    atol: Optional[float] = None
    dense_output: bool = False  # request output on the sample grid, not the solver's


# The Phase 1 reference, spelled out: PyBaMM's own defaults for grid and solver.
DFN_DEFAULT = Reference(kind="DFN")

# Grid- and time-converged references for the Phase 6 SPM goldens. 200 radial
# points and 1e-10 tolerances, for the reasons measured in `Reference`'s docstring.
SPM_CONVERGED = Reference(
    kind="SPM", r_pts=200, rtol=1e-10, atol=1e-10, dense_output=True
)
DFN_CONVERGED = Reference(
    kind="DFN", r_pts=200, rtol=1e-10, atol=1e-10, dense_output=True
)


@dataclass
class ChemFit:
    """OCV table + usable capacity extracted from a PyBaMM parameter set."""

    param_set: str
    pybamm_version: str
    soc: np.ndarray  # SOC breakpoints in [0, 1], ascending
    ocv: np.ndarray  # thermodynamic cell OCV at each SOC [V], ascending
    capacity_ah: float  # usable capacity between the stoichiometry limits [A.h]
    v_min: float  # lower voltage cut-off of the parameter set [V]
    v_max: float  # upper voltage cut-off of the parameter set [V]
    max_lin_err_v: float  # max piecewise-linear interp error of the table [V]


def _ocv_of_z(pv, xmin, xmax, ymin, ymax):
    """Return f(z) -> cell OCV [V] for state of charge z in [0, 1].

    z = 1 is fully charged (negative electrode at xmax, positive at ymin);
    z = 0 is fully discharged. OCV = U_p(y(z)) - U_n(x(z)).
    """
    u_n = pv["Negative electrode OCP [V]"]
    u_p = pv["Positive electrode OCP [V]"]

    def f(z):
        x = xmin + z * (xmax - xmin)
        y = ymax + z * (ymin - ymax)
        up = pv.evaluate(u_p(pybamm.Scalar(y)))
        un = pv.evaluate(u_n(pybamm.Scalar(x)))
        return float(up - un)

    return f


def fit_chemistry(chem_id: str, n_dense: int = 2001) -> ChemFit:
    """Extract OCV(SOC) and usable capacity for a batsim chemistry id.

    The OCV table is chosen on a non-uniform SOC grid that is dense near the
    knees (where LFP in particular is steep) and sparse across the plateau, then
    the worst-case piecewise-linear interpolation error of that grid is measured
    against a dense reference so the caller can judge the table's fidelity.
    """
    param_set = PARAM_SETS[chem_id]
    pv = pybamm.ParameterValues(param_set)
    xmin, xmax, ymin, ymax = pybamm.lithium_ion.get_min_max_stoichiometries(pv)

    lip = pybamm.LithiumIonParameters()
    q_n = float(pv.evaluate(lip.n.Q_init))
    q_use = q_n * (xmax - xmin)  # == q_p*(ymax-ymin) by SOH construction

    ocv_of_z = _ocv_of_z(pv, xmin, xmax, ymin, ymax)

    # Table grid: much denser at the ends, where LFP is steep and convex, so the
    # piecewise-linear table tracks the continuous OCV to a few mV even through
    # the knees. Union of fine knee grids and a coarse plateau grid, dedup+sorted.
    knee = np.concatenate(
        [
            np.linspace(0.0, 0.02, 9),  # steepest bottom knee: 0.25% spacing
            np.linspace(0.02, 0.05, 4)[1:],
            np.linspace(0.05, 0.15, 3)[1:],
            np.linspace(0.85, 0.95, 3),
            np.linspace(0.95, 0.98, 4)[1:],
            np.linspace(0.98, 1.0, 9)[1:],  # steepest top knee: 0.25% spacing
        ]
    )
    plateau = np.linspace(0.15, 0.85, 8)
    soc = np.unique(np.round(np.concatenate([knee, plateau]), 6))
    ocv = np.array([ocv_of_z(z) for z in soc])

    # Worst-case linear-interpolation error of this table vs a dense reference.
    z_dense = np.linspace(0.0, 1.0, n_dense)
    ocv_dense = np.array([ocv_of_z(z) for z in z_dense])
    ocv_interp = np.interp(z_dense, soc, ocv)
    max_lin_err = float(np.max(np.abs(ocv_dense - ocv_interp)))

    return ChemFit(
        param_set=param_set,
        pybamm_version=pybamm.__version__,
        soc=soc,
        ocv=ocv,
        capacity_ah=float(q_use),
        v_min=float(pv["Lower voltage cut-off [V]"]),
        v_max=float(pv["Upper voltage cut-off [V]"]),
        max_lin_err_v=max_lin_err,
    )


def _isothermal(param_set: str, ref: Reference, initial_soc: float = 1.0):
    """A reference model + parameter values + `Simulation` kwargs, isothermal at T_REF_K.

    The initial stoichiometry is pinned to `initial_soc` (1.0 = fully charged, the
    upper voltage cut-off) so the reference starts at the same SOC=1.0 state batsim
    is built at — the parameter set's own default initial concentration is a lower,
    misaligned SOC, which would offset the whole trajectory.

    The returned `kwargs` carry the grid and solver overrides, and are **empty** for
    `DFN_DEFAULT`. That emptiness is the mechanism by which Phase 1's LFP goldens
    are unaffected by Phase 6: with no `var_pts` and no `solver`, the `Simulation`
    call is character-for-character the one that produced the committed CSVs.
    """
    ctor = pybamm.lithium_ion.SPM if ref.kind == "SPM" else pybamm.lithium_ion.DFN
    model = ctor(options={"thermal": "isothermal"})
    pv = pybamm.ParameterValues(param_set)
    pv.update(
        {
            "Ambient temperature [K]": T_REF_K,
            "Initial temperature [K]": T_REF_K,
            "Reference temperature [K]": T_REF_K,
        }
    )
    pv = pv.set_initial_stoichiometries(initial_soc)

    kwargs = {}
    if ref.r_pts is not None:
        var_pts = dict(model.default_var_pts)
        var_pts["r_n"] = ref.r_pts
        var_pts["r_p"] = ref.r_pts
        kwargs["var_pts"] = var_pts
    if ref.rtol is not None:
        kwargs["solver"] = pybamm.IDAKLUSolver(rtol=ref.rtol, atol=ref.atol)
    return model, pv, kwargs


def run_cc_discharge(
    chem_id: str, fit: ChemFit, c_rate: float, dt_s: float, ref: Reference = DFN_DEFAULT
):
    """CC discharge from full at `c_rate`, isothermal, sampled every `dt_s`.

    Returns (time_s, current_a, voltage_v, soc) with discharge-positive current.
    Row 0 is the rested initial state (current 0, V = OCV(1.0)); every later row
    carries the constant applied current. Stops at the parameter set's lower
    cut-off (or one hour of C-rate, whichever comes first).
    """
    param_set = PARAM_SETS[chem_id]
    model, pv, sim_kwargs = _isothermal(param_set, ref)
    i_app = c_rate * fit.capacity_ah  # discharge-positive [A]
    pv.update({"Current function [A]": i_app})

    sim = pybamm.Simulation(model, parameter_values=pv, **sim_kwargs)
    t_end = 3600.0 / c_rate * 1.10  # a little past nominal full discharge
    # The adaptive solver returns its own (non-uniform, front-loaded) grid; pass
    # the interval and resample voltage onto a uniform grid for a compact CSV.
    # `dense_output` additionally asks the solver to evaluate its interpolant on
    # that same grid, so the resampling below cannot chord across a solver step —
    # see `Reference.dense_output` for the 346 mV this is worth.
    solve_kwargs = {}
    if ref.dense_output:
        # Strictly inside the interval: the solver rejects an interpolation point at
        # the far end of `t_eval`.
        grid = np.arange(0.0, t_end, dt_s)
        solve_kwargs["t_interp"] = grid[grid < t_end]
    sol = sim.solve(t_eval=[0.0, t_end], **solve_kwargs)

    t_native = sol["Time [s]"].entries
    v_native = sol["Terminal voltage [V]"].entries
    # PyBaMM "Current [A]" is discharge-positive; confirm it before we rely on it.
    assert np.median(sol["Current [A]"].entries) > 0, "expected discharge-positive current"

    total_s = float(t_native[-1])  # ends at the lower voltage cut-off event
    t = np.arange(0.0, total_s + dt_s, dt_s)
    t = t[t <= total_s]
    v = np.interp(t, t_native, v_native)

    current = np.full_like(t, i_app)
    current[0] = 0.0  # row 0 = rested initial state
    soc = 1.0 - np.cumsum(np.concatenate([[0.0], np.diff(t)]) * current) / (
        3600.0 * fit.capacity_ah
    )
    return t, current, v, soc


def run_pulse_relax(
    chem_id: str, fit: ChemFit, c_rate: float, dt_s: float, ref: Reference = DFN_DEFAULT
):
    """GITT-like pulse train: discharge pulses separated by rests, isothermal.

    Ten (pulse, rest) cycles from full, each pulse removing ~5% SOC followed by a
    rest long enough to relax most of the RC overpotential. Returns the same
    (time_s, current_a, voltage_v, soc) tuple with discharge-positive current and
    a piecewise-constant current profile aligned to the sample grid.
    """
    param_set = PARAM_SETS[chem_id]
    model, pv, sim_kwargs = _isothermal(param_set, ref)
    i_app = c_rate * fit.capacity_ah

    pulse_s = round(0.05 / c_rate * 3600.0)  # ~5% SOC per pulse
    rest_s = 1200.0  # 20 min relaxation
    n_cycles = 10

    # The applied-current profile as (duration, current) segments; used both to
    # build the PyBaMM experiment and to reconstruct the exact replay current on
    # the output grid.
    seg = []  # (duration_s, current_a)
    for _ in range(n_cycles):
        seg.append((pulse_s, i_app))
        seg.append((rest_s, 0.0))

    def current_at(ts):
        out = np.zeros_like(ts)
        acc = 0.0
        for d, cur in seg:
            # A sample exactly on a boundary belongs to the segment ending there.
            mask = (ts > acc) & (ts <= acc + d)
            out[mask] = cur
            acc += d
        return out

    # Run the piecewise profile as a PyBaMM experiment (native step control),
    # then resample onto a uniform grid so the committed CSV is regular. Voltage
    # is continuous, so linear resampling at dt_s (<< the RC/relaxation scales) is
    # accurate; the replay current is set analytically for an exact profile.
    # An experiment is solved step by step, so the `t_interp` used by the CC runner
    # cannot be applied here (its grid would run past the first step's own end).
    # PyBaMM's per-step output `period` is the equivalent knob, and it is what keeps
    # the resampling below from chording across a solver step — see
    # `Reference.dense_output`.
    period = f" ({int(dt_s)} second period)" if ref.dense_output else ""
    experiment = pybamm.Experiment(
        [
            (
                f"Discharge at {i_app:.6f} A for {pulse_s} seconds{period}",
                f"Rest for {int(rest_s)} seconds{period}",
            )
        ]
        * n_cycles
    )
    sim = pybamm.Simulation(
        model, parameter_values=pv, experiment=experiment, **sim_kwargs
    )
    sol = sim.solve()

    t_native = sol["Time [s]"].entries
    v_native = sol["Terminal voltage [V]"].entries
    total_s = float(t_native[-1])
    t = np.arange(0.0, total_s + dt_s, dt_s)
    t = t[t <= total_s]
    v = np.interp(t, t_native, v_native)

    current = current_at(t)
    current[0] = 0.0
    soc = 1.0 - np.cumsum(np.concatenate([[0.0], np.diff(t)]) * current) / (
        3600.0 * fit.capacity_ah
    )
    return t, current, v, soc
