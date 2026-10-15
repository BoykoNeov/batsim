"""Fit the LG M50 equivalent circuit (`[r0]`, `[[rc]]`) to a PyBaMM DFN pulse train.

    python tools/reference/fit_ecm.py --generate    # run the DFN references, write the CSVs
    python tools/reference/fit_ecm.py --fit         # fit from the committed CSVs, print TOML

Phase 11 slice A (docs/plans/phase-11-fitting.md). Not shipped, never on the Rust/CI path.

## The reference

An HPPC-style train on PyBaMM's DFN, isothermal: from full, at each 10 % of charge, a 30 s
discharge pulse at 1C, a 300 s rest, a 30 s charge pulse at 0.75C, a 300 s rest, then a C/2
move of 10 % and a 1800 s rest (the spike measured the voltage still moving at most 0.07 mV
over the last 60 s of those). Solved converged, as every LG M50 golden is (`DFN_CONVERGED`:
200 radial points, `rtol = atol = 1e-10`).

The parameter set is **Chen2020 with two of ORegan2022's temperature dependences added**,
`combined_parameter_values`: solid diffusion's Arrhenius factor (ORegan2022's activation
energies, applied to Chen2020's constant diffusivities) and the electrolyte's (Chen2020's
Nyman 2008 fits times Landesfeind 2019's ratio to its own value at 298.15 K). At 298.15 K both
factors are exactly 1, so the 25 degC reference is Chen2020's own DFN. Away from it, this is
the cell all three of batsim's models are meant to describe (owner decision 2 of the plan),
and slice C gives the engine's `Dfn` the same two terms.

## The fit

The circuit is simulated the engine's way: each RC pair's exact exponential update under the
sample's piecewise-constant current, and the terminal voltage at the end of the sample,
`V = OCV(soc) - I*R0(soc) - sum V_rc`, against the **reference's own OCV** along the coulomb
count (`U_p(y) - U_n(x)` evaluated by PyBaMM), so the `[ocv]` table's interpolation error does
not land in the resistances. The pairs are shared across temperatures, because `[[rc]]` holds
plain numbers; the temperature lives in `R0` alone. Fitted at the design rate, 1C; the 3C run
is scored, not fitted.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np

GOLDEN_DIR = Path(__file__).resolve().parents[2] / "tests" / "golden" / "nmc_21700_lgm50"
CHEM_ID = "nmc_21700_lgm50"
T_REF_K = 298.15
GAS_CONSTANT = 8.314462618  # J/(mol K), the value PyBaMM's constants module uses

# ORegan2022's solid-diffusion activation energies are written `E_D_s = d * R` inside
# graphite_LGM50_diffusivity_ORegan2022 (d = 2092) and nmc_LGM50_diffusivity_ORegan2022
# (d = 1449). Read out of the function source by `oregan_diffusion_d` and checked against
# these, so a set upgrade raises rather than silently moving the reference.
OREGAN_DIFFUSION_D = {"Negative": 2092.0, "Positive": 1449.0}

# The train, as (label, current in C, seconds, sample period). Rests are split so the first
# minute after every current change is sampled every second and the rest coarser: the pulses
# and their first relaxation are where the fit lives, and a uniform 1 s grid over 8 h made
# 29 000-row files for no information.
LEVELS = 10  # pulse levels at 100 %, 90 %, ..., 10 %


def train(pulse_c: float):
    # Ten seconds at rest first, so row 0 is the rested cell at full (V = OCV(1.0)), as in
    # every other golden, rather than PyBaMM's loaded first instant.
    steps = [("rest", 0.0, 10, 1)]
    for k in range(LEVELS):
        steps += [
            ("dis_pulse", pulse_c, 30, 1),
            ("rest", 0.0, 60, 1), ("rest", 0.0, 240, 10),
            ("chg_pulse", -0.75 * pulse_c, 30, 1),
            ("rest", 0.0, 60, 1), ("rest", 0.0, 240, 10),
        ]
        if k < LEVELS - 1:
            steps += [
                ("move", 0.5, 720, 10),
                ("rest", 0.0, 60, 1), ("rest", 0.0, 1740, 30),
            ]
    return steps


RUNS = [
    # file, temperature [K], pulse C-rate
    ("ecm_hppc_1c_25c.csv", 298.15, 1.0),
    ("ecm_hppc_3c_25c.csv", 298.15, 3.0),
    ("ecm_hppc_1c_m10c.csv", 263.15, 1.0),
    ("ecm_hppc_1c_45c.csv", 318.15, 1.0),
]


# ----------------------------------------------------------------------------------------
# Generation (needs PyBaMM)


def oregan_diffusion_d():
    """ORegan2022's `d` in `E_D_s = d * R`, parsed from the function source."""
    import inspect
    import re

    import pybamm

    pv = pybamm.ParameterValues("ORegan2022")
    out = {}
    for el in ("Negative", "Positive"):
        src = inspect.getsource(pv[f"{el} particle diffusivity [m2.s-1]"])
        m = re.search(r"^\s*d\s*=\s*([0-9.eE+-]+)\s*$", src, re.M)
        if m is None or "E_D_s = d * pybamm.constants.R" not in src:
            raise RuntimeError(f"ORegan2022's {el} diffusivity no longer reads `E_D_s = d * R`")
        out[el] = float(m.group(1))
    if out != OREGAN_DIFFUSION_D:
        raise RuntimeError(f"ORegan2022's activation energies moved: {out} != {OREGAN_DIFFUSION_D}")
    return out


