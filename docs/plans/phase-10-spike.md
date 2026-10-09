# Phase 10, the spike — which SEI growth law, and does the roadmap's exit survive it

**Status: RUN 2026-10-09. No Rust.** PyBaMM 26.6.2.0 (`tools/reference/requirements.txt`),
single-particle model, `OKane2022` parameter set (the LG M50 cell), isothermal, one year at
rest at fixed state of charge. Harness, pre-registration (with one dated addendum) and raw
output: `W:/temp/claude/phase10-spike/` (`PREREG.md`, `FINDINGS.md`, `shelf.py`, `ticks.py`,
`out/*.json`). The plan this spike feeds is `phase-10-sei.md`.

## The question

`docs/ROADMAP.md` §Phase 10 proposed a **reaction-limited** SEI on the `Spm`, with the exit
"calendar fade is `sqrt(t)`-shaped without a `sqrt` in the code … and grows resistance without
a `r_growth_per_capacity_loss` coefficient", and said the SEI parameters for the LG M50 are
published. Each of those three claims was measured rather than assumed.

## What PyBaMM's SEI laws do on a shelf

Four of the growth laws in `pybamm/models/submodels/interface/sei/sei_growth.py`, each from
the same starting film (5 nm). "Lithium lost" is PyBaMM's loss of lithium inventory; "slope"
is the log-log slope of lithium lost against time over days 30–365 (0.5 is `sqrt(t)`,
1.0 is linear).

| law | slope (full, 25 °C) | lithium lost in a year at 100 / 70 / 30 % SOC, 25 °C | 100 % vs 30 % | 45 °C vs 25 °C |
| --- | --- | --- | --- | --- |
| reaction limited | **0.962** | 22.5 / 12.2 / 3.97 % | 5.68 | 1.42 |
| solvent-diffusion limited | 0.569 | 0.720 / 0.720 / 0.720 % | **1.000** | 1.67 |
| EC reaction limited | saturates | 65.6 / 45.6 / 18.9 % | 3.48 | 1.01 |
| interstitial-diffusion limited, defaults | 0.907 | 0.033 / 0.022 / 0.002 % | 18.1 | 2.46 |
| interstitial-diffusion limited, `D_li` × 100 | **0.569** | 0.647 / 0.496 / 0.100 % | **6.47** | 1.81 |

**The roadmap's slice and its exit contradict each other.** A reaction-limited film grows
linearly on a shelf (slope 0.962, registered 0.95–1.00): nothing in that law slows it as the
film thickens except the ohmic drop of the side current itself. The `sqrt(t)` shape needs a
transport-limited law.

**Solvent-diffusion is `sqrt(t)` and blind to state of charge**: its rate has no potential in
it, so a cell stored at 30 % ages exactly as fast as one stored full (ratio 1.000 to 1e-8).
The engine's present law teaches "storage at high SOC is worse" through `cal_soc_stress`;
this one cannot.

**The EC law trades one for the other.** Its closed form at frozen potential is
`L + k·L²/(2D) = const + c·k·t`; holding a year's growth fixed and sweeping `D`, the time
slope and the SOC effect move together (sensitivity of growth to the rate = `2·slope − 1`):
`D` = 2e-22 gives slope 0.69 and a 1.76 ratio, 2e-21 gives 0.97 and 4.43, and below
~1.3e-22 a year's growth is unreachable. Fitting it to data can only pick a point on that
line. With PyBaMM's constants it consumes two thirds of the lithium in a year.

**The interstitial-diffusion law gets both**: `j = −(D_li·c_li0·F/L)·exp(−F·Δφ/RT)` is a
1/L transport law, so it is `sqrt(t)` once the film has outgrown its starting thickness, and
its potential factor carries the full `F/RT` inside the transport term, so the SOC effect
survives as `sqrt` of the rate ratio. With PyBaMM's `D_li` the film barely grows (5 → 7.7 nm
in a year, slope 0.907); scaled ×100 it is 0.569 with a 6.47 SOC ratio. Every registered
prediction for both runs held except the temperature ratio (1.81 against 1.6–1.7: the
potential factor grows with temperature and the estimate had its sign backwards).

