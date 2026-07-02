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

import numpy as np
import pybamm

# PyBaMM is noisy with deprecation/solver chatter that would drown the useful
# output; silence it for these batch scripts.
warnings.filterwarnings("ignore")

# batsim chemistry id -> the PyBaMM parameter set it is fitted against.
# Adding a chemistry here is all that is needed to fit/generate for it.
PARAM_SETS = {
    "lfp_26650_generic": "Prada2013",
    "nmc_18650_generic": "Chen2020",
}

# Isothermal reference temperature for every scenario [K] (25 degC). batsim holds
# cell temperature constant in Phase 1, so the DFN reference is run isothermal at
# the same temperature to isolate the electrical comparison.
T_REF_K = 298.15


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


def _isothermal_dfn(param_set: str, initial_soc: float = 1.0):
    """A DFN model + parameter values, isothermal at T_REF_K, at `initial_soc`.

    The initial stoichiometry is pinned to `initial_soc` (1.0 = fully charged, the
    upper voltage cut-off) so the reference starts at the same SOC=1.0 state batsim
    is built at — the parameter set's own default initial concentration is a lower,
    misaligned SOC, which would offset the whole trajectory.
    """
    model = pybamm.lithium_ion.DFN(options={"thermal": "isothermal"})
    pv = pybamm.ParameterValues(param_set)
    pv.update(
        {
            "Ambient temperature [K]": T_REF_K,
            "Initial temperature [K]": T_REF_K,
            "Reference temperature [K]": T_REF_K,
        }
    )
    pv = pv.set_initial_stoichiometries(initial_soc)
    return model, pv


def run_cc_discharge(chem_id: str, fit: ChemFit, c_rate: float, dt_s: float):
    """CC discharge from full at `c_rate`, isothermal, sampled every `dt_s`.

    Returns (time_s, current_a, voltage_v, soc) with discharge-positive current.
    Row 0 is the rested initial state (current 0, V = OCV(1.0)); every later row
    carries the constant applied current. Stops at the parameter set's lower
    cut-off (or one hour of C-rate, whichever comes first).
    """
    param_set = PARAM_SETS[chem_id]
    model, pv = _isothermal_dfn(param_set)
    i_app = c_rate * fit.capacity_ah  # discharge-positive [A]
    pv.update({"Current function [A]": i_app})

    sim = pybamm.Simulation(model, parameter_values=pv)
    t_end = 3600.0 / c_rate * 1.10  # a little past nominal full discharge
    # The adaptive solver returns its own (non-uniform, front-loaded) grid; pass
    # the interval and resample voltage onto a uniform grid for a compact CSV.
    sol = sim.solve(t_eval=[0.0, t_end])

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


def run_pulse_relax(chem_id: str, fit: ChemFit, c_rate: float, dt_s: float):
    """GITT-like pulse train: discharge pulses separated by rests, isothermal.

    Ten (pulse, rest) cycles from full, each pulse removing ~5% SOC followed by a
    rest long enough to relax most of the RC overpotential. Returns the same
    (time_s, current_a, voltage_v, soc) tuple with discharge-positive current and
    a piecewise-constant current profile aligned to the sample grid.
    """
    param_set = PARAM_SETS[chem_id]
    model, pv = _isothermal_dfn(param_set)
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
    experiment = pybamm.Experiment(
        [
            (
                f"Discharge at {i_app:.6f} A for {pulse_s} seconds",
                f"Rest for {int(rest_s)} seconds",
            )
        ]
        * n_cycles
    )
    sim = pybamm.Simulation(model, parameter_values=pv, experiment=experiment)
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
