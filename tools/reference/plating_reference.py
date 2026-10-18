"""Phase 12 slice A: the plating law, closed loop, in PyBaMM — the reference the engine's reaction
is checked against. Design and results in `docs/plans/phase-12-slice-a-reference.md`.

    python tools/reference/plating_reference.py <temp_c> <c_rate> <k_m_per_s> [--cc-only]
        [--dead 0.5] [--width 1e-3] [--x 40,20,40] [--r 200] [--smooth] [--out trace.csv]

A charge from empty on the cell the engine runs (`fit_ecm.combined_parameter_values`, isothermal):
constant current to 4.2 V, then 4.2 V held to C/20, unless `--cc-only`. Prints one summary line;
`--out` also writes the per-second trace.

The law, per negative node, `eta = phi_s - phi_e` (no film), `f = F/RT`, `alpha = 0.65`,
`i0 = F k c_e`:

    J = i0 [exp(-alpha f eta) - exp((1 - alpha) f eta)]      positive plates

PyBaMM ships no such form (`docs/plans/phase-12-spike.md`, A1): its `"reversible"` stripping
exchange current goes as the plated lithium, which starts at zero, so nothing opposes the first
plating. Its `"reversible"` submodel is therefore replaced. With both exchange currents `F k c_e`
it is the law uncapped; the cap acts on the NET current where it strips,

    J_c = J                        where J >= 0 (plating)
    J_c = J c+ / (c+ + w)          where J <  0 (stripping), c+ = max(c_rev, 0)

so a node holding no reversible lithium does nothing above 0 V. A share `d` of what plates is dead
at once and never strips. Gross plated (everything the plating branch put down) is dead / d.
Not shipped; never on the Rust/CI path.
"""

from __future__ import annotations

import argparse
import sys
import warnings
from pathlib import Path

import numpy as np

warnings.filterwarnings("ignore")
sys.path.insert(0, str(Path(__file__).resolve().parent))

import pybamm  # noqa: E402
from pybamm.models.submodels.interface.lithium_plating.base_plating import BasePlating  # noqa: E402

from common import fit_chemistry  # noqa: E402
from fit_ecm import combined_parameter_values  # noqa: E402

# OKane2022's plating constants the submodel needs besides the two exchange currents, which are
# replaced below. Only the partial molar volume and the typical concentration (a solver scale) are
# read; neither enters the law.
OKANE_KEYS = (
    "Lithium metal partial molar volume [m3.mol-1]",
    "Typical plated lithium concentration [mol.m-3]",
    "Dead lithium decay constant [s-1]",
    "Dead lithium decay rate [s-1]",
)
ALPHA = 0.65  # OKane2022's plating transfer coefficient, the plan's alpha
V_HOLD = 4.2
TAPER_C = 0.05
HOLD_LIMIT_H = 12