The `sqrt(t)` shape is **a claim about how far the film has grown**, not about the law alone:
at 30 % SOC the ×100 film is still 13 nm after a year and its slope is 0.78. Any test of the
shape has to state the regime it is in.

## The three side findings

**The film's resistance is a coefficient in disguise.** It is `ρ_sei·L / A_neg` with
`A_neg` = 3.36 m² of graphite surface. Fresh, 0.298 mΩ of a 22.28 mΩ cell (10 s, 1 C pulse).
Since both the film and the lithium it holds grow with the moles of SEI formed, the
resistance per amp-hour lost is fixed: `ρ·V̄·3600/(z·F·A_neg²)` ≈ **63 mΩ per Ah**, the same
for every law (15.6 % for 1.04 % capacity on solvent-diffusion, 490 % for 32 % on
reaction-limited) — about **14 % resistance per 1 % capacity, ten times the shipped
placeholder `r_growth_per_capacity_loss` = 1.5**. The registered prediction (< 0.3 % per %)
was off by fifty; it forgot that the film grows twelvefold while the lithium it costs is
under 1 %. And `ρ_sei` = 2e5 Ω·m is an unsourced PyBaMM default, so "resistance grows
without a coefficient" is true in name only.

**Capacity lost is not lithium lost.** A C/20 check discharge after the year reads 1.46 ×
the lithium-loss percentage (solvent-diffusion, stored full; 1.44 reaction-limited): one is a
share of the whole inventory, the other of what cycles between the voltage limits. The
engine must read capacity off its electrodes. The check protocol itself has an offset that
depends on where the cell starts (0.27 % at 30 % SOC with nothing lost), so the ratio is
quoted from full-charge starts only.

**The constants are not LG M50 measurements.** `OKane2022.py`'s SEI block has no citation
per constant and its docstring says the set "does not claim to be representative of the true
parameter values". The same block, bar the activation energy and the Li:SEI ratio, is in
`Chen2020` and `Ecker2015`. ROADMAP H2's "the SEI parameters for LG M50 are published" is
wrong in the sense it was used.

## Long steps

Freeze the graphite potential and temperature over an aging tick, advance the film in closed
form, move the graphite's lithium by what the film took:

| law | 10 s – 1 day ticks | 30-day ticks |
| --- | --- | --- |
| solvent-diffusion | −0.02 % at a year (exact at fixed T) | the same |
| interstitial ×100 | within 0.02 % | within 0.21 % |
| reaction limited | within 0.15 % at 1 day | +3 to +6 % |

So the sub-clock design carries over: the film needs no sub-stepping at fast-forward step
lengths. "No `sqrt` in the code" is read as **no `sqrt(t)` in the rate law**; a square root
inside the closed-form integral of a 1/L rate is the integral, not a fitted shape.

## Mistakes in the running, for the record

The first two grid runs were lost: the Bash tool's heredoc halved the backslashes in a
Windows path, `\$4` became a literal, and every run overwrote one file. Their stdout
summaries agree with the third run to the printed digits. The advisor proposed the
interstitial law after the first three were in; it was pre-registered in a dated addendum
before it ran, and its predicted slope (0.57) was corrected there to 0.85 for the default
constants, which held (0.907).

## Data for the fit

The WMG calendar-ageing dataset for the LG M50 (Kuzhiyil et al., *Applied Energy* 2025,
doi:10.1016/j.apenergy.2024.125221; zenodo.org/record/14577286, CC-BY-4.0): 0, 25 and 45 °C,
13 storage SOCs each, about two years, with periodic reference tests holding C/20 capacity,
pulse resistance at five SOCs and the full C/20 charge and discharge curves. One analysed
file per condition (39, ~1 GB of a 10 GB zip, fetched by range requests).
