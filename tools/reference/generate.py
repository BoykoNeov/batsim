"""Generate batsim's committed golden-reference CSVs from PyBaMM runs.

    python tools/reference/generate.py            # all chemistries + scenarios
    python tools/reference/generate.py lfp_26650_generic

Writes one CSV per (chemistry, scenario) under `tests/golden/<chem_id>/`. Each
CSV carries a comment header recording the PyBaMM version, parameter set, and
scenario so a committed golden is reproducible against a known reference. The
Rust integration tests (crates/sim-data/tests/pybamm_golden.rs and spm_golden.rs)
replay the `current_a` column through `sim-core` and compare `voltage_v` within a
per-scenario, documented tolerance.

Columns: time_s, current_a (discharge-positive), voltage_v, soc.

Which reference model generates which golden is a per-scenario choice, because
batsim has two cell models:

  * the **LFP** scenarios are DFN references for batsim's ECM — a cross-model
    comparison whose irreducible gap the Rust test documents at length;
  * the **LG M50** `spm_*` scenarios are SPM references for batsim's SPM — the
    same model form on the same parameter set, so the comparison is of two
    implementations rather than of two models, and the tolerance is an order of
    magnitude tighter;
  * the **LG M50** `dfn_*` scenarios are DFN references for batsim's DFN, added in
    Phase 7. `dfn_cc_1c_25c.csv` is also the *same* scenario as `spm_cc_1c_25c.csv`,
    so the SPM-vs-DFN gap can be shown as the physics result it is (`spm_golden.rs`
    uses it that way and asserts nothing about batsim); `dfn_golden.rs` is where
    batsim's own DFN is asserted against both of them.
  * `dfn_cc_3c_25c.csv` is the depletion scenario: at 3C the electrolyte is driven
    to zero, which is the regime a single-particle model cannot represent at all.
    It carries Phase 7's exit criterion 2.

Not shipped; requires PyBaMM (see requirements.txt). Never on the Rust/CI path.
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

from common import (
    DFN_CONVERGED,
    DFN_DEFAULT,
    PARAM_SETS,
    SPM_CONVERGED,
    T_REF_K,
    fit_chemistry,
    run_cc_discharge,
    run_pulse_relax,
)

# Repo root = two levels up from this file (tools/reference/generate.py).
GOLDEN_DIR = Path(__file__).resolve().parents[2] / "tests" / "golden"

# chemistry id -> [(filename, human description, generator, reference, kwargs)].
# The generator returns (time_s, current_a, voltage_v, soc).
#
# Per-chemistry rather than one shared list, which it was through Phase 1: the two
# chemistries validate different cell models against different references, and a
# shared list would have meant generating SPM goldens for a chemistry with no [spm]
# section (LFP is ECM-only, deliberately — see the README's scope note).
SCENARIOS = {
    "lfp_26650_generic": [
        (
            "cc_c20_25c.csv",
            "C/20 constant-current discharge from full, isothermal 25 degC",
            run_cc_discharge,
            DFN_DEFAULT,
            {"c_rate": 1.0 / 20.0, "dt_s": 30.0},
        ),
        (
            "cc_1c_25c.csv",
            "1C constant-current discharge from full, isothermal 25 degC",
            run_cc_discharge,
            DFN_DEFAULT,
            {"c_rate": 1.0, "dt_s": 5.0},
        ),
        (
            "pulse_relax_25c.csv",
            "GITT-like C/2 discharge pulses with 20-min rests, isothermal 25 degC",
            run_pulse_relax,
            DFN_DEFAULT,
            {"c_rate": 0.5, "dt_s": 10.0},
        ),
    ],
    "nmc_21700_lgm50": [
        (
            "spm_cc_c5_25c.csv",
            "C/5 constant-current discharge from full, isothermal 25 degC",
            run_cc_discharge,
            SPM_CONVERGED,
            {"c_rate": 0.2, "dt_s": 30.0},
        ),
        (
            "spm_cc_1c_25c.csv",
            "1C constant-current discharge from full, isothermal 25 degC",
            run_cc_discharge,
            SPM_CONVERGED,
            {"c_rate": 1.0, "dt_s": 5.0},
        ),
        (
            "spm_pulse_relax_25c.csv",
            "GITT-like C/2 discharge pulses with 20-min rests, isothermal 25 degC",
            run_pulse_relax,
            SPM_CONVERGED,
            {"c_rate": 0.5, "dt_s": 10.0},
        ),
        (
            "dfn_cc_1c_25c.csv",
            "1C constant-current discharge from full, isothermal 25 degC "
            "(DFN, for the SPM-vs-DFN physics gap and batsim's own DFN)",
            run_cc_discharge,
            DFN_CONVERGED,
            {"c_rate": 1.0, "dt_s": 5.0},
        ),
        (
            "dfn_cc_3c_25c.csv",
            "3C constant-current discharge from full, isothermal 25 degC "
            "(DFN; the electrolyte depletes — an SPM cannot represent this at all)",
            run_cc_discharge,
            DFN_CONVERGED,
            {"c_rate": 3.0, "dt_s": 2.0},
        ),
    ],
}


def write_csv(path: Path, header_lines: list[str], t, i, v, soc) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="\n", encoding="utf-8") as f:
        for line in header_lines:
            f.write(f"# {line}\n")
        f.write("time_s,current_a,voltage_v,soc\n")
        for ti, ii, vi, si in zip(t, i, v, soc):
            f.write(f"{ti:.6f},{ii:.6f},{vi:.6f},{si:.6f}\n")


def generate_for(chem_id: str) -> None:
    fit = fit_chemistry(chem_id)
    print(f"{chem_id}: {fit.param_set} (pybamm {fit.pybamm_version}), "
          f"Q_use={fit.capacity_ah:.6f} A.h, "
          f"OCV table lin-err={fit.max_lin_err_v * 1e3:.2f} mV")
    for fname, desc, gen, ref, kwargs in SCENARIOS[chem_id]:
        t, i, v, soc = gen(chem_id, fit, ref=ref, **kwargs)
        header = [
            f"batsim golden reference - {chem_id} - {fname}",
            f"scenario: {desc}",
            f"source: PyBaMM {fit.param_set} {ref.kind} (isothermal {T_REF_K:.2f} K), "
            f"pybamm {fit.pybamm_version}",
        ]
        # Only emitted when a scenario overrides PyBaMM's defaults, so a golden
        # generated on them keeps the exact header Phase 1 committed.
        if ref.r_pts is not None:
            header.append(
                f"particle grid: r_n = r_p = {ref.r_pts} points "
                "(converged; see common.Reference for the measurement)"
            )
        if ref.rtol is not None:
            header.append(
                f"solver: IDAKLU, rtol = {ref.rtol:g}, atol = {ref.atol:g} "
                "(PyBaMM's default 1e-6 moves the knee by tens of mV)"
            )
        header += [
            f"capacity_ah (usable, stoichiometry window) = {fit.capacity_ah:.6f}",
            "sign: positive current = discharge (batsim convention)",
            "generated by tools/reference/generate.py - do not edit by hand",
        ]
        out = GOLDEN_DIR / chem_id / fname
        write_csv(out, header, t, i, v, soc)
        print(f"  wrote {out.relative_to(GOLDEN_DIR.parent.parent)} "
              f"({len(t)} rows, V {np.min(v):.3f}..{np.max(v):.3f})")


def main(argv: list[str]) -> int:
    if len(argv) == 1:
        targets = list(SCENARIOS)
    else:
        targets = argv[1:]
        for t in targets:
            if t not in SCENARIOS or t not in PARAM_SETS:
                print(f"unknown chemistry: {t}", file=sys.stderr)
                return 2
    for chem_id in targets:
        generate_for(chem_id)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
