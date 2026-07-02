"""Print a TOML-ready [ocv] block + usable capacity for a batsim chemistry.

Run this once when (re)deriving a chemistry's open-circuit-voltage table from its
PyBaMM parameter set, then paste the emitted block into the matching
`chemistries/*.toml`. This is what makes each chemistry's "Fitted to PyBaMM ..."
provenance literally true.

    python tools/reference/fit_ocv.py lfp_26650_generic
    python tools/reference/fit_ocv.py nmc_18650_generic

Not shipped; requires PyBaMM (see requirements.txt).
"""

from __future__ import annotations

import sys

from common import PARAM_SETS, fit_chemistry


def main(argv: list[str]) -> int:
    if len(argv) != 2 or argv[1] not in PARAM_SETS:
        ids = ", ".join(sorted(PARAM_SETS))
        print(f"usage: python fit_ocv.py <chem_id>   (one of: {ids})", file=sys.stderr)
        return 2

    chem_id = argv[1]
    fit = fit_chemistry(chem_id)

    soc_str = ", ".join(f"{s:.4f}" for s in fit.soc)
    volts_str = ", ".join(f"{v:.4f}" for v in fit.ocv)

    print(f"# --- fitted from PyBaMM {fit.param_set} "
          f"(pybamm {fit.pybamm_version}) by tools/reference/fit_ocv.py ---")
    print(f"# usable capacity between stoichiometry limits = "
          f"{fit.capacity_ah:.6f} A.h")
    print(f"# lower/upper voltage cut-off = {fit.v_min:.3f} / {fit.v_max:.3f} V")
    print(f"# max piecewise-linear interpolation error of this table = "
          f"{fit.max_lin_err_v * 1e3:.2f} mV")
    print()
    print(f"capacity_ah = {fit.capacity_ah:.6f}")
    print()
    print("[ocv]")
    print(f"soc   = [{soc_str}]")
    print(f"volts = [{volts_str}]")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
