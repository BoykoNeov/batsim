"""Print TOML-ready blocks for `chemistries/na_ion_18650_generic.toml`.

Unlike `fit_ocv.py`, which extracts an OCV curve from a PyBaMM parameter set, this
script fits an equivalent-circuit parameter set from **published laboratory
measurements of a real commercial cell**. It needs no PyBaMM.

Source data
-----------
Max Kraft-Schaefer (GitHub `MaxMax-embedded`), *Measurements with Hakadi 18650
Sodium Ion Cells*, https://github.com/MaxMax-embedded/hakadi_soidum_ion_18650,
released under **CC0 1.0** (public domain dedication). Cells are Hakadi 18650
sodium-ion, 1500 mAh nominal, 1.5-4.1 V, layered-oxide cathode.

Two files are read, and neither is committed here (they total ~30 MB; the same
"script in tree, inputs not" split `fit_ocv.py` uses for its PyBaMM install):

    Measurement_Data/OCV_Test/ocv_test_inc_2-2.csv     incremental OCV, cell 2
    Measurement_Data/HPPC_Test/hppc_test_hakadi1500mah_2-2.csv    HPPC, cell 2

Download them into one directory and point this script at it:

    curl -LO https://raw.githubusercontent.com/MaxMax-embedded/hakadi_soidum_ion_18650/main/Measurement_Data/OCV_Test/ocv_test_inc_2-2.csv
    curl -LO https://raw.githubusercontent.com/MaxMax-embedded/hakadi_soidum_ion_18650/main/Measurement_Data/HPPC_Test/hppc_test_hakadi1500mah_2-2.csv
    python tools/reference/fit_na_ion_hakadi.py <dir>

`ocv_test_inc_2-2.csv` is the second OCV run, and it is the one the source repo's
own figure uses: the README records that the first run's coulomb counting was
unreliable, so the two legs of that run do not align.

CSV columns are `step,mode,time,voltage,current`, with time in milliseconds and
**charge current positive** -- the opposite of this project's sign convention, so
every current read here is negated before it reaches a printed number.

What is fitted, and what is not
-------------------------------
Fitted from the data: usable capacity, the OCV table, R0 against SOC, both RC
pairs, and the bracket on the hysteresis half-width.

NOT fitted, because the source measured at room temperature only: the temperature
axis of `[r0]`, and everything in `[thermal]`, `[aging]` and `[safety]`. Those
carry their own provenance notes in the TOML and are not printed here -- a number
this script does not print is a number it did not measure.

Not shipped; not on the Rust build or CI path.
"""

from __future__ import annotations

import csv
import math
import sys
from collections import defaultdict
from pathlib import Path

# --- The step programme of the two source runs -------------------------------
#
# Both runs alternate a current step with a rest, and the step ids below were read
# off the files rather than assumed. `inspect`-style dumps of every step are what
# established them; they are hard-coded because they are a property of these two
# recorded runs and of nothing else.

# ocv_test_inc_2-2.csv: 0.5 A (C/3) steps of 540 s, each followed by a 1200 s rest.
OCV_DISCHARGE_STEPS = list(range(3, 42, 2))
OCV_CHARGE_STEPS = list(range(45, 80, 2)) + [81]
OCV_FULL_REST = 2  # the settled rest at the top, before the discharge leg
OCV_EMPTY_REST = 44  # the settled rest at the bottom, before the charge leg

# hppc_test_hakadi1500mah_2-2.csv: at each 10 % SOC level, a 10 s 2 A discharge
# pulse, 40 s rest, 10 s 2 A charge pulse, 40 s rest; then 540 s at 1 A (0.15 A.h,
# one tenth of rated capacity) and a 1200 s rest.
HPPC_PULSE_PAIRS = [(3, 5), (9, 11), (15, 17), (21, 23), (27, 29),
                    (33, 35), (39, 41), (45, 47), (51, 53), (57, 59)]
HPPC_PULSE_SOC = [1.0 - 0.1 * i for i in range(10)]
HPPC_LONG_STEPS = list(range(7, 62, 6))

Sample = tuple[int, float, float]  # (time_ms, voltage_v, current_a_discharge_positive)


