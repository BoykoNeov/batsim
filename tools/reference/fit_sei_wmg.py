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
~1 GB of a 10 GB zip). Neither is committed. The zip is Deflate64-compressed, which
Python's `zipfile` cannot read; either download it whole and extract with 7-Zip, or read just
the 39 members over HTTP range requests with the `zipfile-deflate64` package:

    pip install zipfile-deflate64
    python - <<'EOF'
    import io, os, urllib.request, zipfile_deflate64 as zf
    URL = "https://zenodo.org/api/records/14577286/files/Calendar%20ageing%20dataset.zip/content"
    class Remote(io.RawIOBase):
        def __init__(s):
            with urllib.request.urlopen(urllib.request.Request(URL, method="HEAD")) as r:
                s.n = int(r.headers["Content-Length"]); s.p = 0
        def readable(s): return True
        def seekable(s): return True
        def tell(s): return s.p
        def seek(s, o, w=0): s.p = (o, s.p + o, s.n + o)[w]; return s.p
        def readinto(s, b):
            k = min(len(b), s.n - s.p)
            if k <= 0: return 0
            rq = urllib.request.Request(URL, headers={"Range": f"bytes={s.p}-{s.p + k - 1}"})
            d = urllib.request.urlopen(rq).read(); b[:len(d)] = d; s.p += len(d); return len(d)
    z = zf.ZipFile(io.BufferedReader(Remote(), buffer_size=8 << 20))
    os.makedirs("wmg", exist_ok=True)
    for i in z.infolist():
        if "Analysed MAT files/calAnal_" in i.filename:
            open(os.path.join("wmg", os.path.basename(i.filename)), "wb").write(z.read(i))
    EOF
    python tools/reference/fit_sei_wmg.py wmg

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
* **The film is fitted to growth after the first post-storage test only.** About half of the
  cells' storage-charge effect appears before that test, at every temperature, 0 C included,
  and then stops growing. A physical overhang arm fitted no better than a one-time step there
  and predicted the held-out 0 C cells worse, so the owner chose (2026-10-09) to model the film
  only and record the early step as real but unexplained. The film is therefore scored on
  `excess(t) - excess(t1)` against `film(t) - film(t1)`, `t1` the first post-storage test, and
  nothing models the step. For comparison the script also fits film + a one-time step
  `A * (x(SOC) - x(0))` over every test; that fit's activation energy is NOT the shipped one,
  because one temperature-independent step cannot cover 45 C's larger first-month gap and so
  pushes the film's temperature dependence up.
* 25 and 45 C are fitted; **0 C is held out**. One cell is excluded by a stated rule: the 45 C /
  90 % condition after day 400, when only its weakest cell remains (14.4 % faded at day 367
  against ~8.7 % for its eight siblings).

The film
--------
`j = -(D_li c_li0 F / L) exp(-F U_n(x) / RT) arrhenius(E_sei)`, integrated in closed form over
one-day ticks with the graphite potential frozen per tick and the graphite's lithium moved by
what the film takes -- the same update the engine will run on its aging sub-clock. Capacity
lost per lithium lost is MEASURED here, in PyBaMM, on WMG's own capacity steps (1.67 A CC charge
to 4.2 V with no CV hold, 1.67 A discharge to 2.5 V), fresh against a year at 85 % / 25 C; the
spike's 1.45 came from a C/20 CC-CV check and is not this protocol. Geometry, OCP and the
electrode windows are Chen2020's (the chemistry's own source).
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


def film_fade(days, soc_pct, tc, d_li, e_sei, k_cap):
    return k_cap * (film_thickness(days, soc_pct / 100, tc + 273.15, d_li, e_sei) - L0) * LLI_PCT_PER_M


def step(days, soc_pct, a):
    return a * (x_of_soc(soc_pct / 100) - x_of_soc(0.0)) * (np.asarray(days) > 0)


def conditions(data, temps):
    return [(tc, s) for (tc, s) in sorted(data) if tc in temps and s > 0]


def growth_rms(data, d_li, e_sei, k_cap, temps):
    """RMS of growth after the first post-storage test: excess(t)-excess(t1) vs film(t)-film(t1)."""
    errs = []
    for tc, s in conditions(data, temps):
        t, ex = excess(data, tc, s)
        t, ex = t[1:], ex[1:]  # drop day 0; t[0] is now the first post-storage test
        model = film_fade(t, s, tc, d_li, e_sei, k_cap)
        errs.append(np.mean(((ex - ex[0]) - (model - model[0])) ** 2))
    return math.sqrt(float(np.mean(errs)))


