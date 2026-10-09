"""Fit the SEI film for `chemistries/nmc_21700_lgm50.toml` to measured calendar ageing.

Phase 10, slice A (`docs/plans/phase-10-slice-a-fit.md`). Fits PyBaMM's
"interstitial-diffusion limited" SEI law -- the one the owner chose in
`docs/plans/phase-10-sei.md` -- to the WMG LG M50 calendar-ageing data, and the film's
resistivity to the same cells' pulse resistance. Prints the fitted constants with their
residuals, and checks the closed form it fits with against PyBaMM at those constants.

Source data
-----------
WMG Calendar Ageing Dataset - LGM50 Commercial Cells (39 Storage Conditions),
J. A. Kuzhiyil and W. D. Widanage, zenodo.org/record/14577286, CC-BY-4.0; the article is
Kuzhiyil et al., Applied Energy 382 (2025) 125221, doi:10.1016/j.apenergy.2024.125221.
Only the 39 analysed files are read (`<T>degC/Analysed MAT files/calAnal_<soc>Per_<T>degC.mat`,
~1 GB of a 10 GB zip, which is Deflate64-compressed). Neither is committed. Extract them into
one directory and point this script at it:

    python tools/reference/fit_sei_wmg.py <dir-with-calAnal-files>

Each file holds `calResults` with per-cell C/3 capacity (`Capacity.indCap`, one row per
reference test), 10 s pulse resistance at five SOCs (`Resistance`, means only) and the
storage day of every test (`Time_d`).

How the data is read
--------------------
* **Per cell, never the mean.** `meanCap` averages a set of cells that changes as cells
  leave for teardown (one condition goes 9 -> 5 -> 1), and `indCap`'s rows are compacted --
  survivors shift left, so a column is not a cell. Cells are tracked by an order-preserving
  minimum-change match and each is normalised to its own day-0 capacity.
* **Storage-driven fade** is a condition's fade minus the 0 %-SOC cells' fade at the same
  temperature. The 0 % cells fade 2-4 % a year at every temperature, most likely from the
  reference tests' own cycling; an interstitial film on near-empty graphite grows by under a
  thousandth of its high-SOC rate, so it is the baseline, not a signal.
* **A one-time step** at the first post-storage test, `A * (x(SOC) - x(0))`, is fitted beside
  the film and then discarded: about half of the cells' storage-charge effect appears in the
  first month at every temperature, 0 C included, and stops growing. A physical overhang arm
  fitted no better than this step and predicted the held-out 0 C cells worse, so the owner chose
  (2026-10-09) to model the film only and record the step as real but unexplained.
* 25 and 45 C are fitted; **0 C is held out**. One cell is excluded by a stated rule: the 45 C /
  90 % condition after day 400, when only its weakest cell remains (14.4 % faded at day 367
  against ~8.7 % for its eight siblings).

The film
--------
`j = -(D_li c_li0 F / L) exp(-F U_n(x) / RT) arrhenius(E_sei)`, integrated in closed form over
one-day ticks with the graphite potential frozen per tick and the graphite's lithium moved by
what the film takes -- the same update the engine will run on its aging sub-clock. Capacity
lost is 1.45 x lithium lost (measured in the Phase 10 spike from a C/20 check after a year).
Geometry, OCP and the electrode windows are Chen2020's (the chemistry's own source).
"""

from __future__ import annotations

import glob
import itertools
import math
import os
import re
import sys
import warnings

import numpy as np
import pybamm
import scipy.io
from scipy.optimize import minimize

warnings.filterwarnings("ignore")

F = 96485.33212
R_GAS = 8.314462618
K_CAP = 1.45  # capacity-% per lithium-% (Phase 10 spike, C/20 check from full charge)
PARAM_SET = "Chen2020"

P = pybamm.ParameterValues(PARAM_SET)
VBAR = P["SEI partial molar volume [m3.mol-1]"]
Z = P["Ratio of lithium moles to SEI moles"]
L0 = P["Initial SEI thickness [m]"]
D_LI_DEFAULT = P["SEI lithium interstitial diffusivity [m2.s-1]"]
C_LI0 = P["Lithium interstitial reference concentration [mol.m-3]"]
RHO_DEFAULT = P["SEI resistivity [Ohm.m]"]
T_REF = P["Reference temperature [K]"]
_A_CC = P["Electrode width [m]"] * P["Electrode height [m]"]
_EPS_N = P["Negative electrode active material volume fraction"]
AREA_N = 3 * _EPS_N / P["Negative particle radius [m]"] * P["Negative electrode thickness [m]"] * _A_CC
MOL_N = P["Maximum concentration in negative electrode [mol.m-3]"] * _EPS_N \
    * P["Negative electrode thickness [m]"] * _A_CC
_MOL_P = P["Maximum concentration in positive electrode [mol.m-3]"] \
    * P["Positive electrode active material volume fraction"] * P["Positive electrode thickness [m]"] * _A_CC
