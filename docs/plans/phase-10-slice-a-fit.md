# Phase 10, slice A — the film's rate fitted to measured cells, and what the cells said back

**Status: RUN 2026-10-09. No Rust, no `[sei]` section.** Plan: `phase-10-sei.md` §A. A stop rule
in that plan fired, and the owner re-decided (below). Pre-registration with one dated addendum,
scripts and outputs: `W:/temp/claude/phase10-spike/` (`FIT_PREREG.md`, `FIT_FINDINGS.md`,
`fit_sei.py`, `checks.py`, `percell.py`, `fit_result_percell.json`).

## Data

The WMG calendar-ageing set for the LG M50 (Kuzhiyil et al., *Applied Energy* 2025,
doi:10.1016/j.apenergy.2024.125221; zenodo.org/record/14577286, CC-BY-4.0). 39 storage
conditions — 0, 25 and 45 °C × 13 SOCs from 0 to 100 % — about two years each, with reference
tests every one to three months. Only the 39 analysed files were fetched (≈1 GB of a 10 GB zip,
by HTTP range requests; the zip is Deflate64). Each holds per-cell C/20 capacity, 10 s and 30 s
pulse resistance at five SOCs, and the C/20 charge and discharge curves. The paper is
paywalled; how its first reference test was conditioned is **not known** here.

**A trap in the data.** `Capacity.meanCap` averages a *changing* set of cells — cells leave for
teardown, and 45 °C / 90 % goes from 9 cells to 5 to 1 — and the per-cell `indCap` rows are
compacted, survivors shifted left, so a column is not a cell. The fit's first pass normalised
the mean and read a jump at 45 °C / 90 % that was the set shrinking to its weakest cell (14.4 %
faded by day 367 against ~8.7 % for its eight siblings). Cells are now tracked by an
order-preserving minimum-change match and each normalised to its own day-0 capacity; that moves
most conditions by ≤ 0.5 points.

## Model of the data, and the fit

`measured fade = baseline(T, t) + 1.45 × lithium lost to the film(T, SOC, t)`, where the
baseline is the 0 %-SOC cells' own fade (about 2–4 % a year at every temperature, most likely the
reference tests' own cycling, since an interstitial film on near-empty graphite grows by under a
thousandth of its high-SOC rate), and 1.45 is the spike's capacity-per-lithium ratio. The film is
the spike's closed form at the `OKane2022` constants except two, fitted to the 25 and 45 °C cells;
0 °C held out.

| registered | measured | |
| --- | --- | --- |
| `D_li` multiplier 300–3000 | **939** (`D_li` = 9.4e-18 m²/s) | hit |
| activation energy 30–60 kJ/mol | **52.8** (47.7 without the outlier cell) | hit |
| rank correlation with the data across SOC at a year, 25 °C, ≥ 0.7 | 0.81 (45 °C 0.64; 0 °C, held out, 0.78) | hit |
| RMS of the storage-driven fade ≤ 0.6 % | 1.15 % (0.96 % without the outlier) | **miss** |
| 0 °C under-predicted by > 3× | 0.8–0.9 against 1.1 % at 85 % | miss — 0 °C is predicted fairly well; the estimate forgot the square root |

## What the cells said back

**Where, yes; when, no.** The film puts the storage-charge effect in the right place — the step
between 60 and 70 % SOC is the graphite potential's — but not at the right time. High-SOC
(70–95 %) minus low-SOC (0–10 %) mean fade:

| | day 30 | day 200 | last common day |
| --- | --- | --- | --- |
| 0 °C | 1.49 | 2.17 | 2.50 (day 746) |
| 25 °C | 1.52 | 2.58 | 2.89 (day 496) |
| 45 °C | 2.94 | 5.33 | 6.64 (day 511) |

A `sqrt(t)` film would put a quarter of the day-496 value in place by day 30; the cells have
half. A 1.5-point gap at 0 °C inside a month is not an Arrhenius film. The fitted film is too low
early and too high late (25 °C / 85 %: 0.63 against 1.58 % at day 24, 3.84 against 3.24 % at day
740). Candidates, not distinguished: lithium moving into the anode overhang (the graphite beyond
the positive electrode's edge — fast, reversible, levelling off), and the first reference test
having started from the as-received state while every later one starts from the storage SOC.

**Above 90 %, at 45 °C, less.** Cells stored at 95–100 % faded 3.2–3.3 % in a year against 6.2 %
at 85 %. No law driven by the graphite potential can do that.

**The film's resistance is 15–25 times the cells'.** The storage-driven resistance rise per Ah
of storage-driven loss is 2.5 mΩ/Ah at 25 °C and 4.0 at 45 °C (10 s, 2C pulse at 50 % SOC, high-
minus low-SOC groups); the film at `ρ_sei` = 2e5 Ω·m gives ~63. The total rise per % of total
fade, 0.5–2.9 % per %, brackets today's placeholder `r_growth_per_capacity_loss` = 1.5.

## The owner's re-decision, 2026-10-09

Asked after the stop rule fired, with three options (film with the miss documented; film plus a
mechanism for the fast early part; fit the drawn curve instead):

1. **Film plus a fast, levelling-off mechanism.** The film stays (interstitial law, fitted rate);
   a second mechanism carries the front-loaded part. Its physical identity is to be established
   by a spike before any engine slice — see `phase-10-sei.md`'s addendum.
2. **Fit the film's resistivity to the data**, reversing decision 3 of the plan on the evidence
   above (it lands roughly 15–25 times below PyBaMM's value).

## Still open

- How the first reference test was conditioned (the paper, if it can be read).
- The 95–100 % dip at 45 °C — a positive-electrode mechanism, most likely; not in scope.
- The 0 %-SOC baseline: any engine law has to say where 2–4 % a year at empty comes from, or the
  lesson has to say the cells were being tested, not left alone.