def combined_parameter_values(temp_k: float):
    """Chen2020 + ORegan2022's solid-diffusion and electrolyte temperature dependence."""
    import pybamm

    pv = pybamm.ParameterValues("Chen2020")
    orv = pybamm.ParameterValues("ORegan2022")
    for el, d in oregan_diffusion_d().items():
        d0 = pv[f"{el} particle diffusivity [m2.s-1]"]
        if callable(d0):
            raise RuntimeError(f"Chen2020's {el} diffusivity is no longer a constant")
        e = d * GAS_CONSTANT

        def diffusivity(sto, T, d0=d0, e=e):
            return d0 * np.exp(e / GAS_CONSTANT * (1.0 / T_REF_K - 1.0 / T))

        pv.update({f"{el} particle diffusivity [m2.s-1]": diffusivity})
    for key in ("Electrolyte diffusivity [m2.s-1]", "Electrolyte conductivity [S.m-1]"):
        g, h = pv[key], orv[key]

        def transport(c_e, T, g=g, h=h):
            return g(c_e, T) * h(c_e, T) / h(c_e, pybamm.Scalar(T_REF_K))

        pv.update({key: transport})
    pv.update({
        "Ambient temperature [K]": temp_k,
        "Initial temperature [K]": temp_k,
        # The train never reaches the set's own cut-offs except by a pulse at the bottom
        # level, and an experiment that stops there drops every level after it.
        "Lower voltage cut-off [V]": 2.0,
        "Upper voltage cut-off [V]": 4.4,
    })
    return pv