_X1, _Y1 = pybamm.lithium_ion.get_initial_stoichiometries(1.0, P)
N_LI = _X1 * MOL_N + _Y1 * _MOL_P  # cyclable lithium inventory [mol]
LLI_PCT_PER_M = Z / VBAR * AREA_N / N_LI * 100

_OCP = P["Negative electrode OCP [V]"]
_XG = np.linspace(0.0, 1.0, 4001)
_UG = np.array([np.asarray(_OCP(pybamm.Scalar(s)).evaluate()).item() for s in _XG])
_X_OF_SOC: dict[float, float] = {}

FIT_TEMPS = (25, 45)
HELD_OUT = 0


def u_neg(x: float) -> float:
    return float(np.interp(x, _XG, _UG))


def x_of_soc(soc: float) -> float:
    if soc not in _X_OF_SOC:
        _X_OF_SOC[soc] = float(pybamm.lithium_ion.get_initial_stoichiometries(soc, P)[0])
    return _X_OF_SOC[soc]


# ---------------------------------------------------------------- data


def track_cells(t, ind):
    """Per-cell fade [%] from compacted per-cell capacities; returns (t, fade, n_cells)."""
    t = np.atleast_1d(t).astype(float)
    ind = np.atleast_2d(ind).astype(float)
    c0 = np.array([x for x in ind[0] if np.isfinite(x)])
    last = c0.copy()
    fades, ns = [], []
    for row in ind:
        vals = [x for x in row if np.isfinite(x)]
        n = min(len(vals), len(c0))
        if n == 0:
            fades.append(np.nan)
            ns.append(0)
            continue
        best = None
        for vsub in itertools.combinations(range(len(vals)), n):
            vv = [vals[i] for i in vsub]
            for sub in itertools.combinations(range(len(c0)), n):
                cost = sum(abs(last[c] - v) for c, v in zip(sub, vv))
                if best is None or cost < best[0]:
                    best = (cost, sub, vv)
        _, sub, vv = best
        for c, v in zip(sub, vv):
            last[c] = v
        fades.append(float(np.mean([(1 - v / c0[c]) * 100 for c, v in zip(sub, vv)])))
        ns.append(n)
    return t, np.array(fades), np.array(ns)


def load(directory: str):
    data = {}
    for f in glob.glob(os.path.join(directory, "calAnal_*.mat")):
        m = re.match(r"calAnal_(\d+)Per_(\d+)degC\.mat", os.path.basename(f))
        soc, tc = int(m.group(1)), int(m.group(2))
        r = scipy.io.loadmat(f, squeeze_me=True, struct_as_record=False)["calResults"]
        t, fade, n = track_cells(r.Time_d, r.Capacity.indCap)
        r50 = np.atleast_1d(r.Resistance.DisRes.Pulse2CR10.SoC50Mean).astype(float)
        keep = np.isfinite(fade) & (n > 0)
        if (tc, soc) == (45, 90):
            keep &= t < 400  # the stated exclusion: only the weakest cell remains after
        full = n == n[0]  # resistance is a mean only: use tests with the day-0 cell set
        data[(tc, soc)] = dict(t=t[keep], fade=fade[keep], r50=r50[keep], full=full[keep])
    if len(data) != 39:
        sys.exit(f"expected 39 calAnal files, found {len(data)}")
    return data


def excess(data, tc, soc, field="fade"):
    d, b = data[(tc, soc)], data[(tc, 0)]
    return d["t"], d[field] - np.interp(d["t"], b["t"], b[field])


# ---------------------------------------------------------------- model


def film_thickness(days, soc, temp_k, d_li, e_sei):
    """Film thickness [m] at each of `days` (sorted), one-day ticks, potential frozen per tick."""
    arr = math.exp(e_sei / R_GAS * (1 / T_REF - 1 / temp_k))
    k = VBAR * d_li * C_LI0 * arr / Z
    length, x, t, j, out = L0, x_of_soc(soc), 0.0, 0, []
    days = np.asarray(days, dtype=float)
    while j < len(days) and days[j] <= 0:
        out.append(L0)
        j += 1
    while j < len(days):
        h = min(1.0, days[-1] - t)
        nxt = math.sqrt(length * length + 2 * k * math.exp(-F * u_neg(x) / (R_GAS * temp_k)) * h * 86400.0)
        x -= Z * (nxt - length) / VBAR * AREA_N / MOL_N
        length, t = nxt, t + h
        while j < len(days) and days[j] <= t + 1e-9:
            out.append(length)
            j += 1
    return np.array(out)


def film_fade(days, soc_pct, tc, d_li, e_sei):
    return K_CAP * (film_thickness(days, soc_pct / 100, tc + 273.15, d_li, e_sei) - L0) * LLI_PCT_PER_M


def step(days, soc_pct, a):
    return a * (x_of_soc(soc_pct / 100) - x_of_soc(0.0)) * (np.asarray(days) > 0)


def conditions(data, temps):
    return [(tc, s) for (tc, s) in sorted(data) if tc in temps and s > 0]