def read_steps(path: Path) -> dict[int, list[Sample]]:
    """Group a source CSV by step index, negating current into discharge-positive."""
    steps: dict[int, list[Sample]] = defaultdict(list)
    with path.open(newline="") as fh:
        for row in csv.DictReader(fh):
            steps[int(row["step"])].append(
                (int(row["time"]), float(row["voltage"]), -float(row["current"]))
            )
    return steps


def amp_hours(rows: list[Sample]) -> float:
    """Trapezoidal charge through a step [A.h], positive when discharging."""
    return sum(
        0.5 * (a[2] + b[2]) * (b[0] - a[0]) / 3.6e6 for a, b in zip(rows, rows[1:])
    )


def lerp(pts: list[tuple[float, float]], x: float) -> float | None:
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        if x0 <= x <= x1:
            return y0 if x1 == x0 else y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    return None


# --- OCV ---------------------------------------------------------------------


def ocv_legs(steps: dict[int, list[Sample]]) -> tuple[list, list, float, float]:
    """Rested OCV against SOC for each leg, SOC normalised to that leg's throughput.

    Each leg spans its own [0, 1]. That is the alignment the source repo's own
    figure uses, and it is the one that respects "the cell was full at both ends":
    the run's measured coulombic efficiency is 1.03, i.e. it reports MORE charge
    out than in, which is physically impossible and is a current-sensor offset.
    Normalising per leg puts that 3 % into the SOC axis instead of into a spurious
    voltage gap. `hysteresis_bracket` below reports what the other choices give.
    """
    dis_total = sum(amp_hours(steps[s]) for s in OCV_DISCHARGE_STEPS)
    chg_total = -sum(amp_hours(steps[s]) for s in OCV_CHARGE_STEPS)

    discharge = [(1.0, steps[OCV_FULL_REST][-1][1])]
    moved = 0.0
    for s in OCV_DISCHARGE_STEPS:
        moved += amp_hours(steps[s])
        if s + 1 in steps:
            discharge.append((1.0 - moved / dis_total, steps[s + 1][-1][1]))

    charge = [(0.0, steps[OCV_EMPTY_REST][-1][1])]
    moved = 0.0
    for s in OCV_CHARGE_STEPS:
        moved += -amp_hours(steps[s])
        if s + 1 in steps:
            charge.append((moved / chg_total, steps[s + 1][-1][1]))

    discharge.sort()
    charge.sort()
    return discharge, charge, dis_total, chg_total


# The SOC grid the printed [ocv] table uses: dense where the curve bends.
OCV_GRID = [0.00, 0.02, 0.05, 0.10, 0.15, 0.20, 0.25, 0.30, 0.35, 0.40, 0.45,
            0.50, 0.55, 0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90, 0.95, 1.00]


def ocv_table(discharge: list, charge: list) -> list[tuple[float, float]]:
    """The loop CENTRE: the mean of the two legs, which is what the model wants.

    `sim_core` puts a discharging cell at `OCV - scale_v` and a charging one at
    `OCV + scale_v`, so `[ocv]` is the midline and not either measured curve.
    Outside the overlap the single available leg is used: the discharge leg
    supplies the full end (the charge leg's last rest is at 96 % and its final
    top-up step has no rest after it) and the charge leg supplies the empty end
    (the discharge leg's last rest is at 2 %).
    """
    out = []
    for soc in OCV_GRID:
        d, c = lerp(discharge, soc), lerp(charge, soc)
        if d is not None and c is not None:
            out.append((soc, 0.5 * (d + c)))
        elif d is not None:
            out.append((soc, d))
        elif c is not None:
            out.append((soc, c))
        else:  # pragma: no cover - the grid is inside both legs' union
            raise SystemExit(f"no OCV data at soc = {soc}")
    return out