def generate(only=None):
    import pybamm

    from common import DFN_CONVERGED, fit_chemistry

    fit = fit_chemistry(CHEM_ID)
    q = fit.capacity_ah
    for name, temp_k, pulse_c in RUNS:
        if only and name != only:
            continue
        model = pybamm.lithium_ion.DFN(options={"thermal": "isothermal"})
        pv = combined_parameter_values(temp_k).set_initial_stoichiometries(1.0)
        var_pts = dict(model.default_var_pts)
        var_pts["r_n"] = var_pts["r_p"] = DFN_CONVERGED.r_pts
        steps = train(pulse_c)
        exp = []
        for _, c, secs, period in steps:
            if c > 0:
                verb = f"Discharge at {c * q:.6f} A"
            elif c < 0:
                verb = f"Charge at {-c * q:.6f} A"
            else:
                verb = "Rest"
            exp.append(f"{verb} for {secs} seconds ({period} second period)")
        sim = pybamm.Simulation(
            model, parameter_values=pv, experiment=pybamm.Experiment(exp), var_pts=var_pts,
            solver=pybamm.IDAKLUSolver(rtol=DFN_CONVERGED.rtol, atol=DFN_CONVERGED.atol))
        sol = sim.solve()
        t = sol["Time [s]"].entries
        v = sol["Terminal voltage [V]"].entries
        i = sol["Current [A]"].entries
        total = sum(s[2] for s in steps)
        if abs(t[-1] - total) > 1e-6:
            raise RuntimeError(f"{name}: ended at {t[-1]} s of {total} ({sol.termination})")
        # One row per sample time. PyBaMM repeats the boundary instant at the start of
        # every step with the new current applied; the row kept is the step's END value,
        # which is the sample the engine reports for the interval ending there.
        keep = np.ones(len(t), bool)
        keep[1:] = np.diff(t) > 1e-6
        t, v, i = t[keep], v[keep], i[keep]
        # The current over (t[k-1], t[k]] is the step's own; read it at the interval's
        # midpoint from the schedule rather than trusting a boundary sample.
        edges = np.cumsum([0] + [s[2] for s in steps])
        amps = np.array([s[1] * q for s in steps])
        mid = 0.5 * (t[1:] + t[:-1])
        cur = np.concatenate([[0.0], amps[np.searchsorted(edges, mid, side="right") - 1]])
        soc = 1.0 - np.cumsum(np.concatenate([[0.0], np.diff(t)]) * cur) / (3600.0 * q)
        path = GOLDEN_DIR / name
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write(f"# batsim golden reference - {CHEM_ID} - {name}\n")
            f.write(f"# scenario: HPPC-style pulse train at {pulse_c:g}C, isothermal "
                    f"{temp_k - 273.15:.2f} degC (Phase 11 slice A: the circuit fit)\n")
            f.write("# source: PyBaMM Chen2020 DFN + ORegan2022 solid-diffusion and "
                    "electrolyte temperature factors (identically 1 at 298.15 K), "
                    f"pybamm {pybamm.__version__}; tools/reference/fit_ecm.py\n")
            f.write(f"# grid: r_n = r_p = {DFN_CONVERGED.r_pts}; solver: IDAKLU, rtol = "
                    f"atol = {DFN_CONVERGED.rtol:g}\n")
            f.write("time_s,current_a,voltage_v,soc\n")
            for row in zip(t, cur, v, soc):
                f.write(f"{row[0]:.3f},{row[1]:.9f},{row[2]:.9f},{row[3]:.9f}\n")
        print(f"wrote {path} ({len(t)} rows)", flush=True)


# ----------------------------------------------------------------------------------------
# The fit (needs PyBaMM only for the reference OCV)


@dataclass
class Run:
    name: str
    temp_k: float
    pulse_c: float
    t: np.ndarray
    i: np.ndarray
    v: np.ndarray
    soc: np.ndarray
    ocv: np.ndarray
    segment: np.ndarray  # 0 rest, 1 move, 2 discharge pulse, 3 charge pulse


def reference_ocv():
    """The reference cell's OCV(soc) on a dense grid: U_p(y) - U_n(x), Chen2020's windows."""
    import pybamm

    pv = pybamm.ParameterValues("Chen2020")
    xmin, xmax, ymin, ymax = pybamm.lithium_ion.get_min_max_stoichiometries(pv)
    u_n = pv["Negative electrode OCP [V]"]
    u_p = pv["Positive electrode OCP [V]"]
    z = np.linspace(0.0, 1.0, 4001)
    ocv = np.array([
        float(pv.evaluate(u_p(pybamm.Scalar(ymax + s * (ymin - ymax))))
              - pv.evaluate(u_n(pybamm.Scalar(xmin + s * (xmax - xmin)))))
        for s in z
    ])
    return z, ocv


def load(name, temp_k, pulse_c, zg, og):
    lines = (GOLDEN_DIR / name).read_text(encoding="utf-8").splitlines()
    body = [ln for ln in lines if ln and not ln.startswith("#")][1:]  # [0] is the header
    d = np.array([[float(x) for x in ln.split(",")] for ln in body])
    t, i, v, soc = d.T
    q = 5.153198  # only to classify segments by C-rate
    c = i / q
    seg = np.zeros(len(t), int)
    seg[np.isclose(c, 0.5, atol=1e-6)] = 1
    seg[c > 0.5 + 1e-6] = 2
    seg[c < -1e-6] = 3
    return Run(name, temp_k, pulse_c, t, i, v, soc, np.interp(soc, zg, og), seg)


