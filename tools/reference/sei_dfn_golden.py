"""Generate the SEI film's DFN golden: the film under cycling in PyBaMM's DFN.

    python tools/reference/sei_dfn_golden.py

Writes `tests/golden/nmc_21700_lgm50/sei_dfn_cycling.csv`, which
`crates/sim-data/tests/sei_dfn.rs` replays through the engine's `Dfn` with the film selected
(Phase 10 slice D, exit 2; `docs/plans/phase-10-sei.md`, slice D addendum).

Not shipped; requires PyBaMM (see requirements.txt). Never on the Rust/CI path.

What is run
-----------
PyBaMM's DFN with `{"SEI": "interstitial-diffusion limited"}` on Chen2020, isothermal at
25 C (the constants' reference temperature, so no Arrhenius factor enters either side), from
50 % SOC at the engine's own stoichiometries: eight cycles of a discharge then a charge, each
moving a quarter of the cell's capacity, at 1C and at 2C (clear of the 3C electrolyte limit).
The current is given in amps from the chemistry file's capacity, so both sides run the same
amps for the same seconds.

* **On the engine's grid**: 10 / 5 / 10 nodes across the cell and 20 shells per particle,
  the engine's `DEFAULT_NODES_*` and `DEFAULT_SHELLS`.
* **On the engine's OCP tables**, both electrodes as linear Interpolants, so the tables'
  1.9 mV interpolation error is not in the comparison.
* **From one hand-set even 50 nm film.** PyBaMM's film resistance is the whole film's,
  `rho L j`; the engine's is its growth past the chemistry's initial thickness. So the engine
  side sets that initial thickness to 1e-12 and starts from 50 nm at every node, and both
  sides compare growth since the start.
* **Twice per rate**: once with the film frozen (`D_li` = 0), which measures how far apart
  the two DFNs' negative surface potential differences sit with the film's resistance present
  and nothing growing — the floor the growing comparison is held to — and once growing, at
  the chemistry's own `D_li`.

Solved at 1e-10 (slice C: PyBaMM's default under-grew the film in the first days).

Columns: rate_c, growing (0/1), t_s, li_lost_mol (grown since t = 0), then dphi_0..9 (the
negative's phi_s - phi_e per node [V], collector first) and dl_0..9 (film grown per node
since t = 0 [m]).
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
OUT = ROOT / "tests" / "golden" / "nmc_21700_lgm50" / "sei_dfn_cycling.csv"

SOC0 = 0.5
TEMP_K = 298.15
L_START = 50e-9
CYCLES = 8
RATES = [1.0, 2.0]
SOLVER_TOL = 1e-10
GRID = {"x_n": 10, "x_s": 5, "x_p": 10, "r_n": 20, "r_p": 20}


def chem() -> dict:
    with open(CHEM, "rb") as f:
        return tomllib.load(f)


def table(e: dict, name: str):
    xs = np.array(e["ocp"]["stoich"])
    vs = np.array(e["ocp"]["volts"])
    order = np.argsort(xs)
    xs, vs = xs[order], vs[order]

    def u(sto):
        return pybamm.Interpolant(xs, vs, sto, name=name, interpolator="linear")

    return u


def params(c: dict, growing: bool) -> pybamm.ParameterValues:
    spm, sei = c["spm"], c["spm"]["sei"]
    neg, pos = spm["negative"], spm["positive"]
    p = pybamm.ParameterValues("Chen2020")
    x = neg["stoich_min"] + SOC0 * (neg["stoich_max"] - neg["stoich_min"])
    y = pos["stoich_max"] - SOC0 * (pos["stoich_max"] - pos["stoich_min"])
    p.update(
        {
            "SEI lithium interstitial diffusivity [m2.s-1]": sei["d_li_m2_per_s"] if growing else 0.0,
            "SEI growth activation energy [J.mol-1]": sei["growth_ea_j_per_mol"],
            "SEI resistivity [Ohm.m]": sei["resistivity_ohm_m"],
            "Initial SEI thickness [m]": L_START,
            "SEI partial molar volume [m3.mol-1]": sei["partial_molar_volume_m3_per_mol"],
            "Lithium interstitial reference concentration [mol.m-3]": sei["c_li0_mol_per_m3"],
            "Ratio of lithium moles to SEI moles": sei["li_per_sei"],
            "Reference temperature [K]": spm["t_ref_k"],
            "Ambient temperature [K]": TEMP_K,
            "Initial temperature [K]": TEMP_K,
            "Initial concentration in negative electrode [mol.m-3]": x * neg["c_max_mol_per_m3"],
            "Initial concentration in positive electrode [mol.m-3]": y * pos["c_max_mol_per_m3"],
            "Negative electrode OCP [V]": table(neg, "engine graphite table"),
            "Positive electrode OCP [V]": table(pos, "engine NMC table"),
        }
    )
    return p


def first(sol, names):
    for n in names:
        try:
            return np.asarray(sol[n].entries)
        except KeyError:
            continue
    sys.exit(f"none of {names} in the solution")


def run(c: dict, rate: float, growing: bool):
    cap = c["cell"]["capacity_ah"]
    amps = rate * cap
    minutes = 15.0 / rate
    steps = []
    for _ in range(CYCLES):
        steps.append(pybamm.step.current(amps, duration=f"{minutes} minutes", period="60 seconds"))
        steps.append(pybamm.step.current(-amps, duration=f"{minutes} minutes", period="60 seconds"))
    model = pybamm.lithium_ion.DFN({"SEI": "interstitial-diffusion limited"})
    solver = pybamm.CasadiSolver(rtol=SOLVER_TOL, atol=SOLVER_TOL, mode="safe")
    sim = pybamm.Simulation(
        model,
        parameter_values=params(c, growing),
        experiment=pybamm.Experiment(steps),
        solver=solver,
        var_pts=GRID,
    )
    sol = sim.solve()
    t = np.asarray(sol["Time [s]"].entries)
    li = first(sol, ["Loss of lithium to negative SEI [mol]", "Loss of lithium to SEI [mol]"])
    dphi = np.asarray(sol["Negative electrode surface potential difference [V]"].entries)
    ell = first(sol, ["Negative total SEI thickness [m]", "Negative SEI thickness [m]"])
    return t, li - li[0], dphi, ell - ell[:, :1]


def main() -> int:
    c = chem()
    print(f"pybamm {pybamm.__version__}")
    rows = []
    for rate in RATES:
        for growing in (False, True):
            t, li, dphi, dl = run(c, rate, growing)
            # The experiment's step boundaries repeat a time; keep the first of each.
            _, keep = np.unique(np.round(t, 6), return_index=True)
            print(
                f"{rate}C growing={growing}: {len(keep)} samples, lithium {li[-1]:.6e} mol, "
                f"spread {dl[-1, -1] / max(dl[-1, 0], 1e-300):.4f}"
            )
            for k in keep:
                rows.append([rate, int(growing), t[k], li[k], *dphi[:, k], *dl[:, k]])
    OUT.parent.mkdir(parents=True, exist_ok=True)
    n = GRID["x_n"]
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(
            f"# generated by tools/reference/sei_dfn_golden.py, pybamm {pybamm.__version__}, "
            f"CasadiSolver rtol=atol={SOLVER_TOL:g}\n"
        )
        f.write("# PyBaMM DFN, SEI interstitial-diffusion limited, Chen2020 + [spm.sei] of\n")
        f.write("# chemistries/nmc_21700_lgm50.toml, both OCPs the engine's tables, grid 10/5/10\n")
        f.write("# r 20; isothermal 25 C from 50 % SOC; 8 cycles of +-0.25 capacity; film from\n")
        f.write("# an even 50 nm. growing=0: D_li = 0. li_lost_mol and dl_* grown since t = 0.\n")
        f.write(
            "rate_c,growing,t_s,li_lost_mol,"
            + ",".join(f"dphi_{i}" for i in range(n))
            + ","
            + ",".join(f"dl_{i}" for i in range(n))
            + "\n"
        )
        for r in rows:
            f.write(",".join(f"{v:.12g}" for v in r) + "\n")
    print(f"wrote {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