def step_rms(data, d_li, e_sei, a, k_cap, temps):
    """RMS over every test of film + a one-time step (the comparison fit)."""
    errs = []
    for tc, s in conditions(data, temps):
        t, ex = excess(data, tc, s)
        errs.append(np.mean((ex - film_fade(t, s, tc, d_li, e_sei, k_cap) - step(t, s, a)) ** 2))
    return math.sqrt(float(np.mean(errs)))


def _minimise(obj, starts, bounds):
    def guarded(th):
        if any(not lo <= v <= hi for v, (lo, hi) in zip(th, bounds)):
            return 1e3
        return obj(th)

    best = None
    for start in starts:
        r = minimize(guarded, start, method="Nelder-Mead", options={"xatol": 1e-5, "fatol": 1e-8, "maxiter": 4000})
        if best is None or r.fun < best.fun:
            best = r
    return best.x


def fit_growth(data, k_cap):
    th = _minimise(lambda th: growth_rms(data, 10 ** th[0] * D_LI_DEFAULT, th[1] * 1e4, k_cap, FIT_TEMPS),
                   ([2.0, 5.0], [1.5, 10.0], [2.5, 3.0]), [(0, 5), (0, 15)])
    return 10 ** th[0] * D_LI_DEFAULT, th[1] * 1e4


def fit_step(data, k_cap):
    th = _minimise(lambda th: step_rms(data, 10 ** th[0] * D_LI_DEFAULT, th[1] * 1e4, th[2], k_cap, FIT_TEMPS),
                   ([2.0, 10.0, 2.5], [3.0, 5.0, 2.0], [1.5, 12.0, 3.0]), [(0, 5), (0, 15), (0, 50)])
    return 10 ** th[0] * D_LI_DEFAULT, th[1] * 1e4, th[2]


def group_growth(data, d_li, e_sei, k_cap, tc):
    """Mean high-SOC minus mean low-SOC growth from the first post-storage test to the last common
    day: cells against film."""
    hi, lo = (70, 80, 85, 90, 95), (2, 5, 10)
    first = max(data[(tc, s)]["t"][1] for s in hi + lo)
    last = min(data[(tc, s)]["t"][-1] for s in hi + lo)
    days = np.array([first, last])

    def cells(s):
        t, ex = excess(data, tc, s)
        return np.interp(days, t, ex)

    def film(s):
        return film_fade(days, s, tc, d_li, e_sei, k_cap)

    g = lambda f: np.mean([f(s) for s in hi], 0) - np.mean([f(s) for s in lo], 0)
    gc, gf = g(cells), g(film)
    return first, last, gc[1] - gc[0], gf[1] - gf[0]


def fit_resistivity(data, d_li, e_sei, temps):
    """Least squares, through the origin, of the storage-driven resistance rise on the film's."""
    xs, ys = [], []
    for tc, s in conditions(data, temps):
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
    return rho, float(np.sqrt(np.mean((ys - rho * xs) ** 2)) * 1e3), float(np.sqrt(np.mean(ys ** 2)) * 1e3), len(xs)


def _spm(d_li, e_sei, rho, tc):
    p = pybamm.ParameterValues(PARAM_SET)
    p.update({"SEI lithium interstitial diffusivity [m2.s-1]": d_li,
              "SEI growth activation energy [J.mol-1]": e_sei,
              "SEI resistivity [Ohm.m]": rho,
              "Ambient temperature [K]": tc + 273.15,
              "Initial temperature [K]": tc + 273.15})
    return p, pybamm.lithium_ion.SPM({"SEI": "interstitial-diffusion limited"})


def capacity_per_lithium(d_li, e_sei, rho):
    """Capacity lost % / lithium lost %, WMG's capacity steps, fresh vs a year at 85 % / 25 C."""
    def run(days):
        p, m = _spm(d_li, e_sei, rho, 25)
        steps = [pybamm.step.string(f"Rest for {days * 24} hours", period="24 hours")] if days else []
        steps += [pybamm.step.string(s, period="2 minutes") for s in (
            "Discharge at 1.67 A until 2.5 V", "Rest for 30 minutes", "Charge at 1.67 A until 4.2 V",
            "Rest for 30 minutes", "Discharge at 1.67 A until 2.5 V")]
        sol = pybamm.Simulation(m, parameter_values=p, experiment=pybamm.Experiment(steps)).solve(initial_soc=0.85)
        q = sol.cycles[-1]["Discharge capacity [A.h]"].entries
        return float(q[-1] - q[0]), float(sol["Loss of lithium inventory [%]"].entries[-1])

    q0, l0 = run(0)
    q1, l1 = run(365)
    return (1 - q1 / q0) * 100 / (l1 - l0)