def simulate(run: Run, nodes, pairs, ref_nodes=None):
    """The engine's ECM over the run's samples.

    `nodes` is the run temperature's `R0` column over `R0_SOC`. Each sample is one engine
    step: `R0`, and with `ref_nodes` the RC pairs' temperature factor `R0(soc, T) /
    R0(soc, T_ref)` (`[r0] rc_t_ref_k`, resistance and time constant scaled together), are
    read at the **start** of the step — the previous sample's charge — as the engine reads
    them; the OCV and the RC voltages at its end.
    """
    dt = np.diff(run.t, prepend=run.t[0])
    soc_start = np.concatenate([[run.soc[0]], run.soc[:-1]])
    r0 = np.interp(soc_start, R0_SOC, nodes)
    f = np.ones_like(r0) if ref_nodes is None else r0 / np.interp(soc_start, R0_SOC, ref_nodes)
    vrc = np.zeros_like(run.v)
    for r, tau in pairs:
        dec = np.exp(-dt / (tau * f))
        amp = r * f * (1.0 - dec)
        x = 0.0
        out = np.empty_like(run.v)
        for k in range(len(dt)):
            x = x * dec[k] + amp[k] * run.i[k]
            out[k] = x
        vrc += out
    return run.ocv - run.i * r0 - vrc


R0_SOC = np.round(np.linspace(0.1, 1.0, 10), 1)  # the pulse levels; below 10 % nothing is
# measured, and the engine's grid lookup holds the 10 % value there (clamped), which is what
# the provenance line says rather than a fitted number at empty.
LAMBDAS = [0.0, 1e-3, 3e-3, 1e-2, 3e-2, 1e-1, 3e-1, 1.0]
PULSE_RMS_SLACK = 1.10  # the smoothness rule, fixed before any fit was seen (see `fit`)


def _weights(run):
    """Each row's share of time: the interval ending at it. A fit weighted per ROW would
    depend on how densely the CSV happens to be sampled (1 s around pulses, up to 30 s in
    long rests); weighted per SECOND it is the reference's own error over time, whatever
    the sampling. Measured: per-row weighting moved the pairs from 40/333 s to 16/115 s."""
    return np.diff(run.t, prepend=run.t[0])


def _rms(e, w, m=None):
    if m is None:
        m = np.ones(len(e), bool)
    return float(np.sqrt(np.sum(w[m] * e[m] ** 2) / np.sum(w[m])))


def _seg_rms(run, e, seg):
    return _rms(e, _weights(run), run.segment == seg)


def _residuals(run, nodes, pairs, lam, ref_nodes=None):
    w = _weights(run)
    from_v = (simulate(run, nodes, pairs, ref_nodes) - run.v) * np.sqrt(w)
    # The smoothness penalty, in volts so it shares the residual's units: the second
    # difference of R0 across the nodes times the 1C current, scaled by sqrt(lambda * T).
    d2 = np.diff(nodes, 2) * 5.153198
    return np.concatenate([from_v, np.sqrt(lam * float(np.sum(w))) * d2])


def fit_warm(run, lam):
    from scipy.optimize import least_squares

    def res(p):
        p = np.exp(p)
        return _residuals(run, p[:10], [(p[10], p[11]), (p[12], p[13])], lam)

    p0 = np.log(np.concatenate([np.full(10, 0.025), [0.015, 40.0, 0.01, 300.0]]))
    sol = least_squares(res, p0, method="trf", x_scale=1.0, max_nfev=2000)
    p = np.exp(sol.x)
    return p[:10], [(p[10], p[11]), (p[12], p[13])]


def fit_r0_only(run, pairs, lam, start, ref_nodes):
    """One temperature's `R0` column with the 25 degC pairs held, and scaled through
    `ref_nodes` by the column being fitted — so the RC rule is inside the fit, not a
    correction applied after it."""
    from scipy.optimize import least_squares

    sol = least_squares(lambda p: _residuals(run, np.exp(p), pairs, lam, ref_nodes),
                        np.log(start), method="trf", max_nfev=2000)
    return np.exp(sol.x)


def report(run, nodes, pairs, label, ref_nodes=None):
    """Time-weighted RMS per segment, and above 5 % charge: the 3C train's last pulse sits
    at ~4 % where the DFN collapses and no circuit follows it, which is a statement about
    that corner, not about the fit."""
    e = simulate(run, nodes, pairs, ref_nodes) - run.v
    w = _weights(run)
    up = run.soc > 0.05
    parts = [f"all {1e3 * _rms(e, w):.2f}", f"soc>5% {1e3 * _rms(e, w, up):.2f}"] + [
        f"{n} {1e3 * _rms(e, w, (run.segment == k) & up):.2f}" for k, n in
        [(2, "dis-pulse"), (3, "chg-pulse"), (1, "move"), (0, "rest")]]
    print(f"  {label:24s} RMS mV: " + ", ".join(parts)
          + f"; max {1e3 * np.max(np.abs(e[up])):.1f} (soc>5%)")
    return e


