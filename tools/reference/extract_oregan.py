"""Print ORegan2022's temperature dependence for the LG M50, as additions to its Chen2020 file.

    python tools/reference/extract_oregan.py

Phase 11 slice C (docs/plans/phase-11-fitting.md, decisions 2 and 3). The LG M50 file's
porous sections are Chen2020, which publishes no entropy coefficient (both keys are literally
0) and no temperature dependence for solid diffusion or the electrolyte. ORegan2022 is
PyBaMM's thermal parameterisation of the same cell and publishes all three. This prints, for
pasting into `chemistries/nmc_21700_lgm50.toml`:

  * `[spm.negative.entropic]` / `[spm.positive.entropic]` — ORegan2022's dU/dT functions,
    tabulated over the full stoichiometry range to a stated tolerance, read on Chen2020's
    stoichiometry (the spike measured mapping them through ORegan2022's own window instead
    moves the cell coefficient by at most 0.07 mV/K);
  * the circuit's `[ocv] docv_dt_v_per_k` column, the same pair on the cell's window at each
    of the `[ocv]` table's nodes, with `t_ref_k`;
  * `diffusivity_ea_j_per_mol` for each electrode, `d * R` read out of ORegan2022's function
    source (fit_ecm.oregan_diffusion_d);
  * `[dfn.electrolyte_temperature]` — Landesfeind 2019's EC:EMC 3:7 coefficients, read out of
    the function source and checked against PyBaMM's own evaluation.

Every number is read from or evaluated by PyBaMM; nothing here is fitted except the table
breakpoints, whose tolerance is printed. Not shipped; never on the Rust/CI path.
"""

from __future__ import annotations

import inspect
import re
import sys
import tomllib
import warnings
from pathlib import Path

import numpy as np

warnings.filterwarnings("ignore")
sys.path.insert(0, str(Path(__file__).resolve().parent))

import pybamm  # noqa: E402

from fit_ecm import GAS_CONSTANT, oregan_diffusion_d  # noqa: E402

CHEM = Path(__file__).resolve().parents[2] / "chemistries" / "nmc_21700_lgm50.toml"
TOL_V_PER_K = 2.0e-6  # table breakpoints: 2 uV/K, ~0.07 mV of potential at 35 K from t_ref
T_REF_K = 298.15


def _entropic_fn(side: str):
    orv = pybamm.ParameterValues("ORegan2022")
    fn = orv[f"{side} electrode OCP entropic change [V.K-1]"]
    return lambda s: float(orv.evaluate(fn(pybamm.Scalar(s))))


def _table(f, tol: float):
    """Greedy bisection to `tol` against a dense reference, extract_spm._ocp_table's rule."""
    dense = np.linspace(0.0, 1.0, 4001)
    dv = np.array([f(s) for s in dense])
    grid = list(np.linspace(0.0, 1.0, 9))
    while len(grid) < 400:
        err = np.abs(dv - np.interp(dense, grid, np.interp(grid, dense, dv)))
        if float(err.max()) <= tol:
            break
        worst = dense[int(np.argmax(err))]
        i = min(max(int(np.searchsorted(grid, worst)), 1), len(grid) - 1)
        grid.insert(i, worst if grid[i - 1] < worst < grid[i] else 0.5 * (grid[i - 1] + grid[i]))
    xs = np.round(np.array(grid), 6)
    xs = np.unique(xs)
    ys = np.array([f(s) for s in xs])
    err = float(np.max(np.abs(dv - np.interp(dense, xs, ys))))
    return xs, ys, err


def _landesfeind(key: str, n: int):
    orv = pybamm.ParameterValues("ORegan2022")
    src = inspect.getsource(orv[key])
    m = re.search(r"coeffs\s*=\s*np\.array\(\[([^\]]*)\]\)", src)
    if m is None or "Landesfeind2019" not in src:
        raise RuntimeError(f"ORegan2022's {key} is no longer Landesfeind 2019's form")
    coeffs = [float(x) for x in m.group(1).split(",")]
    if len(coeffs) != n:
        raise RuntimeError(f"{key}: {len(coeffs)} coefficients, expected {n}")
    return coeffs, orv[key]