def hysteresis_bracket(steps: dict[int, list[Sample]]) -> list[tuple[str, float, float, float]]:
    """Mean/min/max charge-minus-discharge separation under three alignments.

    The three disagree by a factor of eight and the file must not pretend
    otherwise -- see docs/plans/sodium-ion-chemistry.md. All are reported so the
    chosen half-width can be checked against the spread rather than against one
    of them.
    """
    dis, chg, dis_total, chg_total = ocv_legs(steps)
    rows = []

    def gaps(d_pts, c_pts, xs):
        vals = []
        for x in xs:
            a, b = lerp(d_pts, x), lerp(c_pts, x)
            if a is not None and b is not None:
                vals.append((b - a) * 1000.0)
        return vals

    xs = [i / 20 for i in range(1, 20)]
    g = gaps(dis, chg, xs)
    rows.append(("per-leg normalised SOC", sum(g) / len(g), min(g), max(g)))

    # Absolute charge above the empty endpoint: leaves the 3 % inconsistency as a
    # systematic SOC offset, which the steep top of the curve turns into volts.
    dis_ah = [(s * dis_total, v) for s, v in dis]
    chg_ah = [(s * chg_total, v) for s, v in chg]
    xs_ah = [i * 0.05 for i in range(1, 28)]
    g = gaps(dis_ah, chg_ah, xs_ah)
    rows.append(("absolute A.h from empty", sum(g) / len(g), min(g), max(g)))
    return rows


# --- Resistances -------------------------------------------------------------


def r0_against_soc(steps: dict[int, list[Sample]]) -> list[tuple[float, float, float]]:
    """(soc, R0, R_10s) at each HPPC level, averaged over the two pulse directions.

    R0 is taken from the FIRST sample actually under load. The logger emits one
    sample at the step boundary that still carries the pre-pulse voltage, so
    `rows[1]` is the first loaded reading, about 100 ms in. With the fast pair's
    time constant at ~7 s that sample carries under 2 % of the RC pair, so it is
    an ohmic reading rather than a 100 ms one.
    """
    out = []
    for (dis_step, chg_step), soc in zip(HPPC_PULSE_PAIRS, HPPC_PULSE_SOC):
        pair = []
        for step in (dis_step, chg_step):
            settled = steps[step - 1][-1][1]
            rows = steps[step]
            current = sum(r[2] for r in rows[1:]) / (len(rows) - 1)
            pair.append(
                ((settled - rows[1][1]) / current, (settled - rows[-1][1]) / current)
            )
        out.append((soc, (pair[0][0] + pair[1][0]) / 2, (pair[0][1] + pair[1][1]) / 2))
    return out


def tau_fast(steps: dict[int, list[Sample]]) -> float:
    """Fast time constant [s] from the 40 s relaxations after each 2 A pulse.

    Log-linear over the settled tail of the window. The 40 s rest is long enough
    to see this pair and far too short to see the slow one, which is exactly why
    the two are separable here at all.
    """
    taus = []
    for rest in [p[0] + 1 for p in HPPC_PULSE_PAIRS]:
        rows = steps[rest]
        v_inf = rows[-1][1]
        pts = [((r[0] - rows[0][0]) / 1000.0, v_inf - r[1]) for r in rows[1:]]
        pts = [(t, y) for t, y in pts if y > 2e-4]
        tail = pts[int(len(pts) * 0.35):]
        if len(tail) < 20:
            continue
        n = len(tail)
        sx = sum(t for t, _ in tail)
        sy = sum(math.log(y) for _, y in tail)
        sxx = sum(t * t for t, _ in tail)
        sxy = sum(t * math.log(y) for t, y in tail)
        slope = (n * sxy - sx * sy) / (n * sxx - sx * sx)
        if slope < 0:
            taus.append(-1.0 / slope)
    return sum(taus) / len(taus)


def tau_slow(steps: dict[int, list[Sample]]) -> float:
    """Slow time constant [s] from the 1200 s relaxations after each 540 s step."""
    taus = []
    for rest in [s + 1 for s in HPPC_LONG_STEPS]:
        rows = steps.get(rest)
        if not rows or len(rows) < 5000:
            continue
        v_inf = rows[-1][1]
        pts = [((r[0] - rows[0][0]) / 1000.0, v_inf - r[1]) for r in rows[1:]]
        # Start past five fast time constants so only the slow pair is left.
        pts = [(t, y) for t, y in pts if t > 60.0 and y > 2e-4]
        tail = pts[: int(len(pts) * 0.6)]
        if len(tail) < 100:
            continue
        n = len(tail)
        sx = sum(t for t, _ in tail)
        sy = sum(math.log(y) for _, y in tail)
        sxx = sum(t * t for t, _ in tail)
        sxy = sum(t * math.log(y) for t, y in tail)
        slope = (n * sxy - sx * sy) / (n * sxx - sx * sx)
        if slope < 0:
            taus.append(-1.0 / slope)
    return sum(taus) / len(taus)