def fit():
    zg, og = reference_ocv()
    runs = {name: load(name, t, c, zg, og) for name, t, c in RUNS}
    warm = runs["ecm_hppc_1c_25c.csv"]

    # 1. The smoothness: the largest lambda whose 25 degC discharge-pulse RMS stays within
    #    PULSE_RMS_SLACK of the BEST pulse fit found. The rule as first written measured
    #    against lambda = 0, and the first run showed that fit lands in a worse optimum
    #    (16.1 mV against 13.7 at lambda = 0.001), which is not "the best the circuit can
    #    do"; docs/plans/phase-11-fitting.md records the correction.
    trials = []
    for lam in LAMBDAS:
        nodes, pairs = fit_warm(warm, lam)
        e = simulate(warm, nodes, pairs) - warm.v
        rough = float(np.max(np.abs(np.diff(nodes, 2))))
        trials.append((lam, nodes, pairs, _seg_rms(warm, e, 2), rough))
        print(f"  lambda {lam:g}: dis-pulse RMS {1e3 * trials[-1][3]:.3f} mV, "
              f"max |2nd diff R0| {1e3 * rough:.3f} mOhm", flush=True)
    base = min(t[3] for t in trials)
    lam, nodes, pairs, _, _ = [t for t in trials if t[3] <= PULSE_RMS_SLACK * base][-1]
    print(f"chosen lambda = {lam:g}")

    # 2. The other temperatures: R0 alone, the pairs held and scaled by R0's own ratio.
    cols = {298.15: nodes}
    for name in ("ecm_hppc_1c_m10c.csv", "ecm_hppc_1c_45c.csv"):
        r = runs[name]
        cols[r.temp_k] = fit_r0_only(r, pairs, lam, nodes, nodes)
        print(f"  {name}: max |2nd diff R0| {1e3 * np.max(np.abs(np.diff(cols[r.temp_k], 2))):.3f} mOhm")

    print("residuals (engine-shaped circuit against the reference, reference OCV):")
    for name, r in runs.items():
        report(r, cols[r.temp_k], pairs, name, None if r.temp_k == 298.15 else nodes)
    print("for comparison only - the same columns with the RC rule off:")
    for name in ("ecm_hppc_1c_m10c.csv", "ecm_hppc_1c_45c.csv"):
        r = runs[name]
        report(r, fit_r0_only(r, pairs, lam, nodes, None), pairs, name)
    # What the schema costs: the same cold and warm runs with pairs of their own.
    print("for comparison only - pairs fitted per temperature (the schema cannot hold this):")
    for name in ("ecm_hppc_1c_m10c.csv", "ecm_hppc_1c_45c.csv"):
        n2, p2 = fit_warm(runs[name], lam)
        report(runs[name], n2, p2, name)
        print("     pairs: " + ", ".join(f"{1e3 * r:.2f} mOhm / {t:.1f} s" for r, t in p2))
    return lam, cols, pairs, runs


def print_toml(lam, cols, pairs):
    temps = sorted(cols)
    print()
    print("[r0]")
    print("soc    = [" + ", ".join(f"{z:.1f}" for z in R0_SOC) + "]")
    print("temp_k = [" + ", ".join(f"{t:.2f}" for t in temps) + "]")
    print("rc_t_ref_k = 298.15")
    rows = [", ".join(f"{cols[t][k]:.6f}" for t in temps) for k in range(len(R0_SOC))]
    print("ohms   = [[" + "],\n          [".join(rows) + "]]")
    for r, tau in pairs:
        print()
        print("[[rc]]")
        print(f"r_ohms  = {r:.6f}")
        print(f"c_farad = {tau / r:.3f}   # tau = {tau:.2f} s")


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--generate", action="store_true")
    ap.add_argument("--fit", action="store_true")
    ap.add_argument("--only", help="generate one CSV of RUNS by file name")
    a = ap.parse_args(argv)
    if a.generate:
        generate(a.only)
    if a.fit:
        lam, cols, pairs, _ = fit()
        print_toml(lam, cols, pairs)
    if not (a.generate or a.fit):
        ap.print_help()


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    main(sys.argv[1:])