def main() -> int:
    chem = tomllib.loads(CHEM.read_text(encoding="utf-8"))
    spm = chem["spm"]
    neg, pos = _entropic_fn("Negative"), _entropic_fn("Positive")

    print(f"# --- ORegan2022 (pybamm {pybamm.__version__}) temperature dependence, by "
          "tools/reference/extract_oregan.py ---")
    for name, f in (("negative", neg), ("positive", pos)):
        xs, ys, err = _table(f, TOL_V_PER_K)
        # Cross-check at 7 interior points against PyBaMM's own evaluation.
        for s in np.linspace(0.05, 0.95, 7):
            got = float(np.interp(s, xs, ys))
            assert abs(got - f(s)) <= TOL_V_PER_K * 1.0001, (name, s, got, f(s))
        side = "Negative" if name == "negative" else "Positive"
        print(f"# [spm.{name}.entropic]: ORegan2022 \"{side} electrode OCP entropic change "
              f"[V.K-1]\" over the full range, {len(xs)} breakpoints, max interpolation "
              f"error {err * 1e6:.2f} uV/K (cross-checked at 7 points)")
        print(f"[spm.{name}.entropic]")
        print("stoich  = [" + ", ".join(f"{x:.6f}" for x in xs) + "]")
        print("v_per_k = [" + ", ".join(f"{y:.6e}" for y in ys) + "]")
        print()

    d = oregan_diffusion_d()
    for el in ("Negative", "Positive"):
        print(f"# spm.{el.lower()}.diffusivity_ea_j_per_mol = {d[el] * GAS_CONSTANT:.6g}"
              f"   # ORegan2022: E_D_s = d * R, d = {d[el]:g}")
    print()

    # The circuit's column: dU_p/dT(y(soc)) - dU_n/dT(x(soc)) at each [ocv] node.
    n, p = spm["negative"], spm["positive"]
    col = []
    for z in chem["ocv"]["soc"]:
        x = n["stoich_min"] + z * (n["stoich_max"] - n["stoich_min"])
        y = p["stoich_max"] - z * (p["stoich_max"] - p["stoich_min"])
        col.append(pos(y) - neg(x))
    print("# [ocv] additions: the cell coefficient on the window, one value per soc node")
    print("docv_dt_v_per_k = [" + ", ".join(f"{v:.6e}" for v in col) + "]")
    print(f"t_ref_k = {T_REF_K}")
    print()

    dk, d_fn = _landesfeind("Electrolyte diffusivity [m2.s-1]", 4)
    kk, k_fn = _landesfeind("Electrolyte conductivity [S.m-1]", 6)
    orv = pybamm.ParameterValues("ORegan2022")
    for c in (200.0, 1000.0, 2000.0):
        for t in (263.15, 298.15, 318.15):
            # The ratio the engine forms, against PyBaMM's own evaluation of the function.
            ce, cr = c / 1000.0, None
            want = float(orv.evaluate(d_fn(pybamm.Scalar(c), pybamm.Scalar(t)))
                         / orv.evaluate(d_fn(pybamm.Scalar(c), pybamm.Scalar(T_REF_K))))
            got = float(np.exp((dk[2] + dk[3] * ce) * (1 / t - 1 / T_REF_K)))
            assert abs(got / want - 1) < 1e-12, ("diffusivity", c, t, got, want)
    print("# Landesfeind 2019 (J. Electrochem. Soc. 166 (14) A3079), EC:EMC 3:7, as ORegan2022 "
          "uses them; read out of the function source and checked against PyBaMM's own "
          "evaluation")
    print("[dfn.electrolyte_temperature]")
    print('form         = "Landesfeind2019"')
    print(f"t_ref_k      = {T_REF_K}")
    print("diffusivity  = [" + ", ".join(repr(v) for v in dk) + "]")
    print("conductivity = [" + ", ".join(repr(v) for v in kk) + "]")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
