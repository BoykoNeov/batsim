"""The cold DFN golden for Phase 11 exit 5: 1C from full at -10 degC, on the cell the engine runs.

    python tools/reference/dfn_cold_golden.py

Writes `tests/golden/nmc_21700_lgm50/dfn_cc_1c_m10c.csv`: PyBaMM's DFN on Chen2020 with
ORegan2022's solid-diffusion, electrolyte and entropy terms (`fit_ecm.combined_parameter_values`),
isothermal at 263.15 K, converged (`common.DFN_CONVERGED` in the particles, `X_PTS` across the
cell), sampled every 5 s to the 2.5 V cut-off. `crates/sim-data/tests/dfn_cold.rs` replays it through the engine's `Dfn` on the shipped
file. Not shipped; never on the Rust/CI path.
"""

from __future__ import annotations

import sys
import warnings
from pathlib import Path

import numpy as np

warnings.filterwarnings("ignore")
sys.path.insert(0, str(Path(__file__).resolve().parent))

import pybamm  # noqa: E402

from common import DFN_CONVERGED, fit_chemistry  # noqa: E402
from fit_ecm import combined_parameter_values  # noqa: E402

OUT = Path(__file__).resolve().parents[2] / "tests" / "golden" / "nmc_21700_lgm50" / "dfn_cc_1c_m10c.csv"
TEMP_K = 263.15
DT_S = 5.0
# Across the cell, set here and not in `common.DFN_CONVERGED`, so that `generate.py` regenerates
# nothing else on a different grid. PyBaMM's default x-grid (20/20/20) is NOT converged cold:
# the cut-off moves 2787.3 -> 2807.5 -> 2812.1 s on 20/20/20, 40/40/40, 80/40/80 at 1C, -10 degC
# (docs/plans/phase-11-fitting.md, 2026-10-17 addendum), because the electrolyte diffuses 0.4x as
# fast and its profile steepens. The finest grid PyBaMM was run at.
X_PTS = (160, 80, 160)


def main() -> int:
    q = fit_chemistry("nmc_21700_lgm50").capacity_ah
    model = pybamm.lithium_ion.DFN(options={"thermal": "isothermal"})
    pv = combined_parameter_values(TEMP_K)
    pv.update({"Lower voltage cut-off [V]": 2.5, "Current function [A]": q})
    pv = pv.set_initial_stoichiometries(1.0)
    var_pts = dict(model.default_var_pts)
    var_pts["r_n"] = var_pts["r_p"] = DFN_CONVERGED.r_pts
    var_pts["x_n"], var_pts["x_s"], var_pts["x_p"] = X_PTS
    sim = pybamm.Simulation(model, parameter_values=pv, var_pts=var_pts,
                            solver=pybamm.IDAKLUSolver(rtol=DFN_CONVERGED.rtol, atol=DFN_CONVERGED.atol))
    t_end = 3600.0 * 1.1
    grid = np.arange(0.0, t_end, DT_S)
    sol = sim.solve(t_eval=[0.0, t_end], t_interp=grid[grid < t_end])
    if "event" not in str(sol.termination):
        raise RuntimeError(f"ended as {sol.termination!r}, not on the cut-off")
    t_native = sol["Time [s]"].entries
    v_native = sol["Terminal voltage [V]"].entries
    total = float(t_native[-1])
    t = np.arange(0.0, total + DT_S, DT_S)
    t = t[t <= total]
    v = np.interp(t, t_native, v_native)
    i = np.full_like(t, q)
    i[0] = 0.0
    soc = 1.0 - np.cumsum(np.concatenate([[0.0], np.diff(t)]) * i) / (3600.0 * q)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write("# batsim golden reference - nmc_21700_lgm50 - dfn_cc_1c_m10c.csv\n")
        f.write("# scenario: 1C constant-current discharge from full, isothermal -10 degC (Phase 11 exit 5)\n")
        f.write("# source: PyBaMM Chen2020 DFN + ORegan2022 solid-diffusion, electrolyte and entropy "
                f"terms, pybamm {pybamm.__version__}; tools/reference/dfn_cold_golden.py\n")
        f.write(f"# grid: r_n = r_p = {DFN_CONVERGED.r_pts}; x_n/x_s/x_p = {X_PTS[0]}/{X_PTS[1]}/{X_PTS[2]}; solver: IDAKLU, rtol = atol = "
                f"{DFN_CONVERGED.rtol:g}\n")
        f.write("time_s,current_a,voltage_v,soc\n")
        for row in zip(t, i, v, soc):
            f.write(f"{row[0]:.3f},{row[1]:.9f},{row[2]:.9f},{row[3]:.9f}\n")
    print(f"wrote {OUT} ({len(t)} rows, cut-off at {total:.1f} s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