def pybamm_check(d_li, e_sei, rho, soc, tc, days=365):
    """Lithium lost [%] after `days` at rest in PyBaMM's SPM at the fitted constants."""
    p, m = _spm(d_li, e_sei, rho, tc)
    exp = pybamm.Experiment([pybamm.step.string(f"Rest for {days * 24} hours", period="24 hours")])
    sol = pybamm.Simulation(m, parameter_values=p, experiment=exp).solve(initial_soc=soc)
    return float(sol["Loss of lithium inventory [%]"].entries[-1])


def main(directory: str):
    data = load(directory)
    # The ratio depends on the constants it is measured at -- the film's resistance shortens a
    # charge with no CV hold, so rho enters it -- and the fit depends on the ratio: iterate the
    # three to a fixed point.
    k_cap, rho = 1.5, RHO_DEFAULT
    for it in range(6):
        d_li, e_sei = fit_growth(data, k_cap)
        rho, *_ = fit_resistivity(data, d_li, e_sei, FIT_TEMPS)
        k_new = capacity_per_lithium(d_li, e_sei, rho)
        print(f"  iteration {it}: ratio {k_cap:.4f} -> {k_new:.4f}, D_li {d_li:.4e}, "
              f"E {e_sei / 1e3:.2f} kJ/mol, rho {rho:.4g}")
        done = abs(k_new / k_cap - 1) < 0.002
        k_cap = k_new
        if done:
            break
    d_li, e_sei = fit_growth(data, k_cap)
    print(f"capacity lost per lithium lost, WMG's capacity steps at the fitted rho: {k_cap:.3f}")
    print(f"FILM (fitted to growth after the first post-storage test, 25+45 C): "
          f"D_li = {d_li:.4e} m2/s ({d_li / D_LI_DEFAULT:.1f} x {PARAM_SET}'s), E_sei = {e_sei / 1e3:.2f} kJ/mol")
    print(f"  growth RMS: {growth_rms(data, d_li, e_sei, k_cap, FIT_TEMPS):.3f} % (25+45 C, fitted), "
          f"{growth_rms(data, d_li, e_sei, k_cap, (HELD_OUT,)):.3f} % (0 C, held out)")
    for tc in (0, 25, 45):
        first, last, gc, gf = group_growth(data, d_li, e_sei, k_cap, tc)
        print(f"  {tc:2d} C high-minus-low growth, day {first:.0f} -> {last:.0f}: cells {gc:+.2f}, film {gf:+.2f} points")
    sd, se, sa = fit_step(data, k_cap)
    print(f"comparison, film + one-time step over every test: D_li = {sd:.4e}, E_sei = {se / 1e3:.2f} kJ/mol, "
          f"A = {sa:.3f}; RMS {step_rms(data, sd, se, sa, k_cap, FIT_TEMPS):.3f} % (25+45 C), "
          f"{step_rms(data, sd, se, sa, k_cap, (HELD_OUT,)):.3f} % (0 C) -- NOT the shipped constants")
    for temps, label in ((FIT_TEMPS, "25+45 C"), ((25,), "25 C"), ((45,), "45 C")):
        rho, r_rms, sig, n = fit_resistivity(data, d_li, e_sei, temps)
        print(f"film resistivity, {label}: rho = {rho:.4g} ohm m ({rho / RHO_DEFAULT:.3g} x {PARAM_SET}'s); "
              f"residual RMS {r_rms:.3f} mohm against a signal RMS of {sig:.3f} mohm, {n} tests")
    rho, *_ = fit_resistivity(data, d_li, e_sei, FIT_TEMPS)
    print("closed form vs PyBaMM SPM at the fitted constants, one year at rest (lithium lost %):")
    for soc, tc in ((0.85, 25), (0.85, 45), (0.3, 25)):
        ref = pybamm_check(d_li, e_sei, rho, soc, tc)
        cf = (film_thickness([365.0], soc, tc + 273.15, d_li, e_sei)[0] - L0) * LLI_PCT_PER_M
        print(f"  SOC {soc:.2f}, {tc} C: closed form {cf:.4f}  PyBaMM {ref:.4f}  ({(cf / ref - 1) * 100:+.2f} %)")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
