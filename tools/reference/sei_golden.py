"""Generate the SEI film golden: a year at rest in PyBaMM's SPM, interstitial-diffusion law.

    python tools/reference/sei_golden.py

Writes `tests/golden/nmc_21700_lgm50/sei_rest_year.csv`, which
`crates/sim-data/tests/sei_golden.rs` replays through the engine's `Spm` with the film
selected (Phase 10 slice C, exits 1-4; `docs/plans/phase-10-sei.md`).

Not shipped; requires PyBaMM (see requirements.txt). Never on the Rust/CI path.

What is run
-----------
PyBaMM's SPM with `{"SEI": "interstitial-diffusion limited"}` on Chen2020, at rest for 365
days, isothermal, at five conditions: SOC 85 % and 30 % at 25 and 45 C, and SOC 100 % at
25 C (the "stored full" arm of exit 3). The film's constants are read from the chemistry's
own `[spm.sei]` section, in the digits the engine reads, not from the fit's full precision,
so the reference and the engine run the same numbers.

Two things the earlier PyBaMM comparisons did not do, and why
-------------------------------------------------------------
* **The initial state is the engine's**, not `get_initial_stoichiometries(soc)`. The engine
  seeds a cell at `x = x_min + soc (x_max - x_min)` with the limits the chemistry file
  records; the film's rate goes as `exp(-F U_n(x) / RT)`, so a different starting `x` is a
  different rate, and the golden would score the seeding rather than the film.
* **Each condition is run twice**: once with Chen2020's own graphite OCP function, and once
  with the engine's piecewise-linear graphite table (`[spm.negative.ocp]`) as a linear
  `pybamm.Interpolant`. The rate is exponential in the potential, so the table's 1.90 mV
  interpolation error is several percent in the rate. The "table" run is the reference the
  engine's integrator is held to tightly; the "function" run measures what the table itself
  costs, which is the floor on how well the engine can match the physics.

Columns: soc, temp_k, day, li_lost_mol_function, li_lost_mol_table, l_function_m,
l_table_m. `li_lost_mol_*` is the lithium in the film grown since day 0 (the initial film's
lithium excluded), in moles.
"""

from __future__ import annotations

import sys
import tomllib
import warnings
from pathlib import Path

import numpy as np
import pybamm

warnings.filterwarnings("ignore")

ROOT = Path(__file__).resolve().parents[2]
CHEM = ROOT / "chemistries" / "nmc_21700_lgm50.toml"
OUT = ROOT / "tests" / "golden" / "nmc_21700_lgm50" / "sei_rest_year.csv"

CONDITIONS = [(0.85, 25.0), (0.85, 45.0), (0.30, 25.0), (0.30, 45.0), (1.00, 25.0)]
DAYS = 365


def chem() -> dict:
    with open(CHEM, "rb") as f:
        return tomllib.load(f)


def params(c: dict, soc: float, temp_k: float, table: bool) -> pybamm.ParameterValues:
    spm, sei = c["spm"], c["spm"]["sei"]
    neg, pos = spm["negative"], spm["positive"]
    p = pybamm.ParameterValues("Chen2020")
    x = neg["stoich_min"] + soc * (neg["stoich_max"] - neg["stoich_min"])
    y = pos["stoich_max"] - soc * (pos["stoich_max"] - pos["stoich_min"])
    p.update(
        {
            "SEI lithium interstitial diffusivity [m2.s-1]": sei["d_li_m2_per_s"],
            "SEI growth activation energy [J.mol-1]": sei["growth_ea_j_per_mol"],
            "SEI resistivity [Ohm.m]": sei["resistivity_ohm_m"],
            "Initial SEI thickness [m]": sei["initial_thickness_m"],
            "SEI partial molar volume [m3.mol-1]": sei["partial_molar_volume_m3_per_mol"],
            "Lithium interstitial reference concentration [mol.m-3]": sei[
                "c_li0_mol_per_m3"
            ],
            "Ratio of lithium moles to SEI moles": sei["li_per_sei"],
            "Reference temperature [K]": spm["t_ref_k"],
            "Ambient temperature [K]": temp_k,
            "Initial temperature [K]": temp_k,
            "Initial concentration in negative electrode [mol.m-3]": x
            * neg["c_max_mol_per_m3"],
            "Initial concentration in positive electrode [mol.m-3]": y
            * pos["c_max_mol_per_m3"],
        }
    )
    if table:
        xs = np.array(neg["ocp"]["stoich"])
        vs = np.array(neg["ocp"]["volts"])

        def u_neg(sto):
            return pybamm.Interpolant(xs, vs, sto, name="engine graphite table", interpolator="linear")

        p.update({"Negative electrode OCP [V]": u_neg})
    return p


def first(sol, names):
    for n in names:
        try:
            return np.asarray(sol[n].entries)
        except KeyError:
            continue
    sys.exit(f"none of {names} in the solution")


def run(c: dict, soc: float, tc: float, table: bool):
    temp_k = tc + 273.15
    p = params(c, soc, temp_k, table)
    model = pybamm.lithium_ion.SPM({"SEI": "interstitial-diffusion limited"})
    exp = pybamm.Experiment([pybamm.step.string(f"Rest for {DAYS * 24} hours", period="24 hours")])
    sol = pybamm.Simulation(model, parameter_values=p, experiment=exp).solve()
    t_day = np.asarray(sol["Time [s]"].entries) / 86400.0
    li = first(sol, ["Loss of lithium to negative SEI [mol]", "Loss of lithium to SEI [mol]"])
    ell = first(
        sol,
        [
            "X-averaged negative total SEI thickness [m]",
            "X-averaged negative SEI thickness [m]",
            "X-averaged total SEI thickness [m]",
            "X-averaged SEI thickness [m]",
        ],
    )
    return t_day, li - li[0], ell


def main() -> int:
    c = chem()
    print(f"pybamm {pybamm.__version__}")
    rows = []
    for soc, tc in CONDITIONS:
        t_f, li_f, l_f = run(c, soc, tc, table=False)
        t_t, li_t, l_t = run(c, soc, tc, table=True)
        if len(t_f) != len(t_t) or np.max(np.abs(t_f - t_t)) > 1e-9:
            sys.exit("the two runs reported on different days")
        print(
            f"SOC {soc:.2f}, {tc:.0f} C: lithium lost after a year  function {li_f[-1]:.6e} mol  "
            f"table {li_t[-1]:.6e} mol  table/function {li_t[-1] / li_f[-1]:.5f}"
        )
        for k in range(len(t_f)):
            rows.append((soc, tc + 273.15, t_f[k], li_f[k], li_t[k], l_f[k], l_t[k]))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(f"# generated by tools/reference/sei_golden.py, pybamm {pybamm.__version__}\n")
        f.write("# PyBaMM SPM, SEI interstitial-diffusion limited, Chen2020 + [spm.sei] of\n")
        f.write("# chemistries/nmc_21700_lgm50.toml, isothermal, a year at rest; initial\n")
        f.write("# stoichiometries the engine's (x_min + soc*(x_max-x_min)). 'function' = Chen2020's\n")
        f.write("# graphite OCP; 'table' = the engine's [spm.negative.ocp] as a linear Interpolant.\n")
        f.write("# li_lost_mol = lithium in film grown since day 0 [mol]; l = film thickness [m].\n")
        f.write("soc,temp_k,day,li_lost_mol_function,li_lost_mol_table,l_function_m,l_table_m\n")
        for r in rows:
            f.write(",".join(f"{v:.10g}" if isinstance(v, float) else str(v) for v in r) + "\n")
    print(f"wrote {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