def rms(data, theta, temps, with_step=True):
    d_li, e_sei, a = 10 ** theta[0] * D_LI_DEFAULT, theta[1] * 1e4, theta[2]
    errs = []
    for tc, s in conditions(data, temps):
        t, ex = excess(data, tc, s)
        model = film_fade(t, s, tc, d_li, e_sei) + (step(t, s, a) if with_step else 0)
        errs.append(np.mean((ex - model) ** 2))
    return math.sqrt(float(np.mean(errs)))


def fit(data):
    bounds = [(0, 5), (0, 15), (0, 50)]

    def obj(th):
        if any(not lo <= v <= hi for v, (lo, hi) in zip(th, bounds)):
            return 1e3
        return rms(data, th, FIT_TEMPS)

    best = None
    for start in ([2.0, 10.0, 2.5], [3.0, 5.0, 2.0], [1.5, 12.0, 3.0]):
        r = minimize(obj, start, method="Nelder-Mead", options={"xatol": 1e-5, "fatol": 1e-8, "maxiter": 4000})
        if best is None or r.fun < best.fun:
            best = r
    return best.x


def fit_resistivity(data, d_li, e_sei):
    """Least squares, through the origin, of the storage-driven resistance rise on the film's."""
    xs, ys = [], []
    for tc, s in conditions(data, FIT_TEMPS):
        d, b = data[(tc, s)], data[(tc, 0)]
        sel = d["full"] & np.isfinite(d["r50"])
        bsel = np.isfinite(b["r50"])
        if not sel[0] or not bsel[0]:
            continue  # no day-0 resistance to measure a rise from
        t = d["t"][sel]
        dr = (d["r50"][sel] - d["r50"][sel][0]) \
            - (np.interp(t, b["t"][bsel], b["r50"][bsel]) - b["r50"][bsel][0])
        dl = film_thickness(t, s / 100, tc + 273.15, d_li, e_sei) - L0
        dl0 = film_thickness(t, 0.0, tc + 273.15, d_li, e_sei) - L0
        xs.extend((dl - dl0) / AREA_N)
        ys.extend(dr * 1e-3)  # mohm -> ohm
    xs, ys = np.array(xs), np.array(ys)
    rho = float(xs @ ys / (xs @ xs))
    resid = ys - rho * xs
    return rho, float(np.sqrt(np.mean(resid ** 2)) * 1e3), len(xs)


def pybamm_check(d_li, e_sei, rho, soc, tc, days=365):
    """Lithium lost [%] after `days` at rest in PyBaMM's SPM at the fitted constants."""
    p = pybamm.ParameterValues(PARAM_SET)
    p.update({"SEI lithium interstitial diffusivity [m2.s-1]": d_li,
              "SEI growth activation energy [J.mol-1]": e_sei,
              "SEI resistivity [Ohm.m]": rho,
              "Ambient temperature [K]": tc + 273.15,
              "Initial temperature [K]": tc + 273.15})
    model = pybamm.lithium_ion.SPM({"SEI": "interstitial-diffusion limited"})
    exp = pybamm.Experiment([pybamm.step.string(f"Rest for {days * 24} hours", period="24 hours")])
    sol = pybamm.Simulation(model, parameter_values=p, experiment=exp).solve(initial_soc=soc)
    return float(sol["Loss of lithium inventory [%]"].entries[-1])


def main(directory: str):
    data = load(directory)
    th_step = fit(data)
    d_li, e_sei, a = 10 ** th_step[0] * D_LI_DEFAULT, th_step[1] * 1e4, th_step[2]
    fit_rms = rms(data, th_step, FIT_TEMPS)
    held = rms(data, th_step, (HELD_OUT,))
    print(f"film: D_li = {d_li:.4e} m2/s ({d_li / D_LI_DEFAULT:.1f} x {PARAM_SET}'s), "
          f"E_sei = {e_sei / 1e3:.2f} kJ/mol; one-time step A = {a:.3f} (discarded)")
    print(f"RMS of storage-driven fade, film + step: {fit_rms:.3f} % (25+45 C, fitted), "
          f"{held:.3f} % (0 C, held out)")
    print(f"  film alone (step dropped): {rms(data, th_step, FIT_TEMPS, with_step=False):.3f} % (25+45 C)")
    rho, r_rms, n = fit_resistivity(data, d_li, e_sei)
    print(f"film resistivity: rho = {rho:.4g} ohm m ({rho / RHO_DEFAULT:.3g} x {PARAM_SET}'s), "
          f"RMS {r_rms:.3f} mohm over {n} full-set tests")
    print("closed form vs PyBaMM SPM at the fitted constants, one year at rest (lithium lost %):")
    for soc, tc in ((0.85, 25), (0.85, 45), (0.3, 25)):
        ref = pybamm_check(d_li, e_sei, rho, soc, tc)
        cf = (film_thickness([365.0], soc, tc + 273.15, d_li, e_sei)[0] - L0) * LLI_PCT_PER_M
        print(f"  SOC {soc:.2f}, {tc} C: closed form {cf:.4f}  PyBaMM {ref:.4f}  ({(cf / ref - 1) * 100:+.2f} %)")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