def rc_resistances(
    steps: dict[int, list[Sample]], r0: list[tuple[float, float, float]],
    t1: float, t2: float,
) -> tuple[float, float]:
    """Both RC resistances [ohm], from what each pair has had time to develop.

    The fast pair is what the 10 s pulse sees beyond the ohmic drop, undone by how
    far a pair of time constant `t1` gets in 10 s. The slow pair is the rest of the
    overpotential still standing at the end of a 540 s step, undone the same way.
    """
    mid = [row for row in r0 if 0.3 <= row[0] <= 0.7]
    r_fast = sum((r10 - rr) for _, rr, r10 in mid) / len(mid)
    r_fast /= 1.0 - math.exp(-10.0 / t1)

    # Total settled resistance at the end of a 540 s step: the overpotential the
    # 1200 s rest gives back, per amp. Only the mid-SOC steps are used, matching
    # the SOC window `r_fast` was taken over.
    slow = []
    for step, soc in zip(HPPC_LONG_STEPS, HPPC_PULSE_SOC[1:]):
        rest = steps.get(step + 1)
        if not rest or len(rest) < 5000 or not 0.3 <= soc <= 0.7:
            continue
        loaded = steps[step][-1][1]
        current = sum(r[2] for r in steps[step][1:]) / (len(steps[step]) - 1)
        slow.append((rest[-1][1] - loaded) / current)
    r0_mid = sum(rr for _, rr, _ in mid) / len(mid)
    total_mid = sum(slow) / len(slow)
    r_slow = total_mid - r0_mid - r_fast * (1.0 - math.exp(-540.0 / t1))
    r_slow /= 1.0 - math.exp(-540.0 / t2)
    return r_fast, r_slow


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(f"usage: python {Path(argv[0]).name} <dir with the two source CSVs>",
              file=sys.stderr)
        return 2
    root = Path(argv[1])
    ocv_path = root / "ocv_test_inc_2-2.csv"
    hppc_path = root / "hppc_test_hakadi1500mah_2-2.csv"
    for p in (ocv_path, hppc_path):
        if not p.exists():
            print(f"missing {p} -- see this script's docstring for the URLs",
                  file=sys.stderr)
            return 2

    ocv_steps = read_steps(ocv_path)
    hppc_steps = read_steps(hppc_path)

    dis, chg, dis_total, chg_total = ocv_legs(ocv_steps)
    print("# --- fitted from the CC0 Hakadi 18650 Na-ion measurements "
          "by tools/reference/fit_na_ion_hakadi.py ---")
    print(f"# discharge-leg throughput {dis_total:.4f} A.h, "
          f"charge-leg {chg_total:.4f} A.h, ratio {dis_total / chg_total:.4f}")
    print(f"capacity_ah = {dis_total:.4f}")
    print()

    table = ocv_table(dis, chg)
    soc_s = ", ".join(f"{s:.2f}" for s, _ in table)
    v_s = ", ".join(f"{v:.4f}" for _, v in table)
    monotone = all(b > a for (_, a), (_, b) in zip(table, table[1:]))
    print(f"# strictly increasing in soc: {monotone}")
    print("[ocv]")
    print(f"soc   = [{soc_s}]")
    print(f"volts = [{v_s}]")
    print()

    print("# hysteresis: charge-minus-discharge separation [mV], full width")
    for name, mean, lo, hi in hysteresis_bracket(ocv_steps):
        print(f"#   {name:<24} mean {mean:6.1f}   min {lo:6.1f}   max {hi:6.1f}")
    print()

    r0 = r0_against_soc(hppc_steps)
    print("# R0 and 10 s resistance against SOC [ohm], room temperature, "
          "mean of the 2 A discharge and charge pulses")
    for soc, rr, r10 in r0:
        print(f"#   soc {soc:4.2f}   R0 {rr:.4f}   R_10s {r10:.4f}")
    print()

    t1, t2 = tau_fast(hppc_steps), tau_slow(hppc_steps)
    r1, r2 = rc_resistances(hppc_steps, r0, t1, t2)
    print(f"# RC pairs fitted from the relaxations:")
    print(f"#   fast  tau {t1:6.2f} s   R {r1:.5f} ohm   C {t1 / r1:8.1f} F")
    print(f"#   slow  tau {t2:6.2f} s   R {r2:.5f} ohm   C {t2 / r2:8.1f} F")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