def law_submodel(dead_share: float, width: float):
    """`pybamm.lithium_plating.Plating`, its coupled variables and rates replaced by the law."""

    class LawPlating(pybamm.lithium_plating.Plating):
        def get_coupled_variables(self, variables):
            domain, Domain = self.domain_Domain
            pn = self.phase_name
            eta = variables[f"{Domain} electrode surface potential difference [V]"]
            c_e = variables[f"{Domain} electrolyte concentration [mol.m-3]"]
            T = variables[f"{Domain} electrode temperature [K]"]
            c_rev = variables[f"{Domain} {pn}lithium plating concentration [mol.m-3]"]
            i0 = self.phase_param.j0_plating(c_e, c_rev, T)  # F k c_e, OKane2022's form
            f = self.param.F / (self.param.R * T)
            # PyBaMM's sign: positive strips.
            j = i0 * (pybamm.exp((1 - ALPHA) * f * eta) - pybamm.exp(-ALPHA * f * eta))
            c_pos = pybamm.maximum(c_rev, 0)
            j_strip = pybamm.maximum(j, 0) * c_pos / (c_pos + width)
            j_plate = pybamm.minimum(j, 0)
            variables.update(self._get_standard_overpotential_variables(eta))
            variables.update(self._get_standard_reaction_variables(j_strip + j_plate))
            variables[f"{Domain} electrode plating branch current density [A.m-2]"] = j_plate
            variables[f"{Domain} electrode stripping branch current density [A.m-2]"] = j_strip
            return BasePlating.get_coupled_variables(self, variables)

        def set_rhs(self, variables):
            domain, Domain = self.domain_Domain
            pn = self.phase_name
            c_rev = variables[f"{Domain} {pn}lithium plating concentration [mol.m-3]"]
            c_dead = variables[f"{Domain} {pn}dead lithium concentration [mol.m-3]"]
            a = variables[f"{Domain} electrode {pn}surface area to volume ratio [m-1]"]
            j_plate = variables[f"{Domain} electrode plating branch current density [A.m-2]"]
            j_strip = variables[f"{Domain} electrode stripping branch current density [A.m-2]"]
            F = self.param.F
            self.rhs = {
                c_rev: -a * (j_strip + (1 - dead_share) * j_plate) / F,
                c_dead: -a * dead_share * j_plate / F,
            }

    return LawPlating


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("temp_c", type=float)
    ap.add_argument("c_rate", type=float)
    ap.add_argument("k", type=float)
    ap.add_argument("--cc-only", action="store_true")
    ap.add_argument("--dead", type=float, default=0.5)
    ap.add_argument("--width", type=float, default=1e-3)
    ap.add_argument("--x", default="40,20,40")
    ap.add_argument("--r", type=int, default=200)
    ap.add_argument("--smooth", action="store_true", help="PyBaMM's smoothed max/min, k = 100")
    ap.add_argument("--rtol", type=float, default=1e-10)
    ap.add_argument("--out")
    a = ap.parse_args()
    if a.smooth:
        pybamm.settings.set_smoothing_parameters(100)
    temp_k = 273.15 + a.temp_c
    q = fit_chemistry("nmc_21700_lgm50").capacity_ah

    pybamm.lithium_plating.Plating = law_submodel(a.dead, a.width)
    model = pybamm.lithium_ion.DFN(options={"thermal": "isothermal", "lithium plating": "reversible"})
    pv = combined_parameter_values(temp_k)
    ok = pybamm.ParameterValues("OKane2022")
    pv.update({k: ok[k] for k in OKANE_KEYS}, check_already_exists=False)
    pv.update({
        "Lithium plating kinetic rate constant [m.s-1]": a.k,
        "Exchange-current density for plating [A.m-2]":
            lambda c_e, c_Li, T: pybamm.constants.F
            * pybamm.Parameter("Lithium plating kinetic rate constant [m.s-1]") * c_e,
        # Never read by the law; set so the submodel's parameter checks pass.
        "Exchange-current density for stripping [A.m-2]":
            lambda c_e, c_Li, T: pybamm.constants.F
            * pybamm.Parameter("Lithium plating kinetic rate constant [m.s-1]") * c_e,
        "Lithium plating transfer coefficient": ALPHA,
        "Initial plated lithium concentration [mol.m-3]": 0.0,
        "Upper voltage cut-off [V]": V_HOLD + 0.01,
        "Lower voltage cut-off [V]": 2.0,
    }, check_already_exists=False)
    pv = pv.set_initial_stoichiometries(0.0)

    steps = [f"Charge at {a.c_rate * q} A until {V_HOLD} V"]
    if not a.cc_only:
        steps.append(f"Hold at {V_HOLD} V for {HOLD_LIMIT_H} hours or until {TAPER_C * q} A")
    exp = pybamm.Experiment(steps, period="1 second")
    var_pts = dict(model.default_var_pts)
    var_pts["r_n"] = var_pts["r_p"] = a.r
    var_pts["x_n"], var_pts["x_s"], var_pts["x_p"] = (int(v) for v in a.x.split(","))
    sim = pybamm.Simulation(model, parameter_values=pv, var_pts=var_pts, experiment=exp,
                            solver=pybamm.IDAKLUSolver(rtol=a.rtol, atol=a.rtol))
    sol = sim.solve()

    L_n = pv["Negative electrode thickness [m]"]
    area = pv["Electrode height [m]"] * pv["Electrode width [m]"]
    F = pybamm.constants.F.value

    def ah(c_xav):  # x-averaged concentration -> A.h over the negative electrode
        return c_xav * L_n * area * F / 3600.0

    def read(s):
        t = s["Time [s]"].entries
        dead = ah(s["X-averaged negative dead lithium concentration [mol.m-3]"].entries)
        rev = ah(s["X-averaged negative lithium plating concentration [mol.m-3]"].entries)
        i_pl = (s["X-averaged negative electrode lithium plating volumetric interfacial current "
                  "density [A.m-3]"].entries * L_n * area)
        i = s["Current [A]"].entries
        dmin = s["Negative electrode surface potential difference [V]"].entries.min(axis=0)
        return t, s["Terminal voltage [V]"].entries, i, dead, rev, i_pl, dmin

    t, v, i, dead, rev, i_pl, dmin = read(sol)
    gross = dead / a.dead if a.dead > 0 else np.full_like(dead, np.nan)
    share = np.abs(i_pl) / np.maximum(np.abs(i), 1e-12)
    soc = np.concatenate([[0.0], np.cumsum(-0.5 * (i[1:] + i[:-1]) * np.diff(t))]) / (3600.0 * q)
    cc = sol.cycles[0].steps[0]
    n_cc = len(cc["Time [s]"].entries)
    cc_part = (f"cc: end t={t[n_cc - 1]:.1f} s soc={soc[n_cc - 1]:.4f} gross={gross[n_cc - 1]:.6e} Ah "
               f"net={dead[n_cc - 1] + rev[n_cc - 1]:.6e} Ah share_max={share[:n_cc].max():.3e}")
    tail = ""
    if not a.cc_only:
        tail = (f" | cccv: end t={t[-1]:.1f} s soc={soc[-1]:.4f} i_end={i[-1]:.4f} A "
                f"gross={gross[-1]:.6e} Ah net={dead[-1] + rev[-1]:.6e} Ah "
                f"share_max_cv={share[n_cc:].max():.3e} share_end={share[-1]:.3e}")
    print(f"{a.temp_c} C {a.c_rate}C k={a.k:g} d={a.dead} w={a.width:g} x={a.x} r={a.r} "
          f"smooth={a.smooth} | min eta={1e3 * dmin.min():.2f} mV | {cc_part}{tail} | "
          f"gross {100 * gross[-1] / q:.5f} % of {q} Ah | {sol.termination}")
    if a.out:
        with open(a.out, "w", encoding="utf-8", newline="\n") as f:
            f.write("t_s,v,i_a,soc,gross_ah,rev_ah,dead_ah,i_plating_a,eta_min_v\n")
            for kk in range(len(t)):
                f.write(f"{t[kk]:.3f},{v[kk]:.9f},{i[kk]:.9f},{soc[kk]:.9f},{gross[kk]:.9e},"
                        f"{rev[kk]:.9e},{dead[kk]:.9e},{i_pl[kk]:.9e},{dmin[kk]:.9f}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
